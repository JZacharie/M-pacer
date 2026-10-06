//! Interface web locale (127.0.0.1:8077) : page HTML autonome + API JSON.
//!
//! Les gestionnaires ne font que traduire les requetes en appels au
//! planificateur unique (`crate::planner`) : aucune regle n'est dupliquee ici.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::adb::{self, Device};
use crate::library::{LibraryFile, LibraryStore, MAX_FILE_BYTES};
use crate::planner::{self, Progress, ProgressSink, ToolError, TransferRequest, WATCH_MUSIC_DIR};

/// Etat partage du serveur local.
#[derive(Clone)]
pub struct AppState {
    pub adb: Option<PathBuf>,
    pub target_dir: Option<PathBuf>,
    /// Racine de la bibliotheque locale (None = aucun push possible).
    pub library: Option<PathBuf>,
    jobs: Arc<Mutex<HashMap<String, JobHandle>>>,
    counter: Arc<AtomicU64>,
}

impl AppState {
    pub fn new(bootstrap: Bootstrap) -> Self {
        Self {
            adb: bootstrap.adb,
            target_dir: bootstrap.target_dir,
            library: bootstrap.library,
            jobs: Arc::new(Mutex::new(HashMap::new())),
            counter: Arc::new(AtomicU64::new(0)),
        }
    }

    fn next_job_id(&self) -> String {
        let number = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("job-{}-{number}", std::process::id())
    }
}

/// Parametres de demarrage.
#[derive(Debug, Clone, Default)]
pub struct Bootstrap {
    pub adb: Option<PathBuf>,
    pub target_dir: Option<PathBuf>,
    /// Racine de la bibliotheque locale, alimentee par le push web.
    pub library: Option<PathBuf>,
}

/// Erreur d'API rendue en JSON.
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
}

impl From<ToolError> for ApiError {
    fn from(error: ToolError) -> Self {
        let status = if matches!(error, ToolError::Cancelled) {
            StatusCode::CONFLICT
        } else {
            StatusCode::BAD_REQUEST
        };
        Self {
            status,
            message: error.message(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

/// Monte le routeur de l'interface locale (utilise par `serve` et les tests).
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/browse", get(browse))
        .route("/api/devices", get(devices_handler))
        .route("/api/inspect", post(inspect_handler))
        .route("/api/transfer", post(transfer_handler))
        .route("/api/transfer/{job_id}", get(job_status))
        .route("/api/transfer/{job_id}/cancel", post(job_cancel))
        .route(
            "/api/library/{playlist_id}",
            get(library_list)
                .post(library_import)
                .layer(DefaultBodyLimit::max(MAX_FILE_BYTES as usize)),
        )
        .route("/api/library/{playlist_id}/{name}", delete(library_remove))
        .with_state(state)
}

/// Ecoute sur 127.0.0.1:port jusqu'a l'arret du processus.
pub async fn serve(port: u16, bootstrap: Bootstrap) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|error| format!("ecoute sur 127.0.0.1:{port} impossible : {error}"))?;
    axum::serve(listener, router(AppState::new(bootstrap)))
        .await
        .map_err(|error| format!("serveur arrete : {error}"))
}

/// Ouvre l'URL dans le navigateur par defaut (best effort).
pub fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(not(any(windows, target_os = "macos", unix)))]
    {
        let _ = url;
    }
}

async fn index() -> Html<&'static str> {
    Html(crate::web::PAGE)
}

// ------------------------------------------------------------------ browse

#[derive(Debug, Deserialize)]
struct BrowseQuery {
    path: Option<String>,
}

#[derive(Debug, Serialize)]
struct BrowseDir {
    name: String,
    path: String,
}

#[derive(Debug, Serialize)]
struct BrowseResponse {
    path: String,
    parent: Option<String>,
    dirs: Vec<BrowseDir>,
    audio_count: u64,
}

