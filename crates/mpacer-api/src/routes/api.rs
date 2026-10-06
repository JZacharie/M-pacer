//! API de synchronisation consommee par la montre (et le simulateur).
//!
//! Authentification : `Authorization: Bearer <jeton d'appareil>`.
//! Les erreurs respectent le format `{"error": "code", "message": "..."}`.

use crate::auth::device::{
    self, DeviceCodeRequest, DeviceCodeResponse, DeviceTokenRequest, DeviceTokenResponse,
};
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{
    MusicAckRequest, MusicPlanAckRequest, MusicPlaylistDetail, MusicPlanView, MusicTrackView,
    Race, RaceInput, UploadResponse, WorkoutUpload,
};
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::header;
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
        .route("/api/v1/races", get(list_races).post(create_race))
        .route(
            "/api/v1/races/{id}",
            get(get_race).put(update_race).delete(delete_race),
        )
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/export", get(export_pac))
        .route("/api/v1/version", get(version))
        // Musique : la montre recupere la fiche des playlists preparees puis les
        // fichiers audio (meme jeton d'appareil que les seances).
        // Le compagnon n'a qu'un jeton d'appareil : il televerse ses fichiers
        // audio par la meme route que la montre lit les playlists.
        .route(
            "/api/v1/music/playlists",
            get(list_music_playlists)
                .post(upload_music_playlist)
                .layer(DefaultBodyLimit::max(crate::models::MAX_UPLOAD_BYTES as usize)),
        )
        .route("/api/v1/music/playlists/{id}", get(get_music_playlist))
        .route(
            "/api/v1/music/playlists/{id}/ack",
            post(ack_music_playlist),
        )
        .route("/api/v1/music/tracks/{id}/file", get(music_track_file))
        .route("/api/v1/music/prepare", get(music_prepare))
        .route("/api/v1/music/prepare/ack", post(music_prepare_ack))
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
    /// Borne inferieure (horodatage UNIX en millisecondes).
    #[serde(default)]
    from: Option<i64>,
    /// Borne superieure (horodatage UNIX en millisecondes).
    #[serde(default)]
    to: Option<i64>,
}

