//! Réglages du compte (profil, préférences, mot de passe).

use axum::Json;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use super::auth::{password_attempt_succeeded, reserve_password_attempt};
use crate::client_ip::ClientIp;
use crate::domain::catalog::{Locale, UnitSystem};
use crate::error::{ApiError, ApiResult};
use crate::logger::{log_event, log_warn};
use crate::repo::settings::{self, PasswordCheck, ProfileDto, ProfilePatch};
use crate::session::{self, CurrentSession, CurrentUser, SessionUser};
use crate::state::AppState;
use crate::validation::{ValidJson, Validate, Validated, int_range, opt_strict_int, opt_text, raw_text};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    locale: Option<Locale>,
    #[serde(default)]
    unit_system: Option<UnitSystem>,
    #[serde(default, deserialize_with = "opt_strict_int")]
    default_dish_count: Option<i64>,
    #[serde(default, deserialize_with = "opt_strict_int")]
    default_people_count: Option<i64>,
    #[serde(default, deserialize_with = "opt_strict_int")]
    default_days_count: Option<i64>,
    #[serde(default, deserialize_with = "opt_strict_int")]
    rotation_weeks: Option<i64>,
}

impl Validate for ProfileBody {
    type Output = ProfilePatch;
    fn validate(self) -> Validated<ProfilePatch> {
        let range = |v: Option<i64>, max: i64| v.map(|n| int_range(n, 1, max)).transpose();
        let patch = ProfilePatch {
            name: opt_text(self.name, 1, 100)?,
            locale: self.locale,
            unit_system: self.unit_system,
            default_dish_count: range(self.default_dish_count, 10)?,
            default_people_count: range(self.default_people_count, 12)?,
            default_days_count: range(self.default_days_count, 7)?,
            rotation_weeks: range(self.rotation_weeks, 12)?,
        };
        if patch.fields().is_empty() {
            return Err(ApiError::validation());
        }
        Ok(patch)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordChangeBody {
    current_password: String,
    new_password: String,
}

impl Validate for PasswordChangeBody {
    type Output = Self;
    fn validate(self) -> Validated<Self> {
        Ok(PasswordChangeBody {
            current_password: raw_text(self.current_password, 1, 128)?,
            new_password: raw_text(self.new_password, 12, 128)?,
        })
    }
}

pub async fn get_profile(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> ApiResult<Json<ProfileDto>> {
    settings::get_profile(&state.db, user.id)
        .await?
        .map(Json)
        .ok_or_else(ApiError::not_found)
}

pub async fn update_profile(
    State(state): State<AppState>,
    current: CurrentSession,
    ValidJson(patch): ValidJson<ProfileBody>,
) -> ApiResult<Response> {
    let user = current.user.clone();
    let profile = settings::update_profile(&state.db, user.id, &patch)
        .await?
        .ok_or_else(ApiError::not_found)?;
    log_event(
        "settings.updated",
        json!({ "userId": user.id, "fields": patch.fields() }),
    );
    // Le nom affiché vit aussi dans la session : on la re-scelle SANS la
    // rajeunir (même date de connexion, même époque) — sinon ce PATCH
    // prolongerait indéfiniment un cookie volé.
    if let Some(name) = patch.name {
        let epoch = crate::repo::users::get_account_status(&state.db, user.id)
            .await?
            .map_or(0, |s| s.session_epoch);
        let refreshed = current.resealed(SessionUser { name, ..user }, epoch);
        let cookie = session::set_cookie(&state.session_key.seal(&refreshed));
        return Ok(([(SET_COOKIE, cookie)], Json(profile)).into_response());
    }
    Ok(Json(profile).into_response())
}

pub async fn change_password(
    State(state): State<AppState>,
    current: CurrentSession,
    ClientIp(ip): ClientIp,
    ValidJson(body): ValidJson<PasswordChangeBody>,
) -> ApiResult<Response> {
    let user = current.user.clone();
    state.rate_limit(&format!("settings:password:{}", user.id), &ip, 5, 60_000)?;
    reserve_password_attempt(&state, &user.email)?;
    match settings::change_password(&state.db, user.id, &body.current_password, &body.new_password).await? {
        PasswordCheck::Ok(epoch) => {
            password_attempt_succeeded(&state, &user.email);
            log_event("auth.password_changed", json!({ "userId": user.id, "ip": ip }));
            // Les autres sessions sont révoquées ; celle-ci continue à la nouvelle époque.
            let cookie = session::set_cookie(&state.session_key.seal(&current.resealed(user, epoch)));
            Ok(([(SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
        }
        failure => {
            log_warn("auth.password_change_failed", json!({ "userId": user.id, "ip": ip }));
            Err(match failure {
                PasswordCheck::InvalidPassword => ApiError::new(403, "errors.invalidPassword"),
                _ => ApiError::not_found(),
            })
        }
    }
}
