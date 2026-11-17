//! Sessions scellées sans état serveur (PRD F1) : le cookie porte la session
//! chiffrée et authentifiée (AES-256-GCM, clé dérivée de SESSION_PASSWORD).
//! Cookie `httpOnly`, `secure`, `sameSite=lax`, 7 jours ; l'âge est aussi
//! vérifié à l'intérieur du sceau (un cookie rejoué au-delà est refusé).
//!
//! La révocation (compte désactivé/supprimé) est assurée par le middleware
//! `account_status` qui relit la base à chaque requête API.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use axum::extract::FromRequestParts;
use axum::http::HeaderMap;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::client_ip::ClientIp;
use crate::domain::catalog::Role;
use crate::error::ApiError;
use crate::logger::log_warn;
use crate::repo::users;
use crate::state::AppState;

/// Préfixe `__Host-` : le navigateur n'accepte ce cookie que posé par cet hôte
/// exact, en HTTPS, sans attribut Domain — une autre application du même
/// domaine (sous-domaine voisin du home lab) ne peut pas en glisser un.
pub const COOKIE_NAME: &str = "__Host-hebdomiam-session";
pub const MAX_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;

/// Copie publique de l'utilisateur dans la session — le rôle ne sert qu'à
/// l'affichage (nav, middleware client) : le serveur le relit toujours en base.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: Role,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<SessionUser>,
    /// Mot de passe validé, code TOTP attendu — pas encore connecté.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_totp_user_id: Option<Uuid>,
    /// Instant de la CONNEXION (secondes Unix) — conservé quand le cookie est
    /// re-scellé : la durée de vie de 7 jours est absolue, non renouvelable.
    pub issued_at: i64,
    /// Époque de session du compte au scellement (users.session_epoch) : une
    /// époque dépassée = session révoquée (voir le middleware account_status).
    #[serde(default)]
    pub epoch: i32,
}

impl SessionData {
    pub fn for_user(user: SessionUser, epoch: i32) -> Self {
        SessionData {
            user: Some(user),
            pending_totp_user_id: None,
            issued_at: now(),
            epoch,
        }
    }

    pub fn pending_totp(user_id: Uuid) -> Self {
        SessionData {
            user: None,
            pending_totp_user_id: Some(user_id),
            issued_at: now(),
            epoch: 0,
        }
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[derive(Clone)]
pub struct SessionKey {
    cipher: Aes256Gcm,
}

impl SessionKey {
    /// Dérive la clé AES du secret de session par Argon2id (une fois, au
    /// démarrage) : même un secret trop faible ne se devine pas à la vitesse
    /// d'un simple hachage à partir d'un cookie capturé.
    pub fn from_password(password: &str) -> Self {
        let derived = crate::crypto::derive_key(password, b"hebdomiam-session-v2");
        let key = Key::<Aes256Gcm>::from_slice(&derived);
        SessionKey {
            cipher: Aes256Gcm::new(key),
        }
    }

    pub fn seal(&self, data: &SessionData) -> String {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let plaintext = serde_json::to_vec(data).expect("session sérialisable");
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_ref())
            .expect("chiffrement AES-GCM");
        let mut sealed = nonce.to_vec();
        sealed.extend_from_slice(&ciphertext);
        URL_SAFE_NO_PAD.encode(sealed)
    }

    /// Ouvre un sceau ; `None` si falsifié, illisible ou expiré.
    pub fn unseal(&self, value: &str) -> Option<SessionData> {
        self.unseal_at(value, now())
    }

    pub fn unseal_at(&self, value: &str, now: i64) -> Option<SessionData> {
        let sealed = URL_SAFE_NO_PAD.decode(value).ok()?;
        if sealed.len() < 12 {
            return None;
        }
        let (nonce, ciphertext) = sealed.split_at(12);
        let plaintext = self.cipher.decrypt(Nonce::from_slice(nonce), ciphertext).ok()?;
        let data: SessionData = serde_json::from_slice(&plaintext).ok()?;
        let age = now - data.issued_at;
        (0..=MAX_AGE_SECONDS).contains(&age).then_some(data)
    }
}

pub fn set_cookie(value: &str) -> String {
    format!("{COOKIE_NAME}={value}; Path=/; Max-Age={MAX_AGE_SECONDS}; HttpOnly; Secure; SameSite=Lax")
}

pub fn clear_cookie() -> String {
    format!("{COOKIE_NAME}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax")
}

/// Extrait la valeur brute du cookie de session des en-têtes `Cookie`.
pub fn cookie_value(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE_NAME)
        .map(|(_, value)| value.to_string())
}

pub fn read_session(headers: &HeaderMap, key: &SessionKey) -> Option<SessionData> {
    cookie_value(headers).and_then(|value| key.unseal(&value))
}

