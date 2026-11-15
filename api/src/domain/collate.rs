//! Comparaison de libellés « à la française » sans ICU : insensible à la casse
//! et aux accents, comme `localeCompare` pour l'ordre alphabétique usuel
//! (« érable » avant « fraise », pas après « zucchini »).

use std::cmp::Ordering;

fn fold(c: char) -> &'static str {
    match c {
        'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' | 'À' | 'Â' | 'Ä' | 'Á' | 'Ã' | 'Å' => "a",
        'ç' | 'Ç' => "c",
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => "e",
        'î' | 'ï' | 'í' | 'ì' | 'Î' | 'Ï' | 'Í' | 'Ì' => "i",
        'ô' | 'ö' | 'ó' | 'ò' | 'õ' | 'Ô' | 'Ö' | 'Ó' | 'Ò' | 'Õ' => "o",
        'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' | 'Ú' => "u",
        'ÿ' | 'Ÿ' => "y",
        'ñ' | 'Ñ' => "n",
        'œ' | 'Œ' => "oe",
        'æ' | 'Æ' => "ae",
        _ => "",
    }
}

pub fn collation_key(text: &str) -> String {
    let mut key = String::with_capacity(text.len());
    for c in text.chars() {
        let folded = fold(c);
        if folded.is_empty() {
            key.extend(c.to_lowercase());
        } else {
            key.push_str(folded);
        }
    }
    key
}

/// Ordre alphabétique insensible aux accents ; départage stable sur le texte brut.
pub fn compare(a: &str, b: &str) -> Ordering {
    collation_key(a).cmp(&collation_key(b)).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accents_et_casse_ignores() {
        assert_eq!(compare("érable", "fraise"), Ordering::Less);
        assert_eq!(compare("Zucchini", "abricot"), Ordering::Greater);
        assert_eq!(compare("Œuf", "oignon"), Ordering::Less);
    }
}
