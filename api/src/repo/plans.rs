//! Couche d'accès unique aux plans hebdomadaires : l'isolation multi-utilisateur
//! est garantie ICI (filtre par userId sur chaque requête).

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgConnection, PgPool};
use uuid::Uuid;

use super::grocery::generate_grocery_list;
use super::recipes::{self, RecipeListQuery, RecipeSummary};
use super::users::find_by_id;
use crate::domain::catalog::{PlanRecipeStatus, PlanStatus};
use crate::domain::rotation::{RotationCandidate, RotationOptions, compute_portions, pick_suggestions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanError {
    NotFound,
    PlanExists,
    NotEnoughRecipes,
    NoMoreSuggestions,
    InvalidState,
}

impl PlanError {
    pub fn status(self) -> u16 {
        match self {
            PlanError::NotFound => 404,
            _ => 409,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            PlanError::NotFound => "errors.notFound",
            PlanError::PlanExists => "errors.planExists",
            PlanError::NotEnoughRecipes => "errors.notEnoughRecipes",
            PlanError::NoMoreSuggestions => "errors.noMoreSuggestions",
            PlanError::InvalidState => "errors.invalidState",
        }
    }
}

pub type PlanResult<T> = Result<Result<T, PlanError>, sqlx::Error>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRecipeDto {
    pub id: Uuid,
    pub position: i32,
    pub status: PlanRecipeStatus,
    pub target_servings: i32,
    #[serde(serialize_with = "crate::domain::time::serialize_iso_opt")]
    pub cooked_at: Option<DateTime<Utc>>,
    pub recipe: RecipeSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MealPlanDto {
    pub id: Uuid,
    pub week_start_date: NaiveDate,
    pub people_count: i32,
    pub dish_count: i32,
    pub days_count: i32,
    pub total_servings: i32,
    pub status: PlanStatus,
    /// Rangées actives seulement (les « swapped » restent en base comme mémoire).
    pub recipes: Vec<PlanRecipeDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDefaults {
    pub dish_count: i32,
    pub people_count: i32,
    pub days_count: i32,
    pub rotation_weeks: i32,
    pub recipe_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanBundle {
    pub plan: Option<MealPlanDto>,
    pub defaults: PlanDefaults,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementOption {
    #[serde(flatten)]
    pub summary: RecipeSummary,
    #[serde(serialize_with = "crate::domain::time::serialize_iso_opt")]
    pub last_cooked_at: Option<DateTime<Utc>>,
    pub cooked_count: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct PlanCreateInput {
    pub dish_count: i32,
    pub people_count: i32,
    pub days_count: i32,
    /// Remplace le plan existant de la semaine (« Replanifier »).
    pub replace: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanRecipeAction {
    Accept,
    Unaccept,
    Cook,
}

#[derive(Debug, Clone, FromRow)]
struct PlanRow {
    id: Uuid,
    week_start_date: NaiveDate,
    people_count: i32,
    dish_count: i32,
    days_count: i32,
    total_servings: i32,
    status: PlanStatus,
}

#[derive(Debug, Clone, FromRow)]
struct PlanItemRow {
    id: Uuid,
    recipe_id: Uuid,
    position: i32,
    target_servings: i32,
    status: PlanRecipeStatus,
    cooked_at: Option<DateTime<Utc>>,
}

struct LoadedPlan {
    plan: PlanRow,
    items: Vec<PlanItemRow>,
}

/// Colonnes de `PlanRow`. Macro et non constante, pour composer les requêtes
/// par `concat!` : elles restent des `&'static str` fixées à la compilation
/// (voir `user_columns!` dans `users.rs`).
macro_rules! plan_columns {
    () => {
        "id, week_start_date, people_count, dish_count, days_count, total_servings, status"
    };
}

async fn load_items(db: &PgPool, plan_id: Uuid) -> Result<Vec<PlanItemRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, recipe_id, position, target_servings, status, cooked_at FROM meal_plan_recipes WHERE meal_plan_id = $1",
    )
    .bind(plan_id)
    .fetch_all(db)
    .await
}

async fn find_plan(db: &PgPool, user_id: Uuid, plan_id: Uuid) -> Result<Option<LoadedPlan>, sqlx::Error> {
    let plan: Option<PlanRow> = sqlx::query_as(concat!(
        "SELECT ",
        plan_columns!(),
        " FROM meal_plans WHERE id = $1 AND user_id = $2"
    ))
    .bind(plan_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    match plan {
        Some(plan) => Ok(Some(LoadedPlan {
            items: load_items(db, plan.id).await?,
            plan,
        })),
        None => Ok(None),
    }
}

/// Charge le plan en le VERROUILLANT (FOR UPDATE) dans la transaction de la
/// mutation : contrôle d'état et écriture voient le même état. Deux actions
/// simultanées (deux « Cuisinée ✓ », validation pendant un échange) se
/// sérialisent au lieu de se croiser.
async fn lock_plan(conn: &mut PgConnection, user_id: Uuid, plan_id: Uuid) -> Result<Option<LoadedPlan>, sqlx::Error> {
    let plan: Option<PlanRow> = sqlx::query_as(concat!(
        "SELECT ",
        plan_columns!(),
        " FROM meal_plans WHERE id = $1 AND user_id = $2 FOR UPDATE"
    ))
    .bind(plan_id)
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(plan) = plan else { return Ok(None) };
    let items = sqlx::query_as(
        "SELECT id, recipe_id, position, target_servings, status, cooked_at FROM meal_plan_recipes WHERE meal_plan_id = $1",
    )
    .bind(plan.id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Some(LoadedPlan { plan, items }))
}

async fn find_plan_for_week(db: &PgPool, user_id: Uuid, week: NaiveDate) -> Result<Option<LoadedPlan>, sqlx::Error> {
    let plan: Option<PlanRow> = sqlx::query_as(concat!(
        "SELECT ",
        plan_columns!(),
        " FROM meal_plans WHERE user_id = $1 AND week_start_date = $2"
    ))
    .bind(user_id)
    .bind(week)
    .fetch_optional(db)
    .await?;
    match plan {
        Some(plan) => Ok(Some(LoadedPlan {
            items: load_items(db, plan.id).await?,
            plan,
        })),
        None => Ok(None),
    }
}

async fn to_dto(db: &PgPool, loaded: LoadedPlan) -> Result<MealPlanDto, sqlx::Error> {
    let mut active: Vec<PlanItemRow> = loaded
        .items
        .into_iter()
        .filter(|i| i.status != PlanRecipeStatus::Swapped)
        .collect();
    active.sort_by_key(|i| i.position);
    let ids: Vec<Uuid> = active.iter().map(|i| i.recipe_id).collect();
    let summaries = recipes::summaries_by_ids(db, &ids).await?;
    let plan = loaded.plan;
    Ok(MealPlanDto {
        id: plan.id,
        week_start_date: plan.week_start_date,
        people_count: plan.people_count,
        dish_count: plan.dish_count,
        days_count: plan.days_count,
        total_servings: plan.total_servings,
        status: plan.status,
        recipes: active
            .into_iter()
            .filter_map(|item| {
                let recipe = summaries.get(&item.recipe_id).cloned()?;
                Some(PlanRecipeDto {
                    id: item.id,
                    position: item.position,
                    status: item.status,
                    target_servings: item.target_servings,
                    cooked_at: item.cooked_at,
                    recipe,
                })
            })
            .collect(),
    })
}

struct CookHistory {
    last_cooked_at: Option<DateTime<Utc>>,
    cooked_count: i64,
}

/// Dernière cuisson et nombre de cuissons par recette (source : historique des plans).
async fn cook_history(db: &PgPool, user_id: Uuid) -> Result<HashMap<Uuid, CookHistory>, sqlx::Error> {
    let rows: Vec<(Uuid, Option<DateTime<Utc>>, i64)> = sqlx::query_as(
        "SELECT mpr.recipe_id, max(mpr.cooked_at), count(mpr.id)
         FROM meal_plan_recipes mpr JOIN meal_plans mp ON mp.id = mpr.meal_plan_id
         WHERE mp.user_id = $1 AND mpr.status = 'cooked'
         GROUP BY mpr.recipe_id",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, last_cooked_at, cooked_count)| {
            (
                id,
                CookHistory {
                    last_cooked_at,
                    cooked_count,
                },
            )
        })
        .collect())
}

async fn candidates(db: &PgPool, user_id: Uuid) -> Result<Vec<RotationCandidate>, sqlx::Error> {
    let rows: Vec<(Uuid, DateTime<Utc>)> =
        sqlx::query_as("SELECT id, created_at FROM recipes WHERE user_id = $1 AND deleted_at IS NULL")
            .bind(user_id)
            .fetch_all(db)
            .await?;
    let tag_rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT rt.recipe_id, t.name FROM recipe_tags rt JOIN tags t ON t.id = rt.tag_id WHERE t.user_id = $1",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;
    let mut tags: HashMap<Uuid, Vec<String>> = HashMap::new();
    for (recipe_id, name) in tag_rows {
        tags.entry(recipe_id).or_default().push(name);
    }
    let history = cook_history(db, user_id).await?;
    Ok(rows
        .into_iter()
        .map(|(id, created_at)| RotationCandidate {
            id,
            tags: tags.remove(&id).unwrap_or_default(),
            last_cooked_at: history.get(&id).and_then(|h| h.last_cooked_at),
            created_at,
        })
        .collect())
}

async fn rotation_weeks(db: &PgPool, user_id: Uuid) -> Result<i32, sqlx::Error> {
    Ok(find_by_id(db, user_id).await?.map(|u| u.rotation_weeks).unwrap_or(3))
}

pub async fn get(db: &PgPool, user_id: Uuid, plan_id: Uuid) -> Result<Option<MealPlanDto>, sqlx::Error> {
    match find_plan(db, user_id, plan_id).await? {
        Some(loaded) => Ok(Some(to_dto(db, loaded).await?)),
        None => Ok(None),
    }
}

async fn reload(db: &PgPool, user_id: Uuid, plan_id: Uuid) -> PlanResult<MealPlanDto> {
    Ok(get(db, user_id, plan_id).await?.ok_or(PlanError::NotFound))
}

pub async fn current_bundle(db: &PgPool, user_id: Uuid, week: NaiveDate) -> Result<PlanBundle, sqlx::Error> {
    let plan = match find_plan_for_week(db, user_id, week).await? {
        Some(loaded) => Some(to_dto(db, loaded).await?),
        None => None,
    };
    let user = find_by_id(db, user_id).await?;
    let defaults = PlanDefaults {
        dish_count: user.as_ref().map_or(4, |u| u.default_dish_count),
        people_count: user.as_ref().map_or(1, |u| u.default_people_count),
        days_count: user.as_ref().map_or(7, |u| u.default_days_count),
        rotation_weeks: user.as_ref().map_or(3, |u| u.rotation_weeks),
        recipe_count: recipes::count_active(db, user_id).await?,
    };
    Ok(PlanBundle { plan, defaults })
}

pub async fn create(
    db: &PgPool,
    user_id: Uuid,
    input: PlanCreateInput,
    week: NaiveDate,
    now: DateTime<Utc>,
) -> PlanResult<MealPlanDto> {
    // Sérialise les créations concurrentes du même utilisateur (double clic,
    // deux onglets) : sans ce verrou, les deux lisaient « pas de plan » et la
    // seconde échouait en 500 sur la contrainte d'unicité.
    let mut tx = db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let existing: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM meal_plans WHERE user_id = $1 AND week_start_date = $2")
            .bind(user_id)
            .bind(week)
            .fetch_optional(&mut *tx)
            .await?;
    if existing.is_some() && !input.replace {
        return Ok(Err(PlanError::PlanExists));
    }

    let pool = candidates(db, user_id).await?;
    if pool.len() < input.dish_count as usize {
        return Ok(Err(PlanError::NotEnoughRecipes));
    }
    let (total_servings, servings_per_dish) = compute_portions(input.days_count, input.people_count, input.dish_count);
    let suggestions = pick_suggestions(
        &pool,
        &RotationOptions {
            count: input.dish_count as usize,
            rotation_weeks: rotation_weeks(db, user_id).await?,
            exclude_ids: None,
            now,
        },
    );

    if let Some(existing_id) = existing {
        sqlx::query("DELETE FROM meal_plans WHERE id = $1 AND user_id = $2")
            .bind(existing_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }
    let plan_id: Uuid = sqlx::query_scalar(
        "INSERT INTO meal_plans (user_id, week_start_date, people_count, dish_count, days_count, total_servings)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(user_id)
    .bind(week)
    .bind(input.people_count)
    .bind(input.dish_count)
    .bind(input.days_count)
    .bind(total_servings)
    .fetch_one(&mut *tx)
    .await?;
    for (index, recipe_id) in suggestions.iter().enumerate() {
        sqlx::query("INSERT INTO meal_plan_recipes (meal_plan_id, recipe_id, position, target_servings) VALUES ($1, $2, $3, $4)")
            .bind(plan_id)
            .bind(recipe_id)
            .bind(index as i32 + 1)
            .bind(servings_per_dish)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    reload(db, user_id, plan_id).await
}

pub async fn update_recipe_status(
    db: &PgPool,
    user_id: Uuid,
    plan_id: Uuid,
    item_id: Uuid,
    action: PlanRecipeAction,
) -> PlanResult<MealPlanDto> {
    let mut tx = db.begin().await?;
    let Some(loaded) = lock_plan(&mut tx, user_id, plan_id).await? else {
        return Ok(Err(PlanError::NotFound));
    };
    let Some(item) = loaded.items.iter().find(|i| i.id == item_id) else {
        return Ok(Err(PlanError::NotFound));
    };
    let allowed = match action {
        PlanRecipeAction::Accept => {
            loaded.plan.status == PlanStatus::Draft && item.status == PlanRecipeStatus::Suggested
        }
        PlanRecipeAction::Unaccept => {
            loaded.plan.status == PlanStatus::Draft && item.status == PlanRecipeStatus::Accepted
        }
        PlanRecipeAction::Cook => {
            loaded.plan.status == PlanStatus::Validated && item.status == PlanRecipeStatus::Accepted
        }
    };
    if !allowed {
        return Ok(Err(PlanError::InvalidState));
    }

    match action {
        PlanRecipeAction::Cook => {
            sqlx::query("UPDATE meal_plan_recipes SET status = 'cooked', cooked_at = now() WHERE id = $1")
                .bind(item_id)
                .execute(&mut *tx)
                .await?;
            let still_accepted = loaded
                .items
                .iter()
                .any(|i| i.id != item_id && i.status == PlanRecipeStatus::Accepted);
            if !still_accepted {
                sqlx::query("UPDATE meal_plans SET status = 'cooked' WHERE id = $1")
                    .bind(plan_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        PlanRecipeAction::Accept | PlanRecipeAction::Unaccept => {
            let status = if action == PlanRecipeAction::Accept {
                PlanRecipeStatus::Accepted
            } else {
                PlanRecipeStatus::Suggested
            };
            sqlx::query("UPDATE meal_plan_recipes SET status = $2 WHERE id = $1")
                .bind(item_id)
                .bind(status)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    reload(db, user_id, plan_id).await
}

fn is_replaceable(plan: &PlanRow, item: &PlanItemRow) -> bool {
    plan.status == PlanStatus::Draft && matches!(item.status, PlanRecipeStatus::Suggested | PlanRecipeStatus::Accepted)
}

pub async fn swap_recipe(
    db: &PgPool,
    user_id: Uuid,
    plan_id: Uuid,
    item_id: Uuid,
    now: DateTime<Utc>,
) -> PlanResult<MealPlanDto> {
    let mut tx = db.begin().await?;
    let Some(loaded) = lock_plan(&mut tx, user_id, plan_id).await? else {
        return Ok(Err(PlanError::NotFound));
    };
    let Some(item) = loaded.items.iter().find(|i| i.id == item_id).cloned() else {
        return Ok(Err(PlanError::NotFound));
    };
    if !is_replaceable(&loaded.plan, &item) {
        return Ok(Err(PlanError::InvalidState));
    }

    // Mémoire du plan : tout ce qui y est ou y a été (même écarté) est exclu.
    let touched: HashSet<Uuid> = loaded.items.iter().map(|i| i.recipe_id).collect();
    let pool: Vec<RotationCandidate> = candidates(db, user_id)
        .await?
        .into_iter()
        .filter(|c| !touched.contains(&c.id))
        .collect();
    let picked = pick_suggestions(
        &pool,
        &RotationOptions {
            count: 1,
            rotation_weeks: rotation_weeks(db, user_id).await?,
            exclude_ids: None,
            now,
        },
    );
    let Some(new_recipe_id) = picked.first().copied() else {
        return Ok(Err(PlanError::NoMoreSuggestions));
    };

    sqlx::query("UPDATE meal_plan_recipes SET status = 'swapped' WHERE id = $1")
        .bind(item_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO meal_plan_recipes (meal_plan_id, recipe_id, position, target_servings) VALUES ($1, $2, $3, $4)",
    )
    .bind(plan_id)
    .bind(new_recipe_id)
    .bind(item.position)
    .bind(item.target_servings)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    reload(db, user_id, plan_id).await
}

pub async fn replace_recipe(
    db: &PgPool,
    user_id: Uuid,
    plan_id: Uuid,
    item_id: Uuid,
    recipe_id: Uuid,
) -> PlanResult<MealPlanDto> {
    let mut tx = db.begin().await?;
    let Some(loaded) = lock_plan(&mut tx, user_id, plan_id).await? else {
        return Ok(Err(PlanError::NotFound));
    };
    let Some(item) = loaded.items.iter().find(|i| i.id == item_id).cloned() else {
        return Ok(Err(PlanError::NotFound));
    };
    if !is_replaceable(&loaded.plan, &item) {
        return Ok(Err(PlanError::InvalidState));
    }
    let owned: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM recipes WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL")
            .bind(recipe_id)
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    if owned.is_none() {
        return Ok(Err(PlanError::NotFound));
    }
    let already_active = loaded
        .items
        .iter()
        .any(|i| i.status != PlanRecipeStatus::Swapped && i.recipe_id == recipe_id);
    if already_active {
        return Ok(Err(PlanError::InvalidState));
    }

    sqlx::query("UPDATE meal_plan_recipes SET status = 'swapped' WHERE id = $1")
        .bind(item_id)
        .execute(&mut *tx)
        .await?;
    // Choix manuel = intention claire : la rangée arrive déjà validée.
    sqlx::query(
        "INSERT INTO meal_plan_recipes (meal_plan_id, recipe_id, position, target_servings, status) VALUES ($1, $2, $3, $4, 'accepted')",
    )
    .bind(plan_id)
    .bind(recipe_id)
    .bind(item.position)
    .bind(item.target_servings)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    reload(db, user_id, plan_id).await
}

pub async fn validate(db: &PgPool, user_id: Uuid, plan_id: Uuid) -> PlanResult<MealPlanDto> {
    let mut tx = db.begin().await?;
    let Some(loaded) = lock_plan(&mut tx, user_id, plan_id).await? else {
        return Ok(Err(PlanError::NotFound));
    };
    if loaded.plan.status != PlanStatus::Draft {
        return Ok(Err(PlanError::InvalidState));
    }
    let has_active = loaded
        .items
        .iter()
        .any(|i| matches!(i.status, PlanRecipeStatus::Suggested | PlanRecipeStatus::Accepted));
    if !has_active {
        return Ok(Err(PlanError::InvalidState));
    }

    sqlx::query("UPDATE meal_plan_recipes SET status = 'accepted' WHERE meal_plan_id = $1 AND status = 'suggested'")
        .bind(plan_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE meal_plans SET status = 'validated' WHERE id = $1")
        .bind(plan_id)
        .execute(&mut *tx)
        .await?;
    // La liste d'épicerie naît avec la validation (PRD F6) — même transaction.
    generate_grocery_list(&mut tx, plan_id).await?;
    tx.commit().await?;
    reload(db, user_id, plan_id).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementMode {
    Search,
    Suggest,
}

pub async fn replacement_options(
    db: &PgPool,
    user_id: Uuid,
    plan_id: Uuid,
    mode: ReplacementMode,
    q: Option<String>,
    now: DateTime<Utc>,
) -> PlanResult<Vec<ReplacementOption>> {
    let Some(loaded) = find_plan(db, user_id, plan_id).await? else {
        return Ok(Err(PlanError::NotFound));
    };
    let history = cook_history(db, user_id).await?;
    let augment = |summary: RecipeSummary| ReplacementOption {
        last_cooked_at: history.get(&summary.id).and_then(|h| h.last_cooked_at),
        cooked_count: history.get(&summary.id).map_or(0, |h| h.cooked_count),
        summary,
    };

    if mode == ReplacementMode::Search {
        let active: HashSet<Uuid> = loaded
            .items
            .iter()
            .filter(|i| i.status != PlanRecipeStatus::Swapped)
            .map(|i| i.recipe_id)
            .collect();
        let found = recipes::list(
            db,
            user_id,
            &RecipeListQuery {
                q,
                ..Default::default()
            },
        )
        .await?;
        return Ok(Ok(found
            .into_iter()
            .filter(|s| !active.contains(&s.id))
            .map(augment)
            .take(20)
            .collect()));
    }

    // Mode « Me suggérer autre chose » : petit lot choisi par la rotation, en
    // excluant tout ce que le plan a déjà touché (même les écartées).
    let touched: HashSet<Uuid> = loaded.items.iter().map(|i| i.recipe_id).collect();
    let pool: Vec<RotationCandidate> = candidates(db, user_id)
        .await?
        .into_iter()
        .filter(|c| !touched.contains(&c.id))
        .collect();
    let ids = pick_suggestions(
        &pool,
        &RotationOptions {
            count: 3,
            rotation_weeks: rotation_weeks(db, user_id).await?,
            exclude_ids: None,
            now,
        },
    );
    if ids.is_empty() {
        return Ok(Ok(Vec::new()));
    }
    let mut summaries = recipes::summaries_by_ids(db, &ids).await?;
    Ok(Ok(ids
        .iter()
        .filter_map(|id| summaries.remove(id))
        .map(augment)
        .collect()))
}
