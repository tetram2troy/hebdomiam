//! Routes /api/admin/* — toutes passent par l'extracteur `AdminUser` qui relit
//! le rôle et le statut EN BASE (jamais décidé sur le cookie seul).

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::client_ip::ClientIp;
use crate::domain::catalog::{Locale, Role};
use crate::error::{ApiError, ApiResult};
use crate::logger::{log_error, log_event, log_warn};
use crate::mailer::invite_mail;
use crate::repo::admin::{self, AdminRefusal, AdminUserDto, AdminUserPatch, CreateInvited, RenewInvite};
use crate::repo::email_tokens::app_base_url;
use crate::session::AdminUser;
use crate::state::AppState;
use crate::validation::{ValidJson, ValidPath, Validate, Validated, email, text};

/// Liste blanche stricte : aucun champ non déclaré ici ne peut atteindre la
/// base (pas de mot de passe, pas de rôle à la création : tout invité naît 'user').
#[derive(Deserialize)]
pub struct CreateUserBody {
    email: String,
    name: String,
}

impl Validate for CreateUserBody {
    type Output = Self;
    fn validate(self) -> Validated<Self> {
        Ok(CreateUserBody {
            email: email(self.email)?,
            name: text(self.name, 1, 100)?,
        })
    }
}

#[derive(Deserialize)]
pub struct UpdateUserBody {
    #[serde(default)]
    role: Option<Role>,
    #[serde(default)]
    disabled: Option<bool>,
}

impl Validate for UpdateUserBody {
    type Output = AdminUserPatch;
    fn validate(self) -> Validated<AdminUserPatch> {
        if self.role.is_none() && self.disabled.is_none() {
            return Err(ApiError::validation());
        }
        Ok(AdminUserPatch {
            role: self.role,
            disabled: self.disabled,
        })
    }
}

#[derive(Deserialize)]
pub struct IdParams {
    id: Uuid,
}

fn refusal_error(refusal: &AdminRefusal) -> (&'static str, ApiError) {
    match refusal {
        AdminRefusal::NotFound => ("notFound", ApiError::not_found()),
        AdminRefusal::SelfTarget => ("self", ApiError::new(400, "errors.adminSelf")),
        AdminRefusal::LastAdmin => ("lastAdmin", ApiError::new(409, "errors.lastAdmin")),
    }
}

fn invite_url(state: &AppState, headers: &HeaderMap, token: &str) -> String {
    format!("{}/invitation?token={token}", app_base_url(&state.config, headers))
}

pub async fn list_users(State(state): State<AppState>, _admin: AdminUser) -> ApiResult<Json<Vec<AdminUserDto>>> {
    Ok(Json(admin::list_users(&state.db).await?))
}

/// Création d'un compte par un admin : le compte naît 'user', non connectable
/// tant que l'invitation n'est pas acceptée. L'URL d'invitation est retournée
/// à l'admin (copiable dans l'UI) — le courriel n'est qu'un canal de confort.
pub async fn create_user(
    State(state): State<AppState>,
    AdminUser(actor): AdminUser,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    ValidJson(body): ValidJson<CreateUserBody>,
) -> ApiResult<impl IntoResponse> {
    state.rate_limit(&format!("admin:create:{}", actor.id), &ip, 30, 60_000)?;
    let (user, token) = match admin::create_invited_user(&state.db, &body.email, &body.name).await? {
        CreateInvited::Created { user, token } => (user, token),
        CreateInvited::EmailTaken => {
            log_warn("admin.user_create_conflict", json!({ "userId": actor.id, "ip": ip }));
            return Err(ApiError::new(409, "errors.emailTaken"));
        }
    };

    let url = invite_url(&state, &headers, &token);
    // locale du compte à la création = fr (défaut du schéma)
    if let Err(error) = state.mailer.send(invite_mail(&user.email, Locale::Fr, &url)).await {
        log_error(
            "mailer.invite_send_failed",
            &error,
            json!({ "userId": actor.id, "targetUserId": user.id }),
        );
    }
    log_event(
        "admin.user_created",
        json!({ "userId": actor.id, "targetUserId": user.id, "ip": ip }),
    );
    Ok((StatusCode::CREATED, Json(json!({ "user": user, "inviteUrl": url }))))
}

