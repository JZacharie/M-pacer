//! Point d'entree du service M-pacer.

use mpacer_api::config::Config;
use mpacer_api::state::AppState;
use mpacer_api::{db, routes};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = Config::from_env()?;
    tracing::info!(
        environment = ?config.environment,
        bind = %config.bind,
        public_url = %config.public_url,
        database = %config.database.redacted(),
        google = config.google_configured(),
        dev_auth = config.dev_auth,
        "demarrage de mpacer-api"
    );
    if config.dev_auth {
        tracing::warn!("MPACER_DEV_AUTH actif : ne jamais activer cette option en production");
    }

    let pool = db::connect(&config).await?;
    let state = AppState::new(pool, Arc::new(config.clone()));
    let app = routes::router(state);

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!("service a l'ecoute sur http://{}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("arret propre du service");
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,mpacer_api=info,tower_http=warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// Arret propre : Ctrl+C en developpement, SIGTERM en Kubernetes.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("signal d'arret recu (Ctrl+C)"),
        _ = terminate => tracing::info!("signal d'arret recu (SIGTERM)"),
    }
}
