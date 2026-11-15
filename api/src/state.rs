//! État partagé des handlers.

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::import::claude::ClaudeClient;
use crate::import::safe_fetch::SafeFetcher;
use crate::mailer::Mailer;
use crate::rate_limit::RateLimits;
use crate::session::SessionKey;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub session_key: SessionKey,
    pub limits: Arc<RateLimits>,
    pub mailer: Arc<Mailer>,
    pub fetcher: Arc<SafeFetcher>,
    /// Fournisseur d'extraction « claude » — absent sans ANTHROPIC_API_KEY.
    pub claude: Option<Arc<ClaudeClient>>,
}

impl AppState {
    pub fn new(db: PgPool, config: Config) -> Self {
        let session_key = match &config.session_password {
            Some(password) => SessionKey::from_password(password),
            None => {
                // Dev seulement (la config refuse ce cas en production) : les
                // sessions ne survivent pas au redémarrage.
                crate::logger::log_warn("config.session_password_generated", serde_json::json!({}));
                SessionKey::from_password(&crate::crypto::random_hex(32))
            }
        };
        let claude = config
            .anthropic_api_key
            .clone()
            .map(|key| Arc::new(ClaudeClient::new(key, config.extraction_model.clone())));
        AppState {
            db,
            session_key,
            limits: Arc::new(RateLimits::default()),
            mailer: Arc::new(Mailer::new(config.smtp.clone(), config.smtp_from.clone())),
            fetcher: Arc::new(SafeFetcher::new(config.import_allow_private)),
            claude,
            config: Arc::new(config),
        }
    }

    /// Assertion de rate limit pour une IP sur un bucket.
    pub fn rate_limit(&self, bucket: &str, ip: &str, max: usize, window_ms: u64) -> Result<(), crate::error::ApiError> {
        self.limits.assert(bucket, ip, max, window_ms)
    }
}
