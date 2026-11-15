//! Validation systématique de toute entrée serveur (PRD §8) : corps JSON,
//! paramètres de chemin et de requête. Équivalent des schémas Zod de l'ancien
//! backend : les champs inconnus sont ignorés (jamais transmis à la base), les
//! chaînes sont normalisées (trim, minuscules pour les courriels) puis bornées.
//! Tout écart → 400 `Validation Error`.

use std::sync::LazyLock;

use axum::extract::{FromRequest, FromRequestParts, Path, Query, Request};
use axum::http::header::CONTENT_TYPE;
use axum::http::request::Parts;
use regex::Regex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

use crate::error::ApiError;

pub type Validated<T> = Result<T, ApiError>;

/// Normalise et valide une entrée désérialisée (forme brute → forme métier).
pub trait Validate: Sized {
    type Output;
    fn validate(self) -> Validated<Self::Output>;
}

fn invalid<T>() -> Validated<T> {
    Err(ApiError::validation())
}

/// Corps JSON validé. Exige `Content-Type: application/json` : une requête
/// inter-site de type formulaire ne peut pas l'émettre sans préflight CORS
/// (défense CSRF en profondeur, en plus de SameSite=Lax).
pub struct ValidJson<T: Validate>(pub T::Output);

impl<T: DeserializeOwned + Validate, S: Send + Sync> FromRequest<S> for ValidJson<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let is_json = request
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.to_ascii_lowercase().starts_with("application/json"));
        if !is_json {
            return invalid();
        }
        let bytes = axum::body::Bytes::from_request(request, state)
            .await
            .map_err(|rejection| {
                if rejection.status().as_u16() == 413 {
                    ApiError::new(413, "Payload Too Large")
                } else {
                    ApiError::validation()
                }
            })?;
        let value: T = serde_json::from_slice(&bytes).map_err(|_| ApiError::validation())?;
        Ok(ValidJson(value.validate()?))
    }
}

/// Paramètres de chemin validés (UUID mal formé → 400, comme Zod).
pub struct ValidPath<T>(pub T);

impl<T: DeserializeOwned + Send, S: Send + Sync> FromRequestParts<S> for ValidPath<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(value)| ValidPath(value))
            .map_err(|_| ApiError::validation())
    }
}

/// Paramètres de requête validés.
pub struct ValidQuery<T: Validate>(pub T::Output);

impl<T: DeserializeOwned + Validate, S: Send + Sync> FromRequestParts<S> for ValidQuery<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::validation())?;
        Ok(ValidQuery(value.validate()?))
    }
}

// ── Aides de validation ─────────────────────────────────────────────────────

fn len(text: &str) -> usize {
    text.chars().count()
}

/// Chaîne obligatoire : trim puis longueur dans [min, max].
pub fn text(value: String, min: usize, max: usize) -> Validated<String> {
    let trimmed = value.trim().to_string();
    if (min..=max).contains(&len(&trimmed)) {
        Ok(trimmed)
    } else {
        invalid()
    }
}

/// Chaîne sans trim (mots de passe : les espaces comptent).
pub fn raw_text(value: String, min: usize, max: usize) -> Validated<String> {
    if (min..=max).contains(&len(&value)) {
        Ok(value)
    } else {
        invalid()
    }
}

pub fn opt_text(value: Option<String>, min: usize, max: usize) -> Validated<Option<String>> {
    value.map(|v| text(v, min, max)).transpose()
}

static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9_'+\-.]*[A-Za-z0-9_+\-]@(?:[A-Za-z0-9][A-Za-z0-9\-]*\.)+[A-Za-z]{2,}$").unwrap()
});

/// Courriel : trim, minuscules, ≤ 255, forme valide.
pub fn email(value: String) -> Validated<String> {
    let normalized = value.trim().to_lowercase();
    let local = normalized.split('@').next().unwrap_or("");
    let valid =
        len(&normalized) <= 255 && EMAIL.is_match(&normalized) && !local.starts_with('.') && !normalized.contains("..");
    if valid { Ok(normalized) } else { invalid() }
}

/// Schémas acceptés pour une URL fournie par l'utilisateur. Jamais de schéma
/// libre : `javascript:` ou `data:` finiraient dans un `href` du front.
pub const HTTPS_ONLY: &[&str] = &["https"];
pub const WEB: &[&str] = &["http", "https"];

