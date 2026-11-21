//! Planification hebdomadaire (PRD F5).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::domain::rotation::current_week;
use crate::error::{ApiError, ApiResult};
use crate::logger::log_event;
use crate::repo::plans::{
    self, MealPlanDto, PlanBundle, PlanCreateInput, PlanError, PlanRecipeAction, ReplacementMode, ReplacementOption,
};
use crate::session::CurrentUser;
use crate::state::AppState;
use crate::validation::{ValidJson, ValidPath, ValidQuery, Validate, Validated, int_range, query_text, strict_int};

fn plan_error(error: PlanError) -> ApiError {
    ApiError::new(error.status(), error.message())
}

fn unwrap_plan<T>(result: Result<T, PlanError>) -> ApiResult<T> {
    result.map_err(plan_error)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBody {
    #[serde(deserialize_with = "strict_int")]
    dish_count: i64,
    #[serde(deserialize_with = "strict_int")]
    people_count: i64,
    #[serde(deserialize_with = "strict_int")]
    days_count: i64,
    #[serde(default)]
    replace: Option<bool>,
}

impl Validate for CreateBody {
    type Output = PlanCreateInput;
    fn validate(self) -> Validated<PlanCreateInput> {
        Ok(PlanCreateInput {
            dish_count: int_range(self.dish_count, 1, 10)?,
            people_count: int_range(self.people_count, 1, 12)?,
            days_count: int_range(self.days_count, 1, 7)?,
            replace: self.replace.unwrap_or(false),
        })
    }
}

#[derive(Deserialize)]
pub struct ActionBody {
    action: String,
}

impl Validate for ActionBody {
    type Output = PlanRecipeAction;
    fn validate(self) -> Validated<PlanRecipeAction> {
        match self.action.as_str() {
            "accept" => Ok(PlanRecipeAction::Accept),
            "unaccept" => Ok(PlanRecipeAction::Unaccept),
            "cook" => Ok(PlanRecipeAction::Cook),
            _ => Err(ApiError::validation()),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceBody {
    recipe_id: Uuid,
}

impl Validate for ReplaceBody {
    type Output = Uuid;
    fn validate(self) -> Validated<Uuid> {
        Ok(self.recipe_id)
    }
}

#[derive(Deserialize)]
pub struct ReplacementParams {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    q: Option<String>,
}

impl Validate for ReplacementParams {
    type Output = (ReplacementMode, Option<String>);
    fn validate(self) -> Validated<Self::Output> {
        let mode = match self.mode.as_deref() {
            None | Some("") | Some("suggest") => ReplacementMode::Suggest,
            Some("search") => ReplacementMode::Search,
            Some(_) => return Err(ApiError::validation()),
        };
        Ok((mode, query_text(self.q, 1, 100)?))
    }
}

#[derive(Deserialize)]
pub struct PlanParams {
    id: Uuid,
}

#[derive(Deserialize)]
pub struct ItemParams {
    id: Uuid,
    item_id: Uuid,
}

pub async fn current(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> ApiResult<Json<PlanBundle>> {
    Ok(Json(plans::current_bundle(&state.db, user.id, current_week()).await?))
}

pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<PlanParams>,
) -> ApiResult<Json<MealPlanDto>> {
    plans::get(&state.db, user.id, params.id)
        .await?
        .map(Json)
        .ok_or_else(ApiError::not_found)
}

pub async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidJson(input): ValidJson<CreateBody>,
) -> ApiResult<impl IntoResponse> {
    let plan = unwrap_plan(plans::create(&state.db, user.id, input, current_week(), Utc::now()).await?)?;
    log_event(
        "plan.created",
        json!({ "userId": user.id, "planId": plan.id, "weekStartDate": plan.week_start_date }),
    );
    Ok((StatusCode::CREATED, Json(plan)))
}

pub async fn validate(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<PlanParams>,
) -> ApiResult<Json<MealPlanDto>> {
    let plan = unwrap_plan(plans::validate(&state.db, user.id, params.id).await?)?;
    log_event("plan.validated", json!({ "userId": user.id, "planId": params.id }));
    Ok(Json(plan))
}

pub async fn replacements(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<PlanParams>,
    ValidQuery((mode, q)): ValidQuery<ReplacementParams>,
) -> ApiResult<Json<Vec<ReplacementOption>>> {
    let options = plans::replacement_options(&state.db, user.id, params.id, mode, q, Utc::now()).await?;
    Ok(Json(unwrap_plan(options)?))
}

pub async fn update_recipe(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<ItemParams>,
    ValidJson(action): ValidJson<ActionBody>,
) -> ApiResult<Json<MealPlanDto>> {
    let plan = plans::update_recipe_status(&state.db, user.id, params.id, params.item_id, action).await?;
    Ok(Json(unwrap_plan(plan)?))
}

pub async fn swap(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<ItemParams>,
) -> ApiResult<Json<MealPlanDto>> {
    let plan = plans::swap_recipe(&state.db, user.id, params.id, params.item_id, Utc::now()).await?;
    Ok(Json(unwrap_plan(plan)?))
}

pub async fn replace(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ValidPath(params): ValidPath<ItemParams>,
    ValidJson(recipe_id): ValidJson<ReplaceBody>,
) -> ApiResult<Json<MealPlanDto>> {
    let plan = plans::replace_recipe(&state.db, user.id, params.id, params.item_id, recipe_id).await?;
    Ok(Json(unwrap_plan(plan)?))
}
