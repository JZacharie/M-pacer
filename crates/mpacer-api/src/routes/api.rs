//! API de synchronisation consommee par la montre (et le simulateur).
//!
//! Authentification : `Authorization: Bearer <jeton d'appareil>`.
//! Les erreurs respectent le format `{"error": "code", "message": "..."}`.

use crate::auth::device::{
    self, DeviceCodeRequest, DeviceCodeResponse, DeviceTokenRequest, DeviceTokenResponse,
};
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::finishers::{FinishersRace, RaceSearchParams, RaceSearchQuery, RaceSearchResponse};
use crate::friends::{CirclePayload, FriendRequestView, InviteOutcome, RequestOutcome};
use crate::models::{
    MusicPlaylistDetail, MusicStoredFile, MusicTrackView, Race, RaceInput, UploadResponse, User,
    UserCard, WorkoutUpload,
};
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// Routes API (prefixe `/api/v1`).
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/device/code", post(device_code))
        .route("/api/v1/device/token", post(device_token))
        .route("/api/v1/me", get(me))
        // Amis et partage de la position en direct. L'appareil qui publie
        // revendique d'abord son nom (sujet MQTT), puis ses amis voient la
        // position tant que la seance dure et que le partage est actif.
        .route("/api/v1/live/register", post(register_live_device))
        .route("/api/v1/live/route", put(set_live_route))
        .route("/api/v1/friends", get(list_friends))
        .route("/api/v1/friends/invite", post(create_friend_invite))
        .route("/api/v1/friends/accept", post(accept_friend_invite))
        .route("/api/v1/friends/live", get(friends_live))
        .route("/api/v1/friends/share", put(set_friend_share))
        // Demandes d'amitie : on cherche un compte M-pacer par son adresse ou
        // son nom, on envoie une demande, et l'autre la valide. Aucun
        // identifiant de montre ou d'appareil n'entre dans le cercle.
        .route("/api/v1/friends/search", get(search_friends))
        .route(
            "/api/v1/friends/requests",
            get(list_friend_requests).post(send_friend_request),
        )
        .route(
            "/api/v1/friends/requests/{id}/accept",
            post(accept_friend_request),
        )
        .route(
            "/api/v1/friends/requests/{id}/decline",
            post(decline_friend_request),
        )
        // Annulation par l'expediteur : sa demande en attente disparait.
        .route(
            "/api/v1/friends/requests/{id}",
            delete(cancel_friend_request),
        )
        .route("/api/v1/friends/{id}", delete(remove_friend))
        .route("/api/v1/workouts", post(upload_workout).get(list_workouts))
        .route(
            "/api/v1/workouts/{id}",
            get(get_workout).delete(delete_workout),
        )
        .route("/api/v1/workouts/{id}/gpx", get(workout_gpx))
        .route("/api/v1/workouts/{id}/kml", get(workout_kml))
        // Commentaire du coureur apres une seance (texte vide pour l'effacer).
        .route("/api/v1/workouts/{id}/comment", put(set_workout_comment))
        .route("/api/v1/races", get(list_races).post(create_race))
        // Import d'une ancienne course (export Strava/Garmin) : le corps
        // multipart porte un fichier GPX ou TCX, plafonne comme la musique.
        .route(
            "/api/v1/races/import",
            post(import_race).layer(DefaultBodyLimit::max(
                mpacer_core::race_import::MAX_IMPORT_BYTES,
            )),
        )
        // Recherche dans le calendrier Finishers (docs/05) : la meme source que
        // la page /courses/recherche, en JSON pour un client mobile ou une montre.
        .route("/api/v1/races/search", get(search_races))
        .route("/api/v1/races/finishers/{event}", get(finishers_race))
        .route(
            "/api/v1/races/{id}",
            get(get_race).put(update_race).delete(delete_race),
        )
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/export", get(export_pac))
        .route("/api/v1/version", get(version))
        // Musique : la montre lit les fiches, le manifeste et — quand un volume
        // est configure (MPACER_MEDIA_DIR) — les MP3 deposes depuis la page
        // /music. Elle acquitte chaque piste recue, ce qui libere le serveur.
        .route("/api/v1/music/playlists", get(list_music_playlists))
        .route("/api/v1/music/playlists/{id}", get(get_music_playlist))
        .route(
            "/api/v1/music/playlists/{id}/manifest",
            get(get_music_manifest),
        )
        .route("/api/v1/music/playlists/{id}/files", get(list_music_files))
        .route(
            "/api/v1/music/playlists/{id}/tracks/{track_id}/file",
            get(download_music_track),
        )
        .route("/api/v1/music/playlists/{id}/ack", post(ack_music_playlist))
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

