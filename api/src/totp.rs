//! TOTP (RFC 6238) : HMAC-SHA1, 6 chiffres, pas de 30 s — mêmes réglages que
//! otplib (ancien backend) : les secrets déjà enrôlés restent valides.

use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use sha1::Sha1;

const STEP_SECONDS: u64 = 30;
const DIGITS: u32 = 6;
/// Tolère ±30 s de dérive d'horloge entre le téléphone et le serveur (±1 pas).
const SKEW_STEPS: i64 = 1;

/// Nouveau secret : 20 octets aléatoires en base32 (32 caractères).
pub fn generate_secret() -> String {
    BASE32_NOPAD.encode(&crate::crypto::random_bytes(20))
}

fn decode_secret(secret: &str) -> Option<Vec<u8>> {
    let normalized: String = secret
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=')
        .collect::<String>()
        .to_uppercase();
    BASE32_NOPAD.decode(normalized.as_bytes()).ok()
}

fn code_at(key: &[u8], counter: u64) -> String {
    let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("HMAC accepte toute longueur de clé");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = ((digest[offset] as u32 & 0x7f) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | digest[offset + 3] as u32;
    format!("{:0width$}", binary % 10u32.pow(DIGITS), width = DIGITS as usize)
}

/// Code TOTP courant pour `unix_seconds` (utile aux tests).
pub fn generate(secret: &str, unix_seconds: u64) -> Option<String> {
    let key = decode_secret(secret)?;
    Some(code_at(&key, unix_seconds / STEP_SECONDS))
}

/// Vérifie un code avec une tolérance de ±1 pas.
pub fn verify(secret: &str, token: &str, unix_seconds: u64) -> bool {
    verify_step(secret, token, unix_seconds).is_some()
}

/// Comme `verify`, mais retourne le pas de temps du code accepté — à mémoriser
/// pour refuser tout rejeu de ce code (ou d'un code plus ancien).
pub fn verify_step(secret: &str, token: &str, unix_seconds: u64) -> Option<i64> {
    let token = token.trim();
    if token.len() != DIGITS as usize || !token.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let key = decode_secret(secret)?;
    let counter = (unix_seconds / STEP_SECONDS) as i64;
    (-SKEW_STEPS..=SKEW_STEPS)
        .map(|delta| counter + delta)
        .find(|step| *step >= 0 && crate::crypto::constant_time_eq(&code_at(&key, *step as u64), token))
}

fn percent_encode(input: &str) -> String {
    url::form_urlencoded::byte_serialize(input.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

/// URI `otpauth://` pour l'application d'authentification (QR code).
pub fn otpauth_uri(issuer: &str, label: &str, secret: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}",
        percent_encode(issuer),
        percent_encode(label),
        secret,
        percent_encode(issuer)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";

    #[test]
    fn meme_code_qu_otplib() {
        // Vecteur produit par otplib.generateSync({ secret, epoch: 1700000000 }).
        assert_eq!(generate(SECRET, 1_700_000_000).unwrap(), "406058");
    }

    #[test]
    fn tolerance_d_un_pas() {
        let code = generate(SECRET, 1_700_000_000).unwrap();
        assert!(verify(SECRET, &code, 1_700_000_000 + 30));
        assert!(verify(SECRET, &code, 1_700_000_000 - 30));
        assert!(!verify(SECRET, &code, 1_700_000_000 + 90));
    }

    #[test]
    fn code_malforme_refuse() {
        assert!(!verify(SECRET, "abcdef", 1_700_000_000));
        assert!(!verify(SECRET, "12345", 1_700_000_000));
        assert!(!verify("pas-du-base32!", "406058", 1_700_000_000));
    }

    #[test]
    fn secret_genere_32_caracteres() {
        let secret = generate_secret();
        assert_eq!(secret.len(), 32);
        assert!(decode_secret(&secret).is_some());
    }

    #[test]
    fn uri_comme_otplib() {
        assert_eq!(
            otpauth_uri("Hebdomiam", "a@b.c", "OPYAOYGVFSC5UE3HMZKSMZHE333OGJX2"),
            "otpauth://totp/Hebdomiam:a%40b.c?secret=OPYAOYGVFSC5UE3HMZKSMZHE333OGJX2&issuer=Hebdomiam"
        );
    }
}
