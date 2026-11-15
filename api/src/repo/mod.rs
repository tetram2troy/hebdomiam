//! Couche d'accès unique à la base. L'isolation multi-utilisateur est
//! garantie ICI : toute requête sur des données métier filtre par l'userId de
//! la session (PRD §6/§8) — jamais réimplémentée dans les handlers.
//! Les fonctions retournent des résultats métier ; les routes les traduisent en HTTP.

pub mod admin;
pub mod email_tokens;
pub mod grocery;
pub mod plans;
pub mod recipes;
pub mod settings;
pub mod twofa;
pub mod users;
