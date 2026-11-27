//! Couche d'accès unique aux recettes : l'isolation multi-utilisateur (filtre
//! par userId + deleted_at IS NULL) est garantie ICI (PRD §6/§8).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use serde::Serialize;
use sqlx::{FromRow, PgConnection, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::domain::catalog::{Aisle, Unit};
use crate::domain::collate;

const LIST_LIMIT: i64 = 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeSummary {
    pub id: Uuid,
    pub title: String,
    pub image_url: Option<String>,
    pub base_servings: i32,
    pub prep_minutes: Option<i32>,
    pub cook_minutes: Option<i32>,
    pub freezable: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeIngredientDto {
    pub id: Uuid,
    pub position: i32,
    pub quantity: f64,
    pub unit: Option<Unit>,
    pub name: String,
    pub aisle: Aisle,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecipeStepDto {
    pub id: Uuid,
    pub position: i32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeDetail {
    #[serde(flatten)]
    pub summary: RecipeSummary,
    pub description: Option<String>,
    pub source_url: Option<String>,
    pub notes: Option<String>,
    #[serde(serialize_with = "crate::domain::time::serialize_iso")]
    pub created_at: DateTime<Utc>,
    #[serde(serialize_with = "crate::domain::time::serialize_iso")]
    pub updated_at: DateTime<Utc>,
    pub steps: Vec<RecipeStepDto>,
    pub ingredients: Vec<RecipeIngredientDto>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TagSummary {
    pub name: String,
    pub recipe_count: i64,
}

/// Entrée validée (création / mise à jour).
#[derive(Debug, Clone)]
pub struct RecipeInput {
    pub title: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub base_servings: i32,
    pub prep_minutes: Option<i32>,
    pub cook_minutes: Option<i32>,
    pub source_url: Option<String>,
    pub freezable: bool,
    pub notes: Option<String>,
    pub ingredients: Vec<IngredientInput>,
    pub steps: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IngredientInput {
    pub quantity: f64,
    pub unit: Option<Unit>,
    pub name: String,
    pub aisle: Aisle,
}

#[derive(Debug, Default, Clone)]
pub struct RecipeListQuery {
    pub q: Option<String>,
    pub tag: Option<String>,
    pub max_minutes: Option<i32>,
}

#[derive(FromRow)]
struct SummaryRow {
    id: Uuid,
    title: String,
    image_url: Option<String>,
    base_servings: i32,
    prep_minutes: Option<i32>,
    cook_minutes: Option<i32>,
    freezable: bool,
}

/// Colonnes de `SummaryRow`. Macro et non constante, pour composer les
/// requêtes par `concat!` : elles restent des `&'static str` fixées à la
/// compilation (voir `user_columns!` dans `users.rs`).
macro_rules! summary_columns {
    () => {
        "r.id, r.title, r.image_url, r.base_servings, r.prep_minutes, r.cook_minutes, r.freezable"
    };
}

/// Quantité décimale (numeric) → nombre JSON.
pub fn decimal_to_f64(value: Decimal) -> f64 {
    value.to_f64().unwrap_or(0.0)
}

/// Nombre → numeric(…, 2), arrondi au centième (équivalent de toFixed(2)).
pub fn f64_to_decimal(value: f64) -> Decimal {
    Decimal::from_f64(value)
        .unwrap_or_default()
        .round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
}

/// Tags triés de chaque recette demandée (pas de filtre d'appartenance :
/// les ids viennent toujours d'une requête déjà scopée sur l'utilisateur).
async fn tags_by_recipe(db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, Vec<String>>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT rt.recipe_id, t.name FROM recipe_tags rt JOIN tags t ON t.id = rt.tag_id WHERE rt.recipe_id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(db)
    .await?;
    let mut map: HashMap<Uuid, Vec<String>> = HashMap::new();
    for (recipe_id, name) in rows {
        map.entry(recipe_id).or_default().push(name);
    }
    for tags in map.values_mut() {
        tags.sort_by(|a, b| collate::compare(a, b));
    }
    Ok(map)
}

async fn to_summaries(db: &PgPool, rows: Vec<SummaryRow>) -> Result<Vec<RecipeSummary>, sqlx::Error> {
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut tags = tags_by_recipe(db, &ids).await?;
    Ok(rows
        .into_iter()
        .map(|r| RecipeSummary {
            tags: tags.remove(&r.id).unwrap_or_default(),
            id: r.id,
            title: r.title,
            image_url: r.image_url,
            base_servings: r.base_servings,
            prep_minutes: r.prep_minutes,
            cook_minutes: r.cook_minutes,
            freezable: r.freezable,
        })
        .collect())
}

/// Résumés des recettes données, dans l'ordre demandé (usage interne : les
/// ids proviennent d'un plan déjà scopé sur l'utilisateur ; une recette
/// supprimée depuis reste affichable dans l'historique du plan).
pub async fn summaries_by_ids(db: &PgPool, ids: &[Uuid]) -> Result<HashMap<Uuid, RecipeSummary>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<SummaryRow> = sqlx::query_as(concat!(
        "SELECT ",
        summary_columns!(),
        " FROM recipes r WHERE r.id = ANY($1)"
    ))
    .bind(ids)
    .fetch_all(db)
    .await?;
    Ok(to_summaries(db, rows).await?.into_iter().map(|s| (s.id, s)).collect())
}

fn escape_like(text: &str) -> String {
    text.replace('%', "\\%").replace('_', "\\_")
}

pub async fn list(db: &PgPool, user_id: Uuid, query: &RecipeListQuery) -> Result<Vec<RecipeSummary>, sqlx::Error> {
    let mut sql: QueryBuilder<Postgres> = QueryBuilder::new(concat!(
        "SELECT ",
        summary_columns!(),
        " FROM recipes r WHERE r.user_id = "
    ));
    sql.push_bind(user_id).push(" AND r.deleted_at IS NULL");

    if let Some(q) = &query.q {
        let pattern = format!("%{}%", escape_like(q));
        sql.push(" AND (r.title ILIKE ")
            .push_bind(pattern.clone())
            .push(" OR EXISTS (SELECT 1 FROM recipe_ingredients ri WHERE ri.recipe_id = r.id AND ri.name ILIKE ")
            .push_bind(pattern)
            .push("))");
    }
    if let Some(tag) = &query.tag {
        sql.push(
            " AND r.id IN (SELECT rt.recipe_id FROM recipe_tags rt JOIN tags t ON t.id = rt.tag_id WHERE t.user_id = ",
        )
        .push_bind(user_id)
        .push(" AND t.name = ")
        .push_bind(tag.clone())
        .push(")");
    }
    if let Some(max) = query.max_minutes {
        sql.push(" AND coalesce(r.prep_minutes, 0) + coalesce(r.cook_minutes, 0) <= ")
            .push_bind(max);
    }
    sql.push(" ORDER BY r.created_at DESC LIMIT ").push_bind(LIST_LIMIT);

    let rows: Vec<SummaryRow> = sql.build_query_as().fetch_all(db).await?;
    to_summaries(db, rows).await
}

#[derive(FromRow)]
struct DetailRow {
    id: Uuid,
    title: String,
    image_url: Option<String>,
    base_servings: i32,
    prep_minutes: Option<i32>,
    cook_minutes: Option<i32>,
    freezable: bool,
    description: Option<String>,
    source_url: Option<String>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

pub async fn get(db: &PgPool, user_id: Uuid, recipe_id: Uuid) -> Result<Option<RecipeDetail>, sqlx::Error> {
    let row: Option<DetailRow> = sqlx::query_as(
        "SELECT id, title, image_url, base_servings, prep_minutes, cook_minutes, freezable,
                description, source_url, notes, created_at, updated_at
         FROM recipes WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(recipe_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    let Some(row) = row else { return Ok(None) };

    let steps: Vec<(Uuid, i32, String)> =
        sqlx::query_as("SELECT id, position, text FROM recipe_steps WHERE recipe_id = $1 ORDER BY position")
            .bind(recipe_id)
            .fetch_all(db)
            .await?;
    let ingredients: Vec<(Uuid, i32, Decimal, Option<Unit>, String, Aisle)> = sqlx::query_as(
        "SELECT id, position, quantity, unit, name, aisle FROM recipe_ingredients WHERE recipe_id = $1 ORDER BY position",
    )
    .bind(recipe_id)
    .fetch_all(db)
    .await?;
    let tags = tags_by_recipe(db, &[recipe_id])
        .await?
        .remove(&recipe_id)
        .unwrap_or_default();

    Ok(Some(RecipeDetail {
        summary: RecipeSummary {
            id: row.id,
            title: row.title,
            image_url: row.image_url,
            base_servings: row.base_servings,
            prep_minutes: row.prep_minutes,
            cook_minutes: row.cook_minutes,
            freezable: row.freezable,
            tags,
        },
        description: row.description,
        source_url: row.source_url,
        notes: row.notes,
        created_at: row.created_at,
        updated_at: row.updated_at,
        steps: steps
            .into_iter()
            .map(|(id, position, text)| RecipeStepDto { id, position, text })
            .collect(),
        ingredients: ingredients
            .into_iter()
            .map(|(id, position, quantity, unit, name, aisle)| RecipeIngredientDto {
                id,
                position,
                quantity: decimal_to_f64(quantity),
                unit,
                name,
                aisle,
            })
            .collect(),
    }))
}

pub async fn create(db: &PgPool, user_id: Uuid, input: &RecipeInput) -> Result<RecipeDetail, sqlx::Error> {
    let mut tx = db.begin().await?;
    let recipe_id: Uuid = sqlx::query_scalar(
        "INSERT INTO recipes (user_id, title, description, image_url, base_servings, prep_minutes, cook_minutes, source_url, freezable, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
    )
    .bind(user_id)
    .bind(&input.title)
    .bind(&input.description)
    .bind(&input.image_url)
    .bind(input.base_servings)
    .bind(input.prep_minutes)
    .bind(input.cook_minutes)
    .bind(&input.source_url)
    .bind(input.freezable)
    .bind(&input.notes)
    .fetch_one(&mut *tx)
    .await?;
    insert_children(&mut tx, user_id, recipe_id, input).await?;
    tx.commit().await?;
    Ok(get(db, user_id, recipe_id).await?.expect("recette tout juste créée"))
}

pub async fn update(
    db: &PgPool,
    user_id: Uuid,
    recipe_id: Uuid,
    input: &RecipeInput,
) -> Result<Option<RecipeDetail>, sqlx::Error> {
    let mut tx = db.begin().await?;
    let updated: Option<Uuid> = sqlx::query_scalar(
        "UPDATE recipes SET title = $3, description = $4, image_url = $5, base_servings = $6, prep_minutes = $7,
                cook_minutes = $8, source_url = $9, freezable = $10, notes = $11, updated_at = now()
         WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL RETURNING id",
    )
    .bind(recipe_id)
    .bind(user_id)
    .bind(&input.title)
    .bind(&input.description)
    .bind(&input.image_url)
    .bind(input.base_servings)
    .bind(input.prep_minutes)
    .bind(input.cook_minutes)
    .bind(&input.source_url)
    .bind(input.freezable)
    .bind(&input.notes)
    .fetch_optional(&mut *tx)
    .await?;
    if updated.is_none() {
        return Ok(None);
    }
    // Trois requêtes littérales plutôt qu'un nom de table interpolé : le texte
    // SQL reste fixé à la compilation.
    for delete_children in [
        "DELETE FROM recipe_steps WHERE recipe_id = $1",
        "DELETE FROM recipe_ingredients WHERE recipe_id = $1",
        "DELETE FROM recipe_tags WHERE recipe_id = $1",
    ] {
        sqlx::query(delete_children).bind(recipe_id).execute(&mut *tx).await?;
    }
    insert_children(&mut tx, user_id, recipe_id, input).await?;
    tx.commit().await?;
    get(db, user_id, recipe_id).await
}

pub async fn soft_delete(db: &PgPool, user_id: Uuid, recipe_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE recipes SET deleted_at = now(), updated_at = now() WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(recipe_id)
    .bind(user_id)
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn list_tags(db: &PgPool, user_id: Uuid) -> Result<Vec<TagSummary>, sqlx::Error> {
    sqlx::query_as(
        "SELECT t.name, count(r.id) AS recipe_count
         FROM tags t
         JOIN recipe_tags rt ON rt.tag_id = t.id
         JOIN recipes r ON r.id = rt.recipe_id AND r.deleted_at IS NULL
         WHERE t.user_id = $1
         GROUP BY t.name
         ORDER BY count(r.id) DESC, t.name ASC",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
}

/// Nombre de recettes actives (le client peut prévenir avant de proposer).
pub async fn count_active(db: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM recipes WHERE user_id = $1 AND deleted_at IS NULL")
        .bind(user_id)
        .fetch_one(db)
        .await
}

async fn insert_children(
    conn: &mut PgConnection,
    user_id: Uuid,
    recipe_id: Uuid,
    input: &RecipeInput,
) -> Result<(), sqlx::Error> {
    let mut steps = QueryBuilder::<Postgres>::new("INSERT INTO recipe_steps (recipe_id, position, text) ");
    steps.push_values(input.steps.iter().enumerate(), |mut row, (index, text)| {
        row.push_bind(recipe_id)
            .push_bind(index as i32 + 1)
            .push_bind(text.clone());
    });
    steps.build().execute(&mut *conn).await?;

    let mut ingredients = QueryBuilder::<Postgres>::new(
        "INSERT INTO recipe_ingredients (recipe_id, position, quantity, unit, name, aisle) ",
    );
    ingredients.push_values(input.ingredients.iter().enumerate(), |mut row, (index, ingredient)| {
        row.push_bind(recipe_id)
            .push_bind(index as i32 + 1)
            .push_bind(f64_to_decimal(ingredient.quantity))
            .push_bind(ingredient.unit)
            .push_bind(ingredient.name.clone())
            .push_bind(ingredient.aisle);
    });
    ingredients.build().execute(&mut *conn).await?;

    if !input.tags.is_empty() {
        sqlx::query("INSERT INTO tags (user_id, name) SELECT $1, unnest($2::text[]) ON CONFLICT DO NOTHING")
            .bind(user_id)
            .bind(&input.tags)
            .execute(&mut *conn)
            .await?;
        sqlx::query(
            "INSERT INTO recipe_tags (recipe_id, tag_id) SELECT $1, id FROM tags WHERE user_id = $2 AND name = ANY($3)",
        )
        .bind(recipe_id)
        .bind(user_id)
        .bind(&input.tags)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrondi_decimal_au_centieme() {
        assert_eq!(f64_to_decimal(1.256).to_string(), "1.26");
        assert_eq!(f64_to_decimal(400.0).to_string(), "400");
        assert_eq!(decimal_to_f64(f64_to_decimal(0.33333)), 0.33);
    }

    #[test]
    fn echappement_like() {
        assert_eq!(escape_like("50%_x"), "50\\%\\_x");
    }
}
