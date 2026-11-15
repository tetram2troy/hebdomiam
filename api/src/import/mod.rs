//! Import de recettes (PRD F3/F4). L'extraction n'écrit jamais en base : elle
//! produit un brouillon que l'utilisateur relit dans le formulaire de recette.

pub mod claude;
pub mod image;
pub mod ingredient_line;
pub mod safe_fetch;
pub mod schema_org;

use serde::{Deserialize, Serialize};

use crate::domain::catalog::{Aisle, Unit};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftIngredient {
    pub quantity: f64,
    pub unit: Option<Unit>,
    pub name: String,
    pub aisle: Aisle,
}

/// Brouillon d'extraction (URL ou photo).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportDraft {
    pub title: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub base_servings: i32,
    pub prep_minutes: Option<i32>,
    pub cook_minutes: Option<i32>,
    pub freezable: bool,
    pub ingredients: Vec<DraftIngredient>,
    pub steps: Vec<String>,
    pub tags: Vec<String>,
    pub source_url: Option<String>,
    /// Points à vérifier, montrés dans le bandeau de relecture.
    pub warnings: Vec<String>,
    /// Fournisseur qui a produit le brouillon : `schema-org` ou `claude`.
    pub provider: &'static str,
}

/// Tronque à `max` caractères (équivalent de `String.prototype.slice(0, max)`).
pub fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
