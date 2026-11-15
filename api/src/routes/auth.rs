//! Authentification : connexion (+ 2FA), session, inscription bootstrap,
//! invitation, mot de passe oublié, vérification du courriel.

use std::sync::OnceLock;

use axum::Json;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::client_ip::ClientIp;
use crate::crypto::{hash_password, verify_password};
use crate::domain::catalog::{EmailTokenType, Locale};
use crate::error::{ApiError, ApiResult};
use crate::logger::{log_error, log_event, log_warn};
use crate::mailer::{reset_password_mail, verify_email_mail};
use crate::repo::email_tokens::{self, app_base_url};
use crate::repo::twofa;
use crate::repo::users::{self, AcceptInvitation, RegisterResult};
use crate::session::{self, CurrentUser, MaybeSession, SessionData, SessionUser};
use crate::state::AppState;
use crate::validation::{ValidJson, Validate, Validated, email, raw_text, text};

/// Réponse JSON qui (re)pose le cookie de session.
fn with_session(state: &AppState, data: &SessionData, status: StatusCode, body: serde_json::Value) -> Response {
    let cookie = session::set_cookie(&state.session_key.seal(data));
    (status, [(SET_COOKIE, cookie)], Json(body)).into_response()
}

// ── Schémas d'entrée ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginBody {
    email: String,
    password: String,
}

impl Validate for LoginBody {
    type Output = Self;
    fn validate(self) -> Validated<Self> {
        Ok(LoginBody {
            email: email(self.email)?,
            password: raw_text(self.password, 1, 128)?,
        })
    }
}

#[derive(Deserialize)]
pub struct RegisterBody {
    email: String,
    password: String,
    name: String,
}

impl Validate for RegisterBody {
    type Output = Self;
    fn validate(self) -> Validated<Self> {
        Ok(RegisterBody {
            email: email(self.email)?,
            password: raw_text(self.password, 12, 128)?,
            name: text(self.name, 1, 100)?,
        })
    }
}

#[derive(Deserialize)]
pub struct CodeBody {
    pub code: String,
}

impl Validate for CodeBody {
    type Output = Self;
    fn validate(self) -> Validated<Self> {
        Ok(CodeBody {
            code: text(self.code, 6, 16)?,
        })
    }
}

#[derive(Deserialize)]
pub struct EmailBody {
    email: String,
}

impl Validate for EmailBody {
    type Output = Self;
    fn validate(self) -> Validated<Self> {
        Ok(EmailBody {
            email: email(self.email)?,
        })
    }
}

#[derive(Deserialize)]
pub struct TokenBody {
    token: String,
}

impl Validate for TokenBody {
    type Output = Self;
    fn validate(self) -> Validated<Self> {
        Ok(TokenBody {
            token: raw_text(self.token, 32, 128)?,
        })
    }
}

/// Réinitialisation de mot de passe et acceptation d'invitation : même forme.
#[derive(Deserialize)]
pub struct TokenPasswordBody {
    token: String,
    password: String,
}

impl Validate for TokenPasswordBody {
    type Output = Self;
    fn validate(self) -> Validated<Self> {
        Ok(TokenPasswordBody {
            token: raw_text(self.token, 32, 128)?,
            password: raw_text(self.password, 12, 128)?,
        })
    }
}

// ── Session ─────────────────────────────────────────────────────────────────

/// Session vue par le front : `{ user }` ou `{}` (le middleware account_status
/// a déjà purgé une session de compte désactivé ou supprimé).
pub async fn session(MaybeSession(session): MaybeSession) -> Json<serde_json::Value> {
    match session.and_then(|s| s.user) {
        Some(user) => Json(json!({ "user": user })),
        None => Json(json!({})),
    }
}

pub async fn me(CurrentUser(user): CurrentUser) -> Json<serde_json::Value> {
    Json(json!({ "user": user }))
}

pub async fn logout() -> impl IntoResponse {
    ([(SET_COOKIE, session::clear_cookie())], Json(json!({ "ok": true })))
}

// ── Connexion ───────────────────────────────────────────────────────────────

