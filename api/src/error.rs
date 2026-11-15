//! Erreurs HTTP de l'API. Le corps est `{ statusCode, message }` où `message`
//! est une clé i18n (ex. `errors.emailTaken`) traduite côté client
//! (app/utils/error-message.ts lit `data.message`).

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header::SET_COOKIE};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::logger::log_error;
use crate::session;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
    /// Purge le cookie de session dans la réponse (compte supprimé ou désactivé).
    pub clear_session: bool,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            message: message.into(),
            clear_session: false,
        }
    }

    /// Entrée invalide (équivalent du rejet Zod côté Nitro).
    pub fn validation() -> Self {
        Self::new(400, "Validation Error")
    }

    pub fn unauthorized() -> Self {
        Self::new(401, "Unauthorized")
    }

    pub fn not_found() -> Self {
        Self::new(404, "errors.notFound")
    }

    pub fn internal() -> Self {
        Self::new(500, "Internal Server Error")
    }

    pub fn with_cleared_session(mut self) -> Self {
        self.clear_session = true;
        self
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.status.as_u16(), self.message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "statusCode": self.status.as_u16(), "message": self.message }));
        let mut response = (self.status, body).into_response();
        if self.clear_session {
            response.headers_mut().append(
                SET_COOKIE,
                HeaderValue::from_str(&session::clear_cookie()).expect("cookie ASCII"),
            );
        }
        response
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        log_error("db.query_failed", &error, json!({}));
        ApiError::internal()
    }
}
