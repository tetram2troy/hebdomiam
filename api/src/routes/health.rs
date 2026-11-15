//! Sonde de santé (HEALTHCHECK Docker, Uptime Kuma, webServer Playwright) :
//! vérifie ce qui compte — la base répond — sans authentification et sans
//! bruit de journalisation. Exposée sur /api/health et /healthz (contrat
//! d'hébergement, docs/HOSTING.md §1.C).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde_json::json;

use crate::state::AppState;

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "ok" }))),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "db-unreachable" })),
        ),
    }
}
