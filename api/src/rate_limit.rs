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

    /// Le quota de `key` est-il épuisé ? Ne consomme rien.
    pub fn is_exhausted_at(&self, key: &str, now: Instant) -> bool {
        let window_start = now.checked_sub(self.window);
        self.hits.get(key).is_some_and(|times| {
            times
                .iter()
                .filter(|t| window_start.is_none_or(|start| **t > start))
                .count()
                >= self.max
        })
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

    /// Verrou par compte sur les ÉCHECS d'authentification (en plus du quota
    /// par IP, contournable par rotation d'adresses) : 429 dès que `key` a
    /// accumulé `max` échecs dans la fenêtre. Ne consomme rien — les
    /// connexions réussies ne comptent pas (voir `record_failure`).
    pub fn assert_not_locked(&self, bucket: &str, key: &str, max: usize, window_ms: u64) -> Result<(), ApiError> {
        let locked = {
            let mut buckets = self.buckets.lock().expect("verrou rate limit");
            buckets
                .entry(bucket.to_string())
                .or_insert_with(|| RateLimiter::new(max, Duration::from_millis(window_ms)))
                .is_exhausted_at(key, Instant::now())
        };
        if locked {
            // La clé (courriel ou userId) n'est pas journalisée ici : l'appelant
            // a déjà consigné chaque échec avec son contexte.
            log_warn("security.account_locked", json!({ "bucket": bucket }));
            Err(ApiError::new(429, "errors.tooManyRequests"))
        } else {
            Ok(())
        }
    }

    /// Consigne un échec d'authentification pour `key`.
    pub fn record_failure(&self, bucket: &str, key: &str, max: usize, window_ms: u64) {
        let mut buckets = self.buckets.lock().expect("verrou rate limit");
        buckets
            .entry(bucket.to_string())
            .or_insert_with(|| RateLimiter::new(max, Duration::from_millis(window_ms)))
            .check_at(key, Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verrou_par_compte_sur_les_echecs_seulement() {
        let limits = RateLimits::default();
        for _ in 0..3 {
            assert!(limits.assert_not_locked("login:compte", "a@b.c", 3, 60_000).is_ok());
            limits.record_failure("login:compte", "a@b.c", 3, 60_000);
        }
        assert_eq!(
            limits
                .assert_not_locked("login:compte", "a@b.c", 3, 60_000)
                .unwrap_err()
                .status
                .as_u16(),
            429
        );
        // Vérifier ne consomme rien : un autre compte reste libre indéfiniment.
        for _ in 0..10 {
            assert!(limits.assert_not_locked("login:compte", "autre@b.c", 3, 60_000).is_ok());
        }
    }

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
