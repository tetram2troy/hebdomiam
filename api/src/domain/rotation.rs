//! Rotation intelligente (PRD F5) — fonctions pures, sans DB.
//! Règles : exclure ce qui a été cuisiné dans les X dernières semaines,
//! prioriser jamais-cuisinées puis les plus anciennement cuisinées, maximiser
//! la variété de tags au sein d'une même semaine. Repli si la collection est
//! trop petite.

use std::cmp::Ordering;
use std::collections::HashSet;

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RotationCandidate {
    pub id: Uuid,
    pub tags: Vec<String>,
    pub last_cooked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub struct RotationOptions<'a> {
    pub count: usize,
    pub rotation_weeks: i32,
    pub exclude_ids: Option<&'a HashSet<Uuid>>,
    pub now: DateTime<Utc>,
}

/// Portions totales et par plat — dîners + soupers en V1 (décision D7).
pub fn compute_portions(days: i32, people: i32, dishes: i32) -> (i32, i32) {
    const MEALS_PER_DAY: i32 = 2;
    let total = days * MEALS_PER_DAY * people;
    let per_dish = ((total + dishes - 1) / dishes).max(1);
    (total, per_dish)
}

/// Lundi de la semaine contenant `date`.
pub fn monday_of(date: NaiveDate) -> NaiveDate {
    date - Duration::days(i64::from(date.weekday().num_days_from_monday()))
}

/// Lundi de la semaine courante, au fuseau local du serveur (TZ).
pub fn current_week() -> NaiveDate {
    monday_of(chrono::Local::now().date_naive())
}

/// Jamais cuisinées d'abord (les plus anciennes de la collection en premier),
/// puis par dernière cuisson la plus lointaine. Départage stable par id.
fn by_priority(a: &RotationCandidate, b: &RotationCandidate) -> Ordering {
    match (a.last_cooked_at, b.last_cooked_at) {
        (None, Some(_)) => return Ordering::Less,
        (Some(_), None) => return Ordering::Greater,
        (Some(x), Some(y)) if x != y => return x.cmp(&y),
        _ => {}
    }
    a.created_at
        .cmp(&b.created_at)
        .then_with(|| a.id.hyphenated().to_string().cmp(&b.id.hyphenated().to_string()))
}

fn tag_overlap(tags: &[String], picked: &HashSet<String>) -> usize {
    tags.iter().filter(|tag| picked.contains(*tag)).count()
}

