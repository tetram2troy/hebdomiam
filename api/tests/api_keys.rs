//! Clés d'API : gestion scopée par compte (IDOR), authentification Bearer
//! relue en base, portée limitée aux routes /api/v1/*.

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{TestDb, UserOptions, create_user};
use hebdomiam::app;
use hebdomiam::config::Config;
use hebdomiam::crypto::{hash_password, sha256_hex};
use hebdomiam::repo::api_keys::{self, CreateApiKey, MAX_KEYS_PER_USER};
use hebdomiam::state::AppState;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const PASSWORD: &str = "mot-de-passe-tres-long-123";

async fn account(pool: &PgPool) -> (Uuid, String) {
    let id = create_user(
        pool,
        UserOptions {
            password_hash: hash_password(PASSWORD.into()).await,
            ..Default::default()
        },
    )
    .await;
    let email = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    (id, email)
}

async fn new_key(pool: &PgPool, user_id: Uuid, name: &str) -> api_keys::NewApiKey {
    match api_keys::create(pool, user_id, PASSWORD, name).await.unwrap() {
        CreateApiKey::Created(created) => created,
        _ => panic!("création de clé attendue"),
    }
}

// ── Couche repo ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn creation_liste_revocation_et_isolation() {
    let t = TestDb::new().await;
    let (alice, _) = account(&t.pool).await;
    let (bob, _) = account(&t.pool).await;

    let created = new_key(&t.pool, alice, "Claude Code").await;
    assert!(created.token.starts_with(&created.key.prefix));
    // Seule l'empreinte est stockée : le secret n'est nulle part en base.
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM api_keys WHERE id = $1")
        .bind(created.key.id)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(stored, sha256_hex(&created.token));

    let listed = api_keys::list(&t.pool, alice).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "Claude Code");
    assert!(listed[0].last_used_at.is_none());

    // IDOR : Bob ne voit ni ne révoque la clé d'Alice.
    assert!(api_keys::list(&t.pool, bob).await.unwrap().is_empty());
    assert!(!api_keys::revoke(&t.pool, bob, created.key.id).await.unwrap());
    assert_eq!(api_keys::list(&t.pool, alice).await.unwrap().len(), 1);

    // L'authentification retrouve Alice et horodate l'usage.
    let owner = api_keys::authenticate(&t.pool, &sha256_hex(&created.token))
        .await
        .unwrap()
        .expect("clé valide");
    assert_eq!((owner.user_id, owner.key_id), (alice, created.key.id));
    assert!(api_keys::list(&t.pool, alice).await.unwrap()[0].last_used_at.is_some());

    assert!(api_keys::revoke(&t.pool, alice, created.key.id).await.unwrap());
    assert!(
        api_keys::authenticate(&t.pool, &sha256_hex(&created.token))
            .await
            .unwrap()
            .is_none()
    );
    t.drop().await;
}

#[tokio::test]
async fn mot_de_passe_exige_et_plafond_par_compte() {
    let t = TestDb::new().await;
    let (alice, _) = account(&t.pool).await;

    assert!(matches!(
        api_keys::create(&t.pool, alice, "mauvais-mot-de-passe", "x")
            .await
            .unwrap(),
        CreateApiKey::InvalidPassword
    ));
    assert!(matches!(
        api_keys::create(&t.pool, Uuid::new_v4(), PASSWORD, "x").await.unwrap(),
        CreateApiKey::NotFound
    ));
    for i in 0..MAX_KEYS_PER_USER {
        new_key(&t.pool, alice, &format!("clé {i}")).await;
    }
    assert!(matches!(
        api_keys::create(&t.pool, alice, PASSWORD, "de trop").await.unwrap(),
        CreateApiKey::LimitReached
    ));
    t.drop().await;
}

