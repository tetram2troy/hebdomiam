//! IP client derrière Cloudflare puis Traefik (docs/HOSTING.md §2.3).
//! X-Forwarded-For n'est PAS fiable : son entrée la plus à GAUCHE est fournie
//! par le client lui-même — Cloudflare ne fait qu'ajouter l'IP réelle à droite.
//! S'y fier rendrait le rate limiting d'auth contournable par simple rotation
//! d'en-tête (et les logs de sécurité usurpables). Ordre de résolution :
//! 1. cf-connecting-ip — posé par Cloudflare, seul en-tête digne de confiance
//!    en production (le DNS wildcard du home lab est toujours proxifié) ;
//! 2. X-Forwarded-For si TRUST_XFF est défini — les e2e isolent leurs buckets
//!    de rate limiting en simulant des IP distinctes ; jamais en production ;
//! 3. IP socket — l'appelant direct (Traefik, ou le navigateur en dev).

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::HeaderMap;
use axum::http::request::Parts;

use crate::error::ApiError;
use crate::state::AppState;

pub fn resolve_client_ip(headers: &HeaderMap, socket: Option<SocketAddr>, trust_xff: bool) -> String {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    if let Some(cloudflare) = header("cf-connecting-ip") {
        return cloudflare;
    }
    if trust_xff
        && let Some(first) = header("x-forwarded-for").and_then(|v| v.split(',').next().map(|s| s.trim().to_string()))
        && !first.is_empty()
    {
        return first;
    }
    socket
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Extracteur : IP client résolue selon les règles ci-dessus.
pub struct ClientIp(pub String);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let socket = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|info| info.0);
        Ok(ClientIp(resolve_client_ip(
            &parts.headers,
            socket,
            state.config.trust_xff,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (k, v) in pairs {
            map.insert(*k, v.parse().unwrap());
        }
        map
    }

    fn socket() -> Option<SocketAddr> {
        Some("10.0.0.5:4242".parse().unwrap())
    }

    #[test]
    fn cloudflare_prioritaire() {
        let h = headers(&[("cf-connecting-ip", "1.2.3.4"), ("x-forwarded-for", "9.9.9.9")]);
        assert_eq!(resolve_client_ip(&h, socket(), true), "1.2.3.4");
    }

    #[test]
    fn xff_ignore_par_defaut() {
        let h = headers(&[("x-forwarded-for", "9.9.9.9, 8.8.8.8")]);
        assert_eq!(resolve_client_ip(&h, socket(), false), "10.0.0.5");
    }

    #[test]
    fn xff_entree_de_gauche_si_confiance() {
        let h = headers(&[("x-forwarded-for", "203.0.113.31, 8.8.8.8")]);
        assert_eq!(resolve_client_ip(&h, socket(), true), "203.0.113.31");
    }

    #[test]
    fn inconnu_sans_socket() {
        assert_eq!(resolve_client_ip(&HeaderMap::new(), None, false), "unknown");
    }
}
