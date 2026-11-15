//! Parseur de lignes d'ingrédients (« 500 g de lentilles vertes ») — pur, testé.
//! schema.org fournit les ingrédients en texte libre FR/EN ; on en tire
//! quantité + unité + nom + rayon deviné. Ce qui ne se parse pas devient
//! « 1 × nom » avec un avertissement (l'utilisateur relit le brouillon).

use std::sync::LazyLock;

use regex::Regex;

use crate::domain::catalog::{Aisle, Unit};
use crate::domain::units::round2;

/// [motif, unité canonique, facteur multiplicatif]
static UNIT_SYNONYMS: LazyLock<Vec<(Regex, Unit, f64)>> = LazyLock::new(|| {
    let r = |pattern: &str| Regex::new(&format!("(?i)^(?:{pattern})")).unwrap();
    vec![
        (
            r(r"cuill[eè]res? à soupe|c\.?\s*à\s*s(?:oupe)?\.?|càs|tbsp|tablespoons?"),
            Unit::Tbsp,
            1.0,
        ),
        (
            r(r"cuill[eè]res? à (?:thé|café)|c\.?\s*à\s*(?:t(?:hé)?|c(?:afé)?)\.?|càc|càt|tsp|teaspoons?"),
            Unit::Tsp,
            1.0,
        ),
        (r(r"tasses?|cups?"), Unit::Cup, 1.0),
        (r(r"kilogrammes?|kilos?|kg"), Unit::Kg, 1.0),
        (r(r"(?:grammes?|gr?s?)\b"), Unit::G, 1.0),
        (r(r"millilitres?|ml"), Unit::Ml, 1.0),
        (r(r"centilitres?|cl"), Unit::Ml, 10.0),
        (r(r"(?:litres?|liters?|l)\b"), Unit::L, 1.0),
        (r(r"onces?|oz|ounces?"), Unit::Oz, 1.0),
        (r(r"livres?|lbs?|pounds?"), Unit::Lb, 1.0),
        (r(r"pinc[ée]es?|pinch(?:es)?"), Unit::Pinch, 1.0),
        (r(r"unit[ée]s?"), Unit::Unit, 1.0),
    ]
});

/// Les motifs spécifiques (conserves, surgelés) passent avant les génériques :
/// « tomates en dés » = conserve même si « tomate » est un mot de fruits et légumes.
static AISLE_KEYWORDS: LazyLock<Vec<(Regex, Aisle)>> = LazyLock::new(|| {
    let r = |pattern: &str| Regex::new(&format!("(?i){pattern}")).unwrap();
    vec![
        (
            r(
                r"en conserve|en dés|bouillon|lait de coco|pâte de tomate|sauce tomate|pois chiches?|haricots? (?:noirs|rouges|blancs)",
            ),
            Aisle::Canned,
        ),
        (r(r"surgelé|congelé"), Aisle::Frozen),
        (
            r(
                r"tomate|oignon|ail\b|échalote|carotte|céleri|poivron|courge|patate|pomme de terre|salade|laitue|épinard|brocoli|chou|champignon|citron|lime|gingembre|coriandre|persil|basilic frais|menthe|fruit|banane|pomme\b|avocat|concombre|zucchini|courgette|légume",
            ),
            Aisle::Produce,
        ),
        (
            r(r"poulet|bœuf|boeuf|porc|veau|agneau|dinde|jambon|bacon|saucisse|viande|steak|côtelette"),
            Aisle::Butcher,
        ),
        (
            r(r"poisson|saumon|thon|crevette|morue|tilapia|fruits de mer|pétoncle"),
            Aisle::Fish,
        ),
        (
            r(r"lait\b|crème|beurre|fromage|yogourt|yaourt|œufs?|oeufs?|parmesan|mozzarella|cheddar|feta"),
            Aisle::Dairy,
        ),
        (
            r(
                r"farine|riz\b|pâtes|spaghetti|macaroni|lentille|quinoa|orge|couscous|sucre|avoine|céréale|noix|amande|arachide|chapelure",
            ),
            Aisle::Dry,
        ),
        (r(r"pain\b|baguette|tortilla|pita|croûton"), Aisle::Bakery),
        (
            r(
                r"sel\b|poivre|épice|cumin|paprika|cannelle|origan|thym|laurier|curcuma|cari|curry|chili en poudre|muscade|vanille|herbes de provence",
            ),
            Aisle::Spices,
        ),
        (r(r"eau\b|jus\b|vin\b|bière|café\b|thé\b"), Aisle::Beverages),
    ]
});

static VULGAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([½⅓⅔¼¾⅛])\s*").unwrap());
static WRITTEN_FRACTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)\s*/\s*(\d+)\s*").unwrap());
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+(?:[.,]\d+)?)\s*").unwrap());
static RANGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:à|-|–|to)\s*\d+(?:[.,]\d+)?\s*").unwrap());
static CONNECTOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(?:de\s+|d'|of\s+)?").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedIngredientLine {
    pub quantity: f64,
    pub unit: Option<Unit>,
    pub name: String,
    pub aisle: Aisle,
    /// false quand la quantité n'a pas pu être lue (défaut 1).
    pub parsed: bool,
}

fn vulgar_value(symbol: &str) -> f64 {
    match symbol {
        "½" => 0.5,
        "⅓" => 1.0 / 3.0,
        "⅔" => 2.0 / 3.0,
        "¼" => 0.25,
        "¾" => 0.75,
        "⅛" => 0.125,
        _ => 0.0,
    }
}

/// Consomme une fraction (« ½ » ou « 1/2 ») en tête de chaîne.
fn read_fraction(text: &str) -> Option<(f64, &str)> {
    if let Some(caps) = VULGAR.captures(text) {
        return Some((vulgar_value(&caps[1]), &text[caps[0].len()..]));
    }
    let caps = WRITTEN_FRACTION.captures(text)?;
    let numerator: f64 = caps[1].parse().ok()?;
    let denominator: f64 = caps[2].parse().ok()?;
    (denominator != 0.0).then(|| (numerator / denominator, &text[caps[0].len()..]))
}

/// Lit une quantité en tête : « 2 », « 1,5 », « 1/2 », « 1 ½ », « 2 à 3 ».
fn read_quantity(text: &str) -> Option<(f64, &str)> {
    let mut rest = text.trim();
    let mut value;

    // Fraction d'abord : sinon « 1/2 » serait lu comme l'entier 1.
    if let Some((fraction, after)) = read_fraction(rest) {
        value = fraction;
        rest = after;
    } else {
        let caps = NUMBER.captures(rest)?;
        value = caps[1].replace(',', ".").parse().ok()?;
        rest = &rest[caps[0].len()..];
        // Nombre mixte : « 1 ½ » ou « 1 1/2 ».
        if let Some((fraction, after)) = read_fraction(rest) {
            value += fraction;
            rest = after;
        }
    }

    // Fourchette « 2 à 3 » / « 2-3 » : on prend la borne basse.
    if let Some(range) = RANGE.find(rest) {
        rest = &rest[range.end()..];
    }
    (value > 0.0).then(|| (round2(value), rest))
}

pub fn guess_aisle(name: &str) -> Aisle {
    AISLE_KEYWORDS
        .iter()
        .find(|(pattern, _)| pattern.is_match(name))
        .map_or(Aisle::Other, |(_, aisle)| *aisle)
}

fn default_name(name: String) -> String {
    if name.is_empty() {
        "ingrédient".to_string()
    } else {
        name
    }
}