#[tokio::test]
async fn compte_desactive_ou_supprime_refuse() {
    let t = TestDb::new().await;
    let (alice, _) = account(&t.pool).await;
    let created = new_key(&t.pool, alice, "poste").await;
    let hash = sha256_hex(&created.token);

    sqlx::query("UPDATE users SET disabled_at = now() WHERE id = $1")
        .bind(alice)
        .execute(&t.pool)
        .await
        .unwrap();
    assert!(api_keys::authenticate(&t.pool, &hash).await.unwrap().is_none());

    sqlx::query("UPDATE users SET disabled_at = NULL WHERE id = $1")
        .bind(alice)
        .execute(&t.pool)
        .await
        .unwrap();
    assert!(api_keys::authenticate(&t.pool, &hash).await.unwrap().is_some());

    // Suppression du compte : les clés partent en cascade.
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(alice)
        .execute(&t.pool)
        .await
        .unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM api_keys")
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
    t.drop().await;
}

// ── Frontière HTTP ──────────────────────────────────────────────────────────

fn router(pool: &PgPool) -> Router {
    app::build(AppState::new(pool.clone(), Config::for_tests("postgresql://unused")))
}

enum Auth<'a> {
    None,
    Cookie(&'a str),
    Bearer(&'a str),
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    auth: Auth<'_>,
    body: Option<Value>,
    ip: &str,
) -> (StatusCode, Value, Option<String>) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            40000,
        ))))
        .header("x-forwarded-for", ip)
        .header("host", "127.0.0.1:3000");
    request = match auth {
        Auth::None => request,
        Auth::Cookie(cookie) => request.header("cookie", format!("__Host-hebdomiam-session={cookie}")),
        Auth::Bearer(token) => request.header("authorization", format!("Bearer {token}")),
    };
    let request = match body {
        Some(json) => request
            .header("content-type", "application/json")
            .body(Body::from(json.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let cookie = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|v| {
            v.strip_prefix("__Host-hebdomiam-session=")
                .map(|rest| rest.split(';').next().unwrap_or("").to_string())
        });
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null), cookie)
}

