//! Routage HTTP : API de synchronisation et interface web.

pub mod api;
pub mod web;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use axum::body::Body;
use axum::http::header;
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

/// Routeur complet du service.
pub fn router(state: AppState) -> Router {
    let mut app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .merge(api::router())
        .merge(web::router())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http());

    // Un client OAuth peut avoir enregistre une autre URI de redirection (ex. /Authorized) :
    // le callback est alors servi aussi a ce chemin.
    let callback_path = state.config.google_redirect_path();
    if callback_path != "/auth/google/callback" && callback_path != "/" {
        tracing::info!(chemin = %callback_path, "callback OAuth servi a un chemin personnalise");
        app = app.route(&callback_path, get(web::google_callback));
    }
    let spotify_path = state.config.spotify_redirect_path();
    if spotify_path != "/auth/spotify/callback" && spotify_path != "/" {
        tracing::info!(chemin = %spotify_path, "callback Spotify servi a un chemin personnalise");
        app = app.route(&spotify_path, get(web::spotify_callback));
    }
    let deezer_path = state.config.deezer_redirect_path();
    if deezer_path != "/auth/deezer/callback" && deezer_path != "/" {
        tracing::info!(chemin = %deezer_path, "callback Deezer servi a un chemin personnalise");
        app = app.route(&deezer_path, get(web::deezer_callback));
    }

    app.with_state(state)
}

/// Export GPX d'une seance, partage par l'API (jeton) et l'interface web (session).
pub async fn gpx_response(state: &AppState, user_id: &str, id: &str) -> AppResult<Response> {
    let (_workout, payload) = crate::db::get_workout(&state.pool, user_id, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let summary: mpacer_core::history::WorkoutSummary = serde_json::from_str(&payload)
        .map_err(|error| AppError::internal(format!("seance illisible : {error}")))?;
    let gpx = mpacer_core::gpx::export_gpx(&summary);
    Response::builder()
        .header(header::CONTENT_TYPE, "application/gpx+xml; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{id}.gpx\""),
        )
        .body(Body::from(gpx))
        .map_err(|error| AppError::internal(error.to_string()))
}

/// Export KML d'une seance, partage par l'API (jeton) et l'interface web.
///
/// Meme trace que le GPX, dans le format que lisent Google Earth et les
/// applications de cartes hors ligne.
pub async fn kml_response(state: &AppState, user_id: &str, id: &str) -> AppResult<Response> {
    let (_workout, payload) = crate::db::get_workout(&state.pool, user_id, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let summary: mpacer_core::history::WorkoutSummary = serde_json::from_str(&payload)
        .map_err(|error| AppError::internal(format!("seance illisible : {error}")))?;
    let kml = mpacer_core::gpx::export_kml(&summary);
    Response::builder()
        .header(
            header::CONTENT_TYPE,
            "application/vnd.google-earth.kml+xml; charset=utf-8",
        )
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{id}.kml\""),
        )
        .body(Body::from(kml))
        .map_err(|error| AppError::internal(error.to_string()))
}

/// Manifeste de transfert d'une playlist, en piece jointe JSON.
///
/// Partage par l'API (jeton) et l'interface web (session) : c'est exactement le
/// fichier que `mpacer-music` consomme pour copier l'audio par USB.
pub async fn manifest_response(state: &AppState, user_id: &str, id: &str) -> AppResult<Response> {
    let playlist = crate::db::get_music_playlist(&state.pool, user_id, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let tracks = crate::db::list_music_tracks(&state.pool, user_id, id).await?;
    let manifest = crate::models::TransferManifest::from_playlist(&playlist, &tracks);
    let body = serde_json::to_string_pretty(&manifest)
        .map_err(|error| AppError::internal(format!("manifeste illisible : {error}")))?;
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                crate::models::manifest_file_name(&playlist.name)
            ),
        )
        .body(Body::from(body))
        .map_err(|error| AppError::internal(error.to_string()))
}

/// Vivacite : le processus repond.
async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "version": mpacer_core::VERSION }))
}

/// Disponibilite : la base de donnees repond.
async fn readyz(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<serde_json::Value> {
    // CAST explicite : PostgreSQL renvoie un `integer` pour un litteral, alors que
    // sqlx attend un `bigint` pour un i64.
    let database = sqlx::query_scalar::<_, i64>("SELECT 1::bigint")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    Json(
        serde_json::json!({ "status": if database { "ready" } else { "degraded" }, "database": database }),
    )
}
