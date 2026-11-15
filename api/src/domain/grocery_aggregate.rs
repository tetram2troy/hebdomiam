//! Agrégation de la liste d'épicerie (PRD F6) — fonction pure.
//! Deux ingrédients fusionnent si même nom normalisé ET même dimension d'unité ;
//! sinon les lignes restent séparées (« 2 tomates » + « 200 g de tomates »).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::catalog::{Aisle, Unit};
use super::units::{base_unit_of, dimension_of, round2, to_base};

#[derive(Debug, Clone)]
pub struct AggregationInput {
    pub ingredient_id: Uuid,
    pub name: String,
    pub quantity: f64,
    pub unit: Option<Unit>,
    pub aisle: Aisle,
    pub recipe_title: String,
    pub base_servings: i32,
    pub target_servings: i32,
}

/// Contribution d'un ingrédient de recette à un article agrégé (instantané
/// figé à la génération — quantités déjà mises à l'échelle, unité d'origine).
/// Stockée en JSONB (colonne `grocery_items.sources`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroceryItemSource {
    #[serde(default)]
    pub ingredient_id: Option<Uuid>,
    pub recipe_title: String,
    pub quantity: f64,
    pub unit: Option<Unit>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AggregatedItem {
    pub label: String,
    /// Quantité totale en unité de base (g / ml / unités / pincées).
    pub quantity: f64,
    /// Unité de base de la dimension (g, ml, unit, pinch) ou None.
    pub unit: Option<Unit>,
    pub aisle: Aisle,
    pub sources: Vec<GroceryItemSource>,
}

