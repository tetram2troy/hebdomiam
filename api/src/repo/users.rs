//! Cycle de vie des comptes côté inscription publique, connexion et
//! invitation. La gestion par les admins vit dans `admin.rs`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::email_tokens;
use crate::domain::catalog::{EmailTokenType, Locale, Role, UnitSystem};
use crate::session::SessionUser;

#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub name: String,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub role: Role,
    pub disabled_at: Option<DateTime<Utc>>,
    pub locale: Locale,
    pub unit_system: UnitSystem,
    pub default_dish_count: i32,
    pub default_people_count: i32,
    pub default_days_count: i32,
    pub rotation_weeks: i32,
    pub totp_secret: Option<String>,
    pub totp_enabled_at: Option<DateTime<Utc>>,
    pub totp_backup_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl UserRow {
    pub fn session_user(&self) -> SessionUser {
        SessionUser {
            id: self.id,
            email: self.email.clone(),
            name: self.name.clone(),
            role: self.role,
        }
    }
}

const USER_COLUMNS: &str = "id, email, password_hash, name, email_verified_at, role, disabled_at, locale, unit_system,
    default_dish_count, default_people_count, default_days_count, rotation_weeks,
    totp_secret, totp_enabled_at, totp_backup_hash, created_at";

pub async fn find_by_id(db: &PgPool, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn find_by_email(db: &PgPool, email: &str) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as(&format!("SELECT {USER_COLUMNS} FROM users WHERE email = $1"))
        .bind(email)
        .fetch_optional(db)
        .await
}

pub async fn count_users(db: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(db).await
}

/// Statut minimal d'un compte pour les décisions d'autorisation (gardes de
/// session) — None si le compte n'existe plus.
#[derive(Debug, Clone, FromRow)]
pub struct AccountStatus {
    pub role: Role,
    pub disabled_at: Option<DateTime<Utc>>,
}

pub async fn get_account_status(db: &PgPool, id: Uuid) -> Result<Option<AccountStatus>, sqlx::Error> {
    sqlx::query_as("SELECT role, disabled_at FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(db)
        .await
}

pub enum RegisterResult {
    Created(SessionUser),
    Closed,
    EmailTaken,
}

/// Inscription publique : fermée dès qu'un compte existe — seule la première
/// inscription d'une instance vierge passe, et ce compte est promu admin
/// (« premier inscrit = admin »). Les comptes suivants sont créés par un
/// admin via une invitation.
pub async fn register(
    db: &PgPool,
    email: &str,
    name: &str,
    password_hash: &str,
) -> Result<RegisterResult, sqlx::Error> {
    let mut tx = db.begin().await?;
    // Sérialise les inscriptions concurrentes sur base vierge : sans verrou,
    // deux requêtes simultanées liraient « 0 compte » et créeraient deux admins.
    sqlx::query("SELECT pg_advisory_xact_lock(4242)")
        .execute(&mut *tx)
        .await?;
    let existing: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&mut *tx)
        .await?;
    if existing > 0 {
        return Ok(RegisterResult::Closed);
    }
    let created: Option<(Uuid, String, String, Role)> = sqlx::query_as(
        "INSERT INTO users (email, name, password_hash, role) VALUES ($1, $2, $3, 'admin')
         ON CONFLICT (email) DO NOTHING RETURNING id, email, name, role",
    )
    .bind(email)
    .bind(name)
    .bind(password_hash)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(match created {
        Some((id, email, name, role)) => RegisterResult::Created(SessionUser { id, email, name, role }),
        None => RegisterResult::EmailTaken,
    })
}

pub enum AcceptInvitation {
    Accepted(Uuid),
    InvalidToken,
    AlreadyActive,
    Disabled,
}

/// Acceptation d'une invitation : fixe le mot de passe et marque le courriel
/// vérifié (la possession du lien en est la preuve). Refusée si le compte est
/// déjà actif — un jeton remplacé mais pas encore expiré ne doit jamais servir
/// à écraser le mot de passe d'un compte en usage. Le jeton est consommé même
/// en cas de refus : c'est voulu, un lien caduc doit mourir au premier clic.
pub async fn accept_invitation(
    db: &PgPool,
    raw_token: &str,
    password_hash: &str,
) -> Result<AcceptInvitation, sqlx::Error> {
    let Some(user_id) = email_tokens::consume(db, EmailTokenType::Invite, raw_token).await? else {
        return Ok(AcceptInvitation::InvalidToken);
    };
    let Some(user) = find_by_id(db, user_id).await? else {
        return Ok(AcceptInvitation::InvalidToken);
    };
    if user.email_verified_at.is_some() {
        return Ok(AcceptInvitation::AlreadyActive);
    }
    if user.disabled_at.is_some() {
        return Ok(AcceptInvitation::Disabled);
    }
    sqlx::query("UPDATE users SET password_hash = $2, email_verified_at = now() WHERE id = $1")
        .bind(user_id)
        .bind(password_hash)
        .execute(db)
        .await?;
    Ok(AcceptInvitation::Accepted(user_id))
}

pub async fn set_password_hash(db: &PgPool, user_id: Uuid, password_hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1")
        .bind(user_id)
        .bind(password_hash)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn mark_email_verified(db: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET email_verified_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}
