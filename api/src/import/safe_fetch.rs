//! Récupération d'une page externe durcie contre le SSRF (PRD §8) : https
//! seulement, IP privées/loopback/link-local bloquées (y compris via
//! redirections), timeout et taille de réponse plafonnés.
//!
//! La résolution DNS est validée DEUX fois : avant la requête (pour classer le
//! refus en « blocked ») puis par le résolveur du client HTTP lui-même, au
//! moment de la connexion — ce qui ferme la fenêtre de DNS rebinding que
//! l'ancien backend documentait comme limite connue.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, LOCATION, USER_AGENT};
use url::{Host, Url};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REDIRECTS: usize = 3;
const BOT_USER_AGENT: &str = "Mozilla/5.0 (compatible; HebdomiamBot/1.0; import de recettes personnel)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchFailure {
    Unsupported,
    Blocked,
    Unreachable,
    TooLarge,
}

impl FetchFailure {
    pub fn as_str(self) -> &'static str {
        match self {
            FetchFailure::Unsupported => "unsupported",
            FetchFailure::Blocked => "blocked",
            FetchFailure::Unreachable => "unreachable",
            FetchFailure::TooLarge => "tooLarge",
        }
    }
}

#[derive(Debug)]
pub struct FetchedPage {
    pub html: String,
    pub final_url: String,
}

/// Adresse non publique (ou d'usage spécial) : refusée comme cible d'import.
/// Les formes IPv6 qui embarquent une IPv4 (mappée, compatible, NAT64, 6to4)
/// sont ramenées à l'IPv4 embarquée ; les tunnels dont la destination réelle
/// est opaque (Teredo, NAT64 local) sont refusés en bloc.
pub fn is_private_addr(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            a == 0
                || a == 10
                || a == 127
                || (a == 100 && (64..=127).contains(&b)) // CGNAT
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168)
                || (a == 192 && b == 0 && c == 0) // 192.0.0.0/24 (IETF)
                || (a == 192 && b == 0 && c == 2) // TEST-NET-1
                || (a == 198 && (b == 18 || b == 19)) // 198.18.0.0/15 (bancs d'essai)
                || (a == 198 && b == 51 && c == 100) // TEST-NET-2
                || (a == 203 && b == 0 && c == 113) // TEST-NET-3
                || a >= 224 // multicast, réservé, diffusion
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            let embedded = |hi: u16, lo: u16| IpAddr::V4(Ipv4Addr::from((u32::from(hi) << 16) | u32::from(lo)));
            // ::ffff:a.b.c.d (mappée) et ::a.b.c.d (compatible, dépréciée) — couvre aussi :: et ::1.
            if s[..5] == [0, 0, 0, 0, 0] && (s[5] == 0 || s[5] == 0xffff) {
                return is_private_addr(embedded(s[6], s[7]));
            }
            // NAT64 bien connu 64:ff9b::/96
            if s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
                return is_private_addr(embedded(s[6], s[7]));
            }
            // 6to4 2002::/16 : IPv4 dans les bits 16 à 48
            if s[0] == 0x2002 {
                return is_private_addr(embedded(s[1], s[2]));
            }
            (s[0] == 0x64 && s[1] == 0xff9b && s[2] == 1) // NAT64 local 64:ff9b:1::/48
                || (s[0] == 0x2001 && s[1] == 0) // Teredo 2001::/32
                || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation
                || (s[0] & 0xfe00) == 0xfc00 // fc00::/7 (ULA)
                || (s[0] & 0xffc0) == 0xfe80 // fe80::/10 (lien local)
                || (s[0] & 0xffc0) == 0xfec0 // fec0::/10 (site local, déprécié)
                || (s[0] & 0xff00) == 0xff00 // multicast
        }
    }
}

/// Variante texte : une adresse illisible est considérée privée (refus).
pub fn is_private_ip(ip: &str) -> bool {
    ip.parse::<IpAddr>().map_or(true, is_private_addr)
}