#[derive(Debug, Deserialize)]
struct CommentRequest {
    /// Texte du commentaire ; vide pour effacer celui qui existe.
    #[serde(default)]
    comment: String,
}

/// Enregistre le commentaire d'une seance.
///
/// Le commentaire ne fait pas partie du `payload` envoye par la montre : une
/// nouvelle synchronisation de la seance ne l'efface donc jamais.
async fn set_workout_comment(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(request): Json<CommentRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let comment = crate::models::clean_comment(&request.comment);
    let updated =
        crate::db::set_workout_comment(&state.pool, &user.id, &id, comment.as_deref()).await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Json(serde_json::json!({ "id": id, "comment": comment })))
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

/// Export KML d'une seance (Google Earth, cartes hors ligne).
async fn workout_kml(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    super::kml_response(&state, &user.id, &id).await
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

/// Recherche dans le calendrier Finishers (courses a venir en France).
///
/// Memes criteres que la page `/courses/recherche` (docs/05) : texte libre,
/// region, departement, ville, discipline, mois, annee, distances, tri et
/// pagination. Une valeur illisible est refusee avec un message explicite.
async fn search_races(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Query(params): Query<RaceSearchParams>,
) -> AppResult<Json<RaceSearchResponse>> {
    let query = RaceSearchQuery::parse(&params).map_err(AppError::bad_request)?;
    Ok(Json(state.finishers.search(&query).await?))
}

/// Fiche d'une course du calendrier Finishers, par son identifiant d'epreuve.
async fn finishers_race(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(event): Path<String>,
) -> AppResult<Json<FinishersRace>> {
    state
        .finishers
        .event(&event)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

/// Importe une ancienne course depuis un export Strava ou Garmin.
///
/// Corps multipart : un fichier GPX ou TCX (champ libre). La course creee est
/// une course de reference, datee du jour de la course importee.
async fn import_race(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    multipart: Multipart,
) -> AppResult<(axum::http::StatusCode, Json<Race>)> {
    let (filename, bytes) = crate::race_import::collect_race_file(multipart).await?;
    let race =
        crate::race_import::import_uploaded_race(&state, &user.id, &filename, &bytes).await?;
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

/// Liste des playlists avec leurs compteurs (titres, duree cumulee).
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

/// Fiche d'une playlist : metadonnees seules, plus l'adresse de son manifeste.
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
        manifest_url: crate::models::manifest_url(&playlist.id),
        id: playlist.id,
        name: playlist.name,
        source: playlist.source,
        target_bpm: playlist.target_bpm,
        tracks: tracks.iter().map(MusicTrackView::from_track).collect(),
    }))
}

/// Options du manifeste de transfert.
#[derive(Debug, Deserialize)]
struct ManifestQuery {
    /// `files=1` : remplir `file` et `size_bytes` pour les pistes dont les
    /// octets sont stockes sur le serveur (ce que l'appareil ecrit sur son disque).
    #[serde(default)]
    files: Option<String>,
}

/// Manifeste de transfert d'une playlist (piece jointe JSON).
///
/// Sans option, c'est le fichier que l'outil local `mpacer-music` consomme pour
/// apparier les fichiers du disque puis les copier sur la montre par USB. Avec
/// `?files=1`, c'est le manifeste pret pour l'appareil : il porte le nom et la
/// taille des MP3 deposes sur le serveur (`GET .../files`).
async fn get_music_manifest(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Query(query): Query<ManifestQuery>,
) -> AppResult<Response> {
    let with_files = query.files.as_deref() == Some("1");
    if with_files {
        super::manifest_response_with_files(&state, &user.id, &id).await
    } else {
        super::manifest_response(&state, &user.id, &id).await
    }
}

