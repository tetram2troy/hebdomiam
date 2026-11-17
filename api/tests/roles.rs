//! Rôle PostgreSQL applicatif à privilèges minimaux (PRD §8).

mod common;

use common::TestDb;
use hebdomiam::migrate::{APP_ROLE, ensure_app_role};
use sqlx::postgres::PgPoolOptions;

#[tokio::test]
async fn role_applicatif_lit_et_ecrit_sans_pouvoir_toucher_au_schema() {
    let t = TestDb::new().await;
    let password = "mot-de-passe-du-role-applicatif";
    ensure_app_role(&t.pool, password).await.unwrap();
    ensure_app_role(&t.pool, password).await.unwrap(); // idempotent

    let mut url = url::Url::parse(&t.url()).unwrap();
    url.set_username(APP_ROLE).unwrap();
    url.set_password(Some(password)).unwrap();
    let app = PgPoolOptions::new()
        .max_connections(1)
        .connect(url.as_str())
        .await
        .unwrap();

    // Données : lecture et écriture.
    sqlx::query("INSERT INTO users (email, password_hash, name) VALUES ('role@example.com', 'x', 'R')")
        .execute(&app)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&app)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Schéma, superpouvoirs et table des migrations : refusés.
    for forbidden in [
        "CREATE TABLE piege (id int)",
        "DROP TABLE recipes",
        "ALTER TABLE users ADD COLUMN piege int",
        "COPY users TO PROGRAM 'id'",
        "SELECT * FROM hebdomiam_migrations",
    ] {
        assert!(sqlx::raw_sql(forbidden).execute(&app).await.is_err(), "{forbidden}");
    }
    let superuser: bool = sqlx::query_scalar("SELECT rolsuper FROM pg_roles WHERE rolname = current_user")
        .fetch_one(&app)
        .await
        .unwrap();
    assert!(!superuser);

    app.close().await;
    t.drop().await;
}
