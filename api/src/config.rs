//! Contrat de configuration 12-factor (docs/HOSTING.md §2.9) : l'app refuse de
//! démarrer en production si une variable requise manque — jamais de mode
//! dégradé silencieux.
//!
//! Les noms historiques `NUXT_*` (époque Nitro) restent acceptés en repli :
//! un serveur déjà configuré démarre sans retoucher son `.env`.

use std::path::PathBuf;

use crate::client_ip::{Cidr, parse_cidr_list};

/// Lit une variable d'environnement non vide, en essayant les noms dans l'ordre.
fn read(get: &impl Fn(&str) -> Option<String>, names: &[&str]) -> Option<String> {
    names
        .iter()
        .filter_map(|name| get(name))
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

const DATABASE_URL: &[&str] = &["DATABASE_URL", "NUXT_DATABASE_URL"];
const SESSION_PASSWORD: &[&str] = &["SESSION_PASSWORD", "NUXT_SESSION_PASSWORD"];
const PUBLIC_BASE_URL: &[&str] = &["PUBLIC_BASE_URL", "NUXT_PUBLIC_BASE_URL"];
const TRUST_XFF: &[&str] = &["TRUST_XFF", "NUXT_TRUST_XFF"];
const IMPORT_ALLOW_PRIVATE: &[&str] = &["IMPORT_ALLOW_PRIVATE", "NUXT_IMPORT_ALLOW_PRIVATE"];
const EXTRACTION_MODEL: &[&str] = &["EXTRACTION_MODEL", "NUXT_EXTRACTION_MODEL"];

/// Modèle d'extraction par défaut (surchargeable par EXTRACTION_MODEL).
pub const DEFAULT_EXTRACTION_MODEL: &str = "claude-opus-5-5";

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    /// `tls` (TLS implicite), `none` (clair, relais local) ou STARTTLS par défaut.
    pub security: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub session_password: Option<String>,
    pub public_base_url: Option<String>,
    pub port: u16,
    pub static_dir: Option<PathBuf>,
    pub production: bool,
    /// Appelants directs dont les en-têtes de transfert sont crus (client_ip.rs).
    pub trusted_proxies: Vec<Cidr>,
    pub trust_xff: bool,
    pub import_allow_private: bool,
    pub anthropic_api_key: Option<String>,
    pub extraction_model: String,
    pub smtp: Option<SmtpConfig>,
    pub smtp_from: String,
}

/// Vrai en production réelle (APP_ENV=production, posé par l'image Docker).
fn is_production(get: &impl Fn(&str) -> Option<String>) -> bool {
    read(get, &["APP_ENV"]).as_deref() == Some("production")
}

/// Problèmes de configuration bloquants — fonction pure, testée en unitaire.
pub fn env_problems(get: impl Fn(&str) -> Option<String>) -> Vec<String> {
    let mut problems = Vec::new();
    if read(&get, DATABASE_URL).is_none() {
        problems
            .push("DATABASE_URL manquant (URL PostgreSQL, ex. postgresql://user:mdp@db:5432/hebdomiam)".to_string());
    }
    // En dev, un secret de session aléatoire est généré au démarrage ; en
    // production il doit être stable (sinon chaque redémarrage déconnecte tout le monde).
    if is_production(&get) {
        match read(&get, SESSION_PASSWORD) {
            None => problems.push(
                "SESSION_PASSWORD manquant (scelle les cookies de session — openssl rand -base64 32)".to_string(),
            ),
            Some(password) if password.chars().count() < 32 => {
                problems.push("SESSION_PASSWORD trop court (minimum 32 caractères)".to_string())
            }
            Some(_) => {}
        }
        if read(&get, PUBLIC_BASE_URL).is_none() {
            problems.push("PUBLIC_BASE_URL manquant (base des liens de courriels, ex. https://recettes.example.com — sans elle les liens seraient dérivés de l'en-tête Host)".to_string());
        }
        if read(&get, &["SMTP_HOST"]).is_none() {
            problems.push("SMTP_HOST manquant (relais SMTP — sans lui vérifications, invitations et réinitialisations partiraient en silence dans les logs)".to_string());
        }
        // Derrière Traefik, sans proxy de confiance tous les clients auraient
        // l'IP de Traefik : un seul quota d'auth partagé (auto-DoS).
        if read(&get, &["TRUSTED_PROXIES"]).is_none() {
            problems.push("TRUSTED_PROXIES manquant (CIDR du réseau Docker de Traefik, ex. 172.18.0.0/16 — seuls ces appelants peuvent fournir cf-connecting-ip)".to_string());
        }
    } else if let Some(password) = read(&get, SESSION_PASSWORD)
        && password.chars().count() < 32
    {
        problems.push("SESSION_PASSWORD trop court (minimum 32 caractères)".to_string());
    }
    if let Some(list) = read(&get, &["TRUSTED_PROXIES"])
        && let Err(entry) = parse_cidr_list(&list)
    {
        problems.push(format!(
            "TRUSTED_PROXIES illisible : « {entry} » n'est pas un CIDR (ex. 172.18.0.0/16)"
        ));
    }
    problems
}

impl Config {
    pub fn from_env() -> Result<Config, Vec<String>> {
        Self::from_getter(|name| std::env::var(name).ok())
    }

