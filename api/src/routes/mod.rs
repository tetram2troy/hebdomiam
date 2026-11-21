//! Handlers HTTP. Chaque route valide son entrée (`ValidJson`, `ValidPath`,
//! `ValidQuery`), délègue à la couche `repo` et traduit les résultats métier
//! en réponses (clés i18n pour les erreurs).

pub mod account;
pub mod admin;
pub mod api_keys;
pub mod auth;
pub mod grocery;
pub mod health;
pub mod import;
pub mod plans;
pub mod recipes;
pub mod settings;
pub mod twofa;
pub mod v1;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, patch, post};

use crate::state::AppState;

/// Plafond des corps JSON (2 Mo) ; l'import photo a le sien (35 Mo).
pub const JSON_BODY_LIMIT: usize = 2_000_000;
/// 3 images × 10 Mo + enrobage multipart.
pub const UPLOAD_BODY_LIMIT: usize = 35_000_000;

/// Routes montées sous /api.
pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        // Session courante (remplace /api/_auth/session de nuxt-auth-utils).
        .route("/session", get(auth::session))
        // Authentification
        .route("/auth/login", post(auth::login))
        .route("/auth/2fa", post(auth::two_factor))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/register", post(auth::register))
        .route("/auth/registration-open", get(auth::registration_open))
        .route("/auth/invitation", post(auth::accept_invitation))
        .route("/auth/password/forgot", post(auth::forgot_password))
        .route("/auth/password/reset", post(auth::reset_password))
        .route("/auth/verify/confirm", post(auth::verify_confirm))
        .route("/auth/verify/request", post(auth::verify_request))
        // 2FA et compte
        .route("/2fa/setup", post(twofa::setup))
        .route("/2fa/activate", post(twofa::activate))
        .route("/2fa/disable", post(twofa::disable))
        .route("/account", delete(account::delete_account))
        .route("/settings", get(settings::get_profile).patch(settings::update_profile))
        .route("/settings/password", post(settings::change_password))
        // Clés d'API (gérées depuis une session)
        .route("/api-keys", get(api_keys::list).post(api_keys::create))
        .route("/api-keys/{id}", delete(api_keys::revoke))
        // Routes ouvertes aux clés d'API (Authorization: Bearer) — et à elles seules
        .route("/v1/recipes", post(v1::create_recipe_with_key))
        .route("/v1/tags", get(v1::tags))
        // Administration
        .route("/admin/users", get(admin::list_users).post(admin::create_user))
        .route(
            "/admin/users/{id}",
            patch(admin::update_user).delete(admin::delete_user),
        )
        .route("/admin/users/{id}/invite", post(admin::resend_invite))
        // Recettes
        .route("/recipes", get(recipes::list).post(recipes::create))
        .route(
            "/recipes/{id}",
            get(recipes::get).put(recipes::update).delete(recipes::delete),
        )
        .route("/tags", get(recipes::tags))
        // Plans
        .route("/plans", post(plans::create))
        .route("/plans/current", get(plans::current))
        .route("/plans/{id}", get(plans::get))
        .route("/plans/{id}/validate", post(plans::validate))
        .route("/plans/{id}/replacements", get(plans::replacements))
        .route("/plans/{id}/recipes/{item_id}", patch(plans::update_recipe))
        .route("/plans/{id}/recipes/{item_id}/swap", post(plans::swap))
        .route("/plans/{id}/recipes/{item_id}/replace", post(plans::replace))
        // Épicerie
        .route("/grocery-list/current", get(grocery::current))
        .route("/grocery-list/items", post(grocery::add_item))
        .route("/grocery-list/items/merge", post(grocery::merge))
        .route(
            "/grocery-list/items/{id}",
            patch(grocery::update_item).delete(grocery::delete_item),
        )
        // Import
        .route("/import/url", post(import::from_url))
        .route(
            "/import/photo",
            post(import::from_photo).layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT)),
        )
        .layer(DefaultBodyLimit::max(JSON_BODY_LIMIT))
}
