//! Clés d'API personnelles : authentification d'un outil externe (Claude Code)
//! par en-tête `Authorization: Bearer hbd_…`, sans cookie de session.
//!
//! Portée volontairement étroite : une clé ne sert qu'aux routes `/api/v1/*`
//! (créer une recette, lire ses tags) — jamais aux routes de session. Une clé
//! fuitée ne permet ni de lire la collection, ni de toucher au compte.

use axum::extract::FromRequestParts;
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::json;
use uuid::Uuid;

use crate::client_ip::ClientIp;
use crate::crypto::{random_bytes, sha256_hex};
use crate::error::ApiError;
use crate::logger::log_warn;
use crate::repo::api_keys;
use crate::state::AppState;

/// Préfixe reconnaissable (scanners de secrets, journaux de l'utilisateur).
pub const TOKEN_PREFIX: &str = "hbd_";
/// 32 octets d'aléa → 43 caractères base64url sans remplissage.
const SECRET_BYTES: usize = 32;
const SECRET_CHARS: usize = 43;
/// Caractères réaffichables dans la liste des clés (« hbd_ab12cd34 »).
const DISPLAY_PREFIX_CHARS: usize = TOKEN_PREFIX.len() + 8;

/// Échecs d'authentification tolérés par IP (fenêtre de 15 minutes) : la clé
/// est indevinable, mais chaque essai coûte une requête en base.
const AUTH_FAILURE_BUCKET: &str = "apikey:auth:ip";
const AUTH_MAX_FAILURES: usize = 20;
const AUTH_WINDOW_MS: u64 = 15 * 60_000;

/// Clé fraîchement générée : le secret complet n'existe qu'ici et dans la
/// réponse de création — la base ne garde que l'empreinte.
pub struct GeneratedKey {
    pub token: String,
    pub prefix: String,
    pub hash: String,
}

pub fn generate() -> GeneratedKey {
    let token = format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(random_bytes(SECRET_BYTES)));
    GeneratedKey {
        prefix: token[..DISPLAY_PREFIX_CHARS].to_string(),
        hash: sha256_hex(&token),
        token,
    }
}

/// Forme exacte d'une clé émise par `generate` — tout le reste est rejeté
/// sans requête en base.
pub fn is_well_formed(token: &str) -> bool {
    token.strip_prefix(TOKEN_PREFIX).is_some_and(|secret| {
        secret.len() == SECRET_CHARS
            && secret
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    })
}

/// Valeur de `Authorization: Bearer <jeton>` (schéma insensible à la casse,
/// RFC 7235) ; `None` si l'en-tête est absent ou d'un autre schéma.
pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?.trim();
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// Compte authentifié par clé d'API. Clé et statut du compte sont relus EN
/// BASE à chaque appel : une clé révoquée ou un compte désactivé est refusé
/// immédiatement.
pub struct ApiKeyUser {
    pub user_id: Uuid,
    pub key_id: Uuid,
}

impl FromRequestParts<AppState> for ApiKeyUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let Some(token) = bearer_token(&parts.headers).map(str::to_owned) else {
            return Err(ApiError::unauthorized());
        };
        let ClientIp(ip) = ClientIp::from_request_parts(parts, state).await?;
        // Réservée AVANT la vérification et rendue en cas de succès : seuls
        // les échecs restent comptés (même patron que le verrou de connexion).
        state
            .limits
            .reserve_attempt(AUTH_FAILURE_BUCKET, &ip, AUTH_MAX_FAILURES, AUTH_WINDOW_MS)?;
        let owner = if is_well_formed(&token) {
            api_keys::authenticate(&state.db, &sha256_hex(&token)).await?
        } else {
            None
        };
        match owner {
            Some(owner) => {
                state.limits.refund_attempt(AUTH_FAILURE_BUCKET, &ip);
                Ok(ApiKeyUser {
                    user_id: owner.user_id,
                    key_id: owner.key_id,
                })
            }
            None => {
                // Ni la clé ni son préfixe ne sont journalisés.
                log_warn("security.api_key_rejected", json!({ "ip": ip }));
                Err(ApiError::unauthorized())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cle_generee_bien_formee_et_unique() {
        let key = generate();
        assert!(key.token.starts_with("hbd_"));
        assert_eq!(key.token.len(), 4 + 43);
        assert!(is_well_formed(&key.token));
        assert_eq!(key.prefix, key.token[..12]);
        assert_eq!(key.hash, sha256_hex(&key.token));
        assert_ne!(generate().token, key.token);
    }

    #[test]
    fn formes_invalides_rejetees() {
        let valid = generate().token;
        assert!(!is_well_formed(""));
        assert!(!is_well_formed("hbd_"));
        assert!(!is_well_formed(&valid[..valid.len() - 1]));
        assert!(!is_well_formed(&format!("{valid}A")));
        assert!(!is_well_formed(&valid.replacen("hbd_", "xyz_", 1)));
        assert!(!is_well_formed(&format!("hbd_{}", "é".repeat(43))));
        assert!(!is_well_formed(&format!("hbd_{}", "+".repeat(43))));
    }

    #[test]
    fn lecture_de_l_en_tete_bearer() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer_token(&headers), None);
        headers.insert(AUTHORIZATION, "Bearer hbd_abc".parse().unwrap());
        assert_eq!(bearer_token(&headers), Some("hbd_abc"));
        headers.insert(AUTHORIZATION, "bearer   hbd_abc  ".parse().unwrap());
        assert_eq!(bearer_token(&headers), Some("hbd_abc"));
        headers.insert(AUTHORIZATION, "Basic dXNlcjpwYXNz".parse().unwrap());
        assert_eq!(bearer_token(&headers), None);
        headers.insert(AUTHORIZATION, "Bearer".parse().unwrap());
        assert_eq!(bearer_token(&headers), None);
    }
}
