//! Journalisation applicative structurée : une ligne JSON par événement sur la
//! sortie standard — captée par Docker en prod (PRD §8), exploitable avec jq/grep.
//!
//! Règles :
//! - nommage `domaine.action` (auth.login, recipe.deleted…) ;
//! - niveaux : info = opération normale digne d'audit, warn = échec de sécurité
//!   ou comportement suspect, error = défaillance technique ;
//! - jamais de secret (mot de passe, jeton, code TOTP) ni de contenu utilisateur
//!   (titres, libellés) ; PII minimale : userId de préférence — le courriel
//!   uniquement pour les événements d'auth sans compte résolu (détection d'abus).

use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// Construit la ligne JSON (fonction pure, testée).
pub fn format_line(level: Level, event: &str, fields: Value, time: &str) -> String {
    let mut line = Map::new();
    line.insert("time".into(), Value::String(time.to_string()));
    line.insert("level".into(), Value::String(level.as_str().into()));
    line.insert("event".into(), Value::String(event.into()));
    if let Value::Object(extra) = fields {
        for (key, value) in extra {
            // Les champs réservés ne sont jamais écrasés par l'appelant.
            if !matches!(key.as_str(), "time" | "level" | "event") {
                line.insert(key, value);
            }
        }
    }
    Value::Object(line).to_string()
}

fn write(level: Level, event: &str, fields: Value) {
    let time = crate::domain::time::iso_now();
    let line = format_line(level, event, fields, &time);
    match level {
        Level::Info => println!("{line}"),
        Level::Warn | Level::Error => eprintln!("{line}"),
    }
}

/// Opération normale digne d'audit (connexion, création, suppression…).
pub fn log_event(event: &str, fields: Value) {
    write(Level::Info, event, fields);
}

/// Échec de sécurité ou comportement suspect (mot de passe refusé, jeton invalide…).
pub fn log_warn(event: &str, fields: Value) {
    write(Level::Warn, event, fields);
}

/// Défaillance technique inattendue — consigne le message, jamais la charge utile.
pub fn log_error(event: &str, error: &dyn std::fmt::Display, fields: Value) {
    let mut fields = match fields {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    fields.insert("message".into(), Value::String(error.to_string()));
    write(Level::Error, event, Value::Object(fields));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ligne_json_avec_champs() {
        let line = format_line(
            Level::Warn,
            "auth.login_failed",
            json!({ "ip": "1.2.3.4" }),
            "2026-01-01T00:00:00.000Z",
        );
        let parsed: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["level"], "warn");
        assert_eq!(parsed["event"], "auth.login_failed");
        assert_eq!(parsed["ip"], "1.2.3.4");
        assert_eq!(parsed["time"], "2026-01-01T00:00:00.000Z");
    }

    #[test]
    fn champs_reserves_proteges() {
        let line = format_line(Level::Info, "x.y", json!({ "level": "error", "event": "faux" }), "t");
        let parsed: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["level"], "info");
        assert_eq!(parsed["event"], "x.y");
    }

    #[test]
    fn une_seule_ligne() {
        let line = format_line(Level::Info, "x.y", json!({ "a": "multi\nligne" }), "t");
        assert!(!line.contains('\n'));
    }
}
