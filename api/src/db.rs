//! Pool PostgreSQL.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Une attente sans limite se manifeste en requêtes suspendues — on préfère
/// échouer bruyamment après 10 s.
pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(10))
        .connect(url)
        .await
}

/// Pool paresseux : la connexion n'est tentée qu'à la première requête
/// (le serveur démarre même si la base n'est pas encore prête ; /healthz le dira).
pub fn connect_lazy(url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(10))
        .connect_lazy(url)
}
