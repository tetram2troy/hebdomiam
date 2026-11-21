//! Recettes : CRUD, recherche, tags, suppression douce, isolation (IDOR).

mod common;

use common::{TestDb, recipe_input, user};
use hebdomiam::domain::catalog::Unit;
use hebdomiam::repo::recipes::{self, RecipeListQuery};

#[tokio::test]
async fn cree_et_relit_avec_decimales_et_tags() {
    let t = TestDb::new().await;
    let id = user(&t.pool).await;
    let mut input = recipe_input("Chili végé", &["végé", "mijoteuse"]);
    input.ingredients[0].quantity = 0.33;
    input.ingredients[0].unit = Some(Unit::Cup);
    let created = recipes::create(&t.pool, id, &input).await.unwrap();
    assert_eq!(created.ingredients[0].quantity, 0.33, "decimal, jamais float");
    assert_eq!(created.summary.tags, ["mijoteuse", "végé"], "tags triés");
    assert_eq!(created.steps[0].position, 1);

    // Tags réutilisés sans doublon (unicité userId + nom).
    recipes::create(&t.pool, id, &recipe_input("Autre", &["végé"]))
        .await
        .unwrap();
    let tags: i64 = sqlx::query_scalar("SELECT count(*) FROM tags")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(tags, 2);
    let listed = recipes::list_tags(&t.pool, id).await.unwrap();
    assert_eq!((listed[0].name.as_str(), listed[0].recipe_count), ("végé", 2));
    t.drop().await;
}

#[tokio::test]
async fn recherche_par_titre_ingredient_tag_et_temps() {
    let t = TestDb::new().await;
    let id = user(&t.pool).await;
    recipes::create(&t.pool, id, &recipe_input("Soupe aux pois", &["soupe"]))
        .await
        .unwrap();
    let mut long = recipe_input("Ragoût", &[]);
    long.cook_minutes = Some(180);
    long.ingredients[0].name = "bœuf haché".into();
    recipes::create(&t.pool, id, &long).await.unwrap();

    let query = |q: Option<&str>, tag: Option<&str>, max: Option<i32>| RecipeListQuery {
        q: q.map(str::to_string),
        tag: tag.map(str::to_string),
        max_minutes: max,
    };
    assert_eq!(
        recipes::list(&t.pool, id, &query(Some("soupe"), None, None))
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        recipes::list(&t.pool, id, &query(Some("bœuf"), None, None))
            .await
            .unwrap()[0]
            .title,
        "Ragoût"
    );
    assert_eq!(
        recipes::list(&t.pool, id, &query(Some("%"), None, None))
            .await
            .unwrap()
            .len(),
        0,
        "joker échappé"
    );
    assert_eq!(
        recipes::list(&t.pool, id, &query(None, Some("soupe"), None))
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        recipes::list(&t.pool, id, &query(None, None, Some(60)))
            .await
            .unwrap()
            .len(),
        1
    );
    t.drop().await;
}

#[tokio::test]
async fn mise_a_jour_sans_orphelins_et_suppression_douce() {
    let t = TestDb::new().await;
    let id = user(&t.pool).await;
    let created = recipes::create(&t.pool, id, &recipe_input("Avant", &["a"]))
        .await
        .unwrap();
    let mut input = recipe_input("Après", &["b"]);
    input.steps = vec!["Un.".into(), "Deux.".into()];
    let updated = recipes::update(&t.pool, id, created.summary.id, &input)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((updated.summary.title.as_str(), updated.steps.len()), ("Après", 2));
    let steps: i64 = sqlx::query_scalar("SELECT count(*) FROM recipe_steps")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(steps, 2);

    assert!(recipes::soft_delete(&t.pool, id, created.summary.id).await.unwrap());
    assert!(recipes::get(&t.pool, id, created.summary.id).await.unwrap().is_none());
    assert!(
        recipes::list(&t.pool, id, &RecipeListQuery::default())
            .await
            .unwrap()
            .is_empty()
    );
    assert!(recipes::list_tags(&t.pool, id).await.unwrap().is_empty());
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM recipes")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(rows, 1, "la ligne reste en base");
    t.drop().await;
}

#[tokio::test]
async fn isolation_idor() {
    let t = TestDb::new().await;
    let owner = user(&t.pool).await;
    let intruder = user(&t.pool).await;
    let created = recipes::create(&t.pool, owner, &recipe_input("Secrète", &["privé"]))
        .await
        .unwrap();
    recipes::create(&t.pool, intruder, &recipe_input("À moi", &["privé"]))
        .await
        .unwrap();

    assert!(
        recipes::get(&t.pool, intruder, created.summary.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        recipes::update(&t.pool, intruder, created.summary.id, &recipe_input("Volée", &[]))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !recipes::soft_delete(&t.pool, intruder, created.summary.id)
            .await
            .unwrap()
    );
    // Le filtre par tag ne fuit pas les tags homonymes d'un autre compte.
    let by_tag = recipes::list(
        &t.pool,
        intruder,
        &RecipeListQuery {
            tag: Some("privé".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(by_tag.len(), 1);
    assert_eq!(by_tag[0].title, "À moi");
    assert_eq!(
        recipes::get(&t.pool, owner, created.summary.id)
            .await
            .unwrap()
            .unwrap()
            .summary
            .title,
        "Secrète"
    );
    t.drop().await;
}
