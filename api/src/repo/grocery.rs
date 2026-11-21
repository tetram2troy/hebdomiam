//! Couche d'accès unique à la liste d'épicerie : l'isolation multi-utilisateur
//! passe par la chaîne grocery_items → grocery_lists → meal_plans.user_id.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::types::Json;
use sqlx::{FromRow, PgConnection, PgPool};
use uuid::Uuid;

use super::recipes::{decimal_to_f64, f64_to_decimal};
use super::users::find_by_id;
use crate::domain::catalog::{Aisle, Unit, UnitSystem};
use crate::domain::collate;
use crate::domain::grocery_aggregate::{AggregationInput, GroceryItemSource, aggregate_ingredients};
use crate::domain::units::{base_unit_of, dimension_of, display_quantity, to_base};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroceryItemSourceDto {
    pub recipe_title: String,
    pub quantity: f64,
    pub unit: Option<Unit>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroceryItemDto {
    pub id: Uuid,
    pub label: String,
    /// Quantité déjà convertie dans le système d'unités de l'utilisateur.
    pub quantity: Option<f64>,
    pub unit: Option<Unit>,
    pub aisle: Aisle,
    pub checked: bool,
    pub is_manual: bool,
    pub sources: Vec<GroceryItemSourceDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroceryListDto {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub week_start_date: NaiveDate,
    pub checked_count: usize,
    pub total_count: usize,
    /// Triés par ordre de rayon (ordre du magasin) puis par libellé.
    pub items: Vec<GroceryItemDto>,
}

#[derive(Debug, Clone, FromRow)]
struct ItemRow {
    id: Uuid,
    label: String,
    quantity: Option<Decimal>,
    unit: Option<Unit>,
    aisle: Aisle,
    checked: bool,
    is_manual: bool,
    sources: Option<Json<Vec<GroceryItemSource>>>,
}

/// Génère la liste agrégée d'un plan — appelée DANS la transaction de
/// validation du plan.
pub async fn generate_grocery_list(conn: &mut PgConnection, plan_id: Uuid) -> Result<(), sqlx::Error> {
    #[derive(FromRow)]
    struct Row {
        ingredient_id: Uuid,
        name: String,
        quantity: Decimal,
        unit: Option<Unit>,
        aisle: Aisle,
        title: String,
        base_servings: i32,
        target_servings: i32,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT ri.id AS ingredient_id, ri.name, ri.quantity, ri.unit, ri.aisle,
                r.title, r.base_servings, mpr.target_servings
         FROM meal_plan_recipes mpr
         JOIN recipes r ON r.id = mpr.recipe_id
         JOIN recipe_ingredients ri ON ri.recipe_id = r.id
         WHERE mpr.meal_plan_id = $1 AND mpr.status IN ('accepted', 'cooked')
         ORDER BY mpr.position, ri.position",
    )
    .bind(plan_id)
    .fetch_all(&mut *conn)
    .await?;

    let inputs: Vec<AggregationInput> = rows
        .into_iter()
        .map(|r| AggregationInput {
            ingredient_id: r.ingredient_id,
            name: r.name,
            quantity: decimal_to_f64(r.quantity),
            unit: r.unit,
            aisle: r.aisle,
            recipe_title: r.title,
            base_servings: r.base_servings,
            target_servings: r.target_servings,
        })
        .collect();

    let list_id: Uuid = sqlx::query_scalar("INSERT INTO grocery_lists (meal_plan_id) VALUES ($1) RETURNING id")
        .bind(plan_id)
        .fetch_one(&mut *conn)
        .await?;
    for item in aggregate_ingredients(&inputs) {
        sqlx::query(
            "INSERT INTO grocery_items (grocery_list_id, label, quantity, unit, aisle, sources) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(list_id)
        .bind(item.label)
        .bind(stored_decimal(item.quantity))
        .bind(item.unit)
        .bind(item.aisle)
        .bind(Json(item.sources))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

fn to_item_dto(row: ItemRow, system: UnitSystem) -> GroceryItemDto {
    let display = row
        .quantity
        .map(|q| display_quantity(decimal_to_f64(q), row.unit, system));
    GroceryItemDto {
        id: row.id,
        label: row.label,
        quantity: display.map(|d| d.quantity),
        unit: match display {
            Some(d) => d.unit,
            None => row.unit,
        },
        aisle: row.aisle,
        checked: row.checked,
        is_manual: row.is_manual,
        sources: row
            .sources
            .map(|Json(sources)| sources)
            .unwrap_or_default()
            .into_iter()
            .map(|s| GroceryItemSourceDto {
                recipe_title: s.recipe_title,
                quantity: s.quantity,
                unit: s.unit,
            })
            .collect(),
    }
}

struct ListHeader {
    id: Uuid,
    plan_id: Uuid,
    week_start_date: NaiveDate,
}

async fn find_list_for_week(db: &PgPool, user_id: Uuid, week: NaiveDate) -> Result<Option<ListHeader>, sqlx::Error> {
    let row: Option<(Uuid, Uuid, NaiveDate)> = sqlx::query_as(
        "SELECT gl.id, mp.id, mp.week_start_date
         FROM meal_plans mp JOIN grocery_lists gl ON gl.meal_plan_id = mp.id
         WHERE mp.user_id = $1 AND mp.week_start_date = $2",
    )
    .bind(user_id)
    .bind(week)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(id, plan_id, week_start_date)| ListHeader {
        id,
        plan_id,
        week_start_date,
    }))
}

pub async fn list_for_week(db: &PgPool, user_id: Uuid, week: NaiveDate) -> Result<Option<GroceryListDto>, sqlx::Error> {
    let Some(header) = find_list_for_week(db, user_id, week).await? else {
        return Ok(None);
    };
    let system = find_by_id(db, user_id)
        .await?
        .map_or(UnitSystem::Metric, |u| u.unit_system);
    let rows: Vec<ItemRow> = sqlx::query_as(
        "SELECT id, label, quantity, unit, aisle, checked, is_manual, sources FROM grocery_items WHERE grocery_list_id = $1",
    )
    .bind(header.id)
    .fetch_all(db)
    .await?;
    let mut items: Vec<GroceryItemDto> = rows.into_iter().map(|r| to_item_dto(r, system)).collect();
    items.sort_by(|a, b| {
        a.aisle
            .order()
            .cmp(&b.aisle.order())
            .then_with(|| collate::compare(&a.label, &b.label))
    });
    Ok(Some(GroceryListDto {
        id: header.id,
        plan_id: header.plan_id,
        week_start_date: header.week_start_date,
        checked_count: items.iter().filter(|i| i.checked).count(),
        total_count: items.len(),
        items,
    }))
}

/// Plafond de la colonne grocery_items.quantity, numeric(12,2). Une quantité
/// agrégée absurde (99 999 kg × 168 portions) est écrêtée plutôt que de faire
/// échouer la validation du plan en 500.
fn max_stored_quantity() -> Decimal {
    Decimal::new(999_999_999_999, 2)
}

fn stored_decimal(quantity: f64) -> Decimal {
    f64_to_decimal(quantity.min(9_999_999_999.0)).min(max_stored_quantity())
}

/// Nombre d'articles de la liste de la semaine (plafond des ajouts manuels).
pub async fn item_count_for_week(db: &PgPool, user_id: Uuid, week: NaiveDate) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(gi.id) FROM meal_plans mp
         JOIN grocery_lists gl ON gl.meal_plan_id = mp.id
         JOIN grocery_items gi ON gi.grocery_list_id = gl.id
         WHERE mp.user_id = $1 AND mp.week_start_date = $2",
    )
    .bind(user_id)
    .bind(week)
    .fetch_one(db)
    .await
}

/// Vérifie que l'article appartient bien à une liste de l'utilisateur.
async fn owns_item(db: &PgPool, user_id: Uuid, item_id: Uuid) -> Result<bool, sqlx::Error> {
    let found: Option<Uuid> = sqlx::query_scalar(
        "SELECT gi.id FROM grocery_items gi
         JOIN grocery_lists gl ON gl.id = gi.grocery_list_id
         JOIN meal_plans mp ON mp.id = gl.meal_plan_id
         WHERE gi.id = $1 AND mp.user_id = $2",
    )
    .bind(item_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    Ok(found.is_some())
}

/// Quantité saisie (dans l'unité choisie) → stockage en unité de base.
fn stored_quantity(quantity: Option<f64>, unit: Option<Unit>) -> (Option<Decimal>, Option<Unit>) {
    match quantity {
        Some(q) => (Some(stored_decimal(to_base(q, unit))), base_unit_of(dimension_of(unit))),
        None => (None, None),
    }
}

#[derive(Debug, Clone, Default)]
pub struct GroceryItemPatch {
    pub checked: Option<bool>,
    pub aisle: Option<Aisle>,
    pub label: Option<String>,
    /// Some(None) : effacer la quantité ; Some(Some(q)) : nouvelle quantité.
    pub quantity: Option<Option<f64>>,
    pub unit: Option<Unit>,
}

pub async fn update_item(
    db: &PgPool,
    user_id: Uuid,
    item_id: Uuid,
    patch: &GroceryItemPatch,
    week: NaiveDate,
) -> Result<Option<GroceryListDto>, sqlx::Error> {
    if !owns_item(db, user_id, item_id).await? {
        return Ok(None);
    }
    let mut tx = db.begin().await?;
    if let Some(checked) = patch.checked {
        sqlx::query("UPDATE grocery_items SET checked = $2 WHERE id = $1")
            .bind(item_id)
            .bind(checked)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(aisle) = patch.aisle {
        sqlx::query("UPDATE grocery_items SET aisle = $2 WHERE id = $1")
            .bind(item_id)
            .bind(aisle)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(label) = &patch.label {
        sqlx::query("UPDATE grocery_items SET label = $2 WHERE id = $1")
            .bind(item_id)
            .bind(label)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(quantity) = patch.quantity {
        let (quantity, unit) = stored_quantity(quantity, patch.unit);
        sqlx::query("UPDATE grocery_items SET quantity = $2, unit = $3 WHERE id = $1")
            .bind(item_id)
            .bind(quantity)
            .bind(unit)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    list_for_week(db, user_id, week).await
}

#[derive(Debug, Clone)]
pub struct GroceryItemCreate {
    pub label: String,
    pub quantity: Option<f64>,
    pub unit: Option<Unit>,
    pub aisle: Aisle,
}

pub async fn add_manual_item(
    db: &PgPool,
    user_id: Uuid,
    input: &GroceryItemCreate,
    week: NaiveDate,
) -> Result<Option<GroceryListDto>, sqlx::Error> {
    let Some(header) = find_list_for_week(db, user_id, week).await? else {
        return Ok(None);
    };
    let (quantity, unit) = stored_quantity(input.quantity, input.unit);
    sqlx::query(
        "INSERT INTO grocery_items (grocery_list_id, label, quantity, unit, aisle, is_manual) VALUES ($1, $2, $3, $4, $5, true)",
    )
    .bind(header.id)
    .bind(&input.label)
    .bind(quantity)
    .bind(unit)
    .bind(input.aisle)
    .execute(db)
    .await?;
    list_for_week(db, user_id, week).await
}

pub async fn delete_item(
    db: &PgPool,
    user_id: Uuid,
    item_id: Uuid,
    week: NaiveDate,
) -> Result<Option<GroceryListDto>, sqlx::Error> {
    if !owns_item(db, user_id, item_id).await? {
        return Ok(None);
    }
    sqlx::query("DELETE FROM grocery_items WHERE id = $1")
        .bind(item_id)
        .execute(db)
        .await?;
    list_for_week(db, user_id, week).await
}

/// Fusionne plusieurs articles en un seul (regroupement manuel). Le premier id
/// est conservé : libellé et rayon gardés, quantités additionnées si tous les
/// articles partagent la même unité de base (sinon quantité effacée — le
/// détail reste dans les sources).
pub async fn merge_items(
    db: &PgPool,
    user_id: Uuid,
    item_ids: &[Uuid],
    week: NaiveDate,
) -> Result<Option<GroceryListDto>, sqlx::Error> {
    #[derive(FromRow)]
    struct MergeRow {
        id: Uuid,
        grocery_list_id: Uuid,
        quantity: Option<Decimal>,
        unit: Option<Unit>,
        is_manual: bool,
        sources: Option<Json<Vec<GroceryItemSource>>>,
    }
    let rows: Vec<MergeRow> = sqlx::query_as(
        "SELECT gi.id, gi.grocery_list_id, gi.quantity, gi.unit, gi.is_manual, gi.sources
         FROM grocery_items gi
         JOIN grocery_lists gl ON gl.id = gi.grocery_list_id
         JOIN meal_plans mp ON mp.id = gl.meal_plan_id
         WHERE gi.id = ANY($1) AND mp.user_id = $2",
    )
    .bind(item_ids)
    .bind(user_id)
    .fetch_all(db)
    .await?;

    // Tous les articles doivent exister, appartenir à l'utilisateur et à la même liste.
    if rows.len() != item_ids.len() || rows.iter().map(|r| r.grocery_list_id).collect::<HashSet<_>>().len() != 1 {
        return Ok(None);
    }
    let mut by_id: HashMap<Uuid, MergeRow> = rows.into_iter().map(|r| (r.id, r)).collect();
    let ordered: Vec<MergeRow> = item_ids.iter().filter_map(|id| by_id.remove(id)).collect();
    let target = &ordered[0];

    let units: HashSet<Option<Unit>> = ordered.iter().map(|r| r.unit).collect();
    let summable = units.len() == 1 && ordered.iter().all(|r| r.quantity.is_some());
    let quantity: Option<Decimal> = summable.then(|| {
        ordered
            .iter()
            .filter_map(|r| r.quantity)
            .sum::<Decimal>()
            .min(max_stored_quantity())
    });
    let sources: Vec<GroceryItemSource> = ordered
        .iter()
        .flat_map(|r| r.sources.as_ref().map(|Json(s)| s.clone()).unwrap_or_default())
        .collect();

    let mut tx = db.begin().await?;
    sqlx::query(
        // Regroupé pour l'achat : repasse « à acheter » même si une part était cochée.
        "UPDATE grocery_items SET quantity = $2, unit = $3, checked = false, is_manual = $4, sources = $5 WHERE id = $1",
    )
    .bind(target.id)
    .bind(quantity)
    .bind(if summable { target.unit } else { None })
    .bind(ordered.iter().all(|r| r.is_manual))
    .bind(Json(sources))
    .execute(&mut *tx)
    .await?;
    let merged: Vec<Uuid> = ordered[1..].iter().map(|r| r.id).collect();
    sqlx::query("DELETE FROM grocery_items WHERE id = ANY($1)")
        .bind(&merged)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    list_for_week(db, user_id, week).await
}
