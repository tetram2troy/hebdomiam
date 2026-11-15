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

pub async fn start_setup(db: &PgPool, user_id: Uuid, email: &str) -> Result<TotpSetup, sqlx::Error> {
    let secret = totp::generate_secret();
    sqlx::query("UPDATE users SET totp_secret = $2, totp_enabled_at = NULL WHERE id = $1")
        .bind(user_id)
        .bind(&secret)
        .execute(db)
        .await?;
    Ok(TotpSetup {
        otpauth_url: totp::otpauth_uri("Hebdomiam", email, &secret),
        secret,
    })
}

pub enum Activation {
    Activated { backup_code: String },
    InvalidCode,
    NoSetup,
}

pub async fn activate(db: &PgPool, user_id: Uuid, code: &str) -> Result<Activation, sqlx::Error> {
    let Some(user) = find_by_id(db, user_id).await? else {
        return Ok(Activation::NoSetup);
    };
    let Some(secret) = user.totp_secret.filter(|_| user.totp_enabled_at.is_none()) else {
        return Ok(Activation::NoSetup);
    };
    if !totp::verify(&secret, code, unix_now()) {
        return Ok(Activation::InvalidCode);
    }
    let backup_code = random_hex(6); // 12 caractères
    sqlx::query("UPDATE users SET totp_enabled_at = now(), totp_backup_hash = $2 WHERE id = $1")
        .bind(user_id)
        .bind(sha256_hex(&backup_code))
        .execute(db)
        .await?;
    Ok(Activation::Activated { backup_code })
}

/// Désactive la 2FA après vérification du mot de passe ; false si refusé.
pub async fn disable(db: &PgPool, user_id: Uuid, password: &str) -> Result<bool, sqlx::Error> {
    let Some(user) = find_by_id(db, user_id).await? else {
        return Ok(false);
    };
    if !verify_password(user.password_hash, password.to_string()).await {
        return Ok(false);
    }
    sqlx::query("UPDATE users SET totp_secret = NULL, totp_enabled_at = NULL, totp_backup_hash = NULL WHERE id = $1")
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(true)
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
    if totp::verify(secret, code, unix_now()) {
        return Ok(Some(LoginMethod::Totp));
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