async fn resolve_public(host: &str) -> Result<Vec<SocketAddr>, FetchFailure> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, 0))
        .await
        .map_err(|_| FetchFailure::Blocked)?
        .collect();
    if addrs.is_empty() || addrs.iter().any(|a| is_private_addr(a.ip())) {
        return Err(FetchFailure::Blocked);
    }
    Ok(addrs)
}

/// Résolveur du client HTTP : refuse toute résolution vers une IP privée.
struct PublicOnlyResolver;

impl Resolve for PublicOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            match resolve_public(name.as_str()).await {
                Ok(addrs) => Ok(Box::new(addrs.into_iter()) as Addrs),
                Err(_) => Err("résolution vers une adresse non publique refusée".into()),
            }
        })
    }
}

pub struct SafeFetcher {
    client: reqwest::Client,
    /// Les tests E2E servent une page-fixture locale : bascule explicite,
    /// jamais active par défaut (IMPORT_ALLOW_PRIVATE=1).
    allow_private: bool,
    /// Domaine parent du site lui-même (ex. `example.com` pour
    /// `recettes.example.com`) : l'import ne doit pas viser les autres services
    /// du même domaine — la requête y arriverait avec l'IP publique de la
    /// maison, au-delà du geoblock et des listes d'adresses autorisées.
    own_domain: Option<String>,
}

/// Domaine parent d'une URL de base (hôte privé de son premier libellé).
fn parent_domain(base_url: &str) -> Option<String> {
    let host = Url::parse(base_url).ok()?.host_str()?.to_ascii_lowercase();
    let (_, parent) = host.split_once('.')?;
    parent.contains('.').then(|| parent.to_string())
}

impl SafeFetcher {
    pub fn new(allow_private: bool, public_base_url: Option<&str>) -> Self {
        let mut builder = reqwest::Client::builder()
            // Jamais de proxy d'environnement (HTTP_PROXY…) : la garde
            // porterait sur le proxy, pas sur la cible.
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(TIMEOUT)
            .connect_timeout(TIMEOUT);
        if !allow_private {
            builder = builder.dns_resolver(Arc::new(PublicOnlyResolver));
        }
        SafeFetcher {
            client: builder.build().expect("client HTTP"),
            allow_private,
            own_domain: public_base_url.and_then(parent_domain),
        }
    }

    async fn is_url_allowed(&self, url: &Url) -> bool {
        if self.allow_private {
            return matches!(url.scheme(), "https" | "http");
        }
        // https sur le port standard seulement : pas de balayage de ports.
        if url.scheme() != "https" || url.port_or_known_default() != Some(443) {
            return false;
        }
        if let (Some(own), Some(host)) = (&self.own_domain, url.host_str()) {
            // Forme canonique : `url` a déjà mis l'hôte en minuscules et en
            // punycode ; reste le point final (« example.com. » = même hôte).
            let host = host.trim_end_matches('.').to_ascii_lowercase();
            if host == *own || host.ends_with(&format!(".{own}")) {
                return false;
            }
        }
        match url.host() {
            Some(Host::Ipv4(ip)) => !is_private_addr(IpAddr::V4(ip)),
            Some(Host::Ipv6(ip)) => !is_private_addr(IpAddr::V6(ip)),
            Some(Host::Domain(domain)) => resolve_public(domain).await.is_ok(),
            None => false,
        }
    }