async fn login(app: &Router, email: &str, ip: &str) -> String {
    let (status, body, cookie) = call(
        app,
        "POST",
        "/api/auth/login",
        Auth::None,
        Some(json!({ "email": email, "password": PASSWORD })),
        ip,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    cookie.expect("cookie de session")
}

fn recipe_body() -> Value {
    json!({
        "title": "One pot haricots crémeux et saucisses",
        "baseServings": 4,
        "prepMinutes": 15,
        "cookMinutes": 30,
        "sourceUrl": "https://ici.radio-canada.ca/recettes/9398/one-pot-haricots-cremeux-saucisses",
        "ingredients": [
            { "quantity": 540, "unit": "ml", "name": "haricots blancs", "aisle": "dry" },
            { "quantity": 4, "unit": "unit", "name": "saucisses" }
        ],
        "steps": ["Dorer les saucisses.", "Ajouter les haricots et mijoter."],
        "tags": ["one pot", "réconfort"]
    })
}

#[tokio::test]
async fn cle_api_de_bout_en_bout() {
    let t = TestDb::new().await;
    let app = router(&t.pool);
    let ip = "198.51.100.40";
    let (_, alice_email) = account(&t.pool).await;
    let (_, bob_email) = account(&t.pool).await;
    let alice = login(&app, &alice_email, ip).await;
    let bob = login(&app, &bob_email, "198.51.100.41").await;

    // Création : mot de passe revérifié.
    let (status, body, _) = call(
        &app,
        "POST",
        "/api/api-keys",
        Auth::Cookie(&alice),
        Some(json!({ "name": "Claude Code", "password": "faux" })),
        ip,
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::FORBIDDEN, Some("errors.invalidPassword"))
    );
    let (status, created, _) = call(
        &app,
        "POST",
        "/api/api-keys",
        Auth::Cookie(&alice),
        Some(json!({ "name": "  Claude Code  ", "password": PASSWORD })),
        ip,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["key"]["name"], json!("Claude Code"));
    let token = created["token"].as_str().unwrap().to_string();
    let key_id = created["key"]["id"].as_str().unwrap().to_string();

    // La liste ne rend jamais le secret.
    let (_, listed, _) = call(&app, "GET", "/api/api-keys", Auth::Cookie(&alice), None, ip).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(!listed.to_string().contains(&token));
    assert!(listed[0].get("token").is_none());

    // Une clé n'est pas une session : sans en-tête, ou sur une route de session → 401.
    assert_eq!(
        call(&app, "POST", "/api/v1/recipes", Auth::None, Some(recipe_body()), ip)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/api/recipes", Auth::Bearer(&token), None, ip)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/api/api-keys", Auth::Bearer(&token), None, ip)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    // À l'inverse, le cookie seul n'ouvre pas les routes /v1.
    assert_eq!(
        call(&app, "GET", "/api/v1/tags", Auth::Cookie(&alice), None, ip)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    // Création de recette par la clé : même validation que l'app.
    let mut invalid = recipe_body();
    invalid["steps"] = json!([]);
    assert_eq!(
        call(&app, "POST", "/api/v1/recipes", Auth::Bearer(&token), Some(invalid), ip)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (status, recipe, _) = call(
        &app,
        "POST",
        "/api/v1/recipes",
        Auth::Bearer(&token),
        Some(recipe_body()),
        ip,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{recipe}");
    let recipe_id = recipe["id"].as_str().unwrap();
    assert_eq!(recipe["tags"], json!(["one pot", "réconfort"]));

    // La recette appartient à Alice — Bob ne la voit pas (IDOR).
    let path = format!("/api/recipes/{recipe_id}");
    assert_eq!(
        call(&app, "GET", &path, Auth::Cookie(&alice), None, ip).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "GET", &path, Auth::Cookie(&bob), None, "198.51.100.41")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, tags, _) = call(&app, "GET", "/api/v1/tags", Auth::Bearer(&token), None, ip).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tags.as_array().unwrap().len(), 2);

    // Bob ne révoque pas la clé d'Alice ; Alice, si — effet immédiat.
    let revoke_path = format!("/api/api-keys/{key_id}");
    assert_eq!(
        call(&app, "DELETE", &revoke_path, Auth::Cookie(&bob), None, "198.51.100.41")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, "GET", "/api/v1/tags", Auth::Bearer(&token), None, ip)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "DELETE", &revoke_path, Auth::Cookie(&alice), None, ip)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/recipes",
            Auth::Bearer(&token),
            Some(recipe_body()),
            ip
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    t.drop().await;
}

#[tokio::test]
async fn echecs_de_cle_limites_par_ip() {
    let t = TestDb::new().await;
    let app = router(&t.pool);
    let (alice, _) = account(&t.pool).await;
    let token = new_key(&t.pool, alice, "poste").await.token;
    let attacker = "198.51.100.66";

    // 20 échecs (clé mal formée ou inconnue) épuisent le budget de l'IP…
    let unknown = hebdomiam::api_key::generate().token;
    for i in 0..20 {
        let guess = if i % 2 == 0 { "hbd_court" } else { unknown.as_str() };
        assert_eq!(
            call(&app, "GET", "/api/v1/tags", Auth::Bearer(guess), None, attacker)
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    // … même une clé valide est alors refusée depuis cette IP,
    let (status, body, _) = call(&app, "GET", "/api/v1/tags", Auth::Bearer(&token), None, attacker).await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("errors.tooManyRequests"))
    );
    // … mais pas ailleurs : les succès ne sont pas comptés.
    for _ in 0..25 {
        assert_eq!(
            call(&app, "GET", "/api/v1/tags", Auth::Bearer(&token), None, "198.51.100.67")
                .await
                .0,
            StatusCode::OK
        );
    }
    t.drop().await;
}
