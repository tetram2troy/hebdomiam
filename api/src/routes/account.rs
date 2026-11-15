//! Suppression du compte par son propriétaire.

use axum::Json;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::response::IntoResponse;
use serde_json::json;

use super::twofa::PasswordBody;
use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::logger::{log_event, log_warn};
use crate::repo::settings::{DeleteAccount, delete_account as delete};
use crate::session::{CurrentUser, clear_cookie};
use crate::state::AppState;
use crate::validation::ValidJson;

pub async fn delete_account(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ClientIp(ip): ClientIp,
    ValidJson(body): ValidJson<PasswordBody>,
) -> ApiResult<impl IntoResponse> {
    state.rate_limit(&format!("account:delete:{}", user.id), &ip, 3, 60_000)?;
    let (reason, error) = match delete(&state.db, user.id, &body.password).await? {
        DeleteAccount::Ok => {
            log_event("account.deleted", json!({ "userId": user.id, "ip": ip }));
            return Ok(([(SET_COOKIE, clear_cookie())], Json(json!({ "ok": true }))));
        }
        DeleteAccount::InvalidPassword => ("invalidPassword", ApiError::new(403, "errors.invalidPassword")),
        DeleteAccount::NotFound => ("notFound", ApiError::not_found()),
        // Le dernier admin actif ne peut pas s'auto-supprimer : l'instance
        // deviendrait ingérable (même garde-fou que les routes admin).
        DeleteAccount::LastAdmin => ("lastAdmin", ApiError::new(409, "errors.lastAdmin")),
    };
    log_warn(
        "account.delete_failed",
        json!({ "userId": user.id, "ip": ip, "reason": reason }),
    );
    Err(error)
}
