//! Extraction schema.org/Recipe depuis le HTML d'une page (JSON-LD) — pur, testé.
//! Gratuit et instantané : toujours essayé avant le repli IA (PRD F3, D1).

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::{Captures, Regex};
use serde_json::{Map, Value};

use super::ingredient_line::parse_ingredient_line;
use super::{DraftIngredient, ImportDraft, truncate};

static DURATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^-?P(?:(\d+(?:\.\d+)?)D)?T?(?:(\d+(?:\.\d+)?)H)?(?:(\d+(?:\.\d+)?)M)?(?:(\d+(?:\.\d+)?)S)?$")
        .unwrap()
});
static LD_JSON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<script[^>]*type\s*=\s*["']application/ld\+json["'][^>]*>([\s\S]*?)</script>"#).unwrap()
});
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static DECIMAL_ENTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&#(\d+);").unwrap());
static HEX_ENTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)&#x([0-9a-f]+);").unwrap());
static NAMED_ENTITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)&([a-z]+[0-9]*);").unwrap());
static FIRST_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+").unwrap());

/// « PT1H30M » → 90 minutes. None si illisible.
pub fn parse_iso_duration(value: &Value) -> Option<i32> {
    let text = value.as_str()?;
    let caps = DURATION.captures(text)?;
    if (1..=4).all(|i| caps.get(i).is_none()) {
        return None;
    }
    let part = |i: usize| caps.get(i).and_then(|m| m.as_str().parse::<f64>().ok()).unwrap_or(0.0);
    let minutes = part(1) * 24.0 * 60.0 + part(2) * 60.0 + part(3) + part(4) / 60.0;
    let rounded = minutes.round();
    // Borné comme la saisie (24 h) : « P99999999999D » ne produit pas i32::MAX.
    (rounded > 0.0).then_some(rounded.min(1440.0) as i32)
}

fn named_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        "eacute" => "é",
        "egrave" => "è",
        "ecirc" => "ê",
        "euml" => "ë",
        "agrave" => "à",
        "acirc" => "â",
        "icirc" => "î",
        "iuml" => "ï",
        "ocirc" => "ô",
        "ucirc" => "û",
        "ugrave" => "ù",
        "uuml" => "ü",
        "ccedil" => "ç",
        "oelig" => "œ",
        "rsquo" => "’",
        "lsquo" => "‘",
        "laquo" => "«",
        "raquo" => "»",
        "deg" => "°",
        "frac12" => "½",
        "frac14" => "¼",
        "frac34" => "¾",
        _ => return None,
    })
}

fn decode_entities(text: &str) -> String {
    let code_point = |caps: &Captures, radix: u32| {
        u32::from_str_radix(&caps[1], radix)
            .ok()
            .and_then(char::from_u32)
            .map_or_else(|| caps[0].to_string(), String::from)
    };
    let text = DECIMAL_ENTITY.replace_all(text, |caps: &Captures| code_point(caps, 10));
    let text = HEX_ENTITY.replace_all(&text, |caps: &Captures| code_point(caps, 16));
    NAMED_ENTITY
        .replace_all(&text, |caps: &Captures| {
            named_entity(&caps[1].to_lowercase()).map_or_else(|| caps[0].to_string(), str::to_string)
        })
        .into_owned()
}

