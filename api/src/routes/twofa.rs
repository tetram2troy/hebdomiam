//! Activation et désactivation de la 2FA TOTP (réglages du compte).

use axum::Json;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Response};
use qrcode::QrCode;
use qrcode::render::svg;
use serde::Deserialize;
use serde_json::json;

use super::auth::{password_attempt_succeeded, reserve_password_attempt};
use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::logger::{log_event, log_warn};
use crate::repo::twofa::{self, Activation};
use crate::session::{self, CurrentSession, CurrentUser};
use crate::state::AppState;
use crate::validation::{ValidJson, Validate, Validated, raw_text, text};

/// Corps `{ password }` (désactivation 2FA, suppression du compte).
#[derive(Deserialize)]
pub struct PasswordBody {
    pub password: String,
}

impl Validate for PasswordBody {
    type Output = Self;
    fn validate(self) -> Validated<Self> {
        Ok(PasswordBody {
            password: raw_text(self.password, 1, 128)?,
        })
    }
}

fn qr_svg(content: &str) -> String {
    QrCode::new(content.as_bytes())
        .map(|code| {
            code.render::<svg::Color>()
                .min_dimensions(220, 220)
                .quiet_zone(true)
                .build()
        })
        .unwrap_or_default()
}

pub async fn setup(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ClientIp(ip): ClientIp,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit(&format!("2fa:setup:{}", user.id), &ip, 5, 60_000)?;
    // 2FA déjà active : la remplacer passe par /2fa/disable (mot de passe).
    let Some(setup) = twofa::start_setup(&state.db, user.id, &user.email).await? else {
        log_warn("auth.2fa_setup_refused", json!({ "userId": user.id, "ip": ip }));
        return Err(ApiError::new(409, "errors.totpAlreadyEnabled"));
    };
    Ok(Json(
        json!({ "secret": setup.secret, "otpauthUrl": setup.otpauth_url, "qrSvg": qr_svg(&setup.otpauth_url) }),
    ))
}

/// Corps de l'activation : mot de passe du compte + premier code.
#[derive(Deserialize)]
pub struct ActivateBody {
    password: String,
    code: String,
}

impl Validate for ActivateBody {
    type Output = Self;
    fn validate(self) -> Validated<Self> {
        Ok(ActivateBody {
            password: raw_text(self.password, 1, 128)?,
            code: text(self.code, 6, 16)?,
        })
    }
}

/// Réponse JSON qui re-scelle la session courante à la nouvelle époque (les
/// autres sessions du compte viennent d'être révoquées).
fn resealed(state: &AppState, current: &CurrentSession, epoch: i32, body: serde_json::Value) -> Response {
    let data = current.resealed(current.user.clone(), epoch);
    let cookie = session::set_cookie(&state.session_key.seal(&data));
    ([(SET_COOKIE, cookie)], Json(body)).into_response()
}

pub async fn activate(
    State(state): State<AppState>,
    current: CurrentSession,
    ClientIp(ip): ClientIp,
    ValidJson(body): ValidJson<ActivateBody>,
) -> ApiResult<Response> {
    let user = &current.user;
    state.rate_limit(&format!("2fa:activate:{}", user.id), &ip, 5, 60_000)?;
    reserve_password_attempt(&state, &user.email)?;
    let (reason, error) = match twofa::activate(&state.db, user.id, &body.password, &body.code).await? {
        Activation::Activated {
            backup_code,
            session_epoch,
        } => {
            password_attempt_succeeded(&state, &user.email);
            log_event("auth.2fa_enabled", json!({ "userId": user.id, "ip": ip }));
            return Ok(resealed(
                &state,
                &current,
                session_epoch,
                json!({ "backupCode": backup_code }),
            ));
        }
        Activation::InvalidPassword => ("invalidPassword", ApiError::new(403, "errors.invalidPassword")),
        other => {
            // Mot de passe bon : seul le code (ou l'absence de setup) est en cause.
            password_attempt_succeeded(&state, &user.email);
            let reason = if matches!(other, Activation::InvalidCode) {
                "invalidCode"
            } else {
                "noSetup"
            };
            (reason, ApiError::new(403, "errors.invalidCode"))
        }
    };
    log_warn(
        "auth.2fa_activation_failed",
        json!({ "userId": user.id, "reason": reason }),
    );
    Err(error)
}

pub async fn disable(
    State(state): State<AppState>,
    current: CurrentSession,
    ClientIp(ip): ClientIp,
    ValidJson(body): ValidJson<PasswordBody>,
) -> ApiResult<Response> {
    let user = &current.user;
    state.rate_limit(&format!("2fa:disable:{}", user.id), &ip, 5, 60_000)?;
    reserve_password_attempt(&state, &user.email)?;
    let Some(session_epoch) = twofa::disable(&state.db, user.id, &body.password).await? else {
        log_warn("auth.2fa_disable_failed", json!({ "userId": user.id, "ip": ip }));
        return Err(ApiError::new(403, "errors.invalidPassword"));
    };
    password_attempt_succeeded(&state, &user.email);
    log_event("auth.2fa_disabled", json!({ "userId": user.id, "ip": ip }));
    Ok(resealed(&state, &current, session_epoch, json!({ "ok": true })))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_code_svg() {
        let svg = qr_svg("otpauth://totp/Hebdomiam:a%40b.c?secret=ABC&issuer=Hebdomiam");
        assert!(svg.contains("<svg"));
    }
}