/// Reponse de la liste des MP3 stockes.
#[derive(Debug, Serialize)]
struct MusicFilesResponse {
    playlist_id: String,
    files: Vec<MusicStoredFile>,
}

/// MP3 d'une playlist presents sur le serveur (API jeton).
///
/// Sans volume audio, la liste est vide : la montre ne telecharge rien et
/// continue de lire ce que `mpacer-music` a copie par USB.
async fn list_music_files(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<MusicFilesResponse>> {
    crate::db::get_music_playlist(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let tracks = crate::db::list_stored_music_tracks(&state.pool, &user.id, &id).await?;
    let files = tracks
        .iter()
        .filter_map(MusicStoredFile::from_track)
        .collect();
    Ok(Json(MusicFilesResponse {
        playlist_id: id,
        files,
    }))
}

/// Intervalle demande par l'en-tete `Range`, borne au fichier.
///
/// Seul `bytes=debut-\[fin\]` est accepte : c'est ce qu'utilise le client
/// Android pour reprendre un telechargement interrompu.
fn parse_range(value: Option<&str>, total: u64) -> Result<Option<(u64, u64)>, AppError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let Some(spec) = value.strip_prefix("bytes=") else {
        return Err(AppError::BadRequest(
            "en-tete Range illisible : bytes=debut-fin attendu".to_string(),
        ));
    };
    let (debut, fin) = spec.split_once('-').ok_or_else(|| {
        AppError::BadRequest("en-tete Range illisible : bytes=debut-fin attendu".to_string())
    })?;
    let debut: u64 = debut.trim().parse().map_err(|_| {
        AppError::BadRequest("en-tete Range illisible : debut numerique attendu".to_string())
    })?;
    if debut >= total {
        return Err(AppError::BadRequest(format!(
            "plage hors du fichier : debut {debut}, taille {total}"
        )));
    }
    let fin = if fin.trim().is_empty() {
        total - 1
    } else {
        fin.trim()
            .parse::<u64>()
            .map_err(|_| {
                AppError::BadRequest("en-tete Range illisible : fin numerique attendue".to_string())
            })?
            .min(total - 1)
    };
    if fin < debut {
        return Err(AppError::BadRequest(
            "en-tete Range illisible : fin avant le debut".to_string(),
        ));
    }
    Ok(Some((debut, fin)))
}

/// Telecharge le MP3 d'une piste, avec reprise sur coupure (`Range`).
async fn download_music_track(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((id, track_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let Some(root) = state.config.media_dir.clone() else {
        return Err(AppError::Unavailable(
            "le depot d'audio est eteint sur ce serveur".to_string(),
        ));
    };
    crate::db::get_music_playlist(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let ids = vec![track_id.clone()];
    let mut tracks =
        crate::db::stored_music_tracks_by_ids(&state.pool, &user.id, &id, &ids).await?;
    let track = tracks.pop().ok_or(AppError::NotFound)?;
    let key = track
        .storage_path
        .clone()
        .ok_or_else(|| AppError::NotFound)?;
    let path = crate::media::resolve(&root, &key)
        .ok_or_else(|| AppError::internal("chemin audio invalide"))?;
    let total = tokio::fs::metadata(&path)
        .await
        .map_err(|_| AppError::NotFound)?
        .len();
    let file_name = crate::models::stored_file_name(&track, &key);
    let mime = track
        .mime
        .clone()
        .unwrap_or_else(|| crate::media::content_type(&key).to_string());

    let range = parse_range(
        headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok()),
        total,
    )?;
    let mut file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| AppError::NotFound)?;
    let (debut, fin, status) = match range {
        Some((debut, fin)) => (debut, fin, StatusCode::PARTIAL_CONTENT),
        None => (0, total.saturating_sub(1), StatusCode::OK),
    };
    let longueur = if total == 0 { 0 } else { fin - debut + 1 };
    if debut > 0 {
        file.seek(std::io::SeekFrom::Start(debut))
            .await
            .map_err(|error| AppError::internal(error.to_string()))?;
    }
    let flux = tokio_util::io::ReaderStream::new(file.take(longueur));
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, longueur.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{file_name}\""),
        );
    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {debut}-{fin}/{total}"),
        );
    }
    builder
        .body(Body::from_stream(flux))
        .map_err(|error| AppError::internal(error.to_string()))
}