/// Verrous par compte : nombre d'échecs tolérés sur 15 minutes.
const LOGIN_ACCOUNT_BUCKET: &str = "auth:login:account";
const TOTP_ACCOUNT_BUCKET: &str = "auth:2fa:account";
const ACCOUNT_MAX_FAILURES: usize = 10;
const TOTP_MAX_FAILURES: usize = 5;
const ACCOUNT_WINDOW_MS: u64 = 15 * 60_000;
/// Durée de vie de l'étape intermédiaire « mot de passe validé, code attendu ».
const PENDING_TOTP_MAX_AGE_SECONDS: i64 = 10 * 60;

/// Hachage factice vérifié quand le courriel est inconnu : temps de réponse
/// comparable dans les deux cas, pas d'énumération de comptes par timing.
static DUMMY_HASH: OnceLock<String> = OnceLock::new();

async fn dummy_hash() -> String {
    if let Some(hash) = DUMMY_HASH.get() {
        return hash.clone();
    }
    let hash = hash_password("hebdomiam-dummy-timing-password".to_string()).await;
    DUMMY_HASH.get_or_init(|| hash).clone()
}

pub async fn login(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    ValidJson(body): ValidJson<LoginBody>,
) -> ApiResult<Response> {
    state.rate_limit("auth:login", &ip, 5, 60_000)?;
    // Verrou par compte : le quota par IP ne résiste pas à une rotation d'adresses.
    state.limits.assert_not_locked(
        LOGIN_ACCOUNT_BUCKET,
        &body.email,
        ACCOUNT_MAX_FAILURES,
        ACCOUNT_WINDOW_MS,
    )?;

    let user = users::find_by_email(&state.db, &body.email).await?;
    let hash = match &user {
        Some(user) => user.password_hash.clone(),
        None => dummy_hash().await,
    };
    let valid = verify_password(hash, body.password).await;

    let Some(user) = user.filter(|_| valid) else {
        // Le courriel visé (pas le mot de passe !) : détecte le brute-force ciblé.
        log_warn("auth.login_failed", json!({ "email": body.email, "ip": ip }));
        // Compté que le compte existe ou non (pas d'énumération par le verrou).
        state.limits.record_failure(
            LOGIN_ACCOUNT_BUCKET,
            &body.email,
            ACCOUNT_MAX_FAILURES,
            ACCOUNT_WINDOW_MS,
        );
        return Err(ApiError::new(401, "errors.invalidCredentials"));
    };

    // Compte désactivé : refusé seulement après validation du mot de passe —
    // on ne révèle rien à qui ne connaît pas les identifiants.
    if user.disabled_at.is_some() {
        log_warn("auth.login_disabled", json!({ "userId": user.id, "ip": ip }));
        return Err(ApiError::new(403, "errors.accountDisabled"));
    }

    // 2FA active : mot de passe validé mais session incomplète tant que le
    // code TOTP n'est pas fourni (POST /api/auth/2fa).
    if user.totp_enabled_at.is_some() {
        log_event("auth.login_2fa_pending", json!({ "userId": user.id, "ip": ip }));
        return Ok(with_session(
            &state,
            &SessionData::pending_totp(user.id),
            StatusCode::OK,
            json!({ "requires2fa": true }),
        ));
    }

    let public_user = user.session_user();
    log_event("auth.login", json!({ "userId": user.id, "ip": ip }));
    Ok(with_session(
        &state,
        &SessionData::for_user(public_user.clone()),
        StatusCode::OK,
        json!({ "user": public_user }),
    ))
}