/// URL absolue (WHATWG) avec hôte, de longueur ≤ max, au schéma autorisé.
pub fn url(value: String, max: usize, schemes: &[&str]) -> Validated<String> {
    if len(&value) > max {
        return invalid();
    }
    match url::Url::parse(&value) {
        Ok(parsed) if schemes.contains(&parsed.scheme()) && parsed.host().is_some() => Ok(value),
        _ => invalid(),
    }
}

pub fn int_range(value: i64, min: i64, max: i64) -> Validated<i32> {
    if (min..=max).contains(&value) {
        Ok(value as i32)
    } else {
        invalid()
    }
}

/// Nombre strictement positif borné (quantités).
pub fn positive(value: f64, max: f64) -> Validated<f64> {
    if value.is_finite() && value > 0.0 && value <= max {
        Ok(value)
    } else {
        invalid()
    }
}

/// Paramètre de requête : chaîne vide ⇒ absent.
pub fn query_text(value: Option<String>, min: usize, max: usize) -> Validated<Option<String>> {
    match value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        Some(v) => text(v, min, max).map(Some),
        None => Ok(None),
    }
}

/// Champ tri-état d'un PATCH : absent (None), `null` (Some(None)) ou valeur.
pub fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Entier JSON strict : refuse les décimaux (2.5) comme `z.number().int()`.
pub fn strict_int<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    let value = serde_json::Number::deserialize(deserializer)?;
    if let Some(int) = value.as_i64() {
        return Ok(int);
    }
    match value.as_f64() {
        Some(float) if float.fract() == 0.0 && float.abs() < 1e15 => Ok(float as i64),
        _ => Err(serde::de::Error::custom("entier attendu")),
    }
}

pub fn opt_strict_int<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
    #[derive(Deserialize)]
    struct Wrapper(#[serde(deserialize_with = "strict_int")] i64);
    Option::<Wrapper>::deserialize(deserializer).map(|w| w.map(|Wrapper(v)| v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn courriel_normalise() {
        assert_eq!(email("  Fab@Example.COM ".into()).unwrap(), "fab@example.com");
        assert!(email("pas-un-courriel".into()).is_err());
        assert!(email("a@b".into()).is_err());
        assert!(email(".a@b.co".into()).is_err());
        assert!(email("a..b@c.co".into()).is_err());
        assert!(email(format!("{}@example.com", "a".repeat(250))).is_err());
    }

    #[test]
    fn texte_trim_et_bornes() {
        assert_eq!(text("  Fabrice ".into(), 1, 100).unwrap(), "Fabrice");
        assert!(text("   ".into(), 1, 100).is_err());
        assert!(text("x".repeat(101), 1, 100).is_err());
    }

    #[test]
    fn urls() {
        assert!(url("https://exemple.com/img.jpg".into(), 1000, HTTPS_ONLY).is_ok());
        assert!(url("http://exemple.com/img.jpg".into(), 1000, HTTPS_ONLY).is_err());
        assert!(url("pas une url".into(), 1000, WEB).is_err());
        assert!(url("http://exemple.com".into(), 500, WEB).is_ok());
        for dangerous in [
            "javascript:alert(1)",
            "data:text/html,<script>",
            "file:///etc/passwd",
            "mailto:a@b.c",
        ] {
            assert!(url(dangerous.into(), 500, WEB).is_err(), "{dangerous}");
        }
    }

    #[test]
    fn entier_strict() {
        #[derive(Deserialize)]
        struct T {
            #[serde(deserialize_with = "strict_int")]
            n: i64,
        }
        assert_eq!(serde_json::from_str::<T>(r#"{"n":4}"#).unwrap().n, 4);
        assert_eq!(serde_json::from_str::<T>(r#"{"n":4.0}"#).unwrap().n, 4);
        assert!(serde_json::from_str::<T>(r#"{"n":4.5}"#).is_err());
        assert!(serde_json::from_str::<T>(r#"{"n":"4"}"#).is_err());
    }

    #[test]
    fn champ_tri_etat() {
        #[derive(Deserialize)]
        struct Patch {
            #[serde(default, deserialize_with = "double_option")]
            q: Option<Option<f64>>,
        }
        assert_eq!(serde_json::from_str::<Patch>("{}").unwrap().q, None);
        assert_eq!(serde_json::from_str::<Patch>(r#"{"q":null}"#).unwrap().q, Some(None));
        assert_eq!(serde_json::from_str::<Patch>(r#"{"q":2}"#).unwrap().q, Some(Some(2.0)));
    }
}
