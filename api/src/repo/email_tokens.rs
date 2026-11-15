//! Jetons de courriel : seul le SHA-256 est stocké — un vol de base ne permet
//! pas de rejouer les liens. Usage unique, à expiration.

use axum::http::HeaderMap;
use chrono::{Duration, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::config::Config;
use crate::crypto::{random_hex, sha256_hex};
use crate::domain::catalog::EmailTokenType;

/// Invalide les jetons encore vivants d'un utilisateur pour un type donné —
/// un seul jeton d'invitation valide à la fois : un lien remplacé ne doit pas
/// rester un droit d'écrasement de mot de passe pendant son TTL.
pub async fn invalidate(conn: &mut PgConnection, user_id: Uuid, kind: EmailTokenType) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE email_tokens SET used_at = now() WHERE user_id = $1 AND type = $2 AND used_at IS NULL")
        .bind(user_id)
        .bind(kind)
        .execute(conn)
        .await?;
    Ok(())
}

/// Crée un jeton et retourne sa valeur brute (48 caractères hexadécimaux).
pub async fn create(
    conn: &mut PgConnection,
    user_id: Uuid,
    kind: EmailTokenType,
    ttl_minutes: i64,
) -> Result<String, sqlx::Error> {
    let raw = random_hex(24);
    sqlx::query("INSERT INTO email_tokens (user_id, type, token_hash, expires_at) VALUES ($1, $2, $3, $4)")
        .bind(user_id)
        .bind(kind)
        .bind(sha256_hex(&raw))
        .bind(Utc::now() + Duration::minutes(ttl_minutes))
        .execute(conn)
        .await?;
    Ok(raw)
}

/// Consomme un jeton valide (bon type, non expiré, non utilisé) → userId.
pub async fn consume(db: &PgPool, kind: EmailTokenType, raw: &str) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE email_tokens SET used_at = now()
         WHERE token_hash = $1 AND type = $2 AND used_at IS NULL AND expires_at > now()
         RETURNING user_id",
    )
    .bind(sha256_hex(raw))
    .bind(kind)
    .fetch_optional(db)
    .await
}

/// Base des liens de courriel : variable explicite en prod, origine de la requête sinon.
pub fn app_base_url(config: &Config, headers: &HeaderMap) -> String {
    if let Some(base) = &config.public_base_url {
        return base.clone();
    }
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim);
    let proto = header("x-forwarded-proto")
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| *v == "http" || *v == "https")
        .unwrap_or("http");
    let host = header("host").unwrap_or("localhost");
    format!("{proto}://{host}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_explicite_prioritaire() {
        let mut config = Config::for_tests("postgresql://x");
        config.public_base_url = Some("https://recettes.example.com".into());
        let mut headers = HeaderMap::new();
        headers.insert("host", "attaquant.example".parse().unwrap());
        assert_eq!(app_base_url(&config, &headers), "https://recettes.example.com");
    }

    #[test]
    fn origine_de_la_requete_en_dev() {
        let config = Config::for_tests("postgresql://x");
        let mut headers = HeaderMap::new();
        headers.insert("host", "127.0.0.1:3000".parse().unwrap());
        assert_eq!(app_base_url(&config, &headers), "http://127.0.0.1:3000");
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        assert_eq!(app_base_url(&config, &headers), "https://127.0.0.1:3000");
    }
}
