//! Assemblage du routeur : API sous /api, sonde /healthz, front statique (SPA).

use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use serde_json::json;
use tower_http::services::ServeDir;

use crate::client_ip::resolve_client_ip;
use crate::error::ApiError;
use crate::logger::log_warn;
use crate::repo::users;
use crate::routes::{api_router, health};
use crate::session::read_session;
use crate::state::AppState;

pub fn build(state: AppState) -> Router {
    let api = api_router()
        .route("/{*rest}", any(api_not_found))
        .layer(middleware::from_fn_with_state(state.clone(), account_status));

    let mut app = Router::new()
        .nest("/api", api)
        // Alias de /api/health au chemin conventionnel du contrat d'hébergement.
        .route("/healthz", get(health::health));

    if let Some(dir) = state.config.static_dir.clone().filter(|d| d.is_dir()) {
        app = app.fallback_service(spa_service(dir));
    }

    app.layer(middleware::from_fn(security_headers)).with_state(state)
}

async fn api_not_found() -> ApiError {
    ApiError::not_found()
}

/// Révocation des sessions scellées : sur chaque requête /api/* authentifiée
/// (hors /api/auth/*, qui font leurs propres vérifications), le statut du
/// compte est relu en base — un compte désactivé ou supprimé perd l'accès sans
/// attendre l'expiration du cookie (7 j).
async fn account_status(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path();
    // Monté sous /api : le chemin vu ici est relatif (« /auth/login »).
    if path.starts_with("/auth/") {
        return next.run(request).await;
    }
    let Some(user) = read_session(request.headers(), &state.session_key).and_then(|s| s.user) else {
        return next.run(request).await;
    };
    match users::get_account_status(&state.db, user.id).await {
        Err(error) => ApiError::from(error).into_response(),
        // Compte supprimé : la session ne référence plus rien — on la purge
        // sans divulguer la raison (le 401 générique suffit).
        Ok(None) => ApiError::new(401, "errors.invalidCredentials")
            .with_cleared_session()
            .into_response(),
        Ok(Some(status)) if status.disabled_at.is_some() => {
            let socket = request
                .extensions()
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|c| c.0);
            let ip = resolve_client_ip(request.headers(), socket, state.config.trust_xff);
            log_warn(
                "security.disabled_session_blocked",
                json!({ "userId": user.id, "ip": ip }),
            );
            ApiError::new(401, "errors.accountDisabled")
                .with_cleared_session()
                .into_response()
        }
        Ok(Some(_)) => next.run(request).await,
    }
}

/// En-têtes de sécurité (repris de nuxt-security). La CSP reste la propriété
/// du middleware Traefik secure-headers@file du home lab (PRD §8) : une seule
/// couche propriétaire par en-tête.
async fn security_headers(request: Request, next: Next) -> Response {
    let is_asset = request.uri().path().starts_with("/_nuxt/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    let set = |headers: &mut axum::http::HeaderMap, name: &'static str, value: &'static str| {
        headers
            .entry(HeaderName::from_static(name))
            .or_insert(HeaderValue::from_static(value));
    };
    set(headers, "x-content-type-options", "nosniff");
    set(headers, "x-frame-options", "SAMEORIGIN");
    set(headers, "referrer-policy", "no-referrer");
    set(
        headers,
        "strict-transport-security",
        "max-age=15552000; includeSubDomains",
    );
    set(headers, "cross-origin-opener-policy", "same-origin");
    set(headers, "cross-origin-resource-policy", "same-origin");
    set(headers, "origin-agent-cluster", "?1");
    set(headers, "x-dns-prefetch-control", "off");
    set(headers, "x-permitted-cross-domain-policies", "none");
    set(headers, "x-xss-protection", "0");
    set(
        headers,
        "permissions-policy",
        "camera=(), geolocation=(), microphone=(), payment=(), usb=()",
    );
    if is_asset && response.status().is_success() {
        // Fichiers fingerprintés par Vite : immuables.
        response.headers_mut().insert(
            CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    response
}

/// Front Nuxt généré en SPA : fichiers statiques, et pour toute route de
/// l'application (sans extension) le document `index.html`. Un fichier absent
/// (`/_nuxt/disparu.js`) reste un 404.
fn spa_service(dir: PathBuf) -> ServeDir<Router> {
    let index = dir.join("index.html");
    let fallback = Router::new().fallback(move |request: Request| spa_fallback(index.clone(), request));
    // `nuxt generate` écrit aussi des coquilles par route (login/index.html) :
    // on ne les sert pas comme répertoires (redirection « /login/ »), toutes
    // les routes de l'app reçoivent la même coquille index.html.
    ServeDir::new(dir)
        .append_index_html_on_directories(false)
        .fallback(fallback)
}

async fn spa_fallback(index: PathBuf, request: Request) -> Response {
    let last_segment = request.uri().path().rsplit('/').next().unwrap_or("");
    if last_segment.contains('.') {
        return (StatusCode::NOT_FOUND, "Not Found").into_response();
    }
    match tokio::fs::read(&index).await {
        Ok(html) => Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "text/html; charset=utf-8")
            // Le document change à chaque déploiement : jamais mis en cache.
            .header(CACHE_CONTROL, "no-cache")
            .body(Body::from(html))
            .expect("réponse SPA"),
        Err(_) => (StatusCode::NOT_FOUND, "Not Found").into_response(),
    }
}

/// Utilitaire pour les tests : le dossier statique existe-t-il ?
pub fn static_dir_ready(dir: &Path) -> bool {
    dir.join("index.html").is_file()
}
