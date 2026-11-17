//! Assemblage du routeur : API sous /api, sonde /healthz, front statique (SPA).

use std::path::{Path, PathBuf};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::json;
use sha2::{Digest, Sha256};
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
        .layer(middleware::from_fn_with_state(state.clone(), account_status))
        .layer(middleware::from_fn_with_state(state.clone(), same_origin_only));

    let mut app = Router::new()
        .nest("/api", api)
        // Alias de /api/health au chemin conventionnel du contrat d'hébergement.
        .route("/healthz", get(health::health));

    let mut index_html = None;
    if let Some(dir) = state.config.static_dir.clone().filter(|d| d.is_dir()) {
        index_html = std::fs::read_to_string(dir.join("index.html")).ok();
        app = app.fallback_service(spa_service(dir));
    }
    let csp = HeaderValue::from_str(&content_security_policy(index_html.as_deref())).expect("CSP ASCII");

    app.layer(middleware::from_fn_with_state(csp, security_headers))
        .with_state(state)
}

async fn api_not_found() -> ApiError {
    ApiError::not_found()
}

/// Révocation des sessions scellées : sur chaque requête /api/* authentifiée
/// (hors /api/auth/*, qui font leurs propres vérifications), le statut du
/// compte est relu en base — un compte désactivé ou supprimé perd l'accès sans
/// attendre l'expiration du cookie (7 j), et une session dont l'époque est
/// dépassée (users.session_epoch) est refusée.
async fn account_status(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path();
    // Monté sous /api : le chemin vu ici est relatif (« /auth/login »).
    if path.starts_with("/auth/") {
        return next.run(request).await;
    }
    let Some(session) = read_session(request.headers(), &state.session_key) else {
        return next.run(request).await;
    };
    let Some(user) = session.user else {
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
            let ip = resolve_client_ip(
                request.headers(),
                socket,
                &state.config.trusted_proxies,
                state.config.trust_xff,
            );
            log_warn(
                "security.disabled_session_blocked",
                json!({ "userId": user.id, "ip": ip }),
            );
            ApiError::new(401, "errors.accountDisabled")
                .with_cleared_session()
                .into_response()
        }
        // Session révoquée : mot de passe changé ou réinitialisé, 2FA modifiée
        // ou déconnexion depuis le scellement de ce cookie (cookie volé compris).
        Ok(Some(status)) if status.session_epoch != session.epoch => ApiError::new(401, "errors.invalidCredentials")
            .with_cleared_session()
            .into_response(),
        Ok(Some(_)) => next.run(request).await,
    }
}

/// Défense CSRF (PRD §8 : « vérification d'origine ») sur toute méthode à
/// effet. SameSite=Lax ne protège pas d'une page servie par un AUTRE
/// sous-domaine du même site (application voisine du home lab, ou XSS chez
/// elle) : on exige que la requête vienne de notre propre origine.
/// - `Sec-Fetch-Site` (posé par le navigateur, infalsifiable par une page) doit
///   valoir `same-origin` ou `none` ;
/// - à défaut, `Origin` doit être notre origine ;
/// - sans aucun des deux (client non-navigateur : scripts, tests), la requête
///   passe — un tel client ne porte pas les cookies d'une victime.
async fn same_origin_only(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let safe = matches!(request.method(), &Method::GET | &Method::HEAD | &Method::OPTIONS);
    if !safe && !is_same_origin(request.headers(), state.config.public_base_url.as_deref()) {
        log_warn("security.cross_origin_blocked", json!({ "path": request.uri().path() }));
        return ApiError::new(403, "errors.forbidden").into_response();
    }
    next.run(request).await
}

pub fn is_same_origin(headers: &HeaderMap, public_base_url: Option<&str>) -> bool {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim);
    if let Some(site) = header("sec-fetch-site") {
        return matches!(site, "same-origin" | "none");
    }
    let Some(origin) = header("origin") else { return true };
    match public_base_url {
        Some(base) => origin.eq_ignore_ascii_case(base),
        // Dev et tests : l'origine doit désigner l'hôte auquel la requête s'adresse.
        None => header("host").is_some_and(|host| {
            origin.eq_ignore_ascii_case(&format!("http://{host}"))
                || origin.eq_ignore_ascii_case(&format!("https://{host}"))
        }),
    }
}

/// Politique de sécurité du contenu, dont l'API est propriétaire (défense en
/// profondeur : elle protège même sans Traefik devant). Les scripts inline de
/// la coquille générée par Nuxt (configuration, importmap) sont autorisés par
/// leur empreinte SHA-256, calculée au démarrage — jamais par 'unsafe-inline'.
/// `img-src https:` : les photos de recettes sont des URL externes.
pub fn content_security_policy(index_html: Option<&str>) -> String {
    static INLINE_SCRIPT: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?is)<script([^>]*)>(.*?)</script>").unwrap());
    let hashes: Vec<String> = index_html
        .map(|html| {
            INLINE_SCRIPT
                .captures_iter(html)
                .filter(|caps| !caps[1].to_ascii_lowercase().contains("src=") && !caps[2].is_empty())
                .map(|caps| format!(" 'sha256-{}'", STANDARD.encode(Sha256::digest(caps[2].as_bytes()))))
                .collect()
        })
        .unwrap_or_default();
    format!(
        "default-src 'self'; script-src 'self'{}; style-src 'self' 'unsafe-inline'; img-src 'self' https: data:; \
         font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'",
        hashes.concat()
    )
}

/// En-têtes de sécurité, posés sur toute réponse.
async fn security_headers(State(csp): State<HeaderValue>, request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let is_asset = path.starts_with("/_nuxt/");
    let is_api = path.starts_with("/api/") || path == "/healthz";
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    let set = |headers: &mut HeaderMap, name: &'static str, value: &'static str| {
        headers
            .entry(HeaderName::from_static(name))
            .or_insert(HeaderValue::from_static(value));
    };
    headers
        .entry(HeaderName::from_static("content-security-policy"))
        .or_insert(csp);
    set(headers, "x-content-type-options", "nosniff");
    set(headers, "x-frame-options", "DENY");
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
    if is_api {
        // Réponses sensibles (session, secret TOTP, liens d'invitation) :
        // jamais conservées par un cache intermédiaire (Cloudflare) ou local.
        set(headers, "cache-control", "no-store");
    }
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
