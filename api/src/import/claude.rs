//! Fournisseur d'extraction « claude » (PRD §7.4) : repli pour les URLs sans
//! schema.org et voie principale pour les photos. Appel HTTP direct à l'API
//! Messages (pas de SDK Rust officiel). La clé API ne quitte jamais le serveur ;
//! la sortie est traitée comme non fiable : sorties structurées (JSON Schema)
//! côté API, puis désérialisation stricte et bornage côté serveur.

use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};

use super::ingredient_line::guess_aisle;
use super::{DraftIngredient, ImportDraft, truncate};
use crate::domain::catalog::{Aisle, Unit};
use crate::domain::units::round2;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
/// Repli serveur en cas de refus du modèle (désactivable : modèles sans support).
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

const SYSTEM_PROMPT: &str = "Tu extrais des recettes de cuisine vers un format structuré, sans jamais rien inventer.
Règles :
- Conserve la langue d'origine du texte (français ou anglais), sans traduire.
- Convertis les quantités vers les unités permises quand c'est possible (ex. 25 cl → 250 ml) ; sinon unit = null et la quantité reste le nombre d'unités.
- Si une quantité est illisible ou absente, mets null et signale-le dans uncertainties.
- aisle = le rayon d'épicerie le plus plausible pour l'ingrédient.
- Si le contenu ne contient pas de recette exploitable, found = false et laisse le reste vide.
- N'ajoute AUCUN texte hors du format demandé.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionFailure {
    NotFound,
    ProviderError,
}

impl ExtractionFailure {
    pub fn as_str(self) -> &'static str {
        match self {
            ExtractionFailure::NotFound => "notFound",
            ExtractionFailure::ProviderError => "providerError",
        }
    }
}

pub struct ExtractionInput {
    pub html: Option<String>,
    /// Images JPEG ré-encodées, en base64.
    pub images_base64: Vec<String>,
    pub source_url: Option<String>,
}

pub struct ClaudeClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
}

static SCRIPT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<script\b[^>]*>[\s\S]*?</script[^>]*>").unwrap());
static STYLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<style\b[^>]*>[\s\S]*?</style[^>]*>").unwrap());
static CHROME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<(?:nav|footer|header|aside|form|svg)\b[^>]*>[\s\S]*?</(?:nav|footer|header|aside|form|svg)[^>]*>")
        .unwrap()
});
static BLOCK_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>|</(?:p|div|li|h[1-6]|tr)>").unwrap());
static ANY_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static BLANKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+").unwrap());
static EMPTY_LINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n+").unwrap());

/// Réduit le HTML en texte brut pour limiter les jetons envoyés. Pas une
/// frontière de sécurité (le résultat part vers l'extraction, jamais vers un
/// rendu HTML) — mais les balises fermantes tolèrent les variantes du genre
/// « </script > » pour ne pas laisser passer leur contenu.
pub fn html_to_text(html: &str) -> String {
    let text = SCRIPT.replace_all(html, " ");
    let text = STYLE.replace_all(&text, " ");
    let text = CHROME.replace_all(&text, " ");
    let text = BLOCK_END.replace_all(&text, "\n");
    let text = ANY_TAG.replace_all(&text, " ");
    let text = BLANKS.replace_all(&text, " ");
    let text = EMPTY_LINES.replace_all(&text, "\n");
    truncate(text.trim(), 60_000)
}

fn nullable(schema: Value) -> Value {
    json!({ "anyOf": [schema, { "type": "null" }] })
}