/// Acquitte des MP3 : l'appareil les a ecrits, le serveur les supprime.
///
/// Un corps vide (ou `track_ids` vide) acquitte tous les fichiers stockes de la
/// playlist. La fiche de la piste reste : seule la copie serveur disparait.
async fn ack_music_playlist(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(request): Json<MusicAckRequest>,
) -> AppResult<Json<serde_json::Value>> {
    crate::db::get_music_playlist(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let tracks = if request.track_ids.is_empty() {
        crate::db::list_stored_music_tracks(&state.pool, &user.id, &id).await?
    } else {
        crate::db::stored_music_tracks_by_ids(&state.pool, &user.id, &id, &request.track_ids)
            .await?
    };
    if let Some(root) = state.config.media_dir.as_ref() {
        for track in &tracks {
            if let Some(key) = track.storage_path.as_deref() {
                if let Err(error) = crate::media::remove_file(root, key) {
                    tracing::warn!(error = %error, "fichier audio non supprime");
                }
            }
        }
    }
    let ids: Vec<String> = tracks.iter().map(|track| track.id.clone()).collect();
    let acked =
        crate::db::ack_music_tracks(&state.pool, &user.id, &id, &ids, state.now_ms()).await?;
    tracing::info!(utilisateur = %user.id, playlist = %id, pistes = acked, "MP3 acquittes et supprimes du serveur");
    Ok(Json(serde_json::json!({ "acked": acked })))
}

/// Corps de l'acquittement : les pistes recues par l'appareil.
#[derive(Debug, Default, Deserialize)]
struct MusicAckRequest {
    #[serde(default)]
    track_ids: Vec<String>,
}

// ------------------------------------------------------------------ amis

/// Demande de revendication d'un appareil en direct.
#[derive(Debug, Deserialize)]
struct LiveRegisterRequest {
    /// Nom de l'appareil, tel qu'il apparait dans le sujet MQTT
    /// (`<prefixe>/live/<montre>`).
    device: String,
    /// Libelle lisible ("Pixel 8", "Galaxy Watch 6"), facultatif.
    #[serde(default)]
    label: Option<String>,
}

/// Revendique un appareil pour le partage en direct.
///
/// Appele par la montre ou le telephone au depart d'une seance. C'est ce qui
/// relie le nom du sujet MQTT a un compte, et donc ce qui decide qui peut voir
/// la position : sans cette revendication, aucune trace ne sort du serveur.
async fn register_live_device(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<LiveRegisterRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let device = crate::friends::clean_device_name(&request.device);
    if device.is_empty() {
        return Err(AppError::bad_request(
            "nom d'appareil vide : renseignez le nom de l'appareil dans les reglages",
        ));
    }
    let maintenant = state.now_ms();
    // Un appareil muet depuis plus de 24 h n'a plus a bloquer son nom.
    let _ =
        crate::db::prune_live_devices(&state.pool, maintenant - crate::live::SESSION_TTL_MS).await;
    let label = request
        .label
        .as_deref()
        .map(str::trim)
        .filter(|valeur| !valeur.is_empty());
    if !crate::db::claim_live_device(&state.pool, &user.id, &device, label, maintenant).await? {
        return Err(AppError::Conflict(format!(
            "l'appareil '{device}' appartient deja a un autre compte : choisissez un autre nom dans les reglages"
        )));
    }
    tracing::info!(user = %user.email, appareil = %device, "appareil en direct revendique");
    Ok(Json(
        serde_json::json!({ "device": device, "registered": true }),
    ))
}

/// Parcours planifie publie par un appareil au depart d'une seance.
#[derive(Debug, Deserialize)]
struct LiveRouteRequest {
    /// Nom de l'appareil, deja revendique par ce compte.
    device: String,
    /// Points du parcours, dans l'ordre : `[[lat, lon], ...]`.
    ///
    /// Une liste vide efface le parcours (fin de seance, changement de trace).
    #[serde(default)]
    points: Vec<[f64; 2]>,
}

/// Enregistre (ou efface) le parcours planifie d'un appareil.
///
/// Les suiveurs voient alors le trace prevu **et** la position courante, avec
/// le pourcentage de parcours deja couvert : c'est ce qui repond a « ou en
/// est-il sur son parcours ? ».
///
/// Le nom d'appareil doit etre revendique par le compte (`/api/v1/live/register`)
/// avant d'y accrocher un parcours : personne ne peut ainsi planter un faux
/// trace sous le nom d'un autre coureur.
async fn set_live_route(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<LiveRouteRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let device = crate::friends::clean_device_name(&request.device);
    if device.is_empty() {
        return Err(AppError::bad_request(
            "nom d'appareil vide : renseignez le nom de l'appareil dans les reglages",
        ));
    }
    let appareils = crate::db::list_live_devices(&state.pool, &[user.id.clone()]).await?;
    if !appareils.iter().any(|appareil| appareil.device == device) {
        return Err(AppError::bad_request(
            "revendiquez d'abord l'appareil (POST /api/v1/live/register) avant de publier un parcours",
        ));
    }
    if request.points.is_empty() {
        let efface = state.live.clear_route(&device);
        return Ok(Json(serde_json::json!({
            "device": device,
            "points": 0,
            "cleared": efface,
        })));
    }
    match state
        .live
        .set_route(&device, request.points, state.now_ms())
    {
        Ok(route) => {
            tracing::info!(
                user = %user.email,
                appareil = %device,
                points = route.points.len(),
                metres = route.total_m.round(),
                "parcours planifie publie"
            );
            Ok(Json(serde_json::json!({
                "device": device,
                "points": route.points.len(),
                "total_m": route.total_m,
            })))
        }
        Err(message) => Err(AppError::bad_request(message)),
    }
}

/// Liste du cercle : profils, partage et positions courantes.
async fn list_friends(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<CirclePayload>> {
    Ok(Json(
        crate::friends::payload(&state, &user.id, user.share_live, false).await?,
    ))
}

/// Positions seules, pour un rafraichissement frequent.
#[derive(Debug, Deserialize)]
struct FriendsLiveQuery {
    /// Inclure la trace de la seance (carte) ; absent, seules les positions
    /// courantes sont renvoyees, ce qui divise la charge par vingt.
    ///
    /// Laisse en texte : l'application telephone demande trace=1, la page web
    /// trace=true, et le deserialiseur de booleen de serde n'accepte que le
    /// second. Sans cette souplesse, trace=1 repond une erreur 400.
    #[serde(default)]
    trace: Option<String>,
}

impl FriendsLiveQuery {
    /// Vrai si la trace est demandee (trace=1, trace=true, trace=on).
    fn avec_trace(&self) -> bool {
        let Some(valeur) = self.trace.as_deref() else {
            return false;
        };
        matches!(
            valeur.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "on" | "oui"
        )
    }
}

async fn friends_live(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<FriendsLiveQuery>,
) -> AppResult<Json<CirclePayload>> {
    Ok(Json(
        crate::friends::payload(&state, &user.id, user.share_live, query.avec_trace()).await?,
    ))
}

#[derive(Debug, Deserialize)]
struct InviteRequest {
    /// Forcer un nouveau code au lieu de renvoyer celui qui court encore.
    #[serde(default)]
    nouvelle: bool,
}

#[derive(Debug, Serialize)]
struct InviteResponse {
    /// Code a dicter, dans sa forme lisible `BCDF-GHJK`.
    code: String,
    expires_at_ms: i64,
    expires_in_s: i64,
    /// Lien direct a envoyer : il pre-remplit le champ sur la page Amis.
    url: String,
}

/// Cree (ou renvoie) un code d'invitation a usage unique.
async fn create_friend_invite(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<InviteRequest>,
) -> AppResult<Json<InviteResponse>> {
    let maintenant = state.now_ms();
    if !request.nouvelle {
        if let Some((code, expiration)) =
            crate::db::current_friend_invite(&state.pool, &user.id, maintenant).await?
        {
            return Ok(Json(invite_response(&state, &code, expiration, maintenant)));
        }
    }
    // Le code est tire au hasard : on retente si la place est prise (collision
    // improbable, mais un code ne doit jamais ecraser celui d'un autre compte).
    for _ in 0..6 {
        let code = crate::friends::normalize_invite_code(&crate::friends::generate_invite_code());
        let expiration = maintenant + crate::friends::INVITE_TTL_MS;
        if crate::db::insert_friend_invite(&state.pool, &code, &user.id, maintenant, expiration)
            .await?
        {
            tracing::info!(user = %user.email, "code d'invitation emis");
            return Ok(Json(invite_response(&state, &code, expiration, maintenant)));
        }
    }
    Err(AppError::internal(
        "impossible de generer un code d'invitation (collisions repetees)",
    ))
}

fn invite_response(
    state: &AppState,
    code: &str,
    expires_at_ms: i64,
    now_ms: i64,
) -> InviteResponse {
    let affiche = crate::friends::format_invite_code(code);
    InviteResponse {
        code: affiche.clone(),
        expires_at_ms,
        expires_in_s: ((expires_at_ms - now_ms) / 1000).max(0),
        url: crate::friends::invite_url(&state.config.public_url, code),
    }
}

#[derive(Debug, Deserialize)]
struct AcceptRequest {
    code: String,
}

/// Accepte un code d'invitation : l'amitie est creee dans les deux sens.
async fn accept_friend_invite(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<AcceptRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if !crate::friends::is_invite_code(&request.code) {
        return Err(AppError::bad_request(
            "code d'invitation invalide : huit caracteres, du type BCDF-GHJK",
        ));
    }
    let code = crate::friends::normalize_invite_code(&request.code);
    match crate::db::accept_friend_invite(&state.pool, &code, &user.id, state.now_ms()).await? {
        InviteOutcome::Accepted(ami) => {
            tracing::info!(user = %user.email, ami = %ami.email, "amitie creee");
            Ok(Json(serde_json::json!({ "friend": ami })))
        }
        InviteOutcome::Unknown => Err(AppError::bad_request(
            "code inconnu ou deja utilise : demandez-en un nouveau",
        )),
        InviteOutcome::Expired => Err(AppError::bad_request(
            "code expire : demandez-en un nouveau",
        )),
        InviteOutcome::SelfInvite => Err(AppError::bad_request(
            "c'est votre propre code : envoyez-le a la personne a ajouter",
        )),
        InviteOutcome::TooMany => Err(AppError::bad_request(format!(
            "cercle plein : {} amis au maximum",
            crate::friends::MAX_FRIENDS
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct ShareRequest {
    share_live: bool,
}

/// Active ou coupe le partage de sa position avec ses amis.
async fn set_friend_share(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<ShareRequest>,
) -> AppResult<Json<serde_json::Value>> {
    crate::db::set_share_live(&state.pool, &user.id, request.share_live).await?;
    tracing::info!(user = %user.email, partage = request.share_live, "partage en direct regle");
    Ok(Json(
        serde_json::json!({ "share_live": request.share_live }),
    ))
}

/// Retire un ami : plus aucune position ne circule entre les deux comptes.
async fn remove_friend(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if crate::db::remove_friendship(&state.pool, &user.id, &id).await? {
        tracing::info!(user = %user.email, ami = %id, "amitie retiree");
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

// ------------------------------------------------- demandes d'amitie (API)

/// Fiche minimale d'un compte, pour la recherche et les demandes.
fn user_card_of(user: &User) -> UserCard {
    UserCard {
        id: user.id.clone(),
        name: user.name.clone(),
        email: user.email.clone(),
        picture_url: user.picture_url.clone(),
    }
}

/// Parametres de recherche : une adresse ou un nom, meme partiel.
#[derive(Debug, Deserialize)]
struct FriendSearchQuery {
    #[serde(default)]
    q: String,
}

/// Recherche un compte M-pacer par son adresse ou son nom.
///
/// Renvoie au plus [crate::friends::SEARCH_LIMIT] comptes, hors de soi-meme.
/// Une recherche trop courte ne renvoie rien : le service n'est pas un annuaire.
async fn search_friends(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<FriendSearchQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let recherche = crate::friends::clean_search_query(&query.q);
    if !crate::friends::is_searchable(&recherche) {
        return Ok(Json(serde_json::json!({ "query": recherche, "users": [] })));
    }
    let motif = crate::friends::like_pattern(&recherche);
    let cartes =
        crate::db::search_users(&state.pool, &motif, &user.id, crate::friends::SEARCH_LIMIT)
            .await?;
    let comptes: Vec<crate::friends::UserView> = cartes
        .iter()
        .map(crate::friends::UserView::from_card)
        .collect();
    Ok(Json(
        serde_json::json!({ "query": recherche, "users": comptes }),
    ))
}

/// Demandes recues et envoyees, avec le nombre de recues en attente.
#[derive(Debug, Serialize)]
struct FriendRequestsResponse {
    incoming: Vec<FriendRequestView>,
    outgoing: Vec<FriendRequestView>,
    pending: usize,
}

/// Liste les demandes d'amitie du compte.
async fn list_friend_requests(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<FriendRequestsResponse>> {
    let recues = crate::db::incoming_friend_requests(&state.pool, &user.id).await?;
    let envoyees = crate::db::outgoing_friend_requests(&state.pool, &user.id).await?;
    let pending = recues.len();
    Ok(Json(FriendRequestsResponse {
        incoming: recues
            .iter()
            .map(|row| FriendRequestView::from_row(row, &user.id))
            .collect(),
        outgoing: envoyees
            .iter()
            .map(|row| FriendRequestView::from_row(row, &user.id))
            .collect(),
        pending,
    }))
}

/// Corps d'une demande d'amitie : l'adresse du compte vise, et un mot.
#[derive(Debug, Deserialize)]
struct FriendRequestInput {
    /// Adresse du compte a ajouter (identite M-pacer, pas un nom d'appareil).
    email: String,
    #[serde(default)]
    message: Option<String>,
}

/// Envoie une demande d'amitie. L'amitie naitra quand l'autre l'acceptera.
async fn send_friend_request(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<FriendRequestInput>,
) -> AppResult<Json<serde_json::Value>> {
    let email = request.email.trim();
    if email.is_empty() {
        return Err(AppError::bad_request(
            "indiquez l'adresse du compte a ajouter",
        ));
    }
    let Some(cible) = crate::db::user_card_by_email(&state.pool, email).await? else {
        return Err(AppError::NotFound);
    };
    if cible.id == user.id {
        return Err(AppError::bad_request("c'est votre propre compte"));
    }
    let message =
        crate::friends::clean_request_message(request.message.as_deref().unwrap_or_default());
    let expediteur = user_card_of(&user);
    match crate::db::send_friend_request(
        &state.pool,
        &expediteur,
        &cible,
        message.as_deref(),
        state.now_ms(),
    )
    .await?
    {
        RequestOutcome::Sent(demande) => {
            tracing::info!(user = %user.email, cible = %cible.email, "demande d'amitie envoyee");
            Ok(Json(
                serde_json::json!({ "request": demande, "status": "pending" }),
            ))
        }
        RequestOutcome::AlreadyFriends => Err(AppError::Conflict(
            "vous etes deja amis avec ce compte".to_string(),
        )),
        RequestOutcome::AlreadySent => Err(AppError::Conflict(
            "une demande attend deja la reponse de ce compte".to_string(),
        )),
        RequestOutcome::AlreadyIncoming(ami) => {
            tracing::info!(user = %user.email, cible = %cible.email, "demande reciproque : amitie immediate");
            Ok(Json(
                serde_json::json!({ "friend": ami, "status": "accepted" }),
            ))
        }
        RequestOutcome::UnknownUser => Err(AppError::NotFound),
        RequestOutcome::SelfRequest => Err(AppError::bad_request("c'est votre propre compte")),
        RequestOutcome::TooMany => Err(AppError::Conflict(format!(
            "cercle plein : {} amis au maximum",
            crate::friends::MAX_FRIENDS
        ))),
        RequestOutcome::TooManyPending => Err(AppError::Conflict(
            "ce compte a trop de demandes en attente : reessayez plus tard".to_string(),
        )),
    }
}

/// Accepte une demande recue : l'amitie est creee dans les deux sens.
async fn accept_friend_request(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    match crate::db::accept_friend_request(&state.pool, &id, &user.id, state.now_ms()).await? {
        Some(ami) => {
            tracing::info!(user = %user.email, ami = %ami.email, "demande d'amitie acceptee");
            Ok(Json(serde_json::json!({ "friend": ami })))
        }
        None => Err(AppError::NotFound),
    }
}

/// Refuse une demande recue : elle disparait, sans creer d'amitie.
async fn decline_friend_request(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if crate::db::decline_friend_request(&state.pool, &id, &user.id).await? {
        tracing::info!(user = %user.email, demande = %id, "demande d'amitie refusee");
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

/// Annule une demande envoyee : elle disparait de la liste des deux comptes.
async fn cancel_friend_request(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if crate::db::cancel_friend_request(&state.pool, &id, &user.id).await? {
        tracing::info!(user = %user.email, demande = %id, "demande d'amitie annulee");
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_header_is_bounded_to_the_file() {
        // Sans en-tete : tout le fichier.
        assert_eq!(parse_range(None, 100).unwrap(), None);
        assert_eq!(parse_range(Some("  "), 100).unwrap(), None);
        // Plage bornee, puis ouverte jusqu'a la fin.
        assert_eq!(
            parse_range(Some("bytes=10-19"), 100).unwrap(),
            Some((10, 19))
        );
        assert_eq!(parse_range(Some("bytes=10-"), 100).unwrap(), Some((10, 99)));
        // Une fin au-dela du fichier est ramenee au dernier octet.
        assert_eq!(
            parse_range(Some("bytes=90-1000"), 100).unwrap(),
            Some((90, 99))
        );
    }

    #[test]
    fn an_impossible_range_is_refused() {
        assert!(parse_range(Some("elements=0-1"), 100).is_err());
        assert!(parse_range(Some("bytes=abc-"), 100).is_err());
        assert!(parse_range(Some("bytes=10"), 100).is_err());
        assert!(parse_range(Some("bytes=100-"), 100).is_err());
        assert!(parse_range(Some("bytes=20-10"), 100).is_err());
    }

    #[test]
    fn stored_files_are_named_for_the_watch() {
        use crate::models::{MusicStoredFile, MusicTrack};
        let track = MusicTrack {
            id: "t1".into(),
            playlist_id: "p1".into(),
            user_id: "u1".into(),
            position: 0,
            title: "Wake me up".into(),
            artist: Some("Avicii".into()),
            album: None,
            duration_s: Some(249.0),
            bpm: None,
            bpm_source: None,
            deezer_track_id: None,
            mime: Some("audio/mpeg".into()),
            size_bytes: Some(1_234),
            storage_path: Some("u1/t1/peu-importe.mp3".into()),
            created_at_ms: 0,
            downloaded_at_ms: None,
        };
        let file = MusicStoredFile::from_track(&track).expect("fichier stocke");
        assert_eq!(file.file_name, "01 - Avicii - Wake me up.mp3");
        assert_eq!(file.size_bytes, 1_234);
        assert_eq!(file.mime, "audio/mpeg");

        // Une piste sans octets sur le serveur n'apparait pas dans la liste.
        let vide = MusicTrack {
            storage_path: None,
            ..track
        };
        assert!(MusicStoredFile::from_track(&vide).is_none());
    }
}