async fn browse(Query(query): Query<BrowseQuery>) -> Result<Json<BrowseResponse>, ApiError> {
    let path = match query.path.filter(|value| !value.trim().is_empty()) {
        Some(value) => PathBuf::from(value),
        None => home_directory().ok_or_else(|| ApiError::usage("dossier personnel introuvable"))?,
    };
    if !path.is_dir() {
        return Err(ApiError::usage(format!(
            "dossier introuvable : {}",
            path.display()
        )));
    }
    let mut dirs = Vec::new();
    let mut audio_count = 0_u64;
    let entries = fs::read_dir(&path).map_err(|error| {
        ApiError::usage(format!(
            "lecture de {} impossible : {error}",
            path.display()
        ))
    })?;
    for entry in entries.flatten() {
        let entry_path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            dirs.push(BrowseDir {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry_path.to_string_lossy().into_owned(),
            });
        } else if file_type.is_file() && planner::is_audio_path(&entry_path) {
            audio_count += 1;
        }
    }
    dirs.sort_by_key(|dir| dir.name.to_lowercase());
    Ok(Json(BrowseResponse {
        path: path.to_string_lossy().into_owned(),
        parent: path
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned()),
        dirs,
        audio_count,
    }))
}

fn home_directory() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// Traduit un champ texte facultatif en chemin (vide ou absent = None).
fn optional_path(value: &Option<String>) -> Option<PathBuf> {
    match value.as_deref().map(str::trim) {
        Some(trimmed) if !trimmed.is_empty() => Some(PathBuf::from(trimmed)),
        _ => None,
    }
}

/// Dossier de la bibliotheque pour une playlist, si la racine est configuree.
fn library_dir(state: &AppState, playlist_id: &str) -> Option<PathBuf> {
    state
        .library
        .as_ref()
        .map(|root| LibraryStore::new(root).playlist_dir(playlist_id))
}

/// Horodatage courant (ms depuis l'epoque).
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

/// Noms des fichiers de la bibliotheque qui seront copies par ce transfert.
fn managed_library_files(
    state: &AppState,
    playlist_id: &str,
    inspection: &planner::Inspection,
) -> Vec<String> {
    let Some(root) = state.library.as_ref() else {
        return Vec::new();
    };
    let store = LibraryStore::new(root);
    inspection
        .matches
        .iter()
        .filter_map(|entry| entry.path.as_ref())
        .filter(|path| store.is_managed_path(playlist_id, path))
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect()
}

fn mark_library_synced(store: &Option<LibraryStore>, playlist_id: &str, file_names: &[String]) {
    if let Some(store) = store {
        let _ = store.mark_synced(playlist_id, file_names, now_ms());
    }
}

fn mark_library_error(
    store: &Option<LibraryStore>,
    playlist_id: &str,
    file_names: &[String],
    message: &str,
) {
    if let Some(store) = store {
        for file_name in file_names {
            let _ = store.mark_error(playlist_id, file_name, message, now_ms());
        }
    }
}

// ----------------------------------------------------------------- devices

#[derive(Debug, Serialize)]
struct DevicesResponse {
    adb: String,
    devices: Vec<Device>,
}

async fn devices_handler(State(state): State<AppState>) -> Json<DevicesResponse> {
    let Some(adb_path) = state.adb.clone() else {
        return Json(DevicesResponse {
            adb: "absent".to_string(),
            devices: Vec::new(),
        });
    };
    let devices = adb::devices(&adb_path, WATCH_MUSIC_DIR).unwrap_or_default();
    Json(DevicesResponse {
        adb: adb_path.display().to_string(),
        devices,
    })
}

// ----------------------------------------------------------------- inspect

#[derive(Debug, Deserialize)]
struct InspectRequest {
    manifest_path: Option<String>,
    manifest_json: Option<String>,
    folder: Option<String>,
}

async fn inspect_handler(
    State(state): State<AppState>,
    Json(request): Json<InspectRequest>,
) -> Result<Json<planner::Inspection>, ApiError> {
    let manifest = planner::load_manifest(
        request.manifest_path.as_deref(),
        request.manifest_json.as_deref(),
    )?;
    let folder = optional_path(&request.folder);
    let library = library_dir(&state, &manifest.playlist_id);
    if folder.is_none() && library.is_none() {
        return Err(ApiError::usage(
            "aucun dossier audio : designer un dossier du disque ou configurer la bibliotheque",
        ));
    }
    let inspection =
        planner::inspect_with_library(&manifest, folder.as_deref(), library.as_deref())?;
    Ok(Json(inspection))
}