pub fn parse_ingredient_line(line: &str) -> ParsedIngredientLine {
    let cleaned = SPACES.replace_all(line, " ").trim().to_string();
    let Some((mut value, rest)) = read_quantity(&cleaned) else {
        let name = default_name(cleaned);
        return ParsedIngredientLine {
            quantity: 1.0,
            unit: None,
            aisle: guess_aisle(&name),
            name,
            parsed: false,
        };
    };

    let mut rest = rest;
    let mut unit = None;
    for (pattern, canonical, factor) in UNIT_SYNONYMS.iter() {
        if let Some(found) = pattern.find(rest) {
            unit = Some(*canonical);
            value = round2(value * factor);
            rest = &rest[found.end()..];
            break;
        }
    }

    // « 200 g de riz » / « 2 cups of rice » → retirer le connecteur.
    let name = default_name(CONNECTOR.replace(rest, "").trim().to_string());
    ParsedIngredientLine {
        quantity: value,
        unit,
        aisle: guess_aisle(&name),
        name,
        parsed: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(line: &str, quantity: f64, unit: Option<Unit>, name: &str) {
        let parsed = parse_ingredient_line(line);
        assert_eq!(
            (parsed.quantity, parsed.unit, parsed.name.as_str()),
            (quantity, unit, name),
            "{line}"
        );
        assert!(parsed.parsed);
    }

    #[test]
    fn francais() {
        check("500 g de lentilles vertes", 500.0, Some(Unit::G), "lentilles vertes");
        assert_eq!(parse_ingredient_line("500 g de lentilles vertes").aisle, Aisle::Dry);
        check("2 tasses de riz basmati", 2.0, Some(Unit::Cup), "riz basmati");
        check("1 c. à soupe d'huile d'olive", 1.0, Some(Unit::Tbsp), "huile d'olive");
        check("2 c. à thé de cumin moulu", 2.0, Some(Unit::Tsp), "cumin moulu");
        assert_eq!(parse_ingredient_line("2 c. à thé de cumin moulu").aisle, Aisle::Spices);
    }

    #[test]
    fn anglais() {
        check("2 cups of chicken broth", 2.0, Some(Unit::Cup), "chicken broth");
        assert_eq!(parse_ingredient_line("1 lb ground beef").unit, Some(Unit::Lb));
        check("3 tbsp olive oil", 3.0, Some(Unit::Tbsp), "olive oil");
    }

    #[test]
    fn fractions_et_virgules() {
        assert_eq!(parse_ingredient_line("½ tasse de lait").quantity, 0.5);
        assert_eq!(parse_ingredient_line("1/2 c. à thé de sel").quantity, 0.5);
        assert_eq!(parse_ingredient_line("1 ½ tasse de farine").quantity, 1.5);
        let poulet = parse_ingredient_line("1,5 kg de poulet");
        assert_eq!((poulet.quantity, poulet.unit), (1.5, Some(Unit::Kg)));
    }

    #[test]
    fn centilitres() {
        let creme = parse_ingredient_line("25 cl de crème");
        assert_eq!((creme.quantity, creme.unit), (250.0, Some(Unit::Ml)));
    }

    #[test]
    fn fourchette() {
        let parsed = parse_ingredient_line("2 à 3 oignons jaunes");
        assert_eq!(
            (parsed.quantity, parsed.unit, parsed.name.as_str(), parsed.aisle),
            (2.0, None, "oignons jaunes", Aisle::Produce)
        );
    }

    #[test]
    fn sans_quantite() {
        let parsed = parse_ingredient_line("Sel et poivre au goût");
        assert_eq!((parsed.quantity, parsed.unit, parsed.parsed), (1.0, None, false));
    }

    #[test]
    fn rayons() {
        assert_eq!(guess_aisle("hauts de cuisse de poulet"), Aisle::Butcher);
        assert_eq!(guess_aisle("filet de saumon"), Aisle::Fish);
        assert_eq!(guess_aisle("crème 35 %"), Aisle::Dairy);
        assert_eq!(guess_aisle("tomates en dés"), Aisle::Canned);
        assert_eq!(guess_aisle("paprika fumé"), Aisle::Spices);
        assert_eq!(guess_aisle("truc inconnu"), Aisle::Other);
    }
}