fn as_text(value: &Value) -> Option<String> {
    let text = value.as_str()?;
    let without_tags = TAG.replace_all(text, " ");
    let cleaned = SPACES
        .replace_all(&decode_entities(&without_tags), " ")
        .trim()
        .to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Valeur « vraie » au sens JavaScript.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn is_recipe_node(node: &Value) -> bool {
    let Some(kind) = node.get("@type") else { return false };
    match kind {
        Value::String(s) => s.eq_ignore_ascii_case("recipe"),
        Value::Array(types) => types
            .iter()
            .any(|t| t.as_str().is_some_and(|s| s.eq_ignore_ascii_case("recipe"))),
        _ => false,
    }
}

/// Cherche un nœud Recipe dans un document JSON-LD (gère @graph et les tableaux).
fn find_recipe_node(data: &Value) -> Option<&Map<String, Value>> {
    if data.is_object() && is_recipe_node(data) {
        return data.as_object();
    }
    match data {
        Value::Array(items) => items.iter().find_map(find_recipe_node),
        Value::Object(map) => map.get("@graph").filter(|g| truthy(g)).and_then(find_recipe_node),
        _ => None,
    }
}

fn extract_servings(recipe_yield: Option<&Value>) -> Option<i32> {
    let candidates: Vec<&Value> = match recipe_yield {
        Some(Value::Array(items)) => items.iter().collect(),
        Some(value) => vec![value],
        None => vec![],
    };
    candidates.into_iter().find_map(|candidate| match candidate {
        Value::Number(n) => n
            .as_f64()
            .filter(|f| f.is_finite())
            .map(|f| (f.round() as i64).clamp(1, 50) as i32),
        Value::String(s) => FIRST_NUMBER
            .find(s)
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .map(|n| n.clamp(1, 50) as i32),
        _ => None,
    })
}

fn extract_steps(instructions: Option<&Value>) -> Vec<String> {
    fn walk(node: &Value, steps: &mut Vec<String>) {
        match node {
            Value::String(_) => steps.extend(as_text(node)),
            Value::Array(items) => items.iter().for_each(|item| walk(item, steps)),
            Value::Object(obj) => {
                // HowToSection → itemListElement ; HowToStep → text
                if let Some(list) = obj.get("itemListElement").filter(|v| truthy(v)) {
                    walk(list, steps);
                    return;
                }
                let text = obj
                    .get("text")
                    .and_then(as_text)
                    .or_else(|| obj.get("name").and_then(as_text));
                steps.extend(text);
            }
            _ => {}
        }
    }
    let mut steps = Vec::new();
    if let Some(node) = instructions {
        walk(node, &mut steps);
    }
    steps.into_iter().map(|s| truncate(&s, 2000)).take(50).collect()
}

fn extract_image(image: Option<&Value>) -> Option<String> {
    // https seulement : cohérent avec la validation de imageUrl (mixed content).
    match image? {
        Value::String(url) => url.starts_with("https://").then(|| truncate(url, 1000)),
        Value::Array(items) => items.iter().find_map(|item| extract_image(Some(item))),
        Value::Object(obj) => extract_image(obj.get("url")),
        _ => None,
    }
}

fn extract_tags(node: &Map<String, Value>) -> Vec<String> {
    let mut raw: Vec<String> = Vec::new();
    for key in ["keywords", "recipeCategory", "recipeCuisine"] {
        match node.get(key) {
            Some(Value::String(s)) => raw.extend(s.split(',').map(str::to_string)),
            Some(Value::Array(items)) => raw.extend(items.iter().map(|v| v.as_str().unwrap_or("").to_string())),
            _ => {}
        }
    }
    let mut seen = HashSet::new();
    raw.into_iter()
        .map(|t| decode_entities(&t).trim().to_lowercase())
        .filter(|t| (2..=30).contains(&t.chars().count()))
        .filter(|t| seen.insert(t.clone()))
        .take(8)
        .collect()
}

/// Extrait un brouillon de recette du HTML, ou None si aucun JSON-LD Recipe exploitable.
pub fn extract_recipe_from_html(html: &str, source_url: &str) -> Option<ImportDraft> {
    let parsed_blocks: Vec<Value> = LD_JSON
        .captures_iter(html)
        // JSON-LD invalide sur la page : on tente le bloc suivant.
        .filter_map(|caps| serde_json::from_str(caps[1].trim()).ok())
        .collect();
    let recipe = parsed_blocks.iter().find_map(find_recipe_node)?;

    let title = recipe.get("name").and_then(as_text)?;
    let mut warnings = Vec::new();

    let raw_ingredients = match (recipe.get("recipeIngredient"), recipe.get("ingredients")) {
        (Some(Value::Array(items)), _) => items.as_slice(),
        (_, Some(Value::Array(items))) => items.as_slice(),
        _ => &[],
    };
    let ingredients: Vec<DraftIngredient> = raw_ingredients
        .iter()
        .filter_map(as_text)
        .take(100)
        .map(|line| {
            let parsed = parse_ingredient_line(&line);
            if !parsed.parsed {
                warnings.push(format!("Quantité à vérifier : « {line} »"));
            }
            DraftIngredient {
                quantity: parsed.quantity,
                unit: parsed.unit,
                name: truncate(&parsed.name, 200),
                aisle: parsed.aisle,
            }
        })
        .collect();

    let steps = extract_steps(recipe.get("recipeInstructions"));
    if ingredients.is_empty() || steps.is_empty() {
        return None;
    }

    let base_servings = extract_servings(recipe.get("recipeYield"));
    if base_servings.is_none() {
        warnings.push("Nombre de portions non détecté (défaut : 4).".to_string());
    }

    let null = Value::Null;
    let prep_minutes = parse_iso_duration(recipe.get("prepTime").unwrap_or(&null));
    let cook_minutes = parse_iso_duration(recipe.get("cookTime").unwrap_or(&null)).or_else(|| {
        parse_iso_duration(recipe.get("totalTime").unwrap_or(&null))
            .map(|total| (total - prep_minutes.unwrap_or(0)).max(0))
            .filter(|minutes| *minutes > 0)
    });

    Some(ImportDraft {
        title: truncate(&title, 200),
        description: recipe.get("description").and_then(as_text).map(|d| truncate(&d, 2000)),
        image_url: extract_image(recipe.get("image")),
        base_servings: base_servings.unwrap_or(4),
        prep_minutes,
        cook_minutes,
        freezable: false,
        ingredients,
        steps,
        tags: extract_tags(recipe),
        source_url: Some(source_url.to_string()),
        warnings,
        provider: "schema-org",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::catalog::Unit;
    use serde_json::json;

    const SIMPLE: &str = r#"<html><head>
    <script type="application/ld+json">
    {
      "@context": "https://schema.org",
      "@type": "Recipe",
      "name": "Chili v&eacute;g&#233;tarien",
      "description": "Un classique réconfortant.",
      "image": ["https://exemple.com/chili.jpg"],
      "recipeYield": "6 portions",
      "prepTime": "PT15M",
      "cookTime": "PT45M",
      "recipeIngredient": ["500 g de lentilles vertes", "2 oignons jaunes", "Sel au goût"],
      "recipeInstructions": [
        {"@type": "HowToStep", "text": "Hacher les oignons."},
        {"@type": "HowToStep", "text": "Tout mijoter 45 minutes."}
      ],
      "keywords": "végé, mijoteuse",
      "recipeCuisine": "Tex-Mex"
    }
    </script></head><body></body></html>"#;

    #[test]
    fn durees_iso() {
        assert_eq!(parse_iso_duration(&json!("PT30M")), Some(30));
        assert_eq!(parse_iso_duration(&json!("PT1H30M")), Some(90));
        assert_eq!(parse_iso_duration(&json!("PT2H")), Some(120));
        assert_eq!(parse_iso_duration(&json!("P0DT1H15M")), Some(75));
        assert_eq!(parse_iso_duration(&json!("bientôt")), None);
        assert_eq!(parse_iso_duration(&Value::Null), None);
        assert_eq!(parse_iso_duration(&json!("PT")), None);
        assert_eq!(parse_iso_duration(&json!("P99999999999D")), Some(1440));
    }

    #[test]
    fn brouillon_complet() {
        let draft = extract_recipe_from_html(SIMPLE, "https://exemple.com/chili").unwrap();
        assert_eq!(draft.title, "Chili végétarien");
        assert_eq!(draft.base_servings, 6);
        assert_eq!(draft.prep_minutes, Some(15));
        assert_eq!(draft.cook_minutes, Some(45));
        assert_eq!(draft.image_url.as_deref(), Some("https://exemple.com/chili.jpg"));
        assert_eq!(draft.ingredients.len(), 3);
        assert_eq!(
            (draft.ingredients[0].quantity, draft.ingredients[0].unit),
            (500.0, Some(Unit::G))
        );
        assert_eq!(draft.ingredients[0].name, "lentilles vertes");
        assert_eq!(draft.steps, ["Hacher les oignons.", "Tout mijoter 45 minutes."]);
        assert_eq!(draft.tags, ["végé", "mijoteuse", "tex-mex"]);
        assert_eq!(draft.source_url.as_deref(), Some("https://exemple.com/chili"));
        assert_eq!(draft.provider, "schema-org");
        assert!(draft.warnings.iter().any(|w| w.contains("Sel au goût")));
    }

    #[test]
    fn graph_et_sections() {
        let html = r#"<script type="application/ld+json">
        {"@context":"https://schema.org","@graph":[
          {"@type":"WebPage","name":"Page"},
          {"@type":["Recipe","Thing"],"name":"Soupe orge",
           "recipeYield": 4, "totalTime": "PT1H", "prepTime": "PT20M",
           "recipeIngredient": ["250 g d'orge mondé"],
           "recipeInstructions": [{"@type":"HowToSection","itemListElement":[
             {"@type":"HowToStep","text":"Rincer l'orge."},
             {"@type":"HowToStep","text":"Cuire 40 minutes."}
           ]}]}
        ]}
        </script>"#;
        let draft = extract_recipe_from_html(html, "https://exemple.com/soupe").unwrap();
        assert_eq!(draft.title, "Soupe orge");
        assert_eq!(draft.steps.len(), 2);
        assert_eq!(draft.cook_minutes, Some(40));
    }

    #[test]
    fn rien_d_exploitable() {
        assert!(extract_recipe_from_html("<html><body>Rien ici</body></html>", "https://x.test").is_none());
        let incomplete = r#"<script type="application/ld+json">{"@type":"Recipe","name":"Vide","recipeIngredient":[],"recipeInstructions":[]}</script>"#;
        assert!(extract_recipe_from_html(incomplete, "https://x.test").is_none());
    }

    #[test]
    fn premier_bloc_invalide() {
        let html = format!(r#"<script type="application/ld+json">{{pas du json</script>{SIMPLE}"#);
        assert!(extract_recipe_from_html(&html, "https://exemple.com/chili").is_some());
    }

    #[test]
    fn entite_hors_plage_conservee() {
        assert_eq!(decode_entities("a&#99999999;b"), "a&#99999999;b");
    }
}
