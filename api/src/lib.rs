//! Hebdomiam — API (axum + sqlx) et service du front statique (SPA Nuxt).
//!
//! Organisation :
//! - `config`, `logger`, `error`, `state` : socle transverse ;
//! - `session`, `client_ip`, `rate_limit`, `validation` : frontière HTTP ;
//! - `domain` : logique métier pure (unités, agrégation, rotation…), testée unitairement ;
//! - `repo` : couche d'accès unique à la base — l'isolation multi-utilisateur
//!   (filtre par userId) est garantie ICI, jamais dans les handlers (PRD §6/§8) ;
//! - `import` : extraction de recettes (schema.org, API Claude, fetch durci SSRF) ;
//! - `routes` : handlers HTTP, `app` : assemblage du routeur.

pub mod app;
pub mod client_ip;
pub mod config;
pub mod crypto;
pub mod db;
pub mod domain;
pub mod error;
pub mod import;
pub mod logger;
pub mod mailer;
pub mod migrate;
pub mod rate_limit;
pub mod repo;
pub mod routes;
pub mod session;
pub mod state;
pub mod totp;
pub mod validation;