// ---------------------------------------------------------------- transfert

#[derive(Debug, Clone, Serialize)]
pub struct JobState {
    pub state: String,
    pub step: String,
    pub current: usize,
    pub total: usize,
    pub bytes_sent: u64,
    pub error: Option<String>,
    pub logs: Vec<String>,
}

impl JobState {
    fn running() -> Self {
        Self {
            state: "running".to_string(),
            step: "appariement".to_string(),
            current: 0,
            total: 0,
            bytes_sent: 0,
            error: None,
            logs: Vec::new(),
        }
    }
}

struct JobHandle {
    shared: Arc<Mutex<JobState>>,
    cancel: Arc<AtomicBool>,
}

struct JobSink {
    shared: Arc<Mutex<JobState>>,
}

impl ProgressSink for JobSink {
    fn progress(&self, progress: &Progress) {
        if let Ok(mut job) = self.shared.lock() {
            job.step = progress.step.clone();
            job.current = progress.current;
            job.total = progress.total;
            job.bytes_sent = progress.bytes_sent;
        }
    }

    fn log(&self, message: &str) {
        if let Ok(mut job) = self.shared.lock() {
            job.logs.push(message.to_string());
        }
    }
}

#[derive(Debug, Deserialize)]
struct TransferRequestJson {
    manifest_path: Option<String>,
    manifest_json: Option<String>,
    folder: Option<String>,
    serial: Option<String>,
    #[serde(default)]
    prune: bool,
    #[serde(default)]
    dry_run: bool,
    #[serde(default)]
    strict: bool,
    target_dir: Option<String>,
}

#[derive(Debug, Serialize)]
struct JobCreated {
    job_id: String,
}

async fn transfer_handler(
    State(state): State<AppState>,
    Json(request): Json<TransferRequestJson>,
) -> Result<Json<JobCreated>, ApiError> {
    let manifest = planner::load_manifest(
        request.manifest_path.as_deref(),
        request.manifest_json.as_deref(),
    )?;
    let folder = optional_path(&request.folder);
    let library = library_dir(&state, &manifest.playlist_id);
    if folder.is_none() && library.is_none() {
        return Err(ApiError::usage(
            "aucun dossier audio : designer un dossier du disque ou configurer la bibliotheque",
        ));
    }
    // Inspection faite avant le spawn : elle fixe les fichiers de la
    // bibliotheque a marquer selon l'issue du transfert.
    let inspection =
        planner::inspect_with_library(&manifest, folder.as_deref(), library.as_deref())?;
    let playlist_id = manifest.playlist_id.clone();
    let managed = managed_library_files(&state, &playlist_id, &inspection);
    let store = state.library.as_ref().map(LibraryStore::new);
    let dry_run = request.dry_run;

    let job_id = state.next_job_id();
    let cancel = Arc::new(AtomicBool::new(false));
    let shared = Arc::new(Mutex::new(JobState::running()));
    state.jobs.lock().expect("verrou des jobs").insert(
        job_id.clone(),
        JobHandle {
            shared: shared.clone(),
            cancel: cancel.clone(),
        },
    );

    let mut transfer_request = TransferRequest::new(manifest, folder.unwrap_or_default());
    transfer_request.library = library;
    transfer_request.serial = request.serial;
    transfer_request.prune = request.prune;
    transfer_request.dry_run = dry_run;
    transfer_request.strict = request.strict;
    transfer_request.adb = state.adb.clone();
    transfer_request.target_dir = request
        .target_dir
        .map(PathBuf::from)
        .or_else(|| state.target_dir.clone());
    transfer_request.cancel = Some(cancel);

    tokio::task::spawn_blocking(move || {
        let sink = JobSink {
            shared: shared.clone(),
        };
        match planner::transfer(&transfer_request, &sink) {
            Ok(_report) => {
                if !dry_run {
                    mark_library_synced(&store, &playlist_id, &managed);
                }
                finish(&shared, "done", "termine");
            }
            Err(error) => {
                // Une annulation n'est pas un echec : les fichiers restent
                // "a_synchroniser" et seront copies au prochain transfert.
                let cancelled = matches!(error, ToolError::Cancelled);
                if !dry_run && !cancelled {
                    mark_library_error(&store, &playlist_id, &managed, &error.message());
                }
                if cancelled {
                    finish(&shared, "cancelled", "annule");
                } else {
                    fail(&shared, &error);
                }
            }
        }
    });

    Ok(Json(JobCreated { job_id }))
}

