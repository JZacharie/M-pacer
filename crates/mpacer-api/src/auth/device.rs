//! Appairage de la montre : "device authorization grant".
//!
//! La montre ne peut pas ouvrir de navigateur. Elle demande donc un code, affiche
//! un code court a l'utilisateur, qui l'approuve depuis l'interface web (connectee
//! a Google). La montre interroge ensuite le service jusqu'a recevoir son jeton.
//!
//! ```text
//! montre                     backend                      navigateur (Google)
//!   |-- POST /device/code ---->|                                |
//!   |<-- user_code, device_code|                                |
//!   |   (affiche AB12-CD34)    |<---- POST /link (session) -----|
//!   |-- POST /device/token --->|   (en attente)                 |
//!   |<-- access_token ---------|                                |
//! ```

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Alphabet sans caracteres ambigus (pas de O/0, I/1, voyelles).
/// Partage avec les codes d'invitation d'amis ([crate::friends]) : un seul
/// alphabet dans tout le service, donc une seule habitude de lecture.
pub(crate) const ALPHABET: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ23456789";

/// Demande de code.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeRequest {
    /// Nom affiche dans l'interface ("Pixel Watch 3", "Simulateur").
    #[serde(default = "default_label")]
    pub label: String,
}

fn default_label() -> String {
    "Montre".to_string()
}

/// Reponse de demande de code.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: i64,
    pub interval: i64,
}

/// Demande de jeton.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceTokenRequest {
    pub device_code: String,
}

/// Reponse de jeton.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i64>,
    pub label: String,
}

/// Ligne `device_codes`.
#[derive(Debug, Clone, FromRow)]
pub struct DeviceCodeRow {
    pub device_code: String,
    pub user_code: String,
    pub user_id: Option<String>,
    pub label: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub approved_at_ms: Option<i64>,
    pub consumed_at_ms: Option<i64>,
    pub last_poll_ms: Option<i64>,
}

/// Genere un code utilisateur lisible du type `BCDF-GHJK`.
pub fn generate_user_code() -> String {
    let mut raw = String::with_capacity(8);
    let mut remaining = 8;
    while remaining > 0 {
        for byte in uuid::Uuid::new_v4().as_bytes() {
            if remaining == 0 {
                break;
            }
            raw.push(ALPHABET[(*byte as usize) % ALPHABET.len()] as char);
            remaining -= 1;
        }
    }
    format!("{}-{}", &raw[..4], &raw[4..])
}