/// Retourne jusqu'à `count` ids de recettes, dans l'ordre de suggestion.
pub fn pick_suggestions(candidates: &[RotationCandidate], options: &RotationOptions) -> Vec<Uuid> {
    if options.count == 0 {
        return Vec::new();
    }
    let cutoff = options.now - Duration::weeks(i64::from(options.rotation_weeks));
    let eligible: Vec<&RotationCandidate> = candidates
        .iter()
        .filter(|c| options.exclude_ids.is_none_or(|ids| !ids.contains(&c.id)))
        .collect();
    let fresh: Vec<&RotationCandidate> = eligible
        .iter()
        .copied()
        .filter(|c| c.last_cooked_at.is_none_or(|at| at <= cutoff))
        .collect();

    // Repli : collection trop petite pour respecter la rotation → on repêche
    // les cuisinées récemment (le tri les met naturellement en dernier).
    let mut pool = if fresh.len() >= options.count { fresh } else { eligible };
    pool.sort_by(|a, b| by_priority(a, b));

    let mut picked = Vec::new();
    let mut picked_tags: HashSet<String> = HashSet::new();
    while picked.len() < options.count && !pool.is_empty() {
        let mut best_index = 0;
        let mut best_overlap = usize::MAX;
        for (index, candidate) in pool.iter().enumerate() {
            let overlap = tag_overlap(&candidate.tags, &picked_tags);
            if overlap == 0 {
                best_index = index;
                break;
            }
            if overlap < best_overlap {
                best_overlap = overlap;
                best_index = index;
            }
        }
        let choice = pool.remove(best_index);
        picked_tags.extend(choice.tags.iter().cloned());
        picked.push(choice.id);
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 3, 12, 0, 0).unwrap()
    }

    fn days_ago(n: i64) -> DateTime<Utc> {
        now() - Duration::days(n)
    }

    fn candidate(f: impl FnOnce(&mut RotationCandidate)) -> RotationCandidate {
        let n = COUNTER.fetch_add(1, AtomicOrdering::SeqCst) + 1;
        let mut c = RotationCandidate {
            id: Uuid::from_u128(n as u128),
            tags: vec![],
            last_cooked_at: None,
            created_at: days_ago(100),
        };
        f(&mut c);
        c
    }

    fn pick(candidates: &[RotationCandidate], count: usize) -> Vec<Uuid> {
        pick_suggestions(
            candidates,
            &RotationOptions {
                count,
                rotation_weeks: 3,
                exclude_ids: None,
                now: now(),
            },
        )
    }

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn portions() {
        assert_eq!(compute_portions(7, 1, 4), (14, 4));
        assert_eq!(compute_portions(5, 2, 3), (20, 7));
        assert_eq!(compute_portions(1, 1, 4).1, 1);
    }

    #[test]
    fn exclut_les_recentes() {
        let recent = candidate(|c| c.last_cooked_at = Some(days_ago(7)));
        let old = candidate(|c| c.last_cooked_at = Some(days_ago(30)));
        let never = candidate(|_| {});
        let picked = pick(&[recent.clone(), old, never], 2);
        assert_eq!(picked.len(), 2);
        assert!(!picked.contains(&recent.id));
    }

    #[test]
    fn borne_exacte_admissible() {
        let boundary = candidate(|c| c.last_cooked_at = Some(days_ago(21)));
        assert_eq!(pick(std::slice::from_ref(&boundary), 1), vec![boundary.id]);
    }

    #[test]
    fn repli_collection_trop_petite() {
        let yesterday = candidate(|c| c.last_cooked_at = Some(days_ago(1)));
        let last_week = candidate(|c| c.last_cooked_at = Some(days_ago(8)));
        assert_eq!(
            pick(&[yesterday.clone(), last_week.clone()], 2),
            vec![last_week.id, yesterday.id]
        );
    }

    #[test]
    fn ids_exclus() {
        let a = candidate(|_| {});
        let b = candidate(|_| {});
        let exclude: HashSet<Uuid> = [a.id].into();
        let picked = pick_suggestions(
            &[a, b.clone()],
            &RotationOptions {
                count: 2,
                rotation_weeks: 3,
                exclude_ids: Some(&exclude),
                now: now(),
            },
        );
        assert_eq!(picked, vec![b.id]);
    }

    #[test]
    fn jamais_cuisinees_d_abord() {
        let cooked = candidate(|c| c.last_cooked_at = Some(days_ago(300)));
        let never = candidate(|_| {});
        assert_eq!(pick(&[cooked, never.clone()], 1), vec![never.id]);
    }

    #[test]
    fn plus_anciennement_cuisinee_d_abord() {
        let a = candidate(|c| c.last_cooked_at = Some(days_ago(40)));
        let b = candidate(|c| c.last_cooked_at = Some(days_ago(90)));
        assert_eq!(pick(&[a.clone(), b.clone()], 2), vec![b.id, a.id]);
    }

    #[test]
    fn deterministe() {
        let a = candidate(|c| c.created_at = days_ago(10));
        let b = candidate(|c| c.created_at = days_ago(50));
        for _ in 0..3 {
            assert_eq!(pick(&[a.clone(), b.clone()], 2), vec![b.id, a.id]);
        }
    }

    #[test]
    fn variete_de_tags() {
        let pates1 = candidate(|c| {
            c.tags = tags(&["pâtes"]);
            c.created_at = days_ago(90);
        });
        let pates2 = candidate(|c| {
            c.tags = tags(&["pâtes"]);
            c.created_at = days_ago(80);
        });
        let soupe = candidate(|c| {
            c.tags = tags(&["soupe"]);
            c.created_at = days_ago(70);
        });
        assert_eq!(
            pick(&[pates1.clone(), pates2, soupe.clone()], 2),
            vec![pates1.id, soupe.id]
        );
    }

    #[test]
    fn chevauchement_minimal() {
        let italien = candidate(|c| {
            c.tags = tags(&["pâtes", "italien"]);
            c.created_at = days_ago(90);
        });
        let doublon = candidate(|c| {
            c.tags = tags(&["pâtes", "italien"]);
            c.created_at = days_ago(80);
        });
        let cousin = candidate(|c| {
            c.tags = tags(&["pâtes"]);
            c.created_at = days_ago(70);
        });
        assert_eq!(
            pick(&[italien.clone(), doublon, cousin.clone()], 2),
            vec![italien.id, cousin.id]
        );
    }

    #[test]
    fn sans_tag_ne_bloque_pas() {
        assert_eq!(pick(&[candidate(|_| {}), candidate(|_| {})], 2).len(), 2);
    }

    #[test]
    fn lundi_de_la_semaine() {
        let monday = NaiveDate::from_ymd_opt(2026, 6, 29).unwrap();
        assert_eq!(monday_of(NaiveDate::from_ymd_opt(2026, 7, 3).unwrap()), monday);
        assert_eq!(monday_of(NaiveDate::from_ymd_opt(2026, 7, 5).unwrap()), monday);
        assert_eq!(monday_of(monday), monday);
    }

    #[test]
    fn bords() {
        let a = candidate(|_| {});
        let b = candidate(|_| {});
        let mut picked = pick(&[a.clone(), b.clone()], 4);
        picked.sort();
        let mut expected = vec![a.id, b.id];
        expected.sort();
        assert_eq!(picked, expected);
        assert!(pick(&[], 4).is_empty());
        assert!(pick(&[candidate(|_| {})], 0).is_empty());
    }
}
