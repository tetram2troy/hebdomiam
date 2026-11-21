//! Plans et liste d'épicerie : cycle complet, rotation, IDOR.

mod common;

use chrono::{NaiveDate, Utc};
use common::{TestDb, recipe, recipe_input, user};
use hebdomiam::domain::catalog::{Aisle, PlanRecipeStatus, PlanStatus, Unit};
use hebdomiam::repo::grocery::{self, GroceryItemCreate, GroceryItemPatch};
use hebdomiam::repo::plans::{self, PlanCreateInput, PlanError, PlanRecipeAction, ReplacementMode};
use hebdomiam::repo::recipes;
use uuid::Uuid;

fn week() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 9).unwrap()
}

fn input(dishes: i32) -> PlanCreateInput {
    PlanCreateInput {
        dish_count: dishes,
        people_count: 1,
        days_count: 7,
        replace: false,
    }
}

#[tokio::test]
async fn creation_portions_et_refus() {
    let t = TestDb::new().await;
    let id = user(&t.pool).await;
    for n in 0..3 {
        recipe(&t.pool, id, &format!("R{n}")).await;
    }
    assert_eq!(
        plans::create(&t.pool, id, input(4), week(), Utc::now())
            .await
            .unwrap()
            .unwrap_err(),
        PlanError::NotEnoughRecipes
    );

    let plan = plans::create(&t.pool, id, input(2), week(), Utc::now())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (plan.week_start_date, plan.total_servings, plan.status),
        (week(), 14, PlanStatus::Draft)
    );
    assert_eq!(plan.recipes.len(), 2);
    assert!(
        plan.recipes
            .iter()
            .all(|r| r.target_servings == 7 && r.status == PlanRecipeStatus::Suggested)
    );

    assert_eq!(
        plans::create(&t.pool, id, input(2), week(), Utc::now())
            .await
            .unwrap()
            .unwrap_err(),
        PlanError::PlanExists
    );
    let replaced = plans::create(
        &t.pool,
        id,
        PlanCreateInput {
            replace: true,
            ..input(3)
        },
        week(),
        Utc::now(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_ne!(replaced.id, plan.id);
    t.drop().await;
}

#[tokio::test]
async fn rotation_exclut_le_recent() {
    let t = TestDb::new().await;
    let id = user(&t.pool).await;
    let cooked = recipe(&t.pool, id, "Cuisinée hier").await;
    let fresh = recipe(&t.pool, id, "Jamais faite").await;
    // Historique : un ancien plan où « cooked » a été cuisinée hier.
    let old_plan: Uuid = sqlx::query_scalar(
        "INSERT INTO meal_plans (user_id, week_start_date, people_count, dish_count, days_count, total_servings, status)
         VALUES ($1, '2026-11-02', 1, 1, 7, 14, 'cooked') RETURNING id",
    )
    .bind(id)
    .fetch_one(&t.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO meal_plan_recipes (meal_plan_id, recipe_id, position, target_servings, status, cooked_at) VALUES ($1, $2, 1, 4, 'cooked', now() - interval '1 day')")
        .bind(old_plan)
        .bind(cooked)
        .execute(&t.pool)
        .await
        .unwrap();
    let plan = plans::create(&t.pool, id, input(1), week(), Utc::now())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(plan.recipes[0].recipe.id, fresh);
    t.drop().await;
}

#[tokio::test]
async fn cycle_accepter_echanger_remplacer_valider_cuisiner() {
    let t = TestDb::new().await;
    let id = user(&t.pool).await;
    for n in 0..4 {
        recipe(&t.pool, id, &format!("R{n}")).await;
    }
    let plan = plans::create(&t.pool, id, input(2), week(), Utc::now())
        .await
        .unwrap()
        .unwrap();
    let first = plan.recipes[0].id;

    let accepted = plans::update_recipe_status(&t.pool, id, plan.id, first, PlanRecipeAction::Accept)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(accepted.recipes[0].status, PlanRecipeStatus::Accepted);
    plans::update_recipe_status(&t.pool, id, plan.id, first, PlanRecipeAction::Unaccept)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        plans::update_recipe_status(&t.pool, id, plan.id, first, PlanRecipeAction::Cook)
            .await
            .unwrap()
            .unwrap_err(),
        PlanError::InvalidState,
        "pas de cuisson sur un brouillon"
    );

    // Échange : nouvelle recette jamais proposée dans ce plan.
    let before: Vec<Uuid> = plan.recipes.iter().map(|r| r.recipe.id).collect();
    let swapped = plans::swap_recipe(&t.pool, id, plan.id, first, Utc::now())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(swapped.recipes.len(), 2);
    assert!(!before.contains(&swapped.recipes[0].recipe.id));

    // Options : recherche exclut les actives ; suggestion exclut tout ce que le plan a touché.
    let search = plans::replacement_options(&t.pool, id, plan.id, ReplacementMode::Search, None, Utc::now())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(search.len(), 2);
    let suggest = plans::replacement_options(&t.pool, id, plan.id, ReplacementMode::Suggest, None, Utc::now())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(suggest.len(), 1);

    // Remplacement manuel : arrive déjà acceptée.
    let target = suggest[0].summary.id;
    let replaced = plans::replace_recipe(&t.pool, id, plan.id, swapped.recipes[0].id, target)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (replaced.recipes[0].recipe.id, replaced.recipes[0].status),
        (target, PlanRecipeStatus::Accepted)
    );
    // Échange épuisé : plus de candidat.
    assert_eq!(
        plans::swap_recipe(&t.pool, id, plan.id, replaced.recipes[1].id, Utc::now())
            .await
            .unwrap()
            .unwrap_err(),
        PlanError::NoMoreSuggestions
    );

    let validated = plans::validate(&t.pool, id, plan.id).await.unwrap().unwrap();
    assert_eq!(validated.status, PlanStatus::Validated);
    assert!(validated.recipes.iter().all(|r| r.status == PlanRecipeStatus::Accepted));
    assert_eq!(
        plans::validate(&t.pool, id, plan.id).await.unwrap().unwrap_err(),
        PlanError::InvalidState
    );

    for item in &validated.recipes {
        plans::update_recipe_status(&t.pool, id, plan.id, item.id, PlanRecipeAction::Cook)
            .await
            .unwrap()
            .unwrap();
    }
    let done = plans::get(&t.pool, id, plan.id).await.unwrap().unwrap();
    assert_eq!(done.status, PlanStatus::Cooked);
    assert!(done.recipes.iter().all(|r| r.cooked_at.is_some()));
    t.drop().await;
}

