//! Gestion des clés d'API depuis les réglages (session requise).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::auth::{password_attempt_succeeded, reserve_password_attempt};
use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::logger::{log_event, log_warn};
use crate::repo::api_keys::{self, ApiKeyDto, CreateApiKey};
use crate::session::CurrentUser;
use crate::state::AppState;
use crate::validation::{ValidJson, ValidPath, Validate, Validated, raw_text, text};

#[derive(Deserialize)]
pub struct CreateBody {
    name: String,
    password: String,
}

pub struct CreateInput {
    name: String,
    password: String,
}

impl Validate for CreateBody {
    type Output = CreateInput;
    fn validate(self) -> Validated<CreateInput> {
        Ok(CreateInput {
            name: text(self.name, 1, 60)?,
            password: raw_text(self.password, 1, 128)?,
        })
    }
}

#[derive(Deserialize)]
pub struct IdParams {
    id: Uuid,
}

pub async fn list(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> ApiResult<Json<Vec<ApiKeyDto>>> {
    Ok(Json(api_keys::list(&state.db, user.id).await?))
}

pub async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ClientIp(ip): ClientIp,
    ValidJson(input): ValidJson<CreateBody>,
) -> ApiResult<impl IntoResponse> {
    state.rate_limit(&format!("apikey:create:{}", user.id), &ip, 5, 60_000)?;
    reserve_password_attempt(&state, &user.email)?;
    match api_keys::create(&state.db, user.id, &input.password, &input.name).await? {
        CreateApiKey::Created(created) => {
            password_attempt_succeeded(&state, &user.email);
            log_event(
                "apikey.created",
                json!({ "userId": user.id, "apiKeyId": created.key.id, "ip": ip }),
            );
            Ok((StatusCode::CREATED, Json(created)))
        }
        CreateApiKey::InvalidPassword => {
            log_warn("apikey.create_failed", json!({ "userId": user.id, "ip": ip }));
            Err(ApiError::new(403, "errors.invalidPassword"))
        }
        CreateApiKey::LimitReached => {
            // Mot de passe correct : la tentative n'est pas un échec d'authentification.
            password_attempt_succeeded(&state, &user.email);
            Err(ApiError::new(409, "errors.apiKeyLimit"))
        }
        CreateApiKey::NotFound => Err(ApiError::not_found()),
    }
}

pub async fn revoke(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ClientIp(ip): ClientIp,
    ValidPath(params): ValidPath<IdParams>,
) -> ApiResult<Json<serde_json::Value>> {
    if !api_keys::revoke(&state.db, user.id, params.id).await? {
        return Err(ApiError::not_found());
    }
    log_event(
        "apikey.revoked",
        json!({ "userId": user.id, "apiKeyId": params.id, "ip": ip }),
    );
    Ok(Json(json!({ "ok": true })))
}
