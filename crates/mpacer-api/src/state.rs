//! Etat partage du service.

use crate::auth::google::{GoogleOidc, OidcProvider, UnconfiguredOidc};
use crate::config::Config;
use sqlx::PgPool;
use std::sync::Arc;

/// Etat injecte dans tous les gestionnaires.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub http: reqwest::Client,
    /// Fournisseur d'identite (Google en production, faux fournisseur en test).
    pub oidc: Arc<dyn OidcProvider>,
}

impl AppState {
    /// Etat de production : le fournisseur Google est active si les identifiants
    /// sont configures, sinon le service reste utilisable en mode developpement.
    pub fn new(pool: PgPool, config: Arc<Config>) -> Self {
        let http = http_client();
        let oidc: Arc<dyn OidcProvider> =
            match (&config.google_client_id, &config.google_client_secret) {
                (Some(client_id), Some(client_secret)) => Arc::new(GoogleOidc::new(
                    client_id.clone(),
                    client_secret.clone(),
                    http.clone(),
                )),
                _ => Arc::new(UnconfiguredOidc),
            };
        Self {
            pool,
            config,
            http,
            oidc,
        }
    }

    /// Etat avec un fournisseur d'identite impose (tests).
    pub fn with_oidc(pool: PgPool, config: Arc<Config>, oidc: Arc<dyn OidcProvider>) -> Self {
        Self {
            pool,
            config,
            http: http_client(),
            oidc,
        }
    }

    /// Horodatage courant (ms).
    pub fn now_ms(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("mpacer-api/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}