    pub fn from_getter(get: impl Fn(&str) -> Option<String>) -> Result<Config, Vec<String>> {
        let problems = env_problems(&get);
        if !problems.is_empty() {
            return Err(problems);
        }
        let flag = |names: &[&str]| matches!(read(&get, names).as_deref(), Some("1") | Some("true"));

        let smtp = read(&get, &["SMTP_HOST"]).map(|host| SmtpConfig {
            host,
            port: read(&get, &["SMTP_PORT"]).and_then(|p| p.parse().ok()).unwrap_or(587),
            security: read(&get, &["SMTP_SECURITY"]).unwrap_or_else(|| "starttls".to_string()),
            username: read(&get, &["SMTP_USERNAME"]),
            password: read(&get, &["SMTP_PASSWORD"]),
        });

        // STATIC_DIR="" désactive le service du front (dev : Nuxt sert lui-même).
        let static_dir = match get("STATIC_DIR") {
            Some(dir) if dir.trim().is_empty() => None,
            Some(dir) => Some(PathBuf::from(dir.trim())),
            None => Some(PathBuf::from("public")),
        };

        Ok(Config {
            database_url: read(&get, DATABASE_URL).unwrap_or_default(),
            session_password: read(&get, SESSION_PASSWORD),
            public_base_url: read(&get, PUBLIC_BASE_URL).map(|url| url.trim_end_matches('/').to_string()),
            port: read(&get, &["PORT"]).and_then(|p| p.parse().ok()).unwrap_or(3000),
            static_dir,
            production: is_production(&get),
            trusted_proxies: read(&get, &["TRUSTED_PROXIES"])
                .and_then(|list| parse_cidr_list(&list).ok())
                .unwrap_or_default(),
            trust_xff: flag(TRUST_XFF),
            import_allow_private: flag(IMPORT_ALLOW_PRIVATE),
            anthropic_api_key: read(&get, &["ANTHROPIC_API_KEY"]),
            extraction_model: read(&get, EXTRACTION_MODEL).unwrap_or_else(|| DEFAULT_EXTRACTION_MODEL.to_string()),
            smtp,
            smtp_from: read(&get, &["SMTP_FROM"]).unwrap_or_else(|| "hebdomiam@localhost".to_string()),
        })
    }

    /// Configuration minimale pour les tests (aucune variable d'environnement lue).
    pub fn for_tests(database_url: &str) -> Config {
        Config {
            database_url: database_url.to_string(),
            session_password: Some("test-session-password-of-at-least-32-chars".to_string()),
            public_base_url: None,
            port: 0,
            static_dir: None,
            production: false,
            trusted_proxies: parse_cidr_list("127.0.0.0/8").unwrap_or_default(),
            trust_xff: true,
            import_allow_private: false,
            anthropic_api_key: None,
            extraction_model: DEFAULT_EXTRACTION_MODEL.to_string(),
            smtp: None,
            smtp_from: "hebdomiam@localhost".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |name| map.get(name).cloned()
    }

    const VALID_PROD: &[(&str, &str)] = &[
        ("APP_ENV", "production"),
        ("DATABASE_URL", "postgresql://u:p@db/h"),
        ("SESSION_PASSWORD", "0123456789abcdef0123456789abcdef"),
        ("PUBLIC_BASE_URL", "https://recettes.example.com"),
        ("SMTP_HOST", "smtp.example.com"),
        ("TRUSTED_PROXIES", "172.18.0.0/16"),
    ];

    #[test]
    fn production_complete_ok() {
        assert!(env_problems(env(VALID_PROD)).is_empty());
    }

    #[test]
    fn database_url_requise_partout() {
        let problems = env_problems(env(&[]));
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("DATABASE_URL"));
    }

    #[test]
    fn production_exige_secret_base_url_et_smtp() {
        let problems = env_problems(env(&[("APP_ENV", "production"), ("DATABASE_URL", "postgresql://x")]));
        assert_eq!(problems.len(), 4);
        assert!(problems.iter().any(|p| p.contains("TRUSTED_PROXIES")));
        assert!(problems.iter().any(|p| p.contains("SESSION_PASSWORD")));
        assert!(problems.iter().any(|p| p.contains("PUBLIC_BASE_URL")));
        assert!(problems.iter().any(|p| p.contains("SMTP_HOST")));
    }

    #[test]
    fn proxies_de_confiance_illisibles_refuses() {
        let problems = env_problems(env(&[
            ("DATABASE_URL", "postgresql://x"),
            ("TRUSTED_PROXIES", "10.0.0.0/8, oups"),
        ]));
        assert!(problems.iter().any(|p| p.contains("oups")));
        let config = Config::from_getter(env(&[
            ("DATABASE_URL", "postgresql://x"),
            ("TRUSTED_PROXIES", "10.0.0.0/8,fd00::/8"),
        ]))
        .unwrap();
        assert_eq!(config.trusted_proxies.len(), 2);
    }

    #[test]
    fn secret_trop_court_refuse() {
        let problems = env_problems(env(&[
            ("DATABASE_URL", "postgresql://x"),
            ("SESSION_PASSWORD", "court"),
        ]));
        assert!(problems.iter().any(|p| p.contains("trop court")));
    }

    #[test]
    fn noms_nuxt_historiques_acceptes() {
        let config = Config::from_getter(env(&[
            ("NUXT_DATABASE_URL", "postgresql://legacy"),
            ("NUXT_SESSION_PASSWORD", "0123456789abcdef0123456789abcdef"),
            ("NUXT_PUBLIC_BASE_URL", "https://recettes.example.com/"),
            ("NUXT_TRUST_XFF", "1"),
        ]))
        .unwrap();
        assert_eq!(config.database_url, "postgresql://legacy");
        assert_eq!(config.public_base_url.as_deref(), Some("https://recettes.example.com"));
        assert!(config.trust_xff);
        assert!(!config.production);
    }

    #[test]
    fn static_dir_vide_desactive_le_front() {
        let config = Config::from_getter(env(&[("DATABASE_URL", "postgresql://x"), ("STATIC_DIR", "")])).unwrap();
        assert!(config.static_dir.is_none());
    }
}
