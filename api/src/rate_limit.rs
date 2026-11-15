//! Rate limiting applicatif : fenêtre glissante en mémoire — suffisant pour une
//! instance unique. Complété au périmètre par le middleware Traefik
//! rate-limit-auth@file.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::error::ApiError;
use crate::logger::log_warn;

pub struct RateLimiter {
    max: usize,
    window: Duration,
    hits: HashMap<String, Vec<Instant>>,
    next_sweep_at: Option<Instant>,
}

impl RateLimiter {
    pub fn new(max: usize, window: Duration) -> Self {
        RateLimiter {
            max,
            window,
            hits: HashMap::new(),
            next_sweep_at: None,
        }
    }

    pub fn check_at(&mut self, key: &str, now: Instant) -> bool {
        let window_start = now.checked_sub(self.window);
        let in_window = |t: &Instant| window_start.is_none_or(|start| *t > start);

        // Sans purge, chaque clé vue laisse une entrée pour toujours — mémoire
        // non bornée sur un conteneur limité. Balayage amorti : un par fenêtre.
        if self.next_sweep_at.is_none_or(|at| now >= at) {
            self.next_sweep_at = Some(now + self.window);
            self.hits.retain(|_, times| times.iter().any(in_window));
        }

        let recent = self.hits.entry(key.to_string()).or_default();
        recent.retain(in_window);
        if recent.len() >= self.max {
            return false;
        }
        recent.push(now);
        true
    }

    /// Nombre de clés retenues — observable de la purge pour les tests.
    pub fn size(&self) -> usize {
        self.hits.len()
    }
}

/// Registre des limiteurs, un par « bucket » (ex. `auth:login`).
#[derive(Default)]
pub struct RateLimits {
    buckets: Mutex<HashMap<String, RateLimiter>>,
}

impl RateLimits {
    /// Vérifie et consomme ; 429 `errors.tooManyRequests` si le quota est dépassé.
    pub fn assert(&self, bucket: &str, ip: &str, max: usize, window_ms: u64) -> Result<(), ApiError> {
        let allowed = {
            let mut buckets = self.buckets.lock().expect("verrou rate limit");
            buckets
                .entry(bucket.to_string())
                .or_insert_with(|| RateLimiter::new(max, Duration::from_millis(window_ms)))
                .check_at(ip, Instant::now())
        };
        if allowed {
            Ok(())
        } else {
            log_warn("security.rate_limited", json!({ "bucket": bucket, "ip": ip }));
            Err(ApiError::new(429, "errors.tooManyRequests"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bloque_au_dela_du_max() {
        let mut limiter = RateLimiter::new(2, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(limiter.check_at("ip", t0));
        assert!(limiter.check_at("ip", t0));
        assert!(!limiter.check_at("ip", t0));
        assert!(limiter.check_at("autre", t0));
    }

    #[test]
    fn la_fenetre_glisse() {
        let mut limiter = RateLimiter::new(1, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(limiter.check_at("ip", t0));
        assert!(!limiter.check_at("ip", t0 + Duration::from_secs(59)));
        assert!(limiter.check_at("ip", t0 + Duration::from_secs(61)));
    }

    #[test]
    fn purge_des_cles_inactives() {
        let mut limiter = RateLimiter::new(5, Duration::from_secs(60));
        let t0 = Instant::now();
        for i in 0..10 {
            limiter.check_at(&format!("ip-{i}"), t0);
        }
        assert_eq!(limiter.size(), 10);
        limiter.check_at("nouvelle", t0 + Duration::from_secs(120));
        assert_eq!(limiter.size(), 1);
    }

    #[test]
    fn registre_retourne_429() {
        let limits = RateLimits::default();
        assert!(limits.assert("b", "ip", 1, 60_000).is_ok());
        let error = limits.assert("b", "ip", 1, 60_000).unwrap_err();
        assert_eq!(error.status.as_u16(), 429);
        assert_eq!(error.message, "errors.tooManyRequests");
    }
}
