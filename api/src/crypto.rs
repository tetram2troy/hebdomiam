//! Primitives cryptographiques partagées : Argon2id pour les mots de passe,
//! SHA-256 pour les jetons stockés, aléa système.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Argon2id, paramètres recommandés OWASP : 19 Mio de mémoire, 2 itérations.
/// Mêmes paramètres et même format PHC que l'ancien backend (@node-rs/argon2) :
/// les empreintes existantes restent vérifiables.
fn argon2() -> Argon2<'static> {
    let params = Params::new(19_456, 2, 1, None).expect("paramètres Argon2 valides");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Hache un mot de passe (calcul coûteux : hors du runtime async).
pub async fn hash_password(password: String) -> String {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        argon2()
            .hash_password(password.as_bytes(), &salt)
            .expect("hachage Argon2")
            .to_string()
    })
    .await
    .expect("tâche de hachage")
}

/// Vérifie un mot de passe ; une empreinte illisible vaut refus.
pub async fn verify_password(hash: String, password: String) -> bool {
    tokio::task::spawn_blocking(move || match PasswordHash::new(&hash) {
        Ok(parsed) => argon2().verify_password(password.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    })
    .await
    .unwrap_or(false)
}

/// Dérive une clé de 32 octets d'un secret de configuration (Argon2id, sel
/// fixe propre à l'usage). Coûteux par construction : à appeler au démarrage.
pub fn derive_key(secret: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    argon2()
        .hash_password_into(secret.as_bytes(), salt, &mut key)
        .expect("dérivation Argon2 (sel ≥ 8 octets)");
    key
}

pub fn sha256_hex(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes
}

/// `len` octets aléatoires encodés en hexadécimal (2×len caractères).
pub fn random_hex(len: usize) -> String {
    hex::encode(random_bytes(len))
}

/// Comparaison à temps constant (codes de secours).
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hachage_puis_verification() {
        let hash = hash_password("mot-de-passe-tres-long".into()).await;
        assert!(hash.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
        assert!(verify_password(hash.clone(), "mot-de-passe-tres-long".into()).await);
        assert!(!verify_password(hash, "mauvais".into()).await);
    }

    #[tokio::test]
    async fn empreinte_illisible_refusee() {
        assert!(!verify_password("$argon2id$fake-hash-for-tests".into(), "x".into()).await);
    }

    #[tokio::test]
    async fn empreinte_node_rs_compatible() {
        // Empreinte produite par @node-rs/argon2 (ancien backend) pour « hebdomiam-test-123 ».
        let legacy =
            "$argon2id$v=19$m=19456,t=2,p=1$T0vKXBIdSTOlBdjeVTuWsQ$fSOv76RtSA3ctE3n5bHvrAVhbJCWIR9ZzEWvwxakTa8";
        assert!(verify_password(legacy.into(), "hebdomiam-test-123".into()).await);
        assert!(!verify_password(legacy.into(), "autre".into()).await);
    }

    #[test]
    fn derivation_de_cle_deterministe_et_separee_par_sel() {
        let key = derive_key("secret-de-configuration", b"usage-numero-un");
        assert_eq!(key, derive_key("secret-de-configuration", b"usage-numero-un"));
        assert_ne!(key, derive_key("secret-de-configuration", b"usage-numero-deux"));
        assert_ne!(key, derive_key("autre-secret", b"usage-numero-un"));
    }

    #[test]
    fn sha256_connu() {
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn hex_aleatoire_longueur() {
        assert_eq!(random_hex(24).len(), 48);
        assert_ne!(random_hex(24), random_hex(24));
    }
}
