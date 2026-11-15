//! Point d'entrée : `hebdomiam [serve|migrate|healthcheck]`.
//!
//! - `serve` (défaut) : API + front statique, arrêt propre sur SIGTERM (< 10 s) ;
//! - `migrate` : applique les migrations SQL (job dédié au déploiement) ;
//! - `healthcheck` : sonde /healthz locale (HEALTHCHECK Docker, image sans shell).

use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Duration;

use hebdomiam::config::Config;
use hebdomiam::logger::{log_error, log_event};
use hebdomiam::state::AppState;
use hebdomiam::{app, db, migrate};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn load_config() -> Result<Config, ExitCode> {
    Config::from_env().map_err(|problems| {
        for problem in problems {
            eprintln!("[config] {problem}");
        }
        eprintln!("[config] démarrage refusé : configuration incomplète (voir .env.example)");
        ExitCode::FAILURE
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".to_string());
    let result = match command.as_str() {
        "serve" => serve().await,
        "migrate" => run_migrations().await,
        "healthcheck" => healthcheck().await,
        other => {
            eprintln!("commande inconnue « {other} » (serve | migrate | healthcheck)");
            Err(ExitCode::from(2))
        }
    };
    result.err().unwrap_or(ExitCode::SUCCESS)
}

async fn serve() -> Result<(), ExitCode> {
    let config = load_config()?;
    let pool = db::connect_lazy(&config.database_url).map_err(|error| {
        log_error("db.config_invalid", &error, json!({}));
        ExitCode::FAILURE
    })?;
    let port = config.port;
    let state = AppState::new(pool.clone(), config);
    let router = app::build(state);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(|error| {
            log_error("server.bind_failed", &error, json!({ "port": port }));
            ExitCode::FAILURE
        })?;
    log_event(
        "server.started",
        json!({ "port": port, "version": env!("CARGO_PKG_VERSION") }),
    );

    let served = axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await;
    // Arrêt propre (docs/HOSTING.md §2.7) : requêtes drainées, puis pool fermé.
    pool.close().await;
    log_event("server.stopped", json!({}));
    served.map_err(|error| {
        log_error("server.failed", &error, json!({}));
        ExitCode::FAILURE
    })
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    log_event("server.stopping", json!({}));
    // Filet de sécurité : la grâce Docker est de 10 s, on sort avant le SIGKILL.
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(8)).await;
        log_event("server.forced_exit", json!({}));
        std::process::exit(0);
    });
}

async fn run_migrations() -> Result<(), ExitCode> {
    let database_url = std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("NUXT_DATABASE_URL"))
        .map_err(|_| {
            eprintln!("[config] DATABASE_URL manquant");
            ExitCode::FAILURE
        })?;
    let pool = db::connect(&database_url).await.map_err(|error| {
        log_error("db.connect_failed", &error, json!({}));
        ExitCode::FAILURE
    })?;
    let applied = migrate::run(&pool).await.map_err(|error| {
        log_error("db.migration_failed", &error, json!({}));
        ExitCode::FAILURE
    })?;
    log_event("db.migrations_done", json!({ "applied": applied.len() }));
    pool.close().await;
    Ok(())
}

/// Requête HTTP minimale vers /healthz — l'image distroless n'a ni shell ni curl.
async fn healthcheck() -> Result<(), ExitCode> {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);
    let probe = async {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
        stream
            .write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")
            .await?;
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await?;
        Ok::<bool, std::io::Error>(response.starts_with(b"HTTP/1.1 200") || response.starts_with(b"HTTP/1.0 200"))
    };
    match tokio::time::timeout(Duration::from_secs(4), probe).await {
        Ok(Ok(true)) => Ok(()),
        _ => Err(ExitCode::FAILURE),
    }
}
