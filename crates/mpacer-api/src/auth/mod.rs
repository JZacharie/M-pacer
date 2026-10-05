//! Authentification : sessions navigateur (JWT en cookie) et jetons d'appareil.

pub mod device;
pub mod google;

use crate::error::{AppError, AppResult};
use crate::models::User;
use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use base64::Engine as _;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Nom du cookie de session.
pub const SESSION_COOKIE: &str = "mpacer_session";

/// Contenu du JWT de session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionClaims {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub iat: i64,
    pub exp: i64,
}

/// Emet un JWT de session signe (HS256).
pub fn issue_session(
    config: &crate::config::Config,
    user: &User,
    now_ms: i64,
) -> anyhow::Result<String> {
    let claims = SessionClaims {
        sub: user.id.clone(),
        email: user.email.clone(),
        name: user.name.clone(),
        picture: user.picture_url.clone(),
        iat: now_ms / 1000,
        exp: now_ms / 1000 + config.session_ttl.as_secs() as i64,
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.session_secret.as_bytes()),
    )?;
    Ok(token)
}

/// Verifie un JWT de session.
pub fn decode_session(config: &crate::config::Config, token: &str) -> Option<SessionClaims> {
    decode::<SessionClaims>(
        token,
        &DecodingKey::from_secret(config.session_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .ok()
    .map(|data| data.claims)
}

/// Genere un jeton d'appareil opaque (32 octets en hexadecimal).
pub fn new_device_token() -> String {
    let mut bytes = [0u8; 32];
    // Deux UUID v4 fournissent 32 octets aleatoires sans dependance supplementaire.
    bytes[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    hex(&bytes)
}

/// Empreinte SHA-256 d'un jeton (seule valeur stockee en base).
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex(&hasher.finalize())
}

/// Chaine aleatoire URL-safe (PKCE, etats OAuth).
pub fn random_urlsafe(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    for chunk in buffer.chunks_mut(16) {
        let uuid = *uuid::Uuid::new_v4().as_bytes();
        let len = chunk.len();
        chunk.copy_from_slice(&uuid[..len]);
    }
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&buffer)
}

/// Defi PKCE S256 a partir du verificateur.
pub fn pkce_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Utilisateur authentifie, extrait d'un jeton d'appareil ou du cookie de session.
#[derive(Debug, Clone)]
pub struct AuthUser(pub User);

/// Utilisateur eventuellement authentifie (pages publiques).
#[derive(Debug, Clone)]
pub struct OptionalUser(pub Option<User>);

async fn resolve_user(state: &AppState, parts: &Parts) -> AppResult<Option<User>> {
    // 1. Jeton d'appareil (montre) : en-tete Authorization: Bearer ...
    if let Some(value) = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = value.strip_prefix("Bearer ").map(str::trim) {
            if !token.is_empty() {
                let user =
                    crate::db::user_for_token_hash(&state.pool, &hash_token(token), state.now_ms())
                        .await?;
                return Ok(user);
            }
        }
    }

    // 2. Session navigateur : cookie signe.
    let cookie = parts
        .headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|raw| {
            raw.split(';')
                .filter_map(|part| part.trim().split_once('='))
                .find(|(name, _)| *name == SESSION_COOKIE)
                .map(|(_, value)| value.to_string())
        });

    let Some(token) = cookie else { return Ok(None) };
    let Some(claims) = decode_session(&state.config, &token) else {
        return Ok(None);
    };
    Ok(crate::db::find_user_by_id(&state.pool, &claims.sub).await?)
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match resolve_user(state, parts).await? {
            Some(user) => Ok(AuthUser(user)),
            None => Err(AppError::Unauthorized),
        }
    }
}

impl FromRequestParts<AppState> for OptionalUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(OptionalUser(resolve_user(state, parts).await?))
    }
}
