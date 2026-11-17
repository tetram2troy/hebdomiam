//! Frontière HTTP de bout en bout (routeur complet, sans réseau) : sessions
//! scellées, révocation, rôles, validation, rate limiting, service du SPA.

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{TestDb, UserOptions, create_user};
use hebdomiam::app;
use hebdomiam::config::Config;
use hebdomiam::crypto::hash_password;
use hebdomiam::domain::catalog::Role;
use hebdomiam::state::AppState;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const PASSWORD: &str = "mot-de-passe-tres-long-123";

fn router(pool: &PgPool, static_dir: Option<std::path::PathBuf>) -> Router {
    let mut config = Config::for_tests("postgresql://unused");
    config.static_dir = static_dir;
    app::build(AppState::new(pool.clone(), config))
}

struct Reply {
    status: StatusCode,
    cookie: Option<String>,
    headers: axum::http::HeaderMap,
    body: Value,
    text: String,
}

async fn call(app: &Router, method: &str, path: &str, cookie: Option<&str>, body: Option<Value>, ip: &str) -> Reply {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        // Appelant direct = proxy de confiance local (Config::for_tests).
        .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            40000,
        ))))
        .header("x-forwarded-for", ip)
        .header("host", "127.0.0.1:3000");
    if let Some(cookie) = cookie {
        request = request.header("cookie", format!("__Host-hebdomiam-session={cookie}"));
    }
    let request = match body {
        Some(json) => request
            .header("content-type", "application/json")
            .body(Body::from(json.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let cookie = headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|v| {
            v.strip_prefix("__Host-hebdomiam-session=")
                .map(|rest| rest.split(';').next().unwrap_or("").to_string())
        });
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Reply {
        status,
        cookie,
        headers,
        body: serde_json::from_str(&text).unwrap_or(Value::Null),
        text,
    }
}

async fn account(pool: &PgPool, role: Role) -> String {
    let id = create_user(
        pool,
        UserOptions {
            role,
            password_hash: hash_password(PASSWORD.into()).await,
            ..Default::default()
        },
    )
    .await;
    sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn login(app: &Router, email: &str, ip: &str) -> String {
    let reply = call(
        app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
        ip,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    reply.cookie.expect("cookie de session posé")
}

#[tokio::test]
async fn connexion_session_et_deconnexion() {
    let t = TestDb::new().await;
    let app = router(&t.pool, None);
    let email = account(&t.pool, Role::User).await;

    assert_eq!(
        call(&app, "GET", "/api/session", None, None, "198.51.100.1").await.body,
        json!({})
    );
    let wrong = call(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": "faux" })),
        "198.51.100.1",
    )
    .await;
    assert_eq!(
        (wrong.status, wrong.body["message"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("errors.invalidCredentials"))
    );

    let cookie = login(&app, &email.to_uppercase(), "198.51.100.1").await;
    let session = call(&app, "GET", "/api/session", Some(&cookie), None, "198.51.100.1").await;
    assert_eq!(session.body["user"]["email"], json!(email));
    assert_eq!(session.body["user"]["role"], json!("user"));
    assert_eq!(
        call(&app, "GET", "/api/recipes", Some(&cookie), None, "198.51.100.1")
            .await
            .body,
        json!([])
    );

    let logout = call(&app, "POST", "/api/auth/logout", Some(&cookie), None, "198.51.100.1").await;
    assert_eq!(logout.cookie.as_deref(), Some(""), "cookie effacé");
    assert_eq!(
        call(&app, "GET", "/api/recipes", None, None, "198.51.100.1")
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    t.drop().await;
}

#[tokio::test]
async fn compte_desactive_revoque_immediatement() {
    let t = TestDb::new().await;
    let app = router(&t.pool, None);
    let email = account(&t.pool, Role::User).await;
    let cookie = login(&app, &email, "198.51.100.2").await;
    sqlx::query("UPDATE users SET disabled_at = now() WHERE email = $1")
        .bind(&email)
        .execute(&t.pool)
        .await
        .unwrap();

    let blocked = call(&app, "GET", "/api/recipes", Some(&cookie), None, "198.51.100.2").await;
    assert_eq!(
        (blocked.status, blocked.body["message"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("errors.accountDisabled"))
    );
    assert_eq!(blocked.cookie.as_deref(), Some(""), "session purgée");
    let session = call(&app, "GET", "/api/session", Some(&cookie), None, "198.51.100.2").await;
    assert_eq!(session.status, StatusCode::UNAUTHORIZED);

    let relogin = call(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
        "198.51.100.2",
    )
    .await;
    assert_eq!(
        (relogin.status, relogin.body["message"].as_str()),
        (StatusCode::FORBIDDEN, Some("errors.accountDisabled"))
    );
    t.drop().await;
}

#[tokio::test]
async fn role_admin_relu_en_base() {
    let t = TestDb::new().await;
    let app = router(&t.pool, None);
    let admin_email = account(&t.pool, Role::Admin).await;
    let user_email = account(&t.pool, Role::User).await;

    let user_cookie = login(&app, &user_email, "198.51.100.3").await;
    let denied = call(
        &app,
        "GET",
        "/api/admin/users",
        Some(&user_cookie),
        None,
        "198.51.100.3",
    )
    .await;
    assert_eq!(
        (denied.status, denied.body["message"].as_str()),
        (StatusCode::FORBIDDEN, Some("errors.forbidden"))
    );

    let admin_cookie = login(&app, &admin_email, "198.51.100.3").await;
    let created = call(
        &app,
        "POST",
        "/api/admin/users",
        Some(&admin_cookie),
        Some(json!({ "email": "Nouveau@Example.com", "name": "Nouveau" })),
        "198.51.100.3",
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text);
    assert_eq!(created.body["user"]["email"], json!("nouveau@example.com"));
    assert!(
        created.body["inviteUrl"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:3000/invitation#token=")
    );

    // Rétrogradé en base : le cookie (rôle admin) ne suffit plus.
    sqlx::query("UPDATE users SET role = 'user' WHERE email = $1")
        .bind(&admin_email)
        .execute(&t.pool)
        .await
        .unwrap();
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/admin/users",
            Some(&admin_cookie),
            None,
            "198.51.100.3"
        )
        .await
        .status,
        StatusCode::FORBIDDEN
    );
    t.drop().await;
}

#[tokio::test]
async fn validation_et_routes_inconnues() {
    let t = TestDb::new().await;
    let app = router(&t.pool, None);
    let email = account(&t.pool, Role::User).await;
    let cookie = login(&app, &email, "198.51.100.4").await;

    let empty = call(
        &app,
        "POST",
        "/api/recipes",
        Some(&cookie),
        Some(json!({ "title": "X", "baseServings": 4, "ingredients": [], "steps": ["a"] })),
        "198.51.100.4",
    )
    .await;
    assert_eq!(
        (empty.status, empty.body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("Validation Error"))
    );
    let float = call(
        &app,
        "POST",
        "/api/plans",
        Some(&cookie),
        Some(json!({ "dishCount": 2.5, "peopleCount": 1, "daysCount": 7 })),
        "198.51.100.4",
    )
    .await;
    assert_eq!(float.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        call(
            &app,
            "GET",
            "/api/recipes/pas-un-uuid",
            Some(&cookie),
            None,
            "198.51.100.4"
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );

    // Corps JSON exigé (défense CSRF) : un formulaire texte est refusé.
    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "text/plain")
        .body(Body::from(json!({ "email": email, "password": PASSWORD }).to_string()))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );

    let created = call(
        &app,
        "POST",
        "/api/recipes",
        Some(&cookie),
        Some(json!({ "title": " Chili ", "baseServings": 4, "ingredients": [{ "quantity": 1.5, "name": "oignon" }], "steps": ["Cuire"], "tags": ["a", "a"], "role": "admin" })),
        "198.51.100.4",
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.text);
    assert_eq!(created.body["title"], json!("Chili"));
    assert_eq!(created.body["tags"], json!(["a"]));
    assert_eq!(created.body["ingredients"][0]["aisle"], json!("other"));
    assert!(created.body["createdAt"].as_str().unwrap().ends_with('Z'));

    let unknown = call(&app, "GET", "/api/nexiste-pas", None, None, "198.51.100.4").await;
    assert_eq!(
        (unknown.status, unknown.body["message"].as_str()),
        (StatusCode::NOT_FOUND, Some("errors.notFound"))
    );
    assert_eq!(
        call(&app, "GET", "/healthz", None, None, "198.51.100.4").await.body,
        json!({ "status": "ok" })
    );
    t.drop().await;
}

#[tokio::test]
async fn inscription_fermee_et_rate_limit() {
    let t = TestDb::new().await;
    let app = router(&t.pool, None);
    account(&t.pool, Role::Admin).await;
    assert_eq!(
        call(&app, "GET", "/api/auth/registration-open", None, None, "198.51.100.5")
            .await
            .body,
        json!({ "open": false })
    );
    let closed = call(
        &app,
        "POST",
        "/api/auth/register",
        None,
        Some(json!({ "email": "x@example.com", "password": PASSWORD, "name": "X" })),
        "198.51.100.5",
    )
    .await;
    assert_eq!(
        (closed.status, closed.body["message"].as_str()),
        (StatusCode::FORBIDDEN, Some("errors.registrationClosed"))
    );

    for _ in 0..5 {
        call(
            &app,
            "POST",
            "/api/auth/login",
            None,
            Some(json!({ "email": "x@example.com", "password": "faux" })),
            "198.51.100.6",
        )
        .await;
    }
    let limited = call(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": "x@example.com", "password": "faux" })),
        "198.51.100.6",
    )
    .await;
    assert_eq!(
        (limited.status, limited.body["message"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("errors.tooManyRequests"))
    );
    // Une autre IP garde son budget.
    let other = call(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": "x@example.com", "password": "faux" })),
        "198.51.100.7",
    )
    .await;
    assert_eq!(other.status, StatusCode::UNAUTHORIZED);
    t.drop().await;
}

#[tokio::test]
async fn spa_et_en_tetes_de_securite() {
    let t = TestDb::new().await;
    let dir = std::env::temp_dir().join(format!("hebdomiam-spa-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(dir.join("_nuxt")).unwrap();
    std::fs::write(dir.join("index.html"), "<!doctype html><div id=__nuxt></div>").unwrap();
    std::fs::write(dir.join("_nuxt/app.123.js"), "console.log(1)").unwrap();
    let app = router(&t.pool, Some(dir.clone()));

    let page = call(&app, "GET", "/en/planifier", None, None, "198.51.100.8").await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.text.contains("__nuxt"));
    assert_eq!(page.headers["x-content-type-options"], "nosniff");
    assert_eq!(page.headers["cache-control"], "no-cache");

    let asset = call(&app, "GET", "/_nuxt/app.123.js", None, None, "198.51.100.8").await;
    assert_eq!(asset.status, StatusCode::OK);
    assert!(asset.headers["cache-control"].to_str().unwrap().contains("immutable"));
    assert_eq!(
        call(&app, "GET", "/_nuxt/disparu.js", None, None, "198.51.100.8")
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, "GET", "/api/inconnue", None, None, "198.51.100.8")
            .await
            .status,
        StatusCode::NOT_FOUND,
        "l'API ne retombe jamais sur le SPA"
    );

    std::fs::remove_dir_all(dir).ok();
    t.drop().await;
}

#[tokio::test]
async fn verrou_par_compte_malgre_la_rotation_d_ip() {
    let t = TestDb::new().await;
    let app = router(&t.pool, None);
    let email = account(&t.pool, Role::User).await;

    // 10 échecs depuis 10 adresses différentes : le quota par IP ne voit rien.
    for n in 0..10 {
        let ip = format!("198.51.101.{n}");
        let wrong = call(
            &app,
            "POST",
            "/api/auth/login",
            None,
            Some(json!({ "email": email, "password": "faux" })),
            &ip,
        )
        .await;
        assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    }
    // Le compte est verrouillé, même avec le bon mot de passe et une IP neuve.
    let locked = call(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
        "198.51.101.200",
    )
    .await;
    assert_eq!(
        (locked.status, locked.body["message"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("errors.tooManyRequests"))
    );
    // Les autres comptes ne sont pas touchés.
    let other = account(&t.pool, Role::User).await;
    login(&app, &other, "198.51.101.201").await;
    t.drop().await;
}

#[tokio::test]
async fn setup_2fa_refuse_quand_active_et_source_url_web_seulement() {
    let t = TestDb::new().await;
    let app = router(&t.pool, None);
    let email = account(&t.pool, Role::User).await;
    let cookie = login(&app, &email, "198.51.102.1").await;

    let first = call(&app, "POST", "/api/2fa/setup", Some(&cookie), None, "198.51.102.1").await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text);
    sqlx::query("UPDATE users SET totp_enabled_at = now() WHERE email = $1")
        .bind(&email)
        .execute(&t.pool)
        .await
        .unwrap();
    let refused = call(&app, "POST", "/api/2fa/setup", Some(&cookie), None, "198.51.102.1").await;
    assert_eq!(
        (refused.status, refused.body["message"].as_str()),
        (StatusCode::CONFLICT, Some("errors.totpAlreadyEnabled"))
    );
    let enabled: bool = sqlx::query_scalar("SELECT totp_enabled_at IS NOT NULL FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&t.pool)
        .await
        .unwrap();
    assert!(enabled, "la 2FA active n'est pas retirée par un setup");

    let recipe = |source: &str| json!({ "title": "X", "baseServings": 2, "sourceUrl": source, "ingredients": [{ "quantity": 1, "name": "sel" }], "steps": ["a"] });
    let js = call(
        &app,
        "POST",
        "/api/recipes",
        Some(&cookie),
        Some(recipe("javascript:alert(document.cookie)")),
        "198.51.102.1",
    )
    .await;
    assert_eq!(js.status, StatusCode::BAD_REQUEST);
    let ok = call(
        &app,
        "POST",
        "/api/recipes",
        Some(&cookie),
        Some(recipe("http://exemple.com/recette")),
        "198.51.102.1",
    )
    .await;
    assert_eq!(ok.status, StatusCode::CREATED, "{}", ok.text);
    t.drop().await;
}

#[tokio::test]
async fn changement_de_mot_de_passe_et_deconnexion_revoquent_les_sessions() {
    let t = TestDb::new().await;
    let app = router(&t.pool, None);
    let email = account(&t.pool, Role::User).await;
    // Deux sessions du même compte : la légitime et une copie « volée ».
    let mine = login(&app, &email, "198.51.103.1").await;
    let stolen = login(&app, &email, "198.51.103.2").await;

    let changed = call(
        &app,
        "POST",
        "/api/settings/password",
        Some(&mine),
        Some(json!({ "currentPassword": PASSWORD, "newPassword": "nouveau-mot-de-passe-long-456" })),
        "198.51.103.1",
    )
    .await;
    assert_eq!(changed.status, StatusCode::OK, "{}", changed.text);
    let renewed = changed.cookie.expect("la session courante est re-scellée");

    // La copie est morte, la session courante continue.
    let blocked = call(&app, "GET", "/api/recipes", Some(&stolen), None, "198.51.103.2").await;
    assert_eq!(blocked.status, StatusCode::UNAUTHORIZED);
    assert_eq!(blocked.cookie.as_deref(), Some(""), "cookie révoqué purgé");
    assert_eq!(
        call(&app, "GET", "/api/auth/me", Some(&stolen), None, "198.51.103.2")
            .await
            .status,
        StatusCode::UNAUTHORIZED,
        "les routes d'auth à session sont contrôlées aussi"
    );
    assert_eq!(
        call(&app, "GET", "/api/recipes", Some(&mine), None, "198.51.103.1")
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/api/recipes", Some(&renewed), None, "198.51.103.1")
            .await
            .status,
        StatusCode::OK
    );

    // Renommer ne rajeunit pas la session et ne la détache pas de son époque.
    let renamed = call(
        &app,
        "PATCH",
        "/api/settings",
        Some(&renewed),
        Some(json!({ "name": "Nouveau nom" })),
        "198.51.103.1",
    )
    .await;
    let after_rename = renamed.cookie.expect("cookie re-scellé avec le nouveau nom");
    assert_eq!(
        call(&app, "GET", "/api/recipes", Some(&after_rename), None, "198.51.103.1")
            .await
            .status,
        StatusCode::OK
    );

    // Déconnexion : un cookie copié avant ne sert plus à rien.
    call(
        &app,
        "POST",
        "/api/auth/logout",
        Some(&after_rename),
        None,
        "198.51.103.1",
    )
    .await;
    assert_eq!(
        call(&app, "GET", "/api/recipes", Some(&after_rename), None, "198.51.103.1")
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    t.drop().await;
}

#[tokio::test]
async fn requetes_d_une_autre_origine_refusees() {
    let t = TestDb::new().await;
    let app = router(&t.pool, None);
    let email = account(&t.pool, Role::User).await;
    let cookie = login(&app, &email, "198.51.104.1").await;

    let post = |site: Option<&'static str>, origin: Option<&'static str>| {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/2fa/setup")
            .header("host", "127.0.0.1:3000")
            .header("cookie", format!("__Host-hebdomiam-session={cookie}"))
            .extension(axum::extract::ConnectInfo(std::net::SocketAddr::from((
                [127, 0, 0, 1],
                40000,
            ))));
        if let Some(site) = site {
            request = request.header("sec-fetch-site", site);
        }
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        app.clone().oneshot(request.body(Body::empty()).unwrap())
    };
    // Formulaire soumis depuis un sous-domaine voisin (même site, autre origine).
    assert_eq!(
        post(Some("same-site"), None).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(Some("cross-site"), None).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(None, Some("https://autre.example.com")).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    // Notre propre front, ou un client non-navigateur.
    assert_eq!(post(Some("same-origin"), None).await.unwrap().status(), StatusCode::OK);
    assert_eq!(
        post(None, Some("http://127.0.0.1:3000")).await.unwrap().status(),
        StatusCode::OK
    );
    assert_eq!(post(None, None).await.unwrap().status(), StatusCode::OK);
    // Les lectures ne sont pas concernées.
    let get = Request::builder()
        .uri("/api/auth/registration-open")
        .header("sec-fetch-site", "cross-site")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(get).await.unwrap().status(), StatusCode::OK);
    t.drop().await;
}

#[tokio::test]
async fn csp_avec_empreintes_des_scripts_inline_et_api_jamais_en_cache() {
    let t = TestDb::new().await;
    let dir = std::env::temp_dir().join(format!("hebdomiam-csp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("index.html"),
        r#"<!doctype html><script type="importmap">{"imports":{}}</script><script src="/_nuxt/a.js"></script><script>window.__NUXT__={}</script><div id=__nuxt></div>"#,
    )
    .unwrap();
    let app = router(&t.pool, Some(dir.clone()));

    let page = call(&app, "GET", "/login", None, None, "198.51.105.1").await;
    let csp = page.headers["content-security-policy"].to_str().unwrap();
    assert!(csp.contains("script-src 'self' 'sha256-"), "{csp}");
    assert_eq!(
        csp.matches("'sha256-").count(),
        2,
        "un hash par script inline, aucun pour le script externe"
    );
    assert!(
        !csp.contains("script-src 'self' 'unsafe-inline'") && !csp.contains("unsafe-eval"),
        "{csp}"
    );
    for directive in [
        "frame-ancestors 'none'",
        "object-src 'none'",
        "base-uri 'self'",
        "form-action 'self'",
    ] {
        assert!(csp.contains(directive), "{directive}");
    }
    assert_eq!(page.headers["x-frame-options"], "DENY");

    let api = call(&app, "GET", "/api/session", None, None, "198.51.105.1").await;
    assert_eq!(api.headers["cache-control"], "no-store");
    std::fs::remove_dir_all(dir).ok();
    t.drop().await;
}