/// Schéma imposé au modèle via les sorties structurées.
fn extraction_schema() -> Value {
    let units: Vec<&str> = Unit::ALL.iter().map(|u| u.as_str()).collect();
    let aisles: Vec<&str> = Aisle::ALL.iter().map(|a| a.as_str()).collect();
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["found", "title", "description", "baseServings", "prepMinutes", "cookMinutes", "freezable", "ingredients", "steps", "tags", "uncertainties"],
        "properties": {
            "found": { "type": "boolean", "description": "false si le contenu ne contient aucune recette" },
            "title": nullable(json!({ "type": "string" })),
            "description": nullable(json!({ "type": "string" })),
            "baseServings": nullable(json!({ "type": "integer", "description": "nombre de portions de la recette" })),
            "prepMinutes": nullable(json!({ "type": "integer" })),
            "cookMinutes": nullable(json!({ "type": "integer" })),
            "freezable": nullable(json!({ "type": "boolean", "description": "true si la recette se congèle bien" })),
            "ingredients": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["quantity", "unit", "name", "aisle"],
                    "properties": {
                        "quantity": nullable(json!({ "type": "number" })),
                        "unit": nullable(json!({ "type": "string", "enum": units })),
                        "name": { "type": "string" },
                        "aisle": nullable(json!({ "type": "string", "enum": aisles, "description": "rayon d'épicerie" })),
                    }
                }
            },
            "steps": { "type": "array", "items": { "type": "string" } },
            "tags": { "type": "array", "items": { "type": "string" }, "description": "3 à 6 tags courts, en minuscules" },
            "uncertainties": { "type": "array", "items": { "type": "string" }, "description": "champs douteux à faire vérifier (illisible, ambigu)" },
        }
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Extracted {
    found: bool,
    title: Option<String>,
    description: Option<String>,
    base_servings: Option<i64>,
    prep_minutes: Option<i64>,
    cook_minutes: Option<i64>,
    freezable: Option<bool>,
    ingredients: Vec<ExtractedIngredient>,
    steps: Vec<String>,
    tags: Vec<String>,
    uncertainties: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ExtractedIngredient {
    quantity: Option<f64>,
    unit: Option<Unit>,
    name: String,
    aisle: Option<Aisle>,
}

/// Le modèle accepte-t-il le paramètre `fallbacks` (repli serveur sur refus) ?
fn supports_fallbacks(model: &str) -> bool {
    [
        "claude-opus-5-5",
        "claude-opus-5",
        "claude-fable-5-1",
        "claude-fable-5-2",
        "claude-sonnet-5-5",
    ]
    .contains(&model)
}

impl ClaudeClient {
    pub fn new(api_key: String, model: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .expect("client HTTP");
        ClaudeClient { http, api_key, model }
    }

    fn request_body(&self, input: &ExtractionInput) -> Value {
        let mut content: Vec<Value> = input
            .images_base64
            .iter()
            .map(|data| json!({ "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": data } }))
            .collect();
        let text = match &input.html {
            Some(html) => format!("Extrais la recette de cette page :\n\n{}", html_to_text(html)),
            None => "Extrais la recette de ces photos (livre, fiche manuscrite ou capture d'écran).".to_string(),
        };
        content.push(json!({ "type": "text", "text": text }));

        let mut body = json!({
            "model": self.model,
            "max_tokens": 8192,
            "thinking": { "type": "adaptive" },
            "system": SYSTEM_PROMPT,
            "output_config": { "format": { "type": "json_schema", "schema": extraction_schema() } },
            "messages": [{ "role": "user", "content": content }],
        });
        if supports_fallbacks(&self.model) {
            body["fallbacks"] = json!("default");
        }
        body
    }

    pub async fn extract(&self, input: ExtractionInput) -> Result<ImportDraft, ExtractionFailure> {
        let body = self.request_body(&input);
        // Même politique que les SDK officiels : 2 réessais (backoff 1 s puis
        // 2 s) sur 408/409/429/5xx (dont 529 « overloaded ») et coupure réseau.
        // Pas de réessai sur timeout : la requête a pu être facturée.
        let mut attempt = 0;
        let response = loop {
            let mut request = self
                .http
                .post(API_URL)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", API_VERSION)
                .json(&body);
            if supports_fallbacks(&self.model) {
                request = request.header("anthropic-beta", FALLBACK_BETA);
            }
            let retryable = match request.send().await {
                Ok(response) if response.status().is_success() => break response,
                Ok(response) => {
                    let status = response.status().as_u16();
                    crate::logger::log_warn(
                        "import.provider_http_error",
                        json!({ "provider": "claude", "status": status, "attempt": attempt + 1 }),
                    );
                    is_retryable_status(status)
                }
                Err(error) => !error.is_timeout(),
            };
            if !retryable || attempt == MAX_RETRIES {
                return Err(ExtractionFailure::ProviderError);
            }
            tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
            attempt += 1;
        };
        let message: Value = response.json().await.map_err(|_| ExtractionFailure::ProviderError)?;
        parse_response(&message, input.source_url)
    }
}

const MAX_RETRIES: u32 = 2;

fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 429) || status >= 500
}

