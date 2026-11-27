//! Gestion des comptes par un administrateur. L'autorisation (rôle admin) est
//! vérifiée en amont par l'extracteur `AdminUser` — ici vivent la logique et
//! les garde-fous anti-lockout :
//!   - `Self_` : un admin ne se cible jamais lui-même (son compte se gère dans
//!     les réglages) ;
//!   - `LastAdmin` : on ne retire jamais le dernier admin actif de l'instance.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgConnection, PgPool};
use uuid::Uuid;

use super::email_tokens;
use crate::crypto::{hash_password, random_hex};
use crate::domain::catalog::{EmailTokenType, Locale, Role};

/// 7 jours : l'invité n'est pas devant son écran.
pub const INVITE_TTL_MINUTES: i64 = 7 * 24 * 60;

/// Vue admin d'un compte — liste blanche délibérée : jamais de données métier
/// (recettes, plans…) ni de secrets, conformément à D2 « comptes privés ».
#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdminUserDto {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: Role,
    pub email_verified: bool,
    pub disabled: bool,
    #[serde(serialize_with = "crate::domain::time::serialize_iso")]
    pub created_at: DateTime<Utc>,
}

/// Colonnes de `AdminUserDto`. Macro et non constante, pour composer les
/// requêtes par `concat!` : elles restent des `&'static str` fixées à la
/// compilation (voir `user_columns!` dans `users.rs`).
macro_rules! dto_columns {
    () => {
        "id, email, name, role, email_verified_at IS NOT NULL AS email_verified, disabled_at IS NOT NULL AS disabled, created_at"
    };
}

/// Verrouille (FOR UPDATE) puis compte les admins comptés comme relève :
/// rôle admin, non désactivés ET courriel vérifié. Un compte invité jamais
/// activé a un mot de passe inutilisable — le compter permettrait un lockout.
/// Le verrou rend le décompte sûr quand deux admins agissent en même temps
/// (en READ COMMITTED, un simple COUNT laisserait les deux lire « 2 »).
/// Verrous pris dans l'ordre des id et TOUJOURS avant celui de la cible :
/// deux actions admin croisées (A rétrograde B pendant que B rétrograde A)
/// se sérialisent au lieu de s'interbloquer. À appeler dans une transaction.
pub async fn lock_active_admins(conn: &mut PgConnection) -> Result<usize, sqlx::Error> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE role = 'admin' AND disabled_at IS NULL AND email_verified_at IS NOT NULL ORDER BY id FOR UPDATE",
    )
    .fetch_all(conn)
    .await?;
    Ok(ids.len())
}

pub async fn list_users(db: &PgPool) -> Result<Vec<AdminUserDto>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        dto_columns!(),
        " FROM users ORDER BY created_at ASC"
    ))
    .fetch_all(db)
    .await
}

pub enum CreateInvited {
    Created { user: AdminUserDto, token: String },
    EmailTaken,
}

/// Crée un compte invité : mot de passe aléatoire inutilisable — le compte ne
/// devient connectable qu'à l'acceptation de l'invitation.
pub async fn create_invited_user(db: &PgPool, email: &str, name: &str) -> Result<CreateInvited, sqlx::Error> {
    let password_hash = hash_password(random_hex(32)).await;
    // Transaction : pas de compte sans jeton d'invitation.
    let mut tx = db.begin().await?;
    let created: Option<AdminUserDto> = sqlx::query_as(concat!(
        "INSERT INTO users (email, name, password_hash, invite_pending) VALUES ($1, $2, $3, true)
         ON CONFLICT (email) DO NOTHING RETURNING ",
        dto_columns!()
    ))
    .bind(email)
    .bind(name)
    .bind(password_hash)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(user) = created else {
        return Ok(CreateInvited::EmailTaken);
    };
    let token = email_tokens::create(&mut tx, user.id, EmailTokenType::Invite, INVITE_TTL_MINUTES).await?;
    tx.commit().await?;
    Ok(CreateInvited::Created { user, token })
}

pub enum RenewInvite {
    Renewed {
        email: String,
        locale: Locale,
        token: String,
    },
    NotFound,
    AlreadyActive,
    Disabled,
}

