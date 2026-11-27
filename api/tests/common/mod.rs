//! Outillage des tests d'intégration : chaque test reçoit sa PROPRE base,
//! clonée (CREATE DATABASE … TEMPLATE) d'un gabarit migré une fois par
//! version des migrations. Isolation totale et exécution parallèle.
//! Nécessite la DB de dev démarrée (docker compose -f docker-compose.dev.yml up -d)
//! ou le service PostgreSQL de la CI ; URL : TEST_DATABASE_URL.

#![allow(dead_code)]

use std::sync::atomic::{AtomicU32, Ordering};

use chrono::{DateTime, Utc};
use hebdomiam::crypto::sha256_hex;
use hebdomiam::domain::catalog::Role;
use hebdomiam::migrate;
use hebdomiam::repo::recipes::{self, IngredientInput, RecipeInput};
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool};
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

static TEMPLATE: OnceCell<String> = OnceCell::const_new();
static CREATE_LOCK: Mutex<()> = Mutex::const_new(());
static COUNTER: AtomicU32 = AtomicU32::new(0);

fn base_url() -> url::Url {
    let raw = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://hebdomiam:hebdomiam@localhost:5432/hebdomiam_test".to_string());
    url::Url::parse(&raw).expect("TEST_DATABASE_URL valide")
}

fn url_for(database: &str) -> String {
    let mut url = base_url();
    url.set_path(&format!("/{database}"));
    url.to_string()
}

/// `CREATE DATABASE` et `DROP DATABASE` n'acceptent pas de paramètre lié : le
/// nom de base est interpolé dans le texte. Sûr ici — ces noms sont fabriqués
/// par ce module (empreinte hexadécimale des migrations, pid, compteur, UUID),
/// jamais lus d'une entrée.
fn database_ddl(sql: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(sql)
}

async fn admin_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&url_for("postgres"))
        .await
        .expect("connexion PostgreSQL (DB de dev démarrée ?)")
}

/// Gabarit migré, nommé d'après l'empreinte des migrations.
async fn template() -> &'static str {
    TEMPLATE
        .get_or_init(|| async {
            let fingerprint: String = migrate::embedded().iter().map(|m| m.checksum()).collect();
            let name = format!("hebdomiam_tpl_{}", &sha256_hex(&fingerprint)[..12]);
            let admin = admin_pool().await;
            // Verrou inter-processus : les binaires de test peuvent se chevaucher.
            sqlx::query("SELECT pg_advisory_lock(777)")
                .execute(&admin)
                .await
                .unwrap();
            let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
                .bind(&name)
                .fetch_one(&admin)
                .await
                .unwrap();
            if !exists {
                sqlx::raw_sql(database_ddl(format!("CREATE DATABASE \"{name}\"")))
                    .execute(&admin)
                    .await
                    .unwrap();
                let pool = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&url_for(&name))
                    .await
                    .unwrap();
                migrate::run(&pool).await.expect("migrations du gabarit");
                pool.close().await;
            }
            sqlx::query("SELECT pg_advisory_unlock(777)")
                .execute(&admin)
                .await
                .unwrap();
            admin.close().await;
            name
        })
        .await
}

pub struct TestDb {
    pub pool: PgPool,
    name: String,
}

impl TestDb {
    pub async fn new() -> TestDb {
        let template = template().await;
        let name = format!(
            "hebdomiam_t_{}_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
            &Uuid::new_v4().simple().to_string()[..8]
        );
        {
            let _guard = CREATE_LOCK.lock().await;
            let admin = admin_pool().await;
            sqlx::raw_sql(database_ddl(format!(
                "CREATE DATABASE \"{name}\" TEMPLATE \"{template}\""
            )))
            .execute(&admin)
            .await
            .unwrap();
            admin.close().await;
        }
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&url_for(&name))
            .await
            .unwrap();
        TestDb { pool, name }
    }

    /// URL de connexion de la base du test.
    pub fn url(&self) -> String {
        url_for(&self.name)
    }

    /// Supprime la base du test (à appeler en fin de test ; une base orpheline
    /// d'un test en échec est inoffensive et disparaît avec le volume de dev).
    pub async fn drop(self) {
        self.pool.close().await;
        let admin = admin_pool().await;
        let _ = sqlx::raw_sql(database_ddl(format!(
            "DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&admin)
        .await;
        admin.close().await;
    }
}

pub struct UserOptions {
    pub role: Role,
    pub verified: bool,
    pub disabled: bool,
    /// Compte invité dont l'invitation n'est pas encore acceptée.
    pub invite_pending: bool,
    pub password_hash: String,
}

impl Default for UserOptions {
    fn default() -> Self {
        UserOptions {
            role: Role::User,
            verified: true,
            disabled: false,
            invite_pending: false,
            password_hash: "$argon2id$fake-hash-for-tests".into(),
        }
    }
}

pub async fn create_user(db: &PgPool, options: UserOptions) -> Uuid {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let now: Option<DateTime<Utc>> = Some(Utc::now());
    sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, name, role, email_verified_at, disabled_at, invite_pending)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(format!("test-{n}-{}@example.com", Uuid::new_v4().simple()))
    .bind(&options.password_hash)
    .bind(format!("Testeur {n}"))
    .bind(options.role)
    .bind(if options.verified { now } else { None })
    .bind(if options.disabled { now } else { None })
    .bind(options.invite_pending)
    .fetch_one(db)
    .await
    .unwrap()
}

pub async fn user(db: &PgPool) -> Uuid {
    create_user(db, UserOptions::default()).await
}

pub async fn admin(db: &PgPool) -> Uuid {
    create_user(
        db,
        UserOptions {
            role: Role::Admin,
            ..Default::default()
        },
    )
    .await
}

pub fn recipe_input(title: &str, tags: &[&str]) -> RecipeInput {
    RecipeInput {
        title: title.to_string(),
        description: None,
        image_url: None,
        base_servings: 4,
        prep_minutes: Some(10),
        cook_minutes: Some(20),
        source_url: None,
        freezable: false,
        notes: None,
        ingredients: vec![IngredientInput {
            quantity: 200.0,
            unit: Some(hebdomiam::domain::catalog::Unit::G),
            name: "riz basmati".into(),
            aisle: hebdomiam::domain::catalog::Aisle::Dry,
        }],
        steps: vec!["Cuire.".into()],
        tags: tags.iter().map(|t| t.to_string()).collect(),
    }
}

pub async fn recipe(db: &PgPool, user_id: Uuid, title: &str) -> Uuid {
    recipes::create(db, user_id, &recipe_input(title, &[]))
        .await
        .unwrap()
        .summary
        .id
}
