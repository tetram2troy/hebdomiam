//! IP client derrière Cloudflare puis Traefik (docs/HOSTING.md §2.3).
//!
//! Les en-têtes de transfert ne valent que ce que vaut celui qui les pose :
//! ils ne sont lus QUE si l'appelant direct (IP socket) est un proxy de
//! confiance (`TRUSTED_PROXIES`, liste de CIDR — en production le réseau
//! Docker de Traefik). Un appelant quelconque qui forge `cf-connecting-ip`
//! est identifié par son IP socket : pas de contournement du rate limiting
//! d'auth, pas d'usurpation dans les logs de sécurité.
//!
//! Ordre de résolution quand l'appelant est de confiance :
//! 1. cf-connecting-ip — posé par Cloudflare (le DNS du home lab est proxifié) ;
//! 2. X-Forwarded-For si TRUST_XFF est défini — son entrée de gauche est
//!    fournie par le client lui-même : réservé aux e2e (IP simulées par spec),
//!    jamais en production ;
//! 3. IP socket.
//!
//! Limite assumée : Traefik doit lui-même n'accepter `cf-connecting-ip` que
//! de Cloudflare (voir deploy/serveur-perso/README.md) — l'API ne voit que
//! Traefik. Les verrous par compte (routes/auth.rs) couvrent ce cas résiduel.

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::HeaderMap;
use axum::http::request::Parts;

use crate::error::ApiError;
use crate::state::AppState;

/// Bloc d'adresses (ex. `172.16.0.0/12`, `fd00::/8`, ou une adresse seule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    network: IpAddr,
    prefix: u8,
}

/// IPv4 mappée en IPv6 (`::ffff:a.b.c.d`, socket double pile) → IPv4.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    }
}

impl Cidr {
    pub fn parse(text: &str) -> Option<Cidr> {
        let text = text.trim();
        let (address, prefix) = match text.split_once('/') {
            Some((address, prefix)) => (address, Some(prefix.parse::<u8>().ok()?)),
            None => (text, None),
        };
        let network = canonical(address.parse().ok()?);
        let max = if network.is_ipv4() { 32 } else { 128 };
        let prefix = prefix.unwrap_or(max);
        (prefix <= max).then_some(Cidr { network, prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        let bits = |ip: IpAddr| match ip {
            IpAddr::V4(v4) => (u128::from(u32::from(v4)), 32u32),
            IpAddr::V6(v6) => (u128::from(v6), 128u32),
        };
        let (network, width) = bits(self.network);
        let (candidate, candidate_width) = bits(canonical(ip));
        if width != candidate_width {
            return false;
        }
        let shift = width - u32::from(self.prefix);
        // Préfixe 0 : tout le monde (décalage égal à la largeur interdit).
        shift >= width || (network >> shift) == (candidate >> shift)
    }
}

/// Liste de CIDR séparés par des virgules ; `Err` nomme l'entrée illisible.
pub fn parse_cidr_list(text: &str) -> Result<Vec<Cidr>, String> {
    text.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| Cidr::parse(entry).ok_or_else(|| entry.to_string()))
        .collect()
}

pub fn resolve_client_ip(
    headers: &HeaderMap,
    socket: Option<SocketAddr>,
    trusted_proxies: &[Cidr],
    trust_xff: bool,
) -> String {
    let Some(peer) = socket.map(|addr| canonical(addr.ip())) else {
        return "unknown".to_string();
    };
    if !trusted_proxies.iter().any(|cidr| cidr.contains(peer)) {
        return peer.to_string();
    }
    // Une valeur qui n'est pas une adresse IP est ignorée : elle finirait
    // telle quelle dans les logs et les clés de rate limiting.
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .and_then(|v| v.trim().parse::<IpAddr>().ok())
            .map(|ip| canonical(ip).to_string())
    };
    if let Some(cloudflare) = header("cf-connecting-ip") {
        return cloudflare;
    }
    if trust_xff && let Some(first) = header("x-forwarded-for") {
        return first;
    }
    peer.to_string()
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
            &state.config.trusted_proxies,
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

    fn proxy() -> Option<SocketAddr> {
        Some("172.18.0.5:4242".parse().unwrap())
    }

    fn trusted() -> Vec<Cidr> {
        parse_cidr_list("172.16.0.0/12, fd00::/8").unwrap()
    }

    #[test]
    fn cloudflare_prioritaire_depuis_un_proxy_de_confiance() {
        let h = headers(&[("cf-connecting-ip", "1.2.3.4"), ("x-forwarded-for", "9.9.9.9")]);
        assert_eq!(resolve_client_ip(&h, proxy(), &trusted(), true), "1.2.3.4");
    }

    #[test]
    fn en_tetes_ignores_hors_proxy_de_confiance() {
        // Appelant direct qui forge les en-têtes : son IP socket fait foi.
        let h = headers(&[("cf-connecting-ip", "1.2.3.4"), ("x-forwarded-for", "9.9.9.9")]);
        let attacker = Some("203.0.113.50:5555".parse().unwrap());
        assert_eq!(resolve_client_ip(&h, attacker, &trusted(), true), "203.0.113.50");
        // Aucun proxy configuré : personne n'est cru.
        assert_eq!(resolve_client_ip(&h, proxy(), &[], true), "172.18.0.5");
    }

    #[test]
    fn xff_seulement_si_active() {
        let h = headers(&[("x-forwarded-for", "203.0.113.31, 8.8.8.8")]);
        assert_eq!(resolve_client_ip(&h, proxy(), &trusted(), false), "172.18.0.5");
        assert_eq!(resolve_client_ip(&h, proxy(), &trusted(), true), "203.0.113.31");
    }

    #[test]
    fn valeur_non_ip_ignoree() {
        let h = headers(&[("cf-connecting-ip", "admin\" OR 1=1")]);
        assert_eq!(resolve_client_ip(&h, proxy(), &trusted(), false), "172.18.0.5");
    }

    #[test]
    fn inconnu_sans_socket() {
        let h = headers(&[("cf-connecting-ip", "1.2.3.4")]);
        assert_eq!(resolve_client_ip(&h, None, &trusted(), true), "unknown");
    }

    #[test]
    fn cidr() {
        let block = Cidr::parse("172.16.0.0/12").unwrap();
        assert!(block.contains("172.31.255.255".parse().unwrap()));
        assert!(!block.contains("172.32.0.1".parse().unwrap()));
        assert!(
            block.contains("::ffff:172.18.0.5".parse().unwrap()),
            "socket double pile"
        );
        assert!(!block.contains("fd00::1".parse().unwrap()));
        assert!(Cidr::parse("127.0.0.1").unwrap().contains("127.0.0.1".parse().unwrap()));
        assert!(Cidr::parse("0.0.0.0/0").unwrap().contains("8.8.8.8".parse().unwrap()));
        assert!(
            Cidr::parse("fd00::/8")
                .unwrap()
                .contains("fd12:3456::1".parse().unwrap())
        );
        for invalid in ["", "pas-un-cidr", "10.0.0.0/33", "fd00::/129", "10.0.0.0/x"] {
            assert!(Cidr::parse(invalid).is_none(), "{invalid}");
        }
        assert_eq!(parse_cidr_list("10.0.0.0/8, oups").unwrap_err(), "oups");
    }
}