/// Nouveau jeton d'invitation pour un compte jamais activé. Les jetons
/// précédents sont invalidés : un seul lien vivant à la fois.
pub async fn renew_invite(db: &PgPool, target_id: Uuid) -> Result<RenewInvite, sqlx::Error> {
    #[derive(FromRow)]
    struct InviteTarget {
        email: String,
        locale: Locale,
        invite_pending: bool,
        disabled_at: Option<DateTime<Utc>>,
    }
    let row: Option<InviteTarget> =
        sqlx::query_as("SELECT email, locale, invite_pending, disabled_at FROM users WHERE id = $1")
            .bind(target_id)
            .fetch_optional(db)
            .await?;
    let Some(InviteTarget {
        email,
        locale,
        invite_pending,
        disabled_at,
    }) = row
    else {
        return Ok(RenewInvite::NotFound);
    };
    // Seul un compte dont l'invitation est EN ATTENTE reçoit un nouveau lien.
    // « Courriel non vérifié » ne suffit pas : l'admin de bootstrap ou un
    // compte activé par « mot de passe oublié » sont en usage — leur envoyer
    // une invitation reviendrait à offrir leur mot de passe au porteur du lien.
    if !invite_pending {
        return Ok(RenewInvite::AlreadyActive);
    }
    // L'UI masque le bouton, mais l'API est la frontière : pas de lien
    // d'activation pour un compte désactivé.
    if disabled_at.is_some() {
        return Ok(RenewInvite::Disabled);
    }
    let mut tx = db.begin().await?;
    email_tokens::invalidate(&mut tx, target_id, EmailTokenType::Invite).await?;
    let token = email_tokens::create(&mut tx, target_id, EmailTokenType::Invite, INVITE_TTL_MINUTES).await?;
    tx.commit().await?;
    Ok(RenewInvite::Renewed { email, locale, token })
}

#[derive(Debug, Clone, Default)]
pub struct AdminUserPatch {
    pub role: Option<Role>,
    pub disabled: Option<bool>,
}

pub enum AdminRefusal {
    NotFound,
    SelfTarget,
    LastAdmin,
}

#[derive(FromRow)]
struct TargetRow {
    role: Role,
    disabled_at: Option<DateTime<Utc>>,
    email_verified_at: Option<DateTime<Utc>>,
}

impl TargetRow {
    fn is_active_admin(&self) -> bool {
        self.role == Role::Admin && self.disabled_at.is_none() && self.email_verified_at.is_some()
    }
}

async fn lock_target(conn: &mut PgConnection, target_id: Uuid) -> Result<Option<TargetRow>, sqlx::Error> {
    sqlx::query_as("SELECT role, disabled_at, email_verified_at FROM users WHERE id = $1 FOR UPDATE")
        .bind(target_id)
        .fetch_optional(conn)
        .await
}

pub async fn update_user(
    db: &PgPool,
    actor_id: Uuid,
    target_id: Uuid,
    patch: &AdminUserPatch,
) -> Result<Result<AdminUserDto, AdminRefusal>, sqlx::Error> {
    if actor_id == target_id {
        return Ok(Err(AdminRefusal::SelfTarget));
    }
    let mut tx = db.begin().await?;
    let active_admins = lock_active_admins(&mut tx).await?;
    let Some(target) = lock_target(&mut tx, target_id).await? else {
        return Ok(Err(AdminRefusal::NotFound));
    };
    // Seul le retrait d'un admin capable d'administrer (vérifié, actif) peut
    // créer un lockout — rétrograder une invitation en attente est sans risque.
    let removes_active_admin =
        target.is_active_admin() && (patch.role == Some(Role::User) || patch.disabled == Some(true));
    if removes_active_admin && active_admins <= 1 {
        return Ok(Err(AdminRefusal::LastAdmin));
    }

    // Re-désactiver un compte déjà désactivé ne change pas la date d'origine.
    let disabled_at = match patch.disabled {
        Some(true) => Some(target.disabled_at.unwrap_or_else(Utc::now)),
        Some(false) => None,
        None => target.disabled_at,
    };
    let row: Option<AdminUserDto> = sqlx::query_as(concat!(
        "UPDATE users SET role = $2, disabled_at = $3 WHERE id = $1 RETURNING ",
        dto_columns!()
    ))
    .bind(target_id)
    .bind(patch.role.unwrap_or(target.role))
    .bind(disabled_at)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.ok_or(AdminRefusal::NotFound))
}

/// Suppression définitive (cascade sur toutes les données métier).
pub async fn delete_user(
    db: &PgPool,
    actor_id: Uuid,
    target_id: Uuid,
) -> Result<Result<(), AdminRefusal>, sqlx::Error> {
    if actor_id == target_id {
        return Ok(Err(AdminRefusal::SelfTarget));
    }
    let mut tx = db.begin().await?;
    let active_admins = lock_active_admins(&mut tx).await?;
    let Some(target) = lock_target(&mut tx, target_id).await? else {
        return Ok(Err(AdminRefusal::NotFound));
    };
    if target.is_active_admin() && active_admins <= 1 {
        return Ok(Err(AdminRefusal::LastAdmin));
    }
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(target_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Ok(()))
}