#[tokio::test]
async fn liste_epicerie_agregee_et_interactions() {
    let t = TestDb::new().await;
    let id = user(&t.pool).await;
    let mut a = recipe_input("Riz A", &[]);
    a.ingredients[0].quantity = 200.0; // 4 portions → 7 : ×1,75 = 350 g
    let mut b = recipe_input("Riz B", &[]);
    b.ingredients[0].quantity = 0.1;
    b.ingredients[0].unit = Some(Unit::Kg); // 100 g → 175 g
    recipes::create(&t.pool, id, &a).await.unwrap();
    recipes::create(&t.pool, id, &b).await.unwrap();

    let plan = plans::create(&t.pool, id, input(2), week(), Utc::now())
        .await
        .unwrap()
        .unwrap();
    assert!(
        grocery::list_for_week(&t.pool, id, week()).await.unwrap().is_none(),
        "pas de liste avant validation"
    );
    plans::validate(&t.pool, id, plan.id).await.unwrap().unwrap();

    let list = grocery::list_for_week(&t.pool, id, week()).await.unwrap().unwrap();
    assert_eq!(list.items.len(), 1);
    assert_eq!(
        (list.items[0].quantity, list.items[0].unit),
        (Some(525.0), Some(Unit::G))
    );
    assert_eq!(list.items[0].sources.len(), 2);

    // Préférence impériale à l'affichage.
    sqlx::query("UPDATE users SET unit_system = 'imperial' WHERE id = $1")
        .bind(id)
        .execute(&t.pool)
        .await
        .unwrap();
    let imperial = grocery::list_for_week(&t.pool, id, week()).await.unwrap().unwrap();
    assert_eq!(
        (imperial.items[0].quantity, imperial.items[0].unit),
        (Some(1.16), Some(Unit::Lb))
    );
    sqlx::query("UPDATE users SET unit_system = 'metric' WHERE id = $1")
        .bind(id)
        .execute(&t.pool)
        .await
        .unwrap();

    let item = list.items[0].id;
    let checked = grocery::update_item(
        &t.pool,
        id,
        item,
        &GroceryItemPatch {
            checked: Some(true),
            ..Default::default()
        },
        week(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(checked.checked_count, 1);

    let added = grocery::add_manual_item(
        &t.pool,
        id,
        &GroceryItemCreate {
            label: "Essuie-tout".into(),
            quantity: None,
            unit: None,
            aisle: Aisle::Household,
        },
        week(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(added.total_count, 2);
    let manual = added.items.iter().find(|i| i.is_manual).unwrap().id;
    let with_quantity = GroceryItemPatch {
        quantity: Some(Some(0.475)),
        unit: Some(Unit::Kg),
        label: Some("Riz brun".into()),
        ..Default::default()
    };
    let corrected = grocery::update_item(&t.pool, id, manual, &with_quantity, week())
        .await
        .unwrap()
        .unwrap();
    let manual_row = corrected.items.iter().find(|i| i.id == manual).unwrap();
    assert_eq!(
        (manual_row.quantity, manual_row.unit, manual_row.label.as_str()),
        (Some(475.0), Some(Unit::G), "Riz brun")
    );

    // Fusion : même unité de base → somme ; décochée ; isManual seulement si tout est manuel.
    let merged = grocery::merge_items(&t.pool, id, &[item, manual], week())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(merged.total_count, 1);
    let row = &merged.items[0];
    assert_eq!(
        (row.quantity, row.unit, row.checked, row.is_manual),
        (Some(1.0), Some(Unit::Kg), false, false)
    );
    assert_eq!(row.sources.len(), 2);
    assert!(
        grocery::merge_items(&t.pool, id, &[item, Uuid::new_v4()], week())
            .await
            .unwrap()
            .is_none()
    );

    let emptied = grocery::delete_item(&t.pool, id, item, week()).await.unwrap().unwrap();
    assert_eq!(emptied.total_count, 0);
    t.drop().await;
}

#[tokio::test]
async fn isolation_idor_plans_et_epicerie() {
    let t = TestDb::new().await;
    let owner = user(&t.pool).await;
    let intruder = user(&t.pool).await;
    recipe(&t.pool, owner, "R1").await;
    let foreign_recipe = recipe(&t.pool, intruder, "Autre compte").await;
    let plan = plans::create(&t.pool, owner, input(1), week(), Utc::now())
        .await
        .unwrap()
        .unwrap();
    let item = plan.recipes[0].id;

    assert!(plans::get(&t.pool, intruder, plan.id).await.unwrap().is_none());
    assert_eq!(
        plans::validate(&t.pool, intruder, plan.id).await.unwrap().unwrap_err(),
        PlanError::NotFound
    );
    assert_eq!(
        plans::swap_recipe(&t.pool, intruder, plan.id, item, Utc::now())
            .await
            .unwrap()
            .unwrap_err(),
        PlanError::NotFound
    );
    assert_eq!(
        plans::replace_recipe(&t.pool, owner, plan.id, item, foreign_recipe)
            .await
            .unwrap()
            .unwrap_err(),
        PlanError::NotFound,
        "la recette d'un autre compte ne peut pas entrer au plan"
    );

    plans::validate(&t.pool, owner, plan.id).await.unwrap().unwrap();
    let list = grocery::list_for_week(&t.pool, owner, week()).await.unwrap().unwrap();
    let grocery_item = list.items[0].id;
    assert!(
        grocery::list_for_week(&t.pool, intruder, week())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        grocery::update_item(
            &t.pool,
            intruder,
            grocery_item,
            &GroceryItemPatch {
                checked: Some(true),
                ..Default::default()
            },
            week()
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(
        grocery::delete_item(&t.pool, intruder, grocery_item, week())
            .await
            .unwrap()
            .is_none()
    );
    t.drop().await;
}