/// Interprète la réponse de l'API (fonction pure, testée).
pub fn parse_response(message: &Value, source_url: Option<String>) -> Result<ImportDraft, ExtractionFailure> {
    if message["stop_reason"] == "refusal" {
        return Err(ExtractionFailure::ProviderError);
    }
    let text: String = message["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    let parsed: Extracted = serde_json::from_str(&text).map_err(|_| ExtractionFailure::ProviderError)?;
    to_draft(parsed, source_url)
}

fn to_draft(parsed: Extracted, source_url: Option<String>) -> Result<ImportDraft, ExtractionFailure> {
    let title = parsed.title.filter(|t| !t.trim().is_empty());
    let Some(title) = title.filter(|_| parsed.found && !parsed.ingredients.is_empty() && !parsed.steps.is_empty())
    else {
        return Err(ExtractionFailure::NotFound);
    };

    let mut warnings = parsed.uncertainties;
    if parsed.base_servings.is_none() {
        warnings.push("Nombre de portions non détecté (défaut : 4).".to_string());
    }
    let clamp_minutes = |m: Option<i64>| m.map(|v| v.clamp(0, 1440) as i32);

    let ingredients = parsed
        .ingredients
        .into_iter()
        .take(100)
        .map(|ingredient| {
            if ingredient.quantity.is_none() {
                warnings.push(format!("Quantité à vérifier : « {} »", ingredient.name));
            }
            DraftIngredient {
                quantity: match ingredient.quantity {
                    Some(q) if q > 0.0 && q.is_finite() => round2(q.min(99_999.0)),
                    _ => 1.0,
                },
                unit: ingredient.unit,
                aisle: ingredient.aisle.unwrap_or_else(|| guess_aisle(&ingredient.name)),
                name: truncate(&ingredient.name, 200),
            }
        })
        .collect();

    let mut seen = HashSet::new();
    let tags = parsed
        .tags
        .iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| (2..=30).contains(&t.chars().count()))
        .filter(|t| seen.insert(t.clone()))
        .take(8)
        .collect();

    Ok(ImportDraft {
        title: truncate(&title, 200),
        description: parsed.description.map(|d| truncate(&d, 2000)),
        image_url: None,
        base_servings: parsed.base_servings.unwrap_or(4).clamp(1, 50) as i32,
        prep_minutes: clamp_minutes(parsed.prep_minutes),
        cook_minutes: clamp_minutes(parsed.cook_minutes),
        freezable: parsed.freezable.unwrap_or(false),
        ingredients,
        steps: parsed.steps.into_iter().take(50).map(|s| truncate(&s, 2000)).collect(),
        tags,
        source_url,
        warnings,
        provider: "claude",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(payload: Value) -> Value {
        json!({
            "stop_reason": "end_turn",
            "content": [
                { "type": "thinking", "thinking": "" },
                { "type": "text", "text": payload.to_string() }
            ]
        })
    }

    fn valid() -> Value {
        json!({
            "found": true, "title": "Pain aux bananes", "description": null, "baseServings": null,
            "prepMinutes": 15, "cookMinutes": 60, "freezable": true,
            "ingredients": [
                { "quantity": 3, "unit": "unit", "name": "bananes mûres", "aisle": "produce" },
                { "quantity": null, "unit": null, "name": "sel", "aisle": null }
            ],
            "steps": ["Écraser les bananes.", "Cuire 1 h."],
            "tags": ["Dessert", "dessert", "x", "  végé "],
            "uncertainties": ["Température du four illisible"]
        })
    }

    #[test]
    fn brouillon_depuis_sortie_structuree() {
        let draft = parse_response(&message(valid()), Some("https://x.test".into())).unwrap();
        assert_eq!(draft.title, "Pain aux bananes");
        assert_eq!(draft.base_servings, 4);
        assert_eq!(draft.ingredients[1].quantity, 1.0);
        assert_eq!(draft.ingredients[1].aisle, Aisle::Spices);
        assert_eq!(draft.tags, ["dessert", "végé"]);
        assert_eq!(draft.provider, "claude");
        assert!(draft.warnings.iter().any(|w| w.contains("Température")));
        assert!(draft.warnings.iter().any(|w| w.contains("« sel »")));
        assert!(draft.warnings.iter().any(|w| w.contains("portions")));
    }

    #[test]
    fn aucune_recette() {
        let mut payload = valid();
        payload["found"] = json!(false);
        assert_eq!(
            parse_response(&message(payload), None).unwrap_err(),
            ExtractionFailure::NotFound
        );
    }

    #[test]
    fn refus_et_sortie_invalide() {
        let refusal = json!({ "stop_reason": "refusal", "content": [] });
        assert_eq!(
            parse_response(&refusal, None).unwrap_err(),
            ExtractionFailure::ProviderError
        );
        let garbage = json!({ "stop_reason": "end_turn", "content": [{ "type": "text", "text": "pas du json" }] });
        assert_eq!(
            parse_response(&garbage, None).unwrap_err(),
            ExtractionFailure::ProviderError
        );
    }

    #[test]
    fn statuts_reessayables() {
        for status in [408, 409, 429, 500, 529] {
            assert!(is_retryable_status(status), "{status}");
        }
        for status in [400, 401, 403, 404, 413] {
            assert!(!is_retryable_status(status), "{status}");
        }
    }

    #[test]
    fn schema_strict() {
        let schema = extraction_schema();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["ingredients"]["items"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn html_reduit_en_texte() {
        let html = r#"<html><head><style>.x{color:red}</style></head><body>
          <nav class="menu"><a href="/">Accueil</a></nav>
          <h1>Chili végé</h1><p>Un classique réconfortant.</p>
          <script>console.log('pub')</script>
          <footer>© Site</footer></body></html>"#;
        let text = html_to_text(html);
        assert!(text.contains("Chili végé"));
        assert!(text.contains("classique réconfortant"));
        for absent in ["console.log", "color:red", "Accueil"] {
            assert!(!text.contains(absent), "{absent}");
        }
    }

    #[test]
    fn fermetures_farfelues() {
        let html = r#"<p>Avant</p><script>malicieux()</script ><p>Milieu</p>
          <script type="text/javascript">autre()</script foo="bar"><p>Après</p>"#;
        let text = html_to_text(html);
        assert!(!text.contains("malicieux"));
        assert!(!text.contains("autre()"));
        for present in ["Avant", "Milieu", "Après"] {
            assert!(text.contains(present));
        }
    }

    #[test]
    fn sauts_de_ligne() {
        let lines: Vec<String> = html_to_text("<p>Un</p><p>Deux</p><br>Trois")
            .split('\n')
            .map(|l| l.trim().to_string())
            .collect();
        assert_eq!(lines, ["Un", "Deux", "Trois"]);
    }
}
