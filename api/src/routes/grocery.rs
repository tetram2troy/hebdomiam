//! Liste d'épicerie de la semaine (PRD F6).

use std::collections::HashSet;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::domain::catalog::{Aisle, Unit};
use crate::domain::rotation::current_week;
use crate::error::{ApiError, ApiResult};
use crate::logger::log_event;
use crate::repo::grocery::{self, GroceryItemCreate, GroceryItemPatch};
use crate::session::CurrentUser;
use crate::state::AppState;
use crate::validation::{ValidJson, ValidPath, Validate, Validated, double_option, positive, text};

const MAX_LIST_ITEMS: i64 = 500;

#[derive(Deserialize)]
pub struct CreateBody {
    label: String,
    #[serde(default)]
    quantity: Option<f64>,
    #[serde(default)]
    unit: Option<Unit>,
    #[serde(default)]
    aisle: Option<Aisle>,
}

impl Validate for CreateBody {
    type Output = GroceryItemCreate;
    fn validate(self) -> Validated<GroceryItemCreate> {
        Ok(GroceryItemCreate {
            label: text(self.label, 1, 120)?,
            quantity: self.quantity.map(|q| positive(q, 99_999.0)).transpose()?,
            unit: self.unit,
            aisle: self.aisle.unwrap_or(Aisle::Other),
        })
    }
}

#[derive(Deserialize)]
pub struct UpdateBody {
    #[serde(default)]
    checked: Option<bool>,
    #[serde(default)]
    aisle: Option<Aisle>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    quantity: Option<Option<f64>>,
    #[serde(default, deserialize_with = "double_option")]
    unit: Option<Option<Unit>>,
}

impl Validate for UpdateBody {
    type Output = GroceryItemPatch;
    fn validate(self) -> Validated<GroceryItemPatch> {
        let empty = self.checked.is_none()
            && self.aisle.is_none()
            && self.label.is_none()
            && self.quantity.is_none()
            && self.unit.is_none();
        if empty {
            return Err(ApiError::validation());
        }
        Ok(GroceryItemPatch {
            checked: self.checked,
            aisle: self.aisle,
            label: self.label.map(|l| text(l, 1, 120)).transpose()?,
            quantity: match self.quantity {
                Some(Some(q)) => Some(Some(positive(q, 99_999.0)?)),
                other => other,
            },
            unit: self.unit.flatten(),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeBody {
    item_ids: Vec<Uuid>,
}

impl Validate for MergeBody {
    type Output = Vec<Uuid>;
    fn validate(self) -> Validated<Vec<Uuid>> {
        let unique: HashSet<&Uuid> = self.item_ids.iter().collect();
        if !(2..=50).contains(&self.item_ids.len()) || unique.len() != self.item_ids.len() {
            return Err(ApiError::validation());
        }
        Ok(self.item_ids)
    }
}

#[derive(Deserialize)]
pub struct IdParams {
    id: Uuid,
}

pub async fn current(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> ApiResult<Json<serde_json::Value>> {
    let list = grocery::list_for_week(&state.db, user.id, current_week()).await?;
    Ok(Json(json!({ "list": list })))
}

pub async fn add_item(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidJson(input): ValidJson<CreateBody>,
) -> ApiResult<impl IntoResponse> {
    // Plafond de volume : un compte ne remplit pas la base à coups d'articles.
    if grocery::item_count_for_week(&state.db, user.id, current_week()).await? >= MAX_LIST_ITEMS {
        return Err(ApiError::new(409, "errors.quotaExceeded"));
    }
    let list = grocery::add_manual_item(&state.db, user.id, &input, current_week())
        .await?
        .ok_or_else(ApiError::not_found)?;
    log_event("grocery.item_added", json!({ "userId": user.id, "listId": list.id }));
    Ok((StatusCode::CREATED, Json(json!({ "list": list }))))
}

pub async fn update_item(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<IdParams>,
    ValidJson(patch): ValidJson<UpdateBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let list = grocery::update_item(&state.db, user.id, params.id, &patch, current_week())
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(json!({ "list": list })))
}

pub async fn delete_item(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<IdParams>,
) -> ApiResult<Json<serde_json::Value>> {
    let list = grocery::delete_item(&state.db, user.id, params.id, current_week())
        .await?
        .ok_or_else(ApiError::not_found)?;
    log_event(
        "grocery.item_deleted",
        json!({ "userId": user.id, "itemId": params.id }),
    );
    Ok(Json(json!({ "list": list })))
}

pub async fn merge(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidJson(item_ids): ValidJson<MergeBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let list = grocery::merge_items(&state.db, user.id, &item_ids, current_week())
        .await?
        .ok_or_else(ApiError::not_found)?;
    log_event(
        "grocery.items_merged",
        json!({ "userId": user.id, "listId": list.id, "count": item_ids.len() }),
    );
    Ok(Json(json!({ "list": list })))
}