pub async fn update_user(
    State(state): State<AppState>,
    AdminUser(actor): AdminUser,
    ClientIp(ip): ClientIp,
    ValidPath(params): ValidPath<IdParams>,
    ValidJson(patch): ValidJson<UpdateUserBody>,
) -> ApiResult<Json<AdminUserDto>> {
    state.rate_limit(&format!("admin:update:{}", actor.id), &ip, 30, 60_000)?;
    match admin::update_user(&state.db, actor.id, params.id, &patch).await? {
        Ok(user) => {
            let mut fields = Vec::new();
            if patch.role.is_some() {
                fields.push("role");
            }
            if patch.disabled.is_some() {
                fields.push("disabled");
            }
            log_event(
                "admin.user_updated",
                json!({ "userId": actor.id, "targetUserId": params.id, "fields": fields, "ip": ip }),
            );
            Ok(Json(user))
        }
        Err(refusal) => {
            let (reason, error) = refusal_error(&refusal);
            log_warn(
                "admin.user_update_refused",
                json!({ "userId": actor.id, "targetUserId": params.id, "reason": reason, "ip": ip }),
            );
            Err(error)
        }
    }
}

pub async fn delete_user(
    State(state): State<AppState>,
    AdminUser(actor): AdminUser,
    ClientIp(ip): ClientIp,
    ValidPath(params): ValidPath<IdParams>,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit(&format!("admin:delete:{}", actor.id), &ip, 10, 60_000)?;
    match admin::delete_user(&state.db, actor.id, params.id).await? {
        Ok(()) => {
            log_event(
                "admin.user_deleted",
                json!({ "userId": actor.id, "targetUserId": params.id, "ip": ip }),
            );
            Ok(Json(json!({ "ok": true })))
        }
        Err(refusal) => {
            let (reason, error) = refusal_error(&refusal);
            log_warn(
                "admin.user_delete_refused",
                json!({ "userId": actor.id, "targetUserId": params.id, "reason": reason, "ip": ip }),
            );
            Err(error)
        }
    }
}

/// Renvoie une invitation (nouveau jeton) pour un compte jamais activé.
pub async fn resend_invite(
    State(state): State<AppState>,
    AdminUser(actor): AdminUser,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    ValidPath(params): ValidPath<IdParams>,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit(&format!("admin:invite:{}", actor.id), &ip, 30, 60_000)?;
    let (email, locale, token) = match admin::renew_invite(&state.db, params.id).await? {
        RenewInvite::Renewed { email, locale, token } => (email, locale, token),
        refused => {
            let (reason, error) = match refused {
                RenewInvite::AlreadyActive => ("alreadyActive", ApiError::new(409, "errors.inviteAlreadyActive")),
                RenewInvite::Disabled => ("disabled", ApiError::new(409, "errors.accountDisabled")),
                _ => ("notFound", ApiError::not_found()),
            };
            log_warn(
                "admin.invite_resend_refused",
                json!({ "userId": actor.id, "targetUserId": params.id, "reason": reason, "ip": ip }),
            );
            return Err(error);
        }
    };
    let url = invite_url(&state, &headers, &token);
    if let Err(error) = state.mailer.send(invite_mail(&email, locale, &url)).await {
        log_error(
            "mailer.invite_send_failed",
            &error,
            json!({ "userId": actor.id, "targetUserId": params.id }),
        );
    }
    log_event(
        "admin.invite_resent",
        json!({ "userId": actor.id, "targetUserId": params.id, "ip": ip }),
    );
    Ok(Json(json!({ "inviteUrl": url })))
}