async fn job_status(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<JobState>, ApiError> {
    let snapshot = with_job(&state, &job_id, |handle| {
        handle.shared.lock().expect("verrou du job").clone()
    })?;
    Ok(Json(snapshot))
}

async fn job_cancel(
    State(state): State<AppState>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    with_job(&state, &job_id, |handle| {
        handle.cancel.store(true, Ordering::SeqCst);
        if let Ok(mut job) = handle.shared.lock() {
            if job.state == "running" {
                job.step = "annulation".to_string();
            }
        }
    })?;
    Ok(Json(json!({ "cancelled": true })))
}

fn with_job<T>(
    state: &AppState,
    job_id: &str,
    action: impl FnOnce(&JobHandle) -> T,
) -> Result<T, ApiError> {
    let jobs = state.jobs.lock().expect("verrou des jobs");
    let handle = jobs
        .get(job_id)
        .ok_or_else(|| ApiError::not_found(format!("transfert inconnu : {job_id}")))?;
    Ok(action(handle))
}

fn finish(shared: &Arc<Mutex<JobState>>, state: &str, step: &str) {
    if let Ok(mut job) = shared.lock() {
        job.state = state.to_string();
        job.step = step.to_string();
    }
}

fn fail(shared: &Arc<Mutex<JobState>>, error: &ToolError) {
    if let Ok(mut job) = shared.lock() {
        job.state = "failed".to_string();
        job.step = "erreur".to_string();
        job.error = Some(error.message());
    }
}

// ------------------------------------------------------------- bibliotheque

#[derive(Debug, Deserialize)]
struct LibraryQuery {
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct LibraryList {
    files: Vec<LibraryFile>,
}

/// Ajoute un fichier a la bibliotheque (corps brut, nom en parametre).
async fn library_import(
    State(state): State<AppState>,
    AxumPath(playlist_id): AxumPath<String>,
    Query(query): Query<LibraryQuery>,
    body: Bytes,
) -> Result<Json<LibraryFile>, ApiError> {
    let root = state
        .library
        .clone()
        .ok_or_else(|| ApiError::usage("bibliotheque non configuree : passer --library"))?;
    let store = LibraryStore::new(root);
    let file_name = query.name.unwrap_or_default();
    Ok(Json(store.import_bytes(
        &playlist_id,
        &file_name,
        &body,
        now_ms(),
    )?))
}

/// Liste les fichiers de la bibliotheque d'une playlist.
async fn library_list(
    State(state): State<AppState>,
    AxumPath(playlist_id): AxumPath<String>,
) -> Result<Json<LibraryList>, ApiError> {
    let root = state
        .library
        .clone()
        .ok_or_else(|| ApiError::usage("bibliotheque non configuree : passer --library"))?;
    let store = LibraryStore::new(root);
    Ok(Json(LibraryList {
        files: store.list(&playlist_id)?,
    }))
}

/// Supprime un fichier de la bibliotheque d'une playlist.
async fn library_remove(
    State(state): State<AppState>,
    AxumPath((playlist_id, name)): AxumPath<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let root = state
        .library
        .clone()
        .ok_or_else(|| ApiError::usage("bibliotheque non configuree : passer --library"))?;
    let store = LibraryStore::new(root);
    let removed = store.remove(&playlist_id, &name)?;
    Ok(Json(json!({ "removed": removed })))
}
