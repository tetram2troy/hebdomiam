//! Cycle de vie des comptes côté inscription publique, connexion et
//! invitation. La gestion par les admins vit dans `admin.rs`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::email_tokens;
use crate::crypto::hash_password;
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
    /// Dernier pas TOTP accepté (anti-rejeu).
    pub totp_last_step: Option<i64>,
    /// Époque des sessions : un cookie scellé à une époque antérieure est refusé.
    pub session_epoch: i32,
    /// Invitation en attente d'acceptation (distinct de « courriel non vérifié »).
    pub invite_pending: bool,
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
    totp_secret, totp_enabled_at, totp_backup_hash, totp_last_step, session_epoch, invite_pending, created_at";

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
    pub session_epoch: i32,
}

pub async fn get_account_status(db: &PgPool, id: Uuid) -> Result<Option<AccountStatus>, sqlx::Error> {
    sqlx::query_as("SELECT role, disabled_at, session_epoch FROM users WHERE id = $1")
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

/// Révoque toutes les sessions scellées du compte (cookie volé compris) en
/// incrémentant son époque ; retourne la nouvelle époque, à sceller dans le
/// cookie de la session qui doit survivre.
pub async fn bump_session_epoch(db: &PgPool, user_id: Uuid) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar("UPDATE users SET session_epoch = session_epoch + 1 WHERE id = $1 RETURNING session_epoch")
        .bind(user_id)
        .fetch_one(db)
        .await
}

pub enum AcceptInvitation {
    Accepted(Uuid),
    InvalidToken,
    AlreadyActive,
    Disabled,
}

/// Acceptation d'une invitation : fixe le mot de passe et marque le courriel
/// vérifié (la possession du lien en est la preuve). Refusée si le compte n'a
/// pas d'invitation en attente — un jeton remplacé mais pas encore expiré ne
/// doit jamais servir à écraser le mot de passe d'un compte en usage. Le jeton
/// est consommé même en cas de refus : c'est voulu, un lien caduc doit mourir
/// au premier clic. Une seule transaction : pas de jeton brûlé sans activation
/// sur une erreur de base. Le mot de passe n'est haché (coûteux) qu'une fois
/// le jeton reconnu.
pub async fn accept_invitation(
    db: &PgPool,
    raw_token: &str,
    password: String,
) -> Result<AcceptInvitation, sqlx::Error> {
    let mut tx = db.begin().await?;
    let Some(user_id) = email_tokens::consume(&mut tx, EmailTokenType::Invite, raw_token).await? else {
        return Ok(AcceptInvitation::InvalidToken);
    };
    let row: Option<(bool, Option<DateTime<Utc>>)> =
        sqlx::query_as("SELECT invite_pending, disabled_at FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    let refusal = match row {
        None => Some(AcceptInvitation::InvalidToken),
        Some((false, _)) => Some(AcceptInvitation::AlreadyActive),
        Some((true, Some(_))) => Some(AcceptInvitation::Disabled),
        Some((true, None)) => None,
    };
    if let Some(refusal) = refusal {
        tx.commit().await?; // le jeton reste brûlé
        return Ok(refusal);
    }
    let password_hash = hash_password(password).await;
    sqlx::query(
        "UPDATE users SET password_hash = $2, email_verified_at = now(), invite_pending = false,
                session_epoch = session_epoch + 1 WHERE id = $1",
    )
    .bind(user_id)
    .bind(password_hash)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(AcceptInvitation::Accepted(user_id))
}

/// Réinitialisation par lien : consomme le jeton, fixe le mot de passe et, dans
/// la même transaction, ferme tout ce que l'ancien état laissait ouvert —
/// sessions existantes (époque), autres liens de réinitialisation, invitation
/// en attente (le lien d'invitation ne doit plus pouvoir écraser le mot de
/// passe). La possession du lien vaut vérification du courriel.
pub async fn reset_password(db: &PgPool, raw_token: &str, password: String) -> Result<Option<Uuid>, sqlx::Error> {
    let mut tx = db.begin().await?;
    let Some(user_id) = email_tokens::consume(&mut tx, EmailTokenType::Reset, raw_token).await? else {
        return Ok(None);
    };
    let password_hash = hash_password(password).await;
    sqlx::query(
        "UPDATE users SET password_hash = $2, email_verified_at = COALESCE(email_verified_at, now()),
                invite_pending = false, session_epoch = session_epoch + 1 WHERE id = $1",
    )
    .bind(user_id)
    .bind(password_hash)
    .execute(&mut *tx)
    .await?;
    email_tokens::invalidate(&mut tx, user_id, EmailTokenType::Reset).await?;
    email_tokens::invalidate(&mut tx, user_id, EmailTokenType::Invite).await?;
    tx.commit().await?;
    Ok(Some(user_id))
}

/// Changement de mot de passe depuis les réglages : révoque les autres
/// sessions et les liens de réinitialisation en circulation. Retourne la
/// nouvelle époque de session.
pub async fn change_password_hash(db: &PgPool, user_id: Uuid, password_hash: &str) -> Result<i32, sqlx::Error> {
    let mut tx = db.begin().await?;
    let epoch: i32 = sqlx::query_scalar(
        "UPDATE users SET password_hash = $2, session_epoch = session_epoch + 1 WHERE id = $1 RETURNING session_epoch",
    )
    .bind(user_id)
    .bind(password_hash)
    .fetch_one(&mut *tx)
    .await?;
    email_tokens::invalidate(&mut tx, user_id, EmailTokenType::Reset).await?;
    tx.commit().await?;
    Ok(epoch)
}

pub async fn mark_email_verified(db: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET email_verified_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}