pub async fn two_factor(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    MaybeSession(session): MaybeSession,
    ValidJson(body): ValidJson<CodeBody>,
) -> ApiResult<Response> {
    state.rate_limit("auth:2fa", &ip, 5, 60_000)?;

    // L'étape « code attendu » ne vit que quelques minutes : un cookie
    // intermédiaire volé ne sert pas à deviner des codes pendant 7 jours.
    let pending = session.filter(|s| chrono::Utc::now().timestamp() - s.issued_at <= PENDING_TOTP_MAX_AGE_SECONDS);
    let Some(pending_user_id) = pending.and_then(|s| s.pending_totp_user_id) else {
        return Err(ApiError::new(401, "errors.invalidCredentials"));
    };
    // 10^6 codes possibles : sans verrou par compte, une rotation d'IP suffit
    // à contourner le quota par adresse.
    let account = pending_user_id.to_string();
    state
        .limits
        .assert_not_locked(TOTP_ACCOUNT_BUCKET, &account, TOTP_MAX_FAILURES, ACCOUNT_WINDOW_MS)?;
    let Some(method) = twofa::verify_login(&state.db, pending_user_id, &body.code).await? else {
        log_warn("auth.2fa_failed", json!({ "userId": pending_user_id, "ip": ip }));
        state
            .limits
            .record_failure(TOTP_ACCOUNT_BUCKET, &account, TOTP_MAX_FAILURES, ACCOUNT_WINDOW_MS);
        return Err(ApiError::new(401, "errors.invalidCode"));
    };
    let Some(user) = users::find_by_id(&state.db, pending_user_id).await? else {
        return Err(ApiError::new(401, "errors.invalidCredentials"));
    };
    // Fenêtre login → 2FA : le compte a pu être désactivé entre les deux étapes.
    if user.disabled_at.is_some() {
        log_warn("auth.login_disabled", json!({ "userId": user.id, "ip": ip }));
        return Err(ApiError::new(403, "errors.accountDisabled"));
    }

    let public_user = user.session_user();
    // method=backup : authenticateur perdu — ou compromis. À surveiller.
    log_event(
        "auth.login",
        json!({ "userId": user.id, "ip": ip, "twoFactor": method.as_str() }),
    );
    Ok(with_session(
        &state,
        &SessionData::for_user(public_user.clone()),
        StatusCode::OK,
        json!({ "user": public_user }),
    ))
}

// ── Inscription (bootstrap) et invitation ───────────────────────────────────

/// Consommé par le front pour masquer le formulaire d'inscription quand
/// l'instance a déjà un compte. La vraie défense est le 403 de register —
/// cet endpoint ne divulgue que « instance vierge ou non ».
pub async fn registration_open(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(json!({ "open": users::count_users(&state.db).await? == 0 })))
}

/// Inscription publique fermée : seule la première inscription d'une instance
/// vierge passe (bootstrap, compte promu admin).
pub async fn register(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    ValidJson(body): ValidJson<RegisterBody>,
) -> ApiResult<Response> {
    state.rate_limit("auth:register", &ip, 5, 60_000)?;
    let password_hash = hash_password(body.password).await;

    let user: SessionUser = match users::register(&state.db, &body.email, &body.name, &password_hash).await? {
        RegisterResult::Created(user) => user,
        RegisterResult::Closed => {
            log_warn("auth.register_closed", json!({ "ip": ip }));
            return Err(ApiError::new(403, "errors.registrationClosed"));
        }
        RegisterResult::EmailTaken => {
            log_warn("auth.register_email_taken", json!({ "ip": ip }));
            return Err(ApiError::new(409, "errors.emailTaken"));
        }
    };
    log_event(
        "auth.register",
        json!({ "userId": user.id, "ip": ip, "bootstrap": true }),
    );

    // Courriel de vérification — l'inscription n'échoue jamais sur un souci SMTP.
    let sent = async {
        let mut conn = state.db.acquire().await.map_err(|e| e.to_string())?;
        let token = email_tokens::create(&mut conn, user.id, EmailTokenType::Verify, 24 * 60)
            .await
            .map_err(|e| e.to_string())?;
        let link = format!(
            "{}/verifier-courriel?token={token}",
            app_base_url(&state.config, &headers)
        );
        // locale du compte à la création = fr (défaut du schéma)
        state
            .mailer
            .send(verify_email_mail(&user.email, Locale::Fr, &link))
            .await
    }
    .await;
    if let Err(error) = sent {
        log_error("mailer.verify_send_failed", &error, json!({ "userId": user.id }));
    }

    Ok(with_session(
        &state,
        &SessionData::for_user(user.clone()),
        StatusCode::CREATED,
        json!({ "user": user }),
    ))
}

