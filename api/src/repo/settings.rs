//! Réglages du compte — chaque fonction est scopée sur l'utilisateur de la session.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, QueryBuilder};
use uuid::Uuid;

use super::admin::lock_active_admins;
use super::users::find_by_id;
use crate::crypto::{hash_password, verify_password};
use crate::domain::catalog::{Locale, Role, UnitSystem};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto {
    pub name: String,
    pub email: String,
    #[serde(serialize_with = "crate::domain::time::serialize_iso_opt")]
    pub email_verified_at: Option<DateTime<Utc>>,
    pub locale: Locale,
    pub unit_system: UnitSystem,
    pub default_dish_count: i32,
    pub default_people_count: i32,
    pub default_days_count: i32,
    pub rotation_weeks: i32,
    pub totp_enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ProfilePatch {
    pub name: Option<String>,
    pub locale: Option<Locale>,
    pub unit_system: Option<UnitSystem>,
    pub default_dish_count: Option<i32>,
    pub default_people_count: Option<i32>,
    pub default_days_count: Option<i32>,
    pub rotation_weeks: Option<i32>,
}

impl ProfilePatch {
    /// Clés modifiées (audit : les valeurs n'apportent rien).
    pub fn fields(&self) -> Vec<&'static str> {
        let mut fields = Vec::new();
        if self.name.is_some() {
            fields.push("name")
        }
        if self.locale.is_some() {
            fields.push("locale")
        }
        if self.unit_system.is_some() {
            fields.push("unitSystem")
        }
        if self.default_dish_count.is_some() {
            fields.push("defaultDishCount")
        }
        if self.default_people_count.is_some() {
            fields.push("defaultPeopleCount")
        }
        if self.default_days_count.is_some() {
            fields.push("defaultDaysCount")
        }
        if self.rotation_weeks.is_some() {
            fields.push("rotationWeeks")
        }
        fields
    }
}

pub async fn get_profile(db: &PgPool, user_id: Uuid) -> Result<Option<ProfileDto>, sqlx::Error> {
    Ok(find_by_id(db, user_id).await?.map(|user| ProfileDto {
        name: user.name,
        email: user.email,
        email_verified_at: user.email_verified_at,
        locale: user.locale,
        unit_system: user.unit_system,
        default_dish_count: user.default_dish_count,
        default_people_count: user.default_people_count,
        default_days_count: user.default_days_count,
        rotation_weeks: user.rotation_weeks,
        totp_enabled: user.totp_enabled_at.is_some(),
    }))
}

pub async fn update_profile(
    db: &PgPool,
    user_id: Uuid,
    patch: &ProfilePatch,
) -> Result<Option<ProfileDto>, sqlx::Error> {
    let mut query = QueryBuilder::new("UPDATE users SET ");
    let mut set = query.separated(", ");
    if let Some(v) = &patch.name {
        set.push("name = ").push_bind_unseparated(v.clone());
    }
    if let Some(v) = patch.locale {
        set.push("locale = ").push_bind_unseparated(v);
    }
    if let Some(v) = patch.unit_system {
        set.push("unit_system = ").push_bind_unseparated(v);
    }
    if let Some(v) = patch.default_dish_count {
        set.push("default_dish_count = ").push_bind_unseparated(v);
    }
    if let Some(v) = patch.default_people_count {
        set.push("default_people_count = ").push_bind_unseparated(v);
    }
    if let Some(v) = patch.default_days_count {
        set.push("default_days_count = ").push_bind_unseparated(v);
    }
    if let Some(v) = patch.rotation_weeks {
        set.push("rotation_weeks = ").push_bind_unseparated(v);
    }
    if !patch.fields().is_empty() {
        query.push(" WHERE id = ").push_bind(user_id);
        query.build().execute(db).await?;
    }
    get_profile(db, user_id).await
}

pub enum PasswordCheck {
    /// Mot de passe changé ; porte la nouvelle époque de session.
    Ok(i32),
    InvalidPassword,
    NotFound,
}

pub async fn change_password(
    db: &PgPool,
    user_id: Uuid,
    current: &str,
    new: &str,
) -> Result<PasswordCheck, sqlx::Error> {
    let Some(user) = find_by_id(db, user_id).await? else {
        return Ok(PasswordCheck::NotFound);
    };
    if !verify_password(user.password_hash, current.to_string()).await {
        return Ok(PasswordCheck::InvalidPassword);
    }
    let epoch = super::users::change_password_hash(db, user_id, &hash_password(new.to_string()).await).await?;
    Ok(PasswordCheck::Ok(epoch))
}

pub enum DeleteAccount {
    Ok,
    InvalidPassword,
    NotFound,
    LastAdmin,
}

/// Suppression immédiate et définitive (cascade sur toutes les données métier).
/// Refusée au dernier admin actif : l'instance deviendrait ingérable — même
/// garde-fou que dans `admin.rs`, appliqué au seul contournement possible.
pub async fn delete_account(db: &PgPool, user_id: Uuid, password: &str) -> Result<DeleteAccount, sqlx::Error> {
    let Some(user) = find_by_id(db, user_id).await? else {
        return Ok(DeleteAccount::NotFound);
    };
    if !verify_password(user.password_hash.clone(), password.to_string()).await {
        return Ok(DeleteAccount::InvalidPassword);
    }
    let mut tx = db.begin().await?;
    if user.role == Role::Admin && user.disabled_at.is_none() && lock_active_admins(&mut tx).await? <= 1 {
        return Ok(DeleteAccount::LastAdmin);
    }
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(DeleteAccount::Ok)
}
