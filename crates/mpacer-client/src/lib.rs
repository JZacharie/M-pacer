//! # mpacer-client
//!
//! Client de synchronisation : appairage d'une montre puis envoi des seances.
//! Utilise par l'application Wear OS (via `mpacer-ffi`), par le simulateur et par
//! tout script d'import (reprise d'un historique, migration).
//!
//! ```no_run
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! use mpacer_client::MpacerClient;
//!
//! // 1. Demander un code, l'afficher a l'utilisateur, puis attendre l'approbation.
//! let client = MpacerClient::new("https://mpacer.p.zacharie.org")?;
//! let code = client.request_device_code("Pixel Watch 3").await?;
//! println!("Saisissez {} sur {}", code.user_code, code.verification_uri);
//! let token = client.wait_for_token(&code.device_code).await?;
//!
//! // 2. Envoyer les seances enregistrees.
//! let client = client.with_token(token.access_token);
//! // client.upload_workout(&summary).await?;
//! # Ok(()) }
//! ```

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Erreurs du client.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("erreur reseau : {0}")]
    Network(#[from] reqwest::Error),
    #[error("le service a refuse la requete ({status}) : {message}")]
    Api { status: u16, message: String },
    #[error("appairage expire avant approbation")]
    Expired,
    #[error("jeton d'appareil absent : appelez avec_token()")]
    MissingToken,
}

/// Reponse de demande de code d'appairage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: i64,
    pub interval: i64,
}

/// Jeton d'appareil obtenu apres approbation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceToken {
    pub access_token: String,
    pub token_type: String,
    pub label: String,
}

/// Reponse d'envoi de seance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadReceipt {
    pub id: String,
    pub replaced: bool,
}

/// Reponse d'erreur du service.
#[derive(Debug, Deserialize)]
struct ApiError {
    #[serde(default)]
    error: String,
    #[serde(default)]
    message: String,
}

/// Client du backend M-pacer.
#[derive(Debug, Clone)]
pub struct MpacerClient {
    base_url: String,
    token: Option<String>,
    http: reqwest::Client,
}

impl MpacerClient {
    /// Cree un client pour une URL de base (ex. `https://mpacer.p.zacharie.org`).
    pub fn new(base_url: impl Into<String>) -> Result<Self, ClientError> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("mpacer-client/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: None,
            http,
        })
    }

    /// Associe un jeton d'appareil (mode authentifie).
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Demande un code d'appairage a afficher sur la montre.
    pub async fn request_device_code(&self, label: &str) -> Result<DeviceCode, ClientError> {
        let response = self
            .http
            .post(format!("{}/api/v1/device/code", self.base_url))
            .json(&serde_json::json!({ "label": label }))
            .send()
            .await?;
        decode(response).await
    }

    /// Attend l'approbation du code par l'utilisateur (boucle de polling).
    pub async fn wait_for_token(&self, device_code: &str) -> Result<DeviceToken, ClientError> {
        // 10 minutes au maximum, une tentative toutes les 5 secondes.
        for _ in 0..120 {
            let response = self
                .http
                .post(format!("{}/api/v1/device/token", self.base_url))
                .json(&serde_json::json!({ "device_code": device_code }))
                .send()
                .await?;

            let status = response.status();
            let body = response.text().await?;
            if status.is_success() {
                return serde_json::from_str(&body).map_err(|_| ClientError::Api {
                    status: status.as_u16(),
                    message: "reponse illisible".into(),
                });
            }
            let error: ApiError = serde_json::from_str(&body).unwrap_or(ApiError {
                error: String::new(),
                message: body.clone(),
            });
            match error.error.as_str() {
                "authorization_pending" => tokio::time::sleep(Duration::from_secs(5)).await,
                "expired_token" | "already_used" => return Err(ClientError::Expired),
                _ => {
                    return Err(ClientError::Api {
                        status: status.as_u16(),
                        message: if error.message.is_empty() {
                            error.error
                        } else {
                            error.message
                        },
                    })
                }
            }
        }
        Err(ClientError::Expired)
    }

    /// Envoie une seance (idempotent : un nouvel envoi remplace l'ancien).
    pub async fn upload_workout(
        &self,
        summary: &mpacer_core::history::WorkoutSummary,
    ) -> Result<UploadReceipt, ClientError> {
        let token = self.token.as_ref().ok_or(ClientError::MissingToken)?;
        let response = self
            .http
            .post(format!("{}/api/v1/workouts", self.base_url))
            .bearer_auth(token)
            .json(summary)
            .send()
            .await?;
        decode(response).await
    }

    /// Verifie que le jeton est valide et renvoie l'utilisateur associe.
    pub async fn me(&self) -> Result<serde_json::Value, ClientError> {
        let token = self.token.as_ref().ok_or(ClientError::MissingToken)?;
        let response = self
            .http
            .get(format!("{}/api/v1/me", self.base_url))
            .bearer_auth(token)
            .send()
            .await?;
        decode(response).await
    }

    /// Version du service (utile comme test de connectivite).
    pub async fn version(&self) -> Result<serde_json::Value, ClientError> {
        let response = self
            .http
            .get(format!("{}/api/v1/version", self.base_url))
            .send()
            .await?;
        decode(response).await
    }
}

async fn decode<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, ClientError> {
    let status = response.status();
    let body = response.text().await?;
    if status.is_success() {
        serde_json::from_str(&body).map_err(|error| ClientError::Api {
            status: status.as_u16(),
            message: format!("reponse illisible : {error}"),
        })
    } else {
        let error: ApiError = serde_json::from_str(&body).unwrap_or(ApiError {
            error: String::new(),
            message: body,
        });
        Err(ClientError::Api {
            status: status.as_u16(),
            message: if error.message.is_empty() {
                error.error
            } else {
                error.message
            },
        })
    }
}