impl ListQuery {
    fn filter(&self) -> crate::db::WorkoutFilter {
        crate::db::WorkoutFilter {
            limit: self.limit.clamp(1, 200),
            offset: self.offset.max(0),
            from_ms: self.from,
            to_ms: self.to,
        }
    }
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
    let filter = query.filter();
    if let (Some(from), Some(to)) = (filter.from_ms, filter.to_ms) {
        if from > to {
            return Err(AppError::bad_request("la borne 'from' doit preceder 'to'"));
        }
    }
    let items = crate::db::list_workouts(&state.pool, &user.id, &filter).await?;
    let total = crate::db::count_workouts(&state.pool, &user.id, &filter).await?;
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

/// Export de tout l'historique au format `.pac` (JSON versionne).
///
/// Le fichier est reimportable par une montre, un autre backend ou un script :
/// c'est exactement le format produit par le coeur Rust.
async fn export_pac(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Response> {
    let payloads = crate::db::all_workout_payloads(&state.pool, &user.id).await?;
    let mut workouts = Vec::with_capacity(payloads.len());
    for payload in payloads {
        match serde_json::from_str::<mpacer_core::history::WorkoutSummary>(&payload) {
            Ok(summary) => workouts.push(summary),
            Err(error) => tracing::warn!(error = %error, "seance ignoree a l'export"),
        }
    }
    let body = mpacer_core::history::export_pac(&workouts)
        .map_err(|error| AppError::internal(error.to_string()))?;
    let filename = format!("mpacer-{}.pac", chrono::Utc::now().format("%Y%m%d"));
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(Body::from(body))
        .map_err(|error| AppError::internal(error.to_string()))
}

/// Export GPX d'une seance (partage vers Strava/Garmin).
async fn workout_gpx(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    super::gpx_response(&state, &user.id, &id).await
}

// ------------------------------------------------------------------ courses

/// Filtre de la liste des courses.
#[derive(Debug, Deserialize)]
struct RaceListQuery {
    /// Ne renvoyer que les courses a venir (ou sans date connue).
    #[serde(default)]
    upcoming: Option<bool>,
}

#[derive(Debug, Serialize)]
struct RaceListResponse {
    total: usize,
    items: Vec<Race>,
}

/// Liste des courses de l'utilisateur, de la plus proche a la plus lointaine.
async fn list_races(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<RaceListQuery>,
) -> AppResult<Json<RaceListResponse>> {
    let items = if query.upcoming.unwrap_or(false) {
        crate::db::list_upcoming_races(&state.pool, &user.id, state.now_ms()).await?
    } else {
        crate::db::list_races(&state.pool, &user.id).await?
    };
    Ok(Json(RaceListResponse {
        total: items.len(),
        items,
    }))
}

/// Cree une course ; les elements de suivi par defaut sont ajoutes avec elle.
async fn create_race(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(input): Json<RaceInput>,
) -> AppResult<(axum::http::StatusCode, Json<Race>)> {
    let input = input.normalized();
    input.validate().map_err(AppError::bad_request)?;
    let race = crate::db::insert_race(&state.pool, &user.id, &input, state.now_ms()).await?;
    tracing::info!(user = %user.email, race = %race.id, "course enregistree par l'API");
    Ok((axum::http::StatusCode::CREATED, Json(race)))
}

/// Une course et son suivi.
async fn get_race(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let race = crate::db::get_race(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let tasks = crate::db::list_race_tasks(&state.pool, &user.id, &id).await?;
    Ok(Json(serde_json::json!({ "race": race, "tasks": tasks })))
}

/// Met a jour une fiche de course (les elements de suivi ne sont pas touches).
async fn update_race(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(input): Json<RaceInput>,
) -> AppResult<Json<Race>> {
    let input = input.normalized();
    input.validate().map_err(AppError::bad_request)?;
    let race = crate::db::update_race(&state.pool, &user.id, &id, &input, state.now_ms())
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(race))
}

/// Supprime une course et son suivi.
async fn delete_race(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if crate::db::delete_race(&state.pool, &user.id, &id).await? {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
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

// ------------------------------------------------------------------ musique

/// Liste des playlists avec leurs compteurs (titres, octets, pistes servables).
#[derive(Debug, Serialize)]
struct MusicPlaylistListResponse {
    playlists: Vec<crate::models::MusicPlaylistSummary>,
}

async fn list_music_playlists(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<MusicPlaylistListResponse>> {
    let playlists = crate::db::list_music_playlist_summaries(&state.pool, &user.id).await?;
    Ok(Json(MusicPlaylistListResponse { playlists }))
}

/// Televersement de fichiers audio par un appareil (montre ou compagnon).
///
/// Corps `multipart/form-data` : champ `name` (nom de playlist) et champs
/// `files` repetes. Les octets restent sur le serveur, prets pour
/// `GET /api/v1/music/tracks/{id}/file`.
async fn upload_music_playlist(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    let form = crate::media::collect_upload(multipart, "name").await?;
    let (playlist_id, track_count, total_bytes) =
        crate::media::import_uploaded_files(&state, &user.id, &form.name, &form.files).await?;
    tracing::info!(
        user = %user.email,
        playlist = %playlist_id,
        titres = track_count,
        octets = total_bytes,
        "playlist televersee par un appareil"
    );
    Ok(Json(serde_json::json!({
        "playlist_id": playlist_id,
        "name": form.name,
        "track_count": track_count,
        "total_bytes": total_bytes,
        "ready_track_count": track_count,
    })))
}

/// Fiche d'une playlist : ce que la montre telecharge pour une playlist Spotify.
async fn get_music_playlist(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<MusicPlaylistDetail>> {
    let playlist = crate::db::get_music_playlist(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let tracks = crate::db::list_music_tracks(&state.pool, &user.id, &id).await?;
    Ok(Json(MusicPlaylistDetail {
        id: playlist.id,
        name: playlist.name,
        source: playlist.source,
        target_bpm: playlist.target_bpm,
        tracks: tracks.iter().map(MusicTrackView::from_track).collect(),
    }))
}

/// Accuse la recuperation de pistes par la montre.
async fn ack_music_playlist(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(request): Json<MusicAckRequest>,
) -> AppResult<Json<serde_json::Value>> {
    crate::db::get_music_playlist(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let downloaded = crate::db::ack_music_tracks(
        &state.pool,
        &user.id,
        &id,
        &request.track_ids,
        state.now_ms(),
    )
    .await?;
    tracing::info!(user = %user.email, playlist = %id, downloaded, "pistes accusees par la montre");
    Ok(Json(
        serde_json::json!({ "playlist_id": id, "downloaded": downloaded }),
    ))
}

/// Octets audio d'un titre, avec support de `Range` (reprise d'un telechargement).
async fn music_track_file(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
) -> AppResult<Response> {
    let track = crate::db::get_music_track(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let Some(relative) = track.storage_path.as_deref().filter(|path| !path.is_empty()) else {
        // Une playlist Spotify n'a aucun octet sur le serveur : la montre
        // telecommande l'application Spotify installee.
        return Err(AppError::NotFound);
    };
    let path = crate::routes::media_path(&state.config.media_dir, relative)
        .ok_or_else(|| AppError::internal("chemin de fichier audio invalide"))?;
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(track = %id, "fichier audio absent du disque");
            return Err(AppError::NotFound);
        }
        Err(error) => return Err(AppError::internal(format!("lecture audio : {error}"))),
    };

    let mime = track
        .mime
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let total = bytes.len();
    let requested = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| parse_byte_range(value, total));

    let (status, start, end) = match requested {
        Some((start, end)) => (axum::http::StatusCode::PARTIAL_CONTENT, start, end),
        None => (axum::http::StatusCode::OK, 0, total.saturating_sub(1)),
    };
    let body = if total == 0 {
        Vec::new()
    } else {
        bytes[start..=end].to_vec()
    };

    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        // Un fichier audio ne change jamais : la montre peut le garder en cache.
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .header(header::CONTENT_LENGTH, body.len().to_string());
    if status == axum::http::StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        );
    }
    builder
        .body(Body::from(body))
        .map_err(|error| AppError::internal(error.to_string()))
}

/// Dernier plan de telechargement non acquitte (ou `{"plan": null}`).
async fn music_prepare(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<serde_json::Value>> {
    let Some(plan) = crate::db::pending_music_plan(&state.pool, &user.id).await? else {
        return Ok(Json(serde_json::json!({ "plan": null })));
    };
    let playlist = crate::db::get_music_playlist(&state.pool, &user.id, &plan.playlist_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let tracks = crate::db::list_music_tracks(&state.pool, &user.id, &plan.playlist_id).await?;
    let race_name = match plan.race_id.as_deref() {
        Some(race_id) => crate::db::get_race(&state.pool, &user.id, race_id)
            .await?
            .map(|race| race.name),
        None => None,
    };
    let view = MusicPlanView {
        id: plan.id,
        playlist_id: plan.playlist_id,
        name: playlist.name,
        target_bpm: plan.target_bpm,
        race_id: plan.race_id,
        race_name,
        requested_at_ms: plan.requested_at_ms,
        tracks: tracks.iter().map(MusicTrackView::from_track).collect(),
    };
    Ok(Json(serde_json::json!({ "plan": view })))
}

/// Acquitte un plan : la montre a termine son telechargement.
///
/// L'appel est idempotent : un plan deja acquitte renvoie de nouveau `ok`, ce
/// qui evite une erreur si la montre reessaie apres une coupure reseau.
async fn music_prepare_ack(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<MusicPlanAckRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let plan = crate::db::get_music_plan(&state.pool, &user.id, &request.plan_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if plan.acked_at_ms.is_none() {
        crate::db::ack_music_plan(&state.pool, &user.id, &plan.id, state.now_ms()).await?;
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Analyse d'un en-tete `Range: bytes=...` (bornes incluses).
///
/// Seule l'unite `bytes` est acceptee ; une demande illisible est ignoree et le
/// fichier complet est renvoye (comportement tolerant recommande par la RFC 9110).
fn parse_byte_range(value: &str, total: usize) -> Option<(usize, usize)> {
    let spec = value.trim().strip_prefix("bytes=")?.trim();
    if total == 0 || spec.contains(',') {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let suffix: usize = suffix.parse().ok()?;
            if suffix == 0 {
                return None;
            }
            (total.saturating_sub(suffix), total - 1)
        }
        (start, "") => (start.parse().ok()?, total - 1),
        (start, end) => (start.parse().ok()?, end.parse::<usize>().ok()?.min(total - 1)),
    };
    (start <= end && start < total).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::parse_byte_range;

    #[test]
    fn byte_ranges_are_parsed_inclusively() {
        assert_eq!(parse_byte_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_byte_range("bytes=100-", 1000), Some((100, 999)));
        assert_eq!(parse_byte_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_byte_range("bytes=500-99999", 1000), Some((500, 999)));
    }

    #[test]
    fn invalid_ranges_are_ignored() {
        assert_eq!(parse_byte_range("items=0-10", 1000), None);
        assert_eq!(parse_byte_range("bytes=10-5", 1000), None);
        assert_eq!(parse_byte_range("bytes=0-10,20-30", 1000), None);
        assert_eq!(parse_byte_range("bytes=abc-def", 1000), None);
        assert_eq!(parse_byte_range("bytes=0-0", 0), None);
    }
}
