//! API de synchronisation consommee par la montre (et le simulateur).
//!
//! Authentification : `Authorization: Bearer <jeton d'appareil>`.
//! Les erreurs respectent le format `{"error": "code", "message": "..."}`.

use crate::auth::device::{
    self, DeviceCodeRequest, DeviceCodeResponse, DeviceTokenRequest, DeviceTokenResponse,
};
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{UploadResponse, WorkoutUpload};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

/// Routes API (prefixe `/api/v1`).
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/device/code", post(device_code))
        .route("/api/v1/device/token", post(device_token))
        .route("/api/v1/me", get(me))
        .route("/api/v1/workouts", post(upload_workout).get(list_workouts))
        .route(
            "/api/v1/workouts/{id}",
            get(get_workout).delete(delete_workout),
        )
        .route("/api/v1/workouts/{id}/gpx", get(workout_gpx))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/version", get(version))
}

/// Demande un code d'appairage (montre).
async fn device_code(
    State(state): State<AppState>,
    Json(request): Json<DeviceCodeRequest>,
) -> AppResult<Json<DeviceCodeResponse>> {
    Ok(Json(device::create(&state, &request).await?))
}

/// Echange un code approuve contre un jeton d'appareil (montre).
async fn device_token(
    State(state): State<AppState>,
    Json(request): Json<DeviceTokenRequest>,
) -> AppResult<Json<DeviceTokenResponse>> {
    Ok(Json(device::poll(&state, &request).await?))
}

#[derive(Debug, Serialize)]
struct MeResponse {
    id: String,
    email: String,
    name: Option<String>,
    picture: Option<String>,
}

async fn me(AuthUser(user): AuthUser) -> Json<MeResponse> {
    Json(MeResponse {
        id: user.id,
        email: user.email,
        name: user.name,
        picture: user.picture_url,
    })
}

async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "core": mpacer_core::VERSION, "api": env!("CARGO_PKG_VERSION") }))
}

/// Envoie une seance (idempotent : un renvoi remplace la version precedente).
async fn upload_workout(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(upload): Json<WorkoutUpload>,
) -> AppResult<Json<UploadResponse>> {
    upload.validate().map_err(AppError::bad_request)?;
    // On normalise avant stockage : `unit_system` est toujours renseigne, ce qui
    // permet de relire la seance comme un `WorkoutSummary` complet (export GPX).
    let mut normalized = upload.clone();
    normalized.unit_system = Some(upload.units());
    let payload = serde_json::to_string(&normalized)
        .map_err(|error| AppError::internal(error.to_string()))?;
    let replaced =
        crate::db::upsert_workout(&state.pool, &user.id, &upload, &payload, state.now_ms()).await?;
    tracing::info!(user = %user.email, workout = %upload.id, distance_m = upload.distance_m, "seance synchronisee");
    Ok(Json(UploadResponse {
        id: upload.id,
        replaced,
    }))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize)]
struct ListResponse {
    total: i64,
    items: Vec<crate::models::WorkoutRow>,
}

async fn list_workouts(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<ListResponse>> {
    let limit = query.limit.clamp(1, 200);
    let offset = query.offset.max(0);
    let items = crate::db::list_workouts(&state.pool, &user.id, limit, offset).await?;
    let total = crate::db::count_workouts(&state.pool, &user.id).await?;
    Ok(Json(ListResponse { total, items }))
}

async fn get_workout(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let (workout, payload) = crate::db::get_workout(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let payload: serde_json::Value =
        serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null);
    Ok(Json(
        serde_json::json!({ "workout": workout, "summary": payload }),
    ))
}

async fn delete_workout(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if crate::db::delete_workout(&state.pool, &user.id, &id).await? {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

/// Export GPX d'une seance (partage vers Strava/Garmin).
async fn workout_gpx(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    super::gpx_response(&state, &user.id, &id).await
}

#[derive(Debug, Deserialize)]
struct StatsQuery {
    #[serde(default = "default_days")]
    days: i64,
}

fn default_days() -> i64 {
    30
}

async fn stats(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<StatsQuery>,
) -> AppResult<Json<crate::db::Stats>> {
    let days = query.days.clamp(1, 3650);
    Ok(Json(
        crate::db::stats(&state.pool, &user.id, days, state.now_ms()).await?,
    ))
}
