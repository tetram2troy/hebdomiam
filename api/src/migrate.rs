//! Migrations SQL embarquées dans le binaire (`api/migrations/*.sql`, ordre
//! lexical), exécutées en job dédié au déploiement (PRD §7.3) :
//! `hebdomiam migrate` — jamais au démarrage du serveur.
//!
//! Reprise de l'historique Drizzle : sur une base migrée par l'ancien backend
//! (table `drizzle.__drizzle_migrations`), les N premières migrations déjà
//! appliquées par Drizzle sont marquées comme faites sans être rejouées.
//! Les fichiers 0000 à 0006 sont ceux générés par drizzle-kit, inchangés.

use include_dir::{Dir, include_dir};
use serde_json::json;
use sqlx::{PgPool, Row};

use crate::crypto::sha256_hex;
use crate::logger::{log_event, log_warn};

static MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

/// Verrou consultatif : deux jobs de migration simultanés s'attendent.
const LOCK_KEY: i64 = 0x4865_6264_6F6D_6961; // « Hebdomia »

pub struct Migration {
    pub name: String,
    pub sql: String,
}

impl Migration {
    /// Empreinte insensible aux fins de ligne (checkout Windows vs Linux).
    pub fn checksum(&self) -> String {
        sha256_hex(&self.sql.replace("\r\n", "\n"))
    }
}

pub fn embedded() -> Vec<Migration> {
    let mut migrations: Vec<Migration> = MIGRATIONS
        .files()
        .filter(|file| file.path().extension().is_some_and(|ext| ext == "sql"))
        .map(|file| Migration {
            name: file
                .path()
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            sql: file.contents_utf8().expect("migration UTF-8").to_string(),
        })
        .collect();
    migrations.sort_by(|a, b| a.name.cmp(&b.name));
    migrations
}

/// Applique les migrations en attente ; retourne les noms appliqués.
pub async fn run(pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    let mut conn = pool.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(LOCK_KEY)
        .execute(&mut *conn)
        .await?;
    let result = run_locked(&mut conn).await;
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(LOCK_KEY)
        .execute(&mut *conn)
        .await?;
    result
}

async fn run_locked(conn: &mut sqlx::PgConnection) -> Result<Vec<String>, sqlx::Error> {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS hebdomiam_migrations (
            name text PRIMARY KEY,
            checksum text NOT NULL,
            applied_at timestamptz NOT NULL DEFAULT now()
        )",
    )
    .execute(&mut *conn)
    .await?;

    let migrations = embedded();
    baseline_from_drizzle(conn, &migrations).await?;

    let applied: Vec<(String, String)> = sqlx::query_as("SELECT name, checksum FROM hebdomiam_migrations")
        .fetch_all(&mut *conn)
        .await?;

    let mut newly_applied = Vec::new();
    for migration in &migrations {
        match applied.iter().find(|(name, _)| *name == migration.name) {
            Some((_, checksum)) => {
                if *checksum != migration.checksum() {
                    log_warn("db.migration_checksum_mismatch", json!({ "migration": migration.name }));
                }
            }
            None => {
                let mut tx = sqlx::Connection::begin(&mut *conn).await?;
                sqlx::raw_sql(&migration.sql).execute(&mut *tx).await?;
                sqlx::query("INSERT INTO hebdomiam_migrations (name, checksum) VALUES ($1, $2)")
                    .bind(&migration.name)
                    .bind(migration.checksum())
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                log_event("db.migration_applied", json!({ "migration": migration.name }));
                newly_applied.push(migration.name.clone());
            }
        }
    }
    Ok(newly_applied)
}

/// Base gérée jusqu'ici par Drizzle : reprend son historique une seule fois.
async fn baseline_from_drizzle(conn: &mut sqlx::PgConnection, migrations: &[Migration]) -> Result<(), sqlx::Error> {
    let already: i64 = sqlx::query_scalar("SELECT count(*) FROM hebdomiam_migrations")
        .fetch_one(&mut *conn)
        .await?;
    if already > 0 {
        return Ok(());
    }
    let drizzle_table: Option<String> = sqlx::query_scalar("SELECT to_regclass('drizzle.__drizzle_migrations')::text")
        .fetch_one(&mut *conn)
        .await?;
    if drizzle_table.is_none() {
        return Ok(());
    }
    let row = sqlx::query("SELECT count(*) AS n FROM drizzle.__drizzle_migrations")
        .fetch_one(&mut *conn)
        .await?;
    let drizzle_count: i64 = row.get("n");
    let count = usize::try_from(drizzle_count).unwrap_or(0).min(migrations.len());
    for migration in &migrations[..count] {
        sqlx::query("INSERT INTO hebdomiam_migrations (name, checksum) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(&migration.name)
            .bind(migration.checksum())
            .execute(&mut *conn)
            .await?;
    }
    log_event("db.migrations_baselined_from_drizzle", json!({ "count": count }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_embarquees_dans_l_ordre() {
        let names: Vec<_> = embedded().into_iter().map(|m| m.name).collect();
        assert!(names.len() >= 7);
        assert_eq!(names[0], "0000_normal_felicia_hardy");
        assert!(names.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn empreinte_independante_des_fins_de_ligne() {
        let lf = Migration {
            name: "x".into(),
            sql: "SELECT 1;\nSELECT 2;".into(),
        };
        let crlf = Migration {
            name: "x".into(),
            sql: "SELECT 1;\r\nSELECT 2;".into(),
        };
        assert_eq!(lf.checksum(), crlf.checksum());
    }
}
