//! Activation et désactivation de la 2FA TOTP (réglages du compte).

use axum::Json;
use axum::extract::State;
use qrcode::QrCode;
use qrcode::render::svg;
use serde::Deserialize;
use serde_json::json;

use super::auth::CodeBody;
use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::logger::{log_event, log_warn};
use crate::repo::twofa::{self, Activation};
use crate::session::CurrentUser;
use crate::state::AppState;
use crate::validation::{ValidJson, Validate, Validated, raw_text};

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
    let setup = twofa::start_setup(&state.db, user.id, &user.email).await?;
    Ok(Json(
        json!({ "secret": setup.secret, "otpauthUrl": setup.otpauth_url, "qrSvg": qr_svg(&setup.otpauth_url) }),
    ))
}

pub async fn activate(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ClientIp(ip): ClientIp,
    ValidJson(body): ValidJson<CodeBody>,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit(&format!("2fa:activate:{}", user.id), &ip, 5, 60_000)?;
    match twofa::activate(&state.db, user.id, &body.code).await? {
        Activation::Activated { backup_code } => {
            log_event("auth.2fa_enabled", json!({ "userId": user.id, "ip": ip }));
            Ok(Json(json!({ "backupCode": backup_code })))
        }
        failure => {
            let reason = if matches!(failure, Activation::InvalidCode) {
                "invalidCode"
            } else {
                "noSetup"
            };
            log_warn(
                "auth.2fa_activation_failed",
                json!({ "userId": user.id, "reason": reason }),
            );
            Err(ApiError::new(403, "errors.invalidCode"))
        }
    }
}

pub async fn disable(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ClientIp(ip): ClientIp,
    ValidJson(body): ValidJson<PasswordBody>,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit(&format!("2fa:disable:{}", user.id), &ip, 5, 60_000)?;
    if !twofa::disable(&state.db, user.id, &body.password).await? {
        log_warn("auth.2fa_disable_failed", json!({ "userId": user.id, "ip": ip }));
        return Err(ApiError::new(403, "errors.invalidPassword"));
    }
    log_event("auth.2fa_disabled", json!({ "userId": user.id, "ip": ip }));
    Ok(Json(json!({ "ok": true })))
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
