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
use sqlx::{AssertSqlSafe, PgPool, Row};

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
                // Texte SQL sûr sans être `&'static str` : c'est le contenu d'un
                // fichier de `api/migrations/`, embarqué dans le binaire à la
                // compilation (`include_dir!`) — aucune entrée d'exécution.
                sqlx::raw_sql(AssertSqlSafe(migration.sql.as_str()))
                    .execute(&mut *tx)
                    .await?;
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

/// Rôle PostgreSQL de l'application (PRD §8 : privilèges minimaux).
pub const APP_ROLE: &str = "hebdomiam_app";

/// Crée ou met à jour le rôle applicatif : lecture et écriture des données,
/// rien d'autre — ni DDL, ni superutilisateur (une injection SQL ou une fuite
/// de DATABASE_URL ne donne ni `COPY … PROGRAM` ni modification du schéma).
/// Exécuté par le job `migrate` (connecté en propriétaire) quand
/// APP_DB_PASSWORD est défini ; le serveur se connecte ensuite avec ce rôle.
pub async fn ensure_app_role(pool: &PgPool, password: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(
        "DO $$ BEGIN
           IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'hebdomiam_app') THEN
             CREATE ROLE hebdomiam_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
           END IF;
         END $$;",
    )
    .execute(&mut *tx)
    .await?;
    // Les instructions utilitaires n'acceptent pas de paramètres liés : le
    // serveur compose lui-même l'instruction, littéral et identifiant échappés.
    let statements: (String, String) = sqlx::query_as(
        "SELECT format('ALTER ROLE hebdomiam_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE PASSWORD %L', $1::text),
                format('GRANT CONNECT ON DATABASE %I TO hebdomiam_app', current_database())",
    )
    .bind(password)
    .fetch_one(&mut *tx)
    .await?;
    // Texte SQL sûr : c'est PostgreSQL qui vient de composer ces deux
    // instructions par `format()`, le mot de passe échappé en littéral (%L) et
    // le nom de la base en identifiant (%I). Rien n'est concaténé côté Rust, et
    // le mot de passe vient de la configuration du déploiement, pas d'un client.
    sqlx::raw_sql(AssertSqlSafe(statements.0)).execute(&mut *tx).await?;
    sqlx::raw_sql(AssertSqlSafe(statements.1)).execute(&mut *tx).await?;
    sqlx::raw_sql(
        "GRANT USAGE ON SCHEMA public TO hebdomiam_app;
         GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO hebdomiam_app;
         ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO hebdomiam_app;
         REVOKE ALL ON hebdomiam_migrations FROM hebdomiam_app;",
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    log_event("db.app_role_ready", json!({ "role": APP_ROLE }));
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