/// Cree une demande d'appairage.
pub async fn create(
    state: &AppState,
    request: &DeviceCodeRequest,
) -> AppResult<DeviceCodeResponse> {
    let now = state.now_ms();
    let expires_in = state.config.device_code_ttl.as_secs() as i64;
    let device_code = crate::auth::random_urlsafe(32);
    let label = request.label.chars().take(60).collect::<String>();

    // Le code utilisateur doit etre unique : on retente en cas de collision.
    let mut user_code = generate_user_code();
    for attempt in 0..5 {
        let exists: Option<String> =
            sqlx::query_scalar("SELECT user_code FROM device_codes WHERE user_code = $1")
                .bind(&user_code)
                .fetch_optional(&state.pool)
                .await?;
        if exists.is_none() {
            break;
        }
        if attempt == 4 {
            return Err(AppError::internal(
                "impossible de generer un code d'appairage",
            ));
        }
        user_code = generate_user_code();
    }

    sqlx::query(
        "INSERT INTO device_codes (device_code, user_code, label, created_at_ms, expires_at_ms)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&device_code)
    .bind(&user_code)
    .bind(&label)
    .bind(now)
    .bind(now + expires_in * 1000)
    .execute(&state.pool)
    .await?;

    let verification_uri = format!("{}/link", state.config.public_url);
    Ok(DeviceCodeResponse {
        verification_uri_complete: format!("{verification_uri}?code={user_code}"),
        verification_uri,
        device_code,
        user_code,
        expires_in,
        interval: 5,
    })
}

/// Interroge l'etat d'une demande (la montre appelle cette fonction en boucle).
pub async fn poll(
    state: &AppState,
    request: &DeviceTokenRequest,
) -> AppResult<DeviceTokenResponse> {
    let now = state.now_ms();
    let row: Option<DeviceCodeRow> =
        sqlx::query_as("SELECT * FROM device_codes WHERE device_code = $1")
            .bind(&request.device_code)
            .fetch_optional(&state.pool)
            .await?;

    let Some(row) = row else {
        return Err(AppError::OAuth("invalid_device_code".into()));
    };
    if row.consumed_at_ms.is_some() {
        return Err(AppError::OAuth("already_used".into()));
    }
    if row.expires_at_ms < now {
        return Err(AppError::OAuth("expired_token".into()));
    }
    if row.approved_at_ms.is_none() || row.user_id.is_none() {
        // La montre repollera : c'est le fonctionnement normal du flux.
        return Err(AppError::OAuth("authorization_pending".into()));
    }

    // Approuve : on emet un jeton d'appareil unique.
    let token = crate::auth::new_device_token();
    let user_id = row.user_id.clone().unwrap();
    crate::db::insert_api_token(
        &state.pool,
        &user_id,
        &crate::auth::hash_token(&token),
        &row.label,
        now,
    )
    .await?;
    sqlx::query("UPDATE device_codes SET consumed_at_ms = $1 WHERE device_code = $2")
        .bind(now)
        .bind(&row.device_code)
        .execute(&state.pool)
        .await?;

    Ok(DeviceTokenResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: Some(state.config.token_ttl_days * 24 * 3600),
        label: row.label,
    })
}

/// Retrouve une demande par son code utilisateur (saisi dans le navigateur).
pub async fn find_by_user_code(
    state: &AppState,
    user_code: &str,
) -> AppResult<Option<DeviceCodeRow>> {
    let normalized = normalize_user_code(user_code);
    let row = sqlx::query_as::<_, DeviceCodeRow>(
        "SELECT * FROM device_codes WHERE REPLACE(user_code, '-', '') = $1",
    )
    .bind(&normalized)
    .fetch_optional(&state.pool)
    .await?;
    Ok(row)
}

/// Approuve une demande au nom de l'utilisateur connecte.
pub async fn approve(state: &AppState, user_code: &str, user_id: &str) -> AppResult<DeviceCodeRow> {
    let now = state.now_ms();
    let row = find_by_user_code(state, user_code)
        .await?
        .ok_or_else(|| AppError::bad_request("code inconnu"))?;

    if row.expires_at_ms < now {
        return Err(AppError::bad_request(
            "code expire, relancez l'appairage sur la montre",
        ));
    }
    if row.consumed_at_ms.is_some() {
        return Err(AppError::Conflict("code deja utilise".into()));
    }

    sqlx::query("UPDATE device_codes SET user_id = $1, approved_at_ms = $2 WHERE device_code = $3")
        .bind(user_id)
        .bind(now)
        .bind(&row.device_code)
        .execute(&state.pool)
        .await?;

    Ok(row)
}

/// Normalise la saisie utilisateur (`ab12 cd34`, `AB12CD34`, ...).
pub fn normalize_user_code(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_code_has_expected_shape() {
        for _ in 0..50 {
            let code = generate_user_code();
            assert_eq!(code.len(), 9);
            assert_eq!(code.as_bytes()[4], b'-');
            assert!(code
                .chars()
                .all(|c| c == '-' || ALPHABET.contains(&(c as u8))));
        }
    }

    #[test]
    fn user_code_normalisation() {
        assert_eq!(normalize_user_code("bcdf-ghjk"), "BCDFGHJK");
        assert_eq!(normalize_user_code(" BCDF GHJK "), "BCDFGHJK");
    }
}