pub fn normalize_ingredient_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn aggregate_ingredients(rows: &[AggregationInput]) -> Vec<AggregatedItem> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, AggregatedItem> = HashMap::new();

    for row in rows {
        let scaled = row.quantity * f64::from(row.target_servings) / f64::from(row.base_servings);
        let dimension = dimension_of(row.unit);
        let key = format!("{}|{}", normalize_ingredient_name(&row.name), dimension.as_str());

        let group = groups.entry(key.clone()).or_insert_with(|| {
            order.push(key);
            AggregatedItem {
                label: row.name.trim().to_string(),
                quantity: 0.0,
                unit: base_unit_of(dimension),
                aisle: row.aisle,
                sources: Vec::new(),
            }
        });

        group.quantity += to_base(scaled, row.unit);
        // Premier rayon significatif : « other » cède la place à un rayon précis.
        if group.aisle == Aisle::Other && row.aisle != Aisle::Other {
            group.aisle = row.aisle;
        }
        group.sources.push(GroceryItemSource {
            ingredient_id: Some(row.ingredient_id),
            recipe_title: row.recipe_title.clone(),
            quantity: round2(scaled),
            unit: row.unit,
        });
    }

    order
        .into_iter()
        .filter_map(|key| groups.remove(&key))
        .map(|group| AggregatedItem {
            quantity: round2(group.quantity),
            ..group
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn ing(f: impl FnOnce(&mut AggregationInput)) -> AggregationInput {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut input = AggregationInput {
            ingredient_id: Uuid::from_u128(n as u128),
            name: "riz basmati".into(),
            quantity: 200.0,
            unit: Some(Unit::G),
            aisle: Aisle::Dry,
            recipe_title: format!("Recette {n}"),
            base_servings: 4,
            target_servings: 4,
        };
        f(&mut input);
        input
    }

    #[test]
    fn additionne_meme_nom_meme_dimension() {
        let items = aggregate_ingredients(&[
            ing(|i| i.recipe_title = "Poulet au beurre".into()),
            ing(|i| {
                i.quantity = 150.0;
                i.recipe_title = "Tofu Tao".into();
            }),
        ]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "riz basmati");
        assert_eq!(items[0].quantity, 350.0);
        assert_eq!(items[0].unit, Some(Unit::G));
        assert_eq!(items[0].aisle, Aisle::Dry);
        let titles: Vec<_> = items[0].sources.iter().map(|s| s.recipe_title.as_str()).collect();
        assert_eq!(titles, ["Poulet au beurre", "Tofu Tao"]);
    }

    #[test]
    fn convertit_avant_d_additionner() {
        let items = aggregate_ingredients(&[
            ing(|_| {}),
            ing(|i| {
                i.quantity = 0.5;
                i.unit = Some(Unit::Kg);
            }),
        ]);
        assert_eq!(items[0].quantity, 700.0);
        assert_eq!(items[0].unit, Some(Unit::G));
    }

    #[test]
    fn tasses_et_millilitres() {
        let items = aggregate_ingredients(&[
            ing(|i| {
                i.name = "bouillon de légumes".into();
                i.quantity = 1.0;
                i.unit = Some(Unit::Cup);
                i.aisle = Aisle::Canned;
            }),
            ing(|i| {
                i.name = "bouillon de légumes".into();
                i.quantity = 250.0;
                i.unit = Some(Unit::Ml);
                i.aisle = Aisle::Canned;
            }),
        ]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, 500.0);
        assert_eq!(items[0].unit, Some(Unit::Ml));
    }

    #[test]
    fn mise_a_l_echelle_des_portions() {
        let items = aggregate_ingredients(&[
            ing(|i| i.target_servings = 6),
            ing(|i| {
                i.quantity = 100.0;
                i.base_servings = 2;
                i.target_servings = 5;
            }),
        ]);
        assert_eq!(items[0].quantity, 550.0);
        let quantities: Vec<_> = items[0].sources.iter().map(|s| s.quantity).collect();
        assert_eq!(quantities, [300.0, 250.0]);
    }

    #[test]
    fn unites_incompatibles_separees() {
        let items = aggregate_ingredients(&[
            ing(|i| {
                i.name = "tomates".into();
                i.quantity = 2.0;
                i.unit = Some(Unit::Unit);
            }),
            ing(|i| i.name = "tomates".into()),
        ]);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn normalisation_du_nom() {
        assert_eq!(normalize_ingredient_name("  Lentilles  Vertes "), "lentilles vertes");
        let items = aggregate_ingredients(&[
            ing(|i| i.name = "Lentilles vertes".into()),
            ing(|i| i.name = "lentilles  VERTES".into()),
        ]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "Lentilles vertes");
    }

    #[test]
    fn rayon_precis_l_emporte() {
        let items = aggregate_ingredients(&[ing(|i| i.aisle = Aisle::Other), ing(|_| {})]);
        assert_eq!(items[0].aisle, Aisle::Dry);
    }

    #[test]
    fn comptage_et_pincees() {
        let items = aggregate_ingredients(&[
            ing(|i| {
                i.name = "oignons".into();
                i.quantity = 2.0;
                i.unit = Some(Unit::Unit);
            }),
            ing(|i| {
                i.name = "oignons".into();
                i.quantity = 3.0;
                i.unit = Some(Unit::Unit);
            }),
            ing(|i| {
                i.name = "sel".into();
                i.quantity = 1.0;
                i.unit = Some(Unit::Pinch);
            }),
            ing(|i| {
                i.name = "sel".into();
                i.quantity = 2.0;
                i.unit = Some(Unit::Pinch);
            }),
        ]);
        let oignons = items.iter().find(|i| i.label == "oignons").unwrap();
        let sel = items.iter().find(|i| i.label == "sel").unwrap();
        assert_eq!((oignons.quantity, oignons.unit), (5.0, Some(Unit::Unit)));
        assert_eq!((sel.quantity, sel.unit), (3.0, Some(Unit::Pinch)));
    }

    #[test]
    fn pas_de_bruit_flottant() {
        let kg = |i: &mut AggregationInput| {
            i.quantity = 0.1;
            i.unit = Some(Unit::Kg);
        };
        let items = aggregate_ingredients(&[ing(kg), ing(kg), ing(kg)]);
        assert_eq!(items[0].quantity, 300.0);
    }

    #[test]
    fn sources_jsonb_compatibles() {
        // Format écrit par l'ancien backend : unit en chaîne ou null.
        let legacy =
            r#"{"ingredientId":"00000000-0000-0000-0000-000000000001","recipeTitle":"T","quantity":1.5,"unit":null}"#;
        let source: GroceryItemSource = serde_json::from_str(legacy).unwrap();
        assert_eq!(source.quantity, 1.5);
    }
}
