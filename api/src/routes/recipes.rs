//! CRUD des recettes et liste des tags.

use std::collections::HashSet;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::domain::catalog::{Aisle, Unit};
use crate::error::{ApiError, ApiResult};
use crate::logger::log_event;
use crate::repo::recipes::{
    self, IngredientInput, RecipeDetail, RecipeInput, RecipeListQuery, RecipeSummary, TagSummary,
};
use crate::session::CurrentUser;
use crate::state::AppState;
use crate::validation::{
    ValidJson, ValidPath, ValidQuery, Validate, Validated, int_range, opt_strict_int, opt_text, positive, query_text,
    strict_int, text, url,
};

#[derive(Deserialize)]
pub struct IngredientBody {
    quantity: f64,
    #[serde(default)]
    unit: Option<Unit>,
    name: String,
    #[serde(default)]
    aisle: Option<Aisle>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeBody {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(deserialize_with = "strict_int")]
    base_servings: i64,
    #[serde(default, deserialize_with = "opt_strict_int")]
    prep_minutes: Option<i64>,
    #[serde(default, deserialize_with = "opt_strict_int")]
    cook_minutes: Option<i64>,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    freezable: Option<bool>,
    #[serde(default)]
    notes: Option<String>,
    ingredients: Vec<IngredientBody>,
    steps: Vec<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

impl Validate for RecipeBody {
    type Output = RecipeInput;
    fn validate(self) -> Validated<RecipeInput> {
        if !(1..=100).contains(&self.ingredients.len()) || !(1..=50).contains(&self.steps.len()) {
            return Err(ApiError::validation());
        }
        let tags = self.tags.unwrap_or_default();
        if tags.len() > 20 {
            return Err(ApiError::validation());
        }
        let mut seen = HashSet::new();
        let tags: Vec<String> = tags
            .into_iter()
            .map(|t| text(t, 1, 30))
            .collect::<Validated<Vec<_>>>()?
            .into_iter()
            .filter(|t| seen.insert(t.clone()))
            .collect();
        let minutes = |m: Option<i64>| m.map(|v| int_range(v, 0, 1440)).transpose();

        Ok(RecipeInput {
            title: text(self.title, 1, 200)?,
            description: opt_text(self.description, 0, 2000)?,
            // https seulement : une image http:// serait bloquée silencieusement
            // par le navigateur (mixed content) sur le site servi en https.
            image_url: self.image_url.map(|u| url(u, 1000, true)).transpose()?,
            base_servings: int_range(self.base_servings, 1, 50)?,
            prep_minutes: minutes(self.prep_minutes)?,
            cook_minutes: minutes(self.cook_minutes)?,
            source_url: self.source_url.map(|u| url(u, 500, false)).transpose()?,
            freezable: self.freezable.unwrap_or(false),
            notes: opt_text(self.notes, 0, 5000)?,
            ingredients: self
                .ingredients
                .into_iter()
                .map(|i| {
                    Ok(IngredientInput {
                        quantity: positive(i.quantity, 99_999.0)?,
                        unit: i.unit,
                        name: text(i.name, 1, 200)?,
                        aisle: i.aisle.unwrap_or(Aisle::Other),
                    })
                })
                .collect::<Validated<Vec<_>>>()?,
            steps: self
                .steps
                .into_iter()
                .map(|s| text(s, 1, 2000))
                .collect::<Validated<Vec<_>>>()?,
            tags,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListParams {
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    max_minutes: Option<String>,
}

impl Validate for ListParams {
    type Output = RecipeListQuery;
    fn validate(self) -> Validated<RecipeListQuery> {
        let max_minutes = match self.max_minutes.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()) {
            Some(raw) => Some(int_range(
                raw.parse::<i64>().map_err(|_| ApiError::validation())?,
                1,
                1440,
            )?),
            None => None,
        };
        Ok(RecipeListQuery {
            q: query_text(self.q, 1, 100)?,
            tag: query_text(self.tag, 1, 30)?,
            max_minutes,
        })
    }
}

#[derive(Deserialize)]
pub struct IdParams {
    id: Uuid,
}

pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidQuery(query): ValidQuery<ListParams>,
) -> ApiResult<Json<Vec<RecipeSummary>>> {
    Ok(Json(recipes::list(&state.db, user.id, &query).await?))
}

pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<IdParams>,
) -> ApiResult<Json<RecipeDetail>> {
    recipes::get(&state.db, user.id, params.id)
        .await?
        .map(Json)
        .ok_or_else(ApiError::not_found)
}

pub async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidJson(input): ValidJson<RecipeBody>,
) -> ApiResult<impl IntoResponse> {
    let recipe = recipes::create(&state.db, user.id, &input).await?;
    log_event(
        "recipe.created",
        json!({ "userId": user.id, "recipeId": recipe.summary.id }),
    );
    Ok((StatusCode::CREATED, Json(recipe)))
}

pub async fn update(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<IdParams>,
    ValidJson(input): ValidJson<RecipeBody>,
) -> ApiResult<Json<RecipeDetail>> {
    let recipe = recipes::update(&state.db, user.id, params.id, &input)
        .await?
        .ok_or_else(ApiError::not_found)?;
    log_event("recipe.updated", json!({ "userId": user.id, "recipeId": params.id }));
    Ok(Json(recipe))
}

pub async fn delete(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<IdParams>,
) -> ApiResult<Json<serde_json::Value>> {
    if !recipes::soft_delete(&state.db, user.id, params.id).await? {
        return Err(ApiError::not_found());
    }
    log_event("recipe.deleted", json!({ "userId": user.id, "recipeId": params.id }));
    Ok(Json(json!({ "ok": true })))
}

pub async fn tags(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> ApiResult<Json<Vec<TagSummary>>> {
    Ok(Json(recipes::list_tags(&state.db, user.id).await?))
}
