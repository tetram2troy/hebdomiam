//! Clés d'API d'un compte — chaque fonction de gestion est scopée sur
//! l'utilisateur de la session ; `authenticate` part de l'empreinte de la clé.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::users::find_by_id;
use crate::api_key;
use crate::crypto::verify_password;

/// Plafond par compte : assez pour un outil par poste, assez bas pour que la
/// liste reste lisible et qu'une session volée ne sème pas des clés à l'infini.
pub const MAX_KEYS_PER_USER: i64 = 10;

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyDto {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
    #[serde(serialize_with = "crate::domain::time::serialize_iso")]
    pub created_at: DateTime<Utc>,
    #[serde(serialize_with = "crate::domain::time::serialize_iso_opt")]
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Réponse de création : le secret complet n'est rendu qu'ici, une seule fois.
#[derive(Debug, Clone, Serialize)]
pub struct NewApiKey {
    pub key: ApiKeyDto,
    pub token: String,
}

pub enum CreateApiKey {
    Created(NewApiKey),
    InvalidPassword,
    LimitReached,
    NotFound,
}

/// Crée une clé après revérification du mot de passe : une session volée ne
/// peut pas se fabriquer un accès qui survivrait à sa révocation.
pub async fn create(db: &PgPool, user_id: Uuid, password: &str, name: &str) -> Result<CreateApiKey, sqlx::Error> {
    let Some(user) = find_by_id(db, user_id).await? else {
        return Ok(CreateApiKey::NotFound);
    };
    if !verify_password(user.password_hash, password.to_string()).await {
        return Ok(CreateApiKey::InvalidPassword);
    }
    let mut tx = db.begin().await?;
    // Verrou sur la ligne du compte : deux créations simultanées ne peuvent
    // pas toutes deux passer le contrôle du plafond.
    sqlx::query("SELECT 1 FROM users WHERE id = $1 FOR UPDATE")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM api_keys WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    if count >= MAX_KEYS_PER_USER {
        return Ok(CreateApiKey::LimitReached);
    }
    let generated = api_key::generate();
    let key: ApiKeyDto = sqlx::query_as(
        "INSERT INTO api_keys (user_id, name, prefix, token_hash) VALUES ($1, $2, $3, $4)
         RETURNING id, name, prefix, created_at, last_used_at",
    )
    .bind(user_id)
    .bind(name)
    .bind(&generated.prefix)
    .bind(&generated.hash)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(CreateApiKey::Created(NewApiKey {
        key,
        token: generated.token,
    }))
}

pub async fn list(db: &PgPool, user_id: Uuid) -> Result<Vec<ApiKeyDto>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, name, prefix, created_at, last_used_at FROM api_keys
         WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
}

/// Révocation définitive ; `false` si la clé n'existe pas OU appartient à un
/// autre compte (indiscernables : pas d'énumération).
pub async fn revoke(db: &PgPool, user_id: Uuid, key_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM api_keys WHERE id = $1 AND user_id = $2")
        .bind(key_id)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() == 1)
}

#[derive(Debug, Clone, FromRow)]
pub struct ApiKeyOwner {
    pub key_id: Uuid,
    pub user_id: Uuid,
}

/// Compte propriétaire d'une clé, si le compte est actif (ni désactivé, ni
/// invitation en attente) ; horodate la dernière utilisation au passage.
pub async fn authenticate(db: &PgPool, token_hash: &str) -> Result<Option<ApiKeyOwner>, sqlx::Error> {
    sqlx::query_as(
        "UPDATE api_keys k SET last_used_at = now()
         FROM users u
         WHERE k.token_hash = $1 AND u.id = k.user_id
           AND u.disabled_at IS NULL AND NOT u.invite_pending
         RETURNING k.id AS key_id, k.user_id",
    )
    .bind(token_hash)
    .fetch_optional(db)
    .await
}
