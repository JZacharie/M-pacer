//! API de synchronisation consommee par la montre (et le simulateur).
//!
//! Authentification : `Authorization: Bearer <jeton d'appareil>`.
//! Les erreurs respectent le format `{"error": "code", "message": "..."}`.

use crate::auth::device::{
    self, DeviceCodeRequest, DeviceCodeResponse, DeviceTokenRequest, DeviceTokenResponse,
};
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::friends::{CirclePayload, InviteOutcome};
use crate::models::{
    MusicPlaylistDetail, MusicTrackView, Race, RaceInput, UploadResponse, WorkoutUpload,
};
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::header;
use axum::response::Response;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

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
        .route("/api/v1/friends", get(list_friends))
        .route("/api/v1/friends/invite", post(create_friend_invite))
        .route("/api/v1/friends/accept", post(accept_friend_invite))
        .route("/api/v1/friends/live", get(friends_live))
        .route("/api/v1/friends/share", put(set_friend_share))
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
        .route(
            "/api/v1/races/{id}",
            get(get_race).put(update_race).delete(delete_race),
        )
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/export", get(export_pac))
        .route("/api/v1/version", get(version))
        // Musique : metadonnees seules. La montre lit les fiches et le
        // manifeste ; l'audio est copie par USB depuis l'ordinateur avec
        // mpacer-music (aucun octet ne transite par le serveur).
        .route("/api/v1/music/playlists", get(list_music_playlists))
        .route("/api/v1/music/playlists/{id}", get(get_music_playlist))
        .route(
            "/api/v1/music/playlists/{id}/manifest",
            get(get_music_manifest),
        )
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

/// Manifeste de transfert d'une playlist (piece jointe JSON).
///
/// C'est le fichier que l'outil local `mpacer-music` consomme pour apparier les
/// fichiers du disque puis les copier sur la montre par USB.
async fn get_music_manifest(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    super::manifest_response(&state, &user.id, &id).await
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
    #[serde(default)]
    trace: bool,
}

async fn friends_live(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<FriendsLiveQuery>,
) -> AppResult<Json<CirclePayload>> {
    Ok(Json(
        crate::friends::payload(&state, &user.id, user.share_live, query.trace).await?,
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