    pub async fn fetch(&self, raw_url: &str) -> Result<FetchedPage, FetchFailure> {
        let mut url = Url::parse(raw_url).map_err(|_| FetchFailure::Unsupported)?;

        for redirects in 0..=MAX_REDIRECTS {
            if !self.is_url_allowed(&url).await {
                return Err(FetchFailure::Blocked);
            }
            let mut response = self
                .client
                .get(url.clone())
                .header(USER_AGENT, BOT_USER_AGENT)
                .header(ACCEPT, "text/html,application/xhtml+xml")
                .header(ACCEPT_LANGUAGE, "fr-CA,fr;q=0.9,en;q=0.8")
                .send()
                .await
                .map_err(|_| FetchFailure::Unreachable)?;

            let status = response.status();
            if status.is_redirection() {
                let location = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string);
                match location {
                    Some(location) if redirects < MAX_REDIRECTS => {
                        url = url.join(&location).map_err(|_| FetchFailure::Unreachable)?;
                        continue;
                    }
                    _ => return Err(FetchFailure::Unreachable),
                }
            }
            if !status.is_success() {
                return Err(FetchFailure::Unreachable);
            }

            let content_type = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let is_text = ["text/html", "application/xhtml", "text/plain"]
                .iter()
                .any(|t| content_type.contains(t));
            if !content_type.is_empty() && !is_text {
                return Err(FetchFailure::Unsupported);
            }

            let mut body: Vec<u8> = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| FetchFailure::Unreachable)? {
                body.extend_from_slice(&chunk);
                if body.len() > MAX_BYTES {
                    return Err(FetchFailure::TooLarge);
                }
            }
            return Ok(FetchedPage {
                html: String::from_utf8_lossy(&body).into_owned(),
                final_url: url.to_string(),
            });
        }
        Err(FetchFailure::Unreachable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_privees() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "::1",
            "::",
            "fc00::1",
            "fd12::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::127.0.0.1",         // IPv4 compatible (dépréciée)
            "64:ff9b::a9fe:a9fe",  // NAT64 → 169.254.169.254
            "64:ff9b:1::1",        // NAT64 local
            "2002:7f00:1::",       // 6to4 → 127.0.0.1
            "2001:0:4136:e378::1", // Teredo
            "2001:db8::1",         // documentation
            "fec0::1",             // site local
            "ff02::1",             // multicast
            "192.0.0.8",
            "198.18.0.1",
            "198.19.255.255",
            "203.0.113.7",
            "255.255.255.255",
            "pas-une-ip",
        ] {
            assert!(is_private_ip(ip), "{ip}");
        }
    }

    #[test]
    fn ip_publiques() {
        for ip in [
            "8.8.8.8",
            "1.1.1.1",
            "172.32.0.1",
            "100.128.0.1",
            "2001:4860:4860::8888",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808", // NAT64 → 8.8.8.8
            "2002:808:808::",   // 6to4 → 8.8.8.8
            "198.20.0.1",
        ] {
            assert!(!is_private_ip(ip), "{ip}");
        }
    }

    #[tokio::test]
    async fn schemas_et_ip_litterales_refuses() {
        let fetcher = SafeFetcher::new(false, Some("https://recettes.example.com"));
        assert_eq!(
            fetcher.fetch("http://exemple.com/").await.unwrap_err(),
            FetchFailure::Blocked
        );
        assert_eq!(
            fetcher.fetch("https://127.0.0.1/").await.unwrap_err(),
            FetchFailure::Blocked
        );
        assert_eq!(
            fetcher.fetch("https://[::1]/").await.unwrap_err(),
            FetchFailure::Blocked
        );
        assert_eq!(
            fetcher.fetch("ftp://exemple.com/").await.unwrap_err(),
            FetchFailure::Blocked
        );
        assert_eq!(
            fetcher.fetch("pas une url").await.unwrap_err(),
            FetchFailure::Unsupported
        );
    }

    #[tokio::test]
    async fn port_non_standard_et_propre_domaine_refuses() {
        let fetcher = SafeFetcher::new(false, Some("https://recettes.example.com"));
        for url in [
            "https://1.1.1.1:8443/",
            "https://photos.example.com/",
            "https://example.com/",
            "https://recettes.example.com/",
            "https://photos.example.com./",
            "https://PHOTOS.Example.COM/",
        ] {
            assert_eq!(fetcher.fetch(url).await.unwrap_err(), FetchFailure::Blocked, "{url}");
        }
        assert_eq!(
            parent_domain("https://recettes.example.com"),
            Some("example.com".into())
        );
        assert_eq!(parent_domain("http://localhost:3000"), None);
        assert_eq!(parent_domain("https://example.com"), None);
    }

    #[tokio::test]
    async fn localhost_refuse_par_dns() {
        let fetcher = SafeFetcher::new(false, Some("https://recettes.example.com"));
        assert_eq!(
            fetcher.fetch("https://localhost/").await.unwrap_err(),
            FetchFailure::Blocked
        );
    }
}
