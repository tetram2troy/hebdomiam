//! Import de recettes par URL (schema.org puis repli IA) et par photo (IA).

use axum::Json;
use axum::extract::{Multipart, State};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::json;

use crate::client_ip::ClientIp;
use crate::error::{ApiError, ApiResult};
use crate::import::claude::{ExtractionFailure, ExtractionInput};
use crate::import::image::{reencode, sniff};
use crate::import::schema_org::extract_recipe_from_html;
use crate::logger::{log_event, log_warn};
use crate::session::CurrentUser;
use crate::state::AppState;
use crate::validation::{ValidJson, Validate, Validated, WEB, url};

const MAX_FILES: usize = 3;
const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize)]
pub struct UrlBody {
    url: String,
}

impl Validate for UrlBody {
    type Output = String;
    fn validate(self) -> Validated<String> {
        url(self.url, 2000, WEB)
    }
}

fn extraction_error(failure: ExtractionFailure) -> ApiError {
    match failure {
        ExtractionFailure::NotFound => ApiError::new(422, "errors.importFailed"),
        ExtractionFailure::ProviderError => ApiError::new(502, "errors.importProviderError"),
    }
}

pub async fn from_url(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ClientIp(ip): ClientIp,
    ValidJson(raw_url): ValidJson<UrlBody>,
) -> ApiResult<Json<serde_json::Value>> {
    // Quota par utilisateur : l'import déclenche des fetchs sortants et, en
    // repli, des appels IA facturés (PRD D9).
    state.rate_limit(&format!("import:url:{}", user.id), &ip, 10, 60_000)?;

    // Le domaine suffit à l'audit — l'URL complète peut être longue et parlante.
    let host = url::Url::parse(&raw_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();

    let page = state.fetcher.fetch(&raw_url).await.map_err(|failure| {
        log_warn(
            "import.url_unreachable",
            json!({ "userId": user.id, "host": host, "reason": failure.as_str() }),
        );
        ApiError::new(422, "errors.importUnreachable")
    })?;

    // 1. Gratuit et instantané : schema.org/Recipe (JSON-LD).
    if let Some(draft) = extract_recipe_from_html(&page.html, &raw_url) {
        log_event(
            "import.url",
            json!({ "userId": user.id, "host": host, "provider": "schema-org" }),
        );
        return Ok(Json(json!({ "draft": draft })));
    }

    // 2. Repli IA — seulement si configuré (D1).
    let Some(claude) = &state.claude else {
        log_warn(
            "import.url_failed",
            json!({ "userId": user.id, "host": host, "reason": "noRecipeNoProvider" }),
        );
        return Err(ApiError::new(422, "errors.importFailed"));
    };
    let draft = claude
        .extract(ExtractionInput {
            html: Some(page.html),
            images_base64: Vec::new(),
            source_url: Some(raw_url),
        })
        .await
        .map_err(|failure| {
            log_warn(
                "import.url_failed",
                json!({ "userId": user.id, "host": host, "reason": failure.as_str() }),
            );
            extraction_error(failure)
        })?;
    // Repli facturé (PRD D9) : suivre la fréquence d'usage du fournisseur.
    log_event(
        "import.url",
        json!({ "userId": user.id, "host": host, "provider": "claude" }),
    );
    Ok(Json(json!({ "draft": draft })))
}

pub async fn from_photo(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    ClientIp(ip): ClientIp,
    mut multipart: Multipart,
) -> ApiResult<Json<serde_json::Value>> {
    state.rate_limit(&format!("import:photo:{}", user.id), &ip, 5, 60_000)?;
    let Some(claude) = state.claude.clone() else {
        return Err(ApiError::new(503, "errors.importUnavailable"));
    };

    let mut photos: Vec<Vec<u8>> = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::new(400, "errors.invalidImage"))?
    {
        if field.name() != Some("photos") {
            continue;
        }
        let data = field
            .bytes()
            .await
            .map_err(|_| ApiError::new(413, "errors.fileTooLarge"))?;
        if data.is_empty() {
            continue;
        }
        if photos.len() == MAX_FILES {
            return Err(ApiError::new(400, "errors.invalidImage"));
        }
        photos.push(data.to_vec());
    }
    if photos.is_empty() {
        return Err(ApiError::new(400, "errors.invalidImage"));
    }

    let mut images_base64 = Vec::with_capacity(photos.len());
    for photo in photos {
        if photo.len() > MAX_FILE_BYTES {
            return Err(ApiError::new(413, "errors.fileTooLarge"));
        }
        if sniff(&photo).is_none() {
            return Err(ApiError::new(415, "errors.invalidImage"));
        }
        // Ré-encodage systématique : supprime EXIF/métadonnées et neutralise les
        // charges utiles cachées ; borne aussi la taille envoyée au fournisseur.
        let reencoded = tokio::task::spawn_blocking(move || reencode(&photo))
            .await
            .map_err(|_| ApiError::internal())?
            .map_err(|_| ApiError::new(415, "errors.invalidImage"))?;
        images_base64.push(STANDARD.encode(reencoded));
    }

    let count = images_base64.len();
    let draft = claude
        .extract(ExtractionInput {
            html: None,
            images_base64,
            source_url: None,
        })
        .await
        .map_err(|failure| {
            log_warn(
                "import.photo_failed",
                json!({ "userId": user.id, "photos": count, "reason": failure.as_str() }),
            );
            extraction_error(failure)
        })?;
    // Chaque appel vision est facturé (PRD D9) : suivre le volume par utilisateur.
    log_event("import.photo", json!({ "userId": user.id, "photos": count }));
    Ok(Json(json!({ "draft": draft })))
}