/// Session courante, éventuellement vide.
pub struct MaybeSession(pub Option<SessionData>);

impl FromRequestParts<AppState> for MaybeSession {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        Ok(MaybeSession(read_session(&parts.headers, &state.session_key)))
    }
}

/// Session connectée complète (utilisateur, date de connexion, époque) — pour
/// les routes qui re-scellent le cookie sans le rajeunir.
pub struct CurrentSession {
    pub user: SessionUser,
    pub issued_at: i64,
    /// Époque scellée dans le cookie (vérifiée par le middleware account_status).
    pub epoch: i32,
}

impl CurrentSession {
    /// Même session (même date de connexion), nom ou époque mis à jour.
    pub fn resealed(&self, user: SessionUser, epoch: i32) -> SessionData {
        SessionData {
            user: Some(user),
            pending_totp_user_id: None,
            issued_at: self.issued_at,
            epoch,
        }
    }
}

impl FromRequestParts<AppState> for CurrentSession {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let session = read_session(&parts.headers, &state.session_key).ok_or_else(ApiError::unauthorized)?;
        let user = session.user.ok_or_else(ApiError::unauthorized)?;
        Ok(CurrentSession {
            user,
            issued_at: session.issued_at,
            epoch: session.epoch,
        })
    }
}

/// Utilisateur connecté exigé (équivalent de `requireUserSession`).
pub struct CurrentUser(pub SessionUser);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        read_session(&parts.headers, &state.session_key)
            .and_then(|session| session.user)
            .map(CurrentUser)
            .ok_or_else(ApiError::unauthorized)
    }
}

/// Session admin exigée pour les routes /api/admin/*.
/// Le rôle et le statut sont relus EN BASE à chaque appel : le cookie scellé
/// peut être périmé (démotion, désactivation) — on ne lui fait jamais confiance
/// pour une décision d'autorisation.
pub struct AdminUser(pub SessionUser);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let CurrentUser(user) = CurrentUser::from_request_parts(parts, state).await?;
        let status = users::get_account_status(&state.db, user.id).await?;
        let allowed = matches!(&status, Some(s) if s.role == Role::Admin && s.disabled_at.is_none());
        if !allowed {
            let ClientIp(ip) = ClientIp::from_request_parts(parts, state).await?;
            log_warn("security.admin_denied", json!({ "userId": user.id, "ip": ip }));
            return Err(ApiError::new(403, "errors.forbidden"));
        }
        Ok(AdminUser(user))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> SessionUser {
        SessionUser {
            id: Uuid::nil(),
            email: "a@b.c".into(),
            name: "A".into(),
            role: Role::User,
        }
    }

    #[test]
    fn scelle_puis_ouvre() {
        let key = SessionKey::from_password("0123456789abcdef0123456789abcdef");
        let data = SessionData::for_user(user(), 0);
        assert_eq!(key.unseal(&key.seal(&data)), Some(data));
    }

    #[test]
    fn autre_cle_refusee() {
        let sealed = SessionKey::from_password("a".repeat(32).as_str()).seal(&SessionData::for_user(user(), 0));
        assert!(
            SessionKey::from_password("b".repeat(32).as_str())
                .unseal(&sealed)
                .is_none()
        );
    }

    #[test]
    fn sceau_falsifie_refuse() {
        let key = SessionKey::from_password("0123456789abcdef0123456789abcdef");
        let mut sealed = key.seal(&SessionData::for_user(user(), 0));
        let last = sealed.pop().unwrap();
        sealed.push(if last == 'A' { 'B' } else { 'A' });
        assert!(key.unseal(&sealed).is_none());
        assert!(key.unseal("pas-un-sceau").is_none());
    }

    #[test]
    fn expiration_dans_le_sceau() {
        let key = SessionKey::from_password("0123456789abcdef0123456789abcdef");
        let data = SessionData::for_user(user(), 0);
        let sealed = key.seal(&data);
        assert!(key.unseal_at(&sealed, data.issued_at + MAX_AGE_SECONDS).is_some());
        assert!(key.unseal_at(&sealed, data.issued_at + MAX_AGE_SECONDS + 1).is_none());
    }

    #[test]
    fn lecture_du_cookie_parmi_d_autres() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "cookie",
            "theme=dark; hebdomiam-session=faux; __Host-hebdomiam-session=abc; autre=1"
                .parse()
                .unwrap(),
        );
        assert_eq!(cookie_value(&headers).as_deref(), Some("abc"));
    }

    #[test]
    fn attributs_du_cookie() {
        let cookie = set_cookie("v");
        for attr in ["HttpOnly", "Secure", "SameSite=Lax", "Path=/", "Max-Age=604800"] {
            assert!(cookie.contains(attr), "{attr}");
        }
    }
}
