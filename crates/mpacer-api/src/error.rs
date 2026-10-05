//! Erreurs HTTP : une seule representation, journalisee cote serveur.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Erreur applicative.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("ressource introuvable")]
    NotFound,
    #[error("authentification requise")]
    Unauthorized,
    #[error("acces refuse")]
    Forbidden,
    #[error("requete invalide : {0}")]
    BadRequest(String),
    #[error("conflit : {0}")]
    Conflict(String),
    #[error("base de donnees : {0}")]
    Database(#[from] sqlx::Error),
    #[error("erreur reseau : {0}")]
    Network(#[from] reqwest::Error),
    #[error("erreur interne : {0}")]
    Internal(#[from] anyhow::Error),
    /// Erreur du flux OAuth/appairage : le code est le message (RFC 8628).
    #[error("{0}")]
    OAuth(String),
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        AppError::BadRequest(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        AppError::Internal(anyhow::anyhow!(message.into()))
    }

    pub fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::OAuth(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Database(_) | AppError::Network(_) | AppError::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    /// Code stable renvoye dans les reponses JSON (et exploitable par la montre).
    pub fn code(&self) -> String {
        match self {
            AppError::NotFound => "not_found".to_string(),
            AppError::Unauthorized => "unauthorized".to_string(),
            AppError::Forbidden => "forbidden".to_string(),
            AppError::BadRequest(_) => "invalid_request".to_string(),
            AppError::Conflict(_) => "conflict".to_string(),
            AppError::OAuth(kind) => kind.clone(),
            AppError::Database(_) => "database_error".to_string(),
            AppError::Network(_) => "network_error".to_string(),
            AppError::Internal(_) => "internal_error".to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let code = self.code();
        let message = self.to_string();
        if status.is_server_error() {
            tracing::error!(error = %message, "erreur serveur");
        } else {
            tracing::debug!(error = %message, "requete refusee");
        }
        let body = serde_json::json!({ "error": code, "message": message });
        (status, axum::Json(body)).into_response()
    }
}

/// Resultat applicatif.
pub type AppResult<T> = Result<T, AppError>;