/// Acceptation d'une invitation créée par un admin : la possession du lien
/// vaut vérification du courriel. Pas de session posée : l'invité se connecte ensuite.
pub async fn accept_invitation(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    ValidJson(body): ValidJson<TokenPasswordBody>,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit("auth:invitation", &ip, 5, 60_000)?;
    let password_hash = hash_password(body.password).await;
    let refusal = match users::accept_invitation(&state.db, &body.token, &password_hash).await? {
        AcceptInvitation::Accepted(user_id) => {
            log_event("auth.invite_accepted", json!({ "userId": user_id, "ip": ip }));
            return Ok(Json(json!({ "ok": true })));
        }
        AcceptInvitation::InvalidToken => ("invalidToken", ApiError::new(400, "errors.invalidToken")),
        // Compte déjà actif : un jeton remplacé ne doit jamais écraser le mot de
        // passe d'un compte en usage (le jeton vient d'être brûlé, c'est voulu).
        AcceptInvitation::AlreadyActive => ("alreadyActive", ApiError::new(409, "errors.inviteAlreadyActive")),
        AcceptInvitation::Disabled => ("disabled", ApiError::new(403, "errors.accountDisabled")),
    };
    log_warn("auth.invite_refused", json!({ "reason": refusal.0, "ip": ip }));
    Err(refusal.1)
}

// ── Mot de passe oublié ─────────────────────────────────────────────────────

pub async fn forgot_password(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    ValidJson(body): ValidJson<EmailBody>,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit("password:forgot", &ip, 3, 10 * 60_000)?;

    // Réponse identique que le compte existe ou non (anti-énumération).
    match users::find_by_email(&state.db, &body.email).await? {
        Some(user) => {
            let mut conn = state.db.acquire().await?;
            let token = email_tokens::create(&mut conn, user.id, EmailTokenType::Reset, 60).await?;
            let link = format!("{}/reinitialiser?token={token}", app_base_url(&state.config, &headers));
            state
                .mailer
                .send(reset_password_mail(&user.email, user.locale, &link))
                .await
                .map_err(|error| {
                    log_error("mailer.reset_send_failed", &error, json!({ "userId": user.id }));
                    ApiError::internal()
                })?;
            log_event("auth.reset_requested", json!({ "userId": user.id, "ip": ip }));
        }
        // Courriel sans compte : du probing, ou un utilisateur qui se trompe d'adresse.
        None => log_warn("auth.reset_requested_unknown", json!({ "email": body.email, "ip": ip })),
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn reset_password(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    ValidJson(body): ValidJson<TokenPasswordBody>,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit("password:reset", &ip, 5, 60_000)?;
    let Some(user_id) = email_tokens::consume(&state.db, EmailTokenType::Reset, &body.token).await? else {
        log_warn("auth.reset_invalid_token", json!({ "ip": ip }));
        return Err(ApiError::new(400, "errors.invalidToken"));
    };
    users::set_password_hash(&state.db, user_id, &hash_password(body.password).await).await?;
    log_event("auth.password_reset", json!({ "userId": user_id, "ip": ip }));
    Ok(Json(json!({ "ok": true })))
}

// ── Vérification du courriel ────────────────────────────────────────────────

pub async fn verify_confirm(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    ValidJson(body): ValidJson<TokenBody>,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit("verify:confirm", &ip, 10, 60_000)?;
    let Some(user_id) = email_tokens::consume(&state.db, EmailTokenType::Verify, &body.token).await? else {
        log_warn("auth.verify_invalid_token", json!({ "ip": ip }));
        return Err(ApiError::new(400, "errors.invalidToken"));
    };
    users::mark_email_verified(&state.db, user_id).await?;
    log_event("auth.email_verified", json!({ "userId": user_id }));
    Ok(Json(json!({ "ok": true })))
}

pub async fn verify_request(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit(&format!("verify:request:{}", user.id), &ip, 3, 10 * 60_000)?;
    let Some(row) = users::find_by_id(&state.db, user.id)
        .await?
        .filter(|r| r.email_verified_at.is_none())
    else {
        return Ok(Json(json!({ "ok": true })));
    };
    let mut conn = state.db.acquire().await?;
    let token = email_tokens::create(&mut conn, user.id, EmailTokenType::Verify, 24 * 60).await?;
    let link = format!(
        "{}/verifier-courriel?token={token}",
        app_base_url(&state.config, &headers)
    );
    state
        .mailer
        .send(verify_email_mail(&row.email, row.locale, &link))
        .await
        .map_err(|error| {
            log_error("mailer.verify_send_failed", &error, json!({ "userId": user.id }));
            ApiError::internal()
        })?;
    log_event("auth.verify_resent", json!({ "userId": user.id }));
    Ok(Json(json!({ "ok": true })))
}
