//! 2FA TOTP optionnelle (PRD §8, M5). Le secret est posé au setup et n'active
//! la 2FA qu'après vérification d'un premier code ; un code de secours unique
//! (haché) permet de rentrer si l'authenticateur est perdu.

use sqlx::PgPool;
use uuid::Uuid;

use super::users::find_by_id;
use crate::crypto::{constant_time_eq, random_hex, sha256_hex, verify_password};
use crate::totp;

fn unix_now() -> u64 {
    chrono::Utc::now().timestamp().max(0) as u64
}

pub struct TotpSetup {
    pub secret: String,
    pub otpauth_url: String,
}

/// Pose un nouveau secret en attente d'activation. `None` si la 2FA est déjà
/// active : la remplacer exige de la désactiver d'abord (mot de passe requis)
/// — sinon un cookie de session volé suffirait à retirer le second facteur.
/// Condition dans l'UPDATE lui-même : pas de course entre lecture et écriture.
pub async fn start_setup(db: &PgPool, user_id: Uuid, email: &str) -> Result<Option<TotpSetup>, sqlx::Error> {
    let secret = totp::generate_secret();
    let updated = sqlx::query("UPDATE users SET totp_secret = $2 WHERE id = $1 AND totp_enabled_at IS NULL")
        .bind(user_id)
        .bind(&secret)
        .execute(db)
        .await?;
    Ok((updated.rows_affected() == 1).then(|| TotpSetup {
        otpauth_url: totp::otpauth_uri("Hebdomiam", email, &secret),
        secret,
    }))
}

pub enum Activation {
    /// 2FA activée ; `session_epoch` : nouvelle époque (autres sessions révoquées).
    Activated {
        backup_code: String,
        session_epoch: i32,
    },
    InvalidPassword,
    InvalidCode,
    NoSetup,
}

/// Active la 2FA : mot de passe ET premier code exigés. Le mot de passe
/// empêche le détenteur d'un cookie volé d'enrôler SON authenticateur et de
/// verrouiller le propriétaire hors de son compte.
pub async fn activate(db: &PgPool, user_id: Uuid, password: &str, code: &str) -> Result<Activation, sqlx::Error> {
    let Some(user) = find_by_id(db, user_id).await? else {
        return Ok(Activation::NoSetup);
    };
    if !verify_password(user.password_hash, password.to_string()).await {
        return Ok(Activation::InvalidPassword);
    }
    let Some(secret) = user.totp_secret.filter(|_| user.totp_enabled_at.is_none()) else {
        return Ok(Activation::NoSetup);
    };
    let Some(step) = totp::verify_step(&secret, code, unix_now()) else {
        return Ok(Activation::InvalidCode);
    };
    let backup_code = random_hex(6); // 12 caractères
    let session_epoch: Option<i32> = sqlx::query_scalar(
        "UPDATE users SET totp_enabled_at = now(), totp_backup_hash = $2, totp_last_step = $3,
                session_epoch = session_epoch + 1
         WHERE id = $1 AND totp_enabled_at IS NULL RETURNING session_epoch",
    )
    .bind(user_id)
    .bind(sha256_hex(&backup_code))
    .bind(step)
    .fetch_optional(db)
    .await?;
    Ok(match session_epoch {
        Some(session_epoch) => Activation::Activated {
            backup_code,
            session_epoch,
        },
        None => Activation::NoSetup,
    })
}

/// Désactive la 2FA après vérification du mot de passe ; `None` si refusé,
/// sinon la nouvelle époque de session (autres sessions révoquées).
pub async fn disable(db: &PgPool, user_id: Uuid, password: &str) -> Result<Option<i32>, sqlx::Error> {
    let Some(user) = find_by_id(db, user_id).await? else {
        return Ok(None);
    };
    if !verify_password(user.password_hash, password.to_string()).await {
        return Ok(None);
    }
    sqlx::query_scalar(
        "UPDATE users SET totp_secret = NULL, totp_enabled_at = NULL, totp_backup_hash = NULL, totp_last_step = NULL,
                session_epoch = session_epoch + 1
         WHERE id = $1 RETURNING session_epoch",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginMethod {
    Totp,
    Backup,
}

impl LoginMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            LoginMethod::Totp => "totp",
            LoginMethod::Backup => "backup",
        }
    }
}

/// Vérifie un code TOTP (ou le code de secours, consommé à l'usage) au login.
/// Retourne la méthode acceptée — l'usage du code de secours mérite d'être
/// journalisé (authenticateur perdu… ou compromis).
pub async fn verify_login(db: &PgPool, user_id: Uuid, code: &str) -> Result<Option<LoginMethod>, sqlx::Error> {
    let Some(user) = find_by_id(db, user_id).await? else {
        return Ok(None);
    };
    let (Some(secret), Some(_)) = (&user.totp_secret, user.totp_enabled_at) else {
        return Ok(None);
    };
    if let Some(step) = totp::verify_step(secret, code, unix_now()) {
        // Anti-rejeu : le pas accepté doit être strictement plus récent que le
        // dernier utilisé. Un code observé (par-dessus l'épaule, page relais)
        // ne resservira pas, même dans sa fenêtre de tolérance.
        let fresh = sqlx::query(
            "UPDATE users SET totp_last_step = $2 WHERE id = $1 AND (totp_last_step IS NULL OR totp_last_step < $2)",
        )
        .bind(user_id)
        .bind(step)
        .execute(db)
        .await?;
        return Ok((fresh.rows_affected() == 1).then_some(LoginMethod::Totp));
    }
    if let Some(backup_hash) = &user.totp_backup_hash
        && constant_time_eq(backup_hash, &sha256_hex(code.trim()))
    {
        // Consommation conditionnelle : deux requêtes simultanées avec le
        // même code de secours ne passent pas toutes les deux.
        let consumed = sqlx::query("UPDATE users SET totp_backup_hash = NULL WHERE id = $1 AND totp_backup_hash = $2")
            .bind(user_id)
            .bind(backup_hash)
            .execute(db)
            .await?;
        if consumed.rows_affected() == 1 {
            return Ok(Some(LoginMethod::Backup));
        }
    }
    Ok(None)
}
