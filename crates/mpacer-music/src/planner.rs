//! Planificateur unique de `mpacer-music`.
//!
//! Un seul endroit pour : le scan recursif du dossier choisi, l'appariement
//! fichiers <-> pistes (`mpacer_core::music`), la lecture des balises BPM, le
//! controle de l'espace libre de la montre, la copie (adb ou dossier local) et
//! l'ecriture du `manifest.json` final. La CLI et l'interface web locale
//! appellent toutes les deux ces fonctions.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use mpacer_core::music::{
    bpm_from_tags, match_tracks, parse_manifest, suggested_file_name, LocalFile, TransferManifest,
};
use serde::Serialize;

/// Dossier musique de la montre (`getExternalFilesDir("Music")`).
pub const WATCH_MUSIC_DIR: &str = "/sdcard/Android/data/com.mpacer.watch/files/Music";
/// Dossier musique de l'application telephone (meme contrat que la montre).
pub const PHONE_MUSIC_DIR: &str = "/sdcard/Android/data/com.mpacer.phone/files/Music";
/// Paquet de l'application montre : sa presence distingue la cible d'un telephone.
pub const WATCH_PACKAGE: &str = "com.mpacer.watch";
/// Paquet de l'application telephone.
pub const PHONE_PACKAGE: &str = "com.mpacer.phone";
/// Octets lus au debut d'un fichier pour y chercher une balise BPM.
pub const TAG_PROBE_BYTES: u64 = 256 * 1024;
/// Extensions audio reconnues (contrat, section 3.1) : definies une seule
/// fois dans le coeur.
pub use mpacer_core::music::AUDIO_EXTENSIONS;

/// Erreur de l'outil, avec le code de sortie exact du contrat.
#[derive(Debug, Clone)]
pub enum ToolError {
    /// Usage ou entree invalide (code 2).
    Usage(String),
    /// adb introuvable (code 3).
    AdbMissing(String),
    /// Aucune montre disponible (code 4).
    NoDevice(String),
    /// Espace insuffisant sur la montre (code 5).
    NotEnoughSpace { needed: u64, free: u64 },
    /// Titres manquants avec --strict (code 6).
    MissingTracks(usize),
    /// Transfert interrompu par l'utilisateur.
    Cancelled,
    /// Erreur d'entree/sortie locale (code 2).
    Io(String),
}

impl ToolError {
    /// Code de sortie du contrat.
    pub fn exit_code(&self) -> i32 {
        match self {
            ToolError::AdbMissing(_) => 3,
            ToolError::NoDevice(_) => 4,
            ToolError::NotEnoughSpace { .. } => 5,
            ToolError::MissingTracks(_) => 6,
            ToolError::Cancelled => 0,
            ToolError::Usage(_) | ToolError::Io(_) => 2,
        }
    }

    /// Message utilisateur, sans accent.
    pub fn message(&self) -> String {
        match self {
            ToolError::Usage(message) | ToolError::Io(message) => message.clone(),
            ToolError::AdbMissing(message) | ToolError::NoDevice(message) => message.clone(),
            ToolError::NotEnoughSpace { needed, free } => format!(
                "espace insuffisant sur la montre : {needed} octets requis, {free} disponibles"
            ),
            ToolError::MissingTracks(count) => {
                format!("{count} titre(s) manquant(s) : transfert annule (--strict)")
            }
            ToolError::Cancelled => "transfert annule".to_string(),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message())
    }
}

impl std::error::Error for ToolError {}

/// Avancement d'un transfert, partage entre la CLI et l'interface web.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Progress {
    pub step: String,
    pub current: usize,
    pub total: usize,
    pub bytes_sent: u64,
}

/// Destinataire des mises a jour (impression CLI, job web, silence).
pub trait ProgressSink {
    fn progress(&self, progress: &Progress);
    fn log(&self, message: &str);
}

/// Sink silencieux : tests et appels sans suivi.
pub struct SilentSink;

impl ProgressSink for SilentSink {
    fn progress(&self, _progress: &Progress) {}
    fn log(&self, _message: &str) {}
}

// --------------------------------------------------------------------- scan

/// Vrai si le chemin porte une extension audio reconnue.
pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .is_some_and(|extension| AUDIO_EXTENSIONS.contains(&extension.as_str()))
}

/// Scan recursif du dossier : fichiers audio tries par chemin, balises BPM et
/// duree lues au passage. Un dossier vide renvoie une liste vide.
pub fn scan_folder(folder: &Path) -> Result<Vec<LocalFile>, ToolError> {
    if !folder.is_dir() {
        return Err(ToolError::Usage(format!(
            "dossier introuvable : {}",
            folder.display()
        )));
    }
    let mut files = Vec::new();
    collect_audio_files(folder, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_audio_files(directory: &Path, files: &mut Vec<LocalFile>) -> Result<(), ToolError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        ToolError::Io(format!(
            "lecture de {} impossible : {error}",
            directory.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ToolError::Io(format!(
                "lecture de {} impossible : {error}",
                directory.display()
            ))
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| ToolError::Io(format!("{} : {error}", path.display())))?;
        if file_type.is_dir() {
            collect_audio_files(&path, files)?;
        } else if file_type.is_file() && is_audio_path(&path) {
            let size_bytes = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
            files.push(LocalFile {
                path: path.to_string_lossy().into_owned(),
                file_name: entry.file_name().to_string_lossy().into_owned(),
                size_bytes,
                duration_s: probe_duration_s(&path),
                bpm: probe_bpm(&path),
            });
        }
    }
    Ok(())
}

/// Lit les 256 premiers Ko du fichier et cherche une balise BPM (ID3v2 TBPM,
/// commentaire Vorbis BPM=, atome MP4 tmpo).
pub fn probe_bpm(path: &Path) -> Option<f64> {
    let bytes = read_head(path, TAG_PROBE_BYTES).ok()?;
    let name = path.file_name()?.to_string_lossy().into_owned();
    bpm_from_tags(&bytes, &name)
}

fn read_head(path: &Path, limit: u64) -> Result<Vec<u8>, std::io::Error> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Duree estimee (s) : WAV par son en-tete, MP3 par le debit de la premiere
/// trame. None pour les autres formats : la duree reste optionnelle dans le
/// contrat et l'appariement s'en passe.
pub fn probe_duration_s(path: &Path) -> Option<f64> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let file_size = fs::metadata(path).ok()?.len();
    let bytes = read_head(path, TAG_PROBE_BYTES).ok()?;
    match extension.as_str() {
        "wav" => wav_duration_s(&bytes),
        "mp3" => mp3_duration_s(&bytes, file_size),
        _ => None,
    }
}

fn wav_duration_s(bytes: &[u8]) -> Option<f64> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF".as_slice() || &bytes[8..12] != b"WAVE".as_slice()
    {
        return None;
    }
    let mut position = 12usize;
    let mut byte_rate = 0u32;
    while position + 8 <= bytes.len() {
        let id = &bytes[position..position + 4];
        let size = u32::from_le_bytes(bytes[position + 4..position + 8].try_into().ok()?) as usize;
        let body = position + 8;
        if id == b"fmt ".as_slice() && body + 12 <= bytes.len() {
            byte_rate = u32::from_le_bytes(bytes[body + 8..body + 12].try_into().ok()?);
        } else if id == b"data".as_slice() {
            if byte_rate == 0 || size == 0 {
                return None;
            }
            return Some(size as f64 / byte_rate as f64);
        }
        position = body.checked_add(size + (size % 2))?;
    }
    None
}

fn mp3_duration_s(bytes: &[u8], file_size: u64) -> Option<f64> {
    let tag_size = id3v2_size(bytes);
    let mut position = tag_size.min(bytes.len());
    while position + 4 <= bytes.len() {
        if bytes[position] == 0xFF && bytes[position + 1] & 0xE0 == 0xE0 {
            if let Some(bitrate_kbps) = mp3_bitrate_kbps(&bytes[position..position + 4]) {
                let audio_bytes = file_size.saturating_sub(tag_size as u64);
                if audio_bytes == 0 {
                    return None;
                }
                return Some(audio_bytes as f64 * 8.0 / (bitrate_kbps as f64 * 1000.0));
            }
        }
        position += 1;
    }
    None
}

fn id3v2_size(bytes: &[u8]) -> usize {
    if bytes.len() < 10 || &bytes[0..3] != b"ID3".as_slice() {
        return 0;
    }
    if bytes[6] & 0x80 != 0 || bytes[7] & 0x80 != 0 || bytes[8] & 0x80 != 0 || bytes[9] & 0x80 != 0
    {
        return 0;
    }
    let size = ((bytes[6] as usize) << 21)
        | ((bytes[7] as usize) << 14)
        | ((bytes[8] as usize) << 7)
        | bytes[9] as usize;
    let footer = if bytes[3] == 4 && bytes[5] & 0x10 != 0 {
        10
    } else {
        0
    };
    10 + size + footer
}

fn mp3_bitrate_kbps(header: &[u8]) -> Option<u32> {
    const MPEG1_LAYER3: [u32; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    let version = (header[1] >> 3) & 0x03;
    let layer = (header[1] >> 1) & 0x03;
    let bitrate_index = ((header[2] >> 4) & 0x0F) as usize;
    // Version 3 = MPEG 1, layer 1 = Layer III.
    if version == 3 && layer == 1 {
        let bitrate = MPEG1_LAYER3[bitrate_index];
        (bitrate > 0).then_some(bitrate)
    } else {
        None
    }
}

// ---------------------------------------------------------------- inspection

/// Resume de la playlist inspectee.
#[derive(Debug, Clone, Serialize)]
pub struct PlaylistSummary {
    pub id: String,
    pub name: String,
    pub source: String,
    pub target_bpm: Option<f64>,
    pub track_count: usize,
}

/// Une piste apres appariement (une entree par piste du manifeste, dans
/// l'ordre). `file` vaut None quand rien n'a ete trouve.
#[derive(Debug, Clone, Serialize)]
pub struct MatchEntry {
    pub track_id: String,
    pub position: u32,
    pub title: String,
    pub artist: Option<String>,
    /// Nom du fichier tel qu'il sera ecrit sur la montre.
    pub file: Option<String>,
    pub score: f64,
    pub bpm: Option<f64>,
    pub size_bytes: Option<u64>,
    pub duration_s: Option<f64>,
    /// Chemin du fichier source (interne, non serialise).
    #[serde(skip)]
    pub path: Option<PathBuf>,
    /// Extension du fichier source (interne, non serialisee).
    #[serde(skip)]
    pub extension: Option<String>,
}

/// Piste signalee comme manquante.
#[derive(Debug, Clone, Serialize)]
pub struct MissingEntry {
    pub track_id: String,
    pub position: u32,
    pub title: String,
    pub artist: Option<String>,
    pub score: f64,
}

/// Fichier audio du dossier qui n'a servi a aucune piste.
#[derive(Debug, Clone, Serialize)]
pub struct UnusedEntry {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
}

/// Resultat complet de l'inspection (forme exacte de `/api/inspect`).
#[derive(Debug, Clone, Serialize)]
pub struct Inspection {
    pub playlist: PlaylistSummary,
    pub matches: Vec<MatchEntry>,
    pub missing: Vec<MissingEntry>,
    pub unused_files: Vec<UnusedEntry>,
    pub total_bytes: u64,
    /// Manifeste d'origine, interne : sert a ecrire le manifeste final.
    #[serde(skip)]
    pub manifest: TransferManifest,
}

impl Inspection {
    /// Nombre de titres apparies.
    pub fn matched_count(&self) -> usize {
        self.matches
            .iter()
            .filter(|entry| entry.file.is_some())
            .count()
    }

    /// Nombre de titres manquants.
    pub fn missing_count(&self) -> usize {
        self.missing.len()
    }

    /// Manifeste final : chaque piste appariee porte `file` et `size_bytes`.
    pub fn final_manifest(&self) -> TransferManifest {
        let mut manifest = self.manifest.clone();
        for (track, entry) in manifest.tracks.iter_mut().zip(self.matches.iter()) {
            track.file = entry.file.clone();
            track.size_bytes = entry.size_bytes;
        }
        manifest
    }
}

/// Apparie le manifeste au dossier choisi. Seule E/S : le scan du dossier.
pub fn inspect(manifest: &TransferManifest, folder: &Path) -> Result<Inspection, ToolError> {
    inspect_with_library(manifest, Some(folder), None)
}

/// Apparie le manifeste a un ou deux dossiers : le dossier choisi sur le disque
/// et/ou la bibliotheque geree par l'application. Les dossiers absents sont
/// ignores, les fichiers sont dedupliques par chemin, puis la logique est
/// exactement celle de inspect. Erreur seulement si aucun dossier n'est fourni.
pub fn inspect_with_library(
    manifest: &TransferManifest,
    folder: Option<&Path>,
    library: Option<&Path>,
) -> Result<Inspection, ToolError> {
    let mut provided = false;
    let mut files: Vec<LocalFile> = Vec::new();
    for directory in [folder, library].into_iter().flatten() {
        provided = true;
        if !directory.is_dir() {
            continue;
        }
        files.extend(scan_folder(directory)?);
    }
    if !provided {
        return Err(ToolError::Usage(
            "aucun dossier audio : designer un dossier du disque ou une bibliotheque".to_string(),
        ));
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files.dedup_by(|left, right| left.path == right.path);
    Ok(build_inspection(manifest, files))
}

/// Appariement pur, a partir des fichiers deja scannes : une entree par piste,
/// file vaut None quand rien n'a ete trouve, les autres fichiers du scan sont
/// listes dans unused_files.
fn build_inspection(manifest: &TransferManifest, files: Vec<LocalFile>) -> Inspection {
    let found = match_tracks(&manifest.tracks, &files);
    let mut matches = Vec::with_capacity(manifest.tracks.len());
    let mut missing = Vec::new();
    let mut used_paths: Vec<&str> = Vec::new();
    let mut total_bytes = 0_u64;

    for (track, result) in manifest.tracks.iter().zip(found.iter()) {
        let source = result.file.as_ref();
        let extension = source.map(|file| extension_of(&file.file_name));
        let file_name = match extension.as_deref() {
            Some(extension) if source.is_some() => Some(suggested_file_name(track, extension)),
            _ => None,
        };
        if let Some(source) = source {
            used_paths.push(source.path.as_str());
            total_bytes = total_bytes.saturating_add(source.size_bytes);
        }
        matches.push(MatchEntry {
            track_id: track.id.clone(),
            position: track.position,
            title: track.title.clone(),
            artist: track.artist.clone(),
            file: file_name,
            score: result.score,
            bpm: track.bpm.or_else(|| source.and_then(|file| file.bpm)),
            size_bytes: source.map(|file| file.size_bytes),
            duration_s: source.and_then(|file| file.duration_s).or(track.duration_s),
            path: source.map(|file| PathBuf::from(&file.path)),
            extension,
        });
        if source.is_none() {
            missing.push(MissingEntry {
                track_id: track.id.clone(),
                position: track.position,
                title: track.title.clone(),
                artist: track.artist.clone(),
                score: result.score,
            });
        }
    }

    let unused_files = files
        .iter()
        .filter(|file| !used_paths.contains(&file.path.as_str()))
        .map(|file| UnusedEntry {
            path: file.path.clone(),
            file_name: file.file_name.clone(),
            size_bytes: file.size_bytes,
        })
        .collect();

    Inspection {
        playlist: PlaylistSummary {
            id: manifest.playlist_id.clone(),
            name: manifest.name.clone(),
            source: manifest.source.clone(),
            target_bpm: manifest.target_bpm,
            track_count: manifest.tracks.len(),
        },
        matches,
        missing,
        unused_files,
        total_bytes,
        manifest: manifest.clone(),
    }
}

fn extension_of(file_name: &str) -> String {
    Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .unwrap_or_else(|| "mp3".to_string())
}

/// Charge un manifeste depuis un chemin ou depuis du JSON en ligne.
pub fn load_manifest(
    manifest_path: Option<&str>,
    manifest_json: Option<&str>,
) -> Result<TransferManifest, ToolError> {
    if let Some(path) = manifest_path {
        let json = fs::read_to_string(path)
            .map_err(|error| ToolError::Usage(format!("manifeste illisible ({path}) : {error}")))?;
        return parse_manifest(&json)
            .ok_or_else(|| ToolError::Usage(format!("manifeste invalide : {path}")));
    }
    if let Some(json) = manifest_json {
        return parse_manifest(json)
            .ok_or_else(|| ToolError::Usage("manifeste JSON invalide".to_string()));
    }
    Err(ToolError::Usage(
        "manifeste manquant : fournir manifest_path ou manifest_json".to_string(),
    ))
}

// ------------------------------------------------------------------- copie

/// Arborescence copiee localement (staging adb ou `--target-dir`).
#[derive(Debug, Clone)]
pub struct Materialized {
    pub playlist_dir: PathBuf,
    pub bytes: u64,
}

/// Copie les titres apparies sous `<root>/<playlist_id>/` et ecrit
/// `manifest.json`. Sert au staging avant `adb push` comme a
/// `--target-dir` (tests et simulation sans montre).
pub fn materialize(inspection: &Inspection, root: &Path) -> Result<Materialized, ToolError> {
    let playlist_dir = root.join(&inspection.playlist.id);
    fs::create_dir_all(&playlist_dir).map_err(|error| {
        ToolError::Io(format!(
            "creation de {} impossible : {error}",
            playlist_dir.display()
        ))
    })?;
    let mut bytes = 0_u64;
    for entry in copy_order(inspection) {
        let (Some(source), Some(name)) = (entry.path.as_ref(), entry.file.as_ref()) else {
            continue;
        };
        let destination = playlist_dir.join(name);
        fs::copy(source, &destination).map_err(|error| {
            ToolError::Io(format!(
                "copie de {} impossible : {error}",
                source.display()
            ))
        })?;
        bytes = bytes.saturating_add(
            fs::metadata(&destination)
                .map(|metadata| metadata.len())
                .unwrap_or(0),
        );
    }
    let manifest = inspection.final_manifest();
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|error| ToolError::Io(format!("manifeste non serialisable : {error}")))?;
    let manifest_path = playlist_dir.join("manifest.json");
    fs::write(&manifest_path, format!("{json}\n")).map_err(|error| {
        ToolError::Io(format!(
            "ecriture de {} impossible : {error}",
            manifest_path.display()
        ))
    })?;
    Ok(Materialized {
        playlist_dir,
        bytes,
    })
}

/// Ordre de copie : plus petits fichiers d'abord (tri par taille), puis par
/// position de piste puis identifiant. Un seul endroit pour cet ordre, partage
/// par la CLI, l'interface web et la copie locale.
pub fn copy_order(inspection: &Inspection) -> Vec<&MatchEntry> {
    let mut entries: Vec<&MatchEntry> = inspection
        .matches
        .iter()
        .filter(|entry| entry.file.is_some() && entry.path.is_some())
        .collect();
    entries.sort_by(|left, right| {
        left.size_bytes
            .unwrap_or(0)
            .cmp(&right.size_bytes.unwrap_or(0))
            .then_with(|| left.position.cmp(&right.position))
            .then_with(|| left.track_id.cmp(&right.track_id))
    });
    entries
}

// --------------------------------------------------------------- transfert

/// Demande de transfert, partagee par la CLI et l'interface web.
pub struct TransferRequest {
    pub manifest: TransferManifest,
    pub folder: PathBuf,
    /// Dossier supplementaire : la bibliotheque de cette playlist (facultatif).
    pub library: Option<PathBuf>,
    pub serial: Option<String>,
    pub prune: bool,
    pub dry_run: bool,
    pub strict: bool,
    /// Ecrit l'arborescence dans ce dossier au lieu d'utiliser adb.
    pub target_dir: Option<PathBuf>,
    /// Chemin d'adb deja resolu (None = pas d'adb disponible).
    pub adb: Option<PathBuf>,
    /// Dossier distant sur la montre.
    pub watch_dir: String,
    /// Drapeau d'annulation partage avec l'interface web.
    pub cancel: Option<Arc<AtomicBool>>,
}

impl TransferRequest {
    pub fn new(manifest: TransferManifest, folder: PathBuf) -> Self {
        Self {
            manifest,
            folder,
            library: None,
            serial: None,
            prune: false,
            dry_run: false,
            strict: false,
            target_dir: None,
            adb: None,
            watch_dir: WATCH_MUSIC_DIR.to_string(),
            cancel: None,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }
}

/// Compte rendu d'un transfert (ou de sa simulation).
#[derive(Debug, Clone, Serialize)]
pub struct TransferReport {
    pub playlist_id: String,
    pub name: String,
    pub mode: String,
    pub destination: String,
    pub matched: usize,
    pub missing: usize,
    pub unused_files: usize,
    pub copied_files: usize,
    pub bytes: u64,
    pub pruned: usize,
    pub dry_run: bool,
}

/// Execute un transfert, ou sa simulation. Seul chemin de copie, partage par
/// la CLI et l'interface web.
pub fn transfer(
    request: &TransferRequest,
    sink: &dyn ProgressSink,
) -> Result<TransferReport, ToolError> {
    sink.progress(&Progress {
        step: "appariement".to_string(),
        current: 0,
        total: request.manifest.tracks.len(),
        bytes_sent: 0,
    });
    let folder = (!request.folder.as_os_str().is_empty()).then_some(request.folder.as_path());
    let inspection = inspect_with_library(&request.manifest, folder, request.library.as_deref())?;
    let matched = inspection.matched_count();
    sink.log(&format!(
        "{matched} titre(s) apparie(s), {} manquant(s), {} fichier(s) ignore(s)",
        inspection.missing_count(),
        inspection.unused_files.len()
    ));
    if request.cancelled() {
        return Err(ToolError::Cancelled);
    }
    if request.strict && inspection.missing_count() > 0 {
        return Err(ToolError::MissingTracks(inspection.missing_count()));
    }

    let remote_root = format!(
        "{}/{}",
        request.watch_dir.trim_end_matches('/'),
        inspection.playlist.id
    );

    if request.dry_run {
        for entry in copy_order(&inspection) {
            sink.log(&format!(
                "copierait {} -> {remote_root}/{}",
                entry
                    .path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
                entry.file.clone().unwrap_or_default()
            ));
        }
        return Ok(report(
            &inspection,
            "simulation",
            remote_root,
            0,
            0,
            0,
            true,
        ));
    }

    if let Some(target) = &request.target_dir {
        let materialized = materialize(&inspection, target)?;
        let copied = copy_order(&inspection).len();
        sink.progress(&Progress {
            step: "copie".to_string(),
            current: copied,
            total: copied,
            bytes_sent: materialized.bytes,
        });
        return Ok(report(
            &inspection,
            "dossier local",
            materialized.playlist_dir.display().to_string(),
            copied,
            materialized.bytes,
            0,
            false,
        ));
    }

    let adb = request.adb.clone().ok_or_else(|| {
        ToolError::AdbMissing(
            "adb introuvable : installer les platform-tools Android, definir ANDROID_HOME ou passer --adb"
                .to_string(),
        )
    })?;
    let serial = select_device(&adb, request.serial.as_deref())?;
    let needed = inspection.total_bytes.saturating_add(4096);
    let (free, _total) = crate::adb::free_space(&adb, &serial, &request.watch_dir);
    if let Some(free) = free {
        if free < needed {
            return Err(ToolError::NotEnoughSpace { needed, free });
        }
    }

    crate::adb::mkdirs(&adb, &serial, &remote_root).map_err(ToolError::Io)?;
    let staging = staging_directory()?;
    let materialized = materialize(&inspection, &staging)?;
    let ordered = copy_order(&inspection);
    let total = ordered.len();
    let mut bytes_sent = 0_u64;
    for (index, entry) in ordered.iter().enumerate() {
        if request.cancelled() {
            cleanup(&staging);
            return Err(ToolError::Cancelled);
        }
        let name = entry.file.clone().unwrap_or_default();
        let local = materialized.playlist_dir.join(&name);
        crate::adb::push_file(&adb, &serial, &local, &format!("{remote_root}/{name}"))
            .map_err(ToolError::Io)?;
        bytes_sent = bytes_sent.saturating_add(entry.size_bytes.unwrap_or(0));
        sink.progress(&Progress {
            step: "copie".to_string(),
            current: index + 1,
            total,
            bytes_sent,
        });
    }
    let manifest_path = materialized.playlist_dir.join("manifest.json");
    crate::adb::push_file(
        &adb,
        &serial,
        &manifest_path,
        &format!("{remote_root}/manifest.json"),
    )
    .map_err(ToolError::Io)?;

    let mut pruned = 0usize;
    if request.prune {
        pruned = prune_remote(&adb, &serial, &remote_root, &inspection)?;
    }
    cleanup(&staging);
    Ok(report(
        &inspection,
        "adb",
        remote_root,
        total,
        bytes_sent,
        pruned,
        false,
    ))
}

fn select_device(adb: &Path, serial: Option<&str>) -> Result<String, ToolError> {
    let devices = crate::adb::devices(adb, WATCH_MUSIC_DIR).map_err(ToolError::Io)?;
    if devices.is_empty() {
        return Err(ToolError::NoDevice(
            "aucune montre detectee par adb : brancher la montre en USB et activer le debogage USB"
                .to_string(),
        ));
    }
    if let Some(serial) = serial {
        return devices
            .iter()
            .find(|device| device.serial == serial)
            .map(|device| device.serial.clone())
            .ok_or_else(|| ToolError::NoDevice(format!("montre {serial} introuvable")));
    }
    devices
        .iter()
        .find(|device| device.state == "device")
        .map(|device| device.serial.clone())
        .ok_or_else(|| {
            ToolError::NoDevice("montre detectee mais non autorisee (voir adb devices)".to_string())
        })
}

fn prune_remote(
    adb: &Path,
    serial: &str,
    remote_root: &str,
    inspection: &Inspection,
) -> Result<usize, ToolError> {
    let expected: Vec<String> = inspection
        .matches
        .iter()
        .filter_map(|entry| entry.file.clone())
        .chain(std::iter::once("manifest.json".to_string()))
        .collect();
    let listing = crate::adb::list_remote(adb, serial, remote_root).map_err(ToolError::Io)?;
    let mut removed = 0usize;
    for name in listing {
        if name.is_empty() || expected.contains(&name) {
            continue;
        }
        crate::adb::remove_remote(adb, serial, &format!("{remote_root}/{name}"))
            .map_err(ToolError::Io)?;
        removed += 1;
    }
    Ok(removed)
}

fn staging_directory() -> Result<PathBuf, ToolError> {
    let unique = format!("{}-{}", std::process::id(), unique_suffix());
    let working = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut last_error = String::new();
    for base in [
        std::env::temp_dir().join("mpacer-music"),
        working.join(".mpacer-music-staging"),
    ] {
        let staging = base.join(&unique);
        match fs::create_dir_all(&staging) {
            Ok(()) => return Ok(staging),
            Err(error) => last_error = format!("{} : {error}", staging.display()),
        }
    }
    Err(ToolError::Io(format!(
        "dossier de travail temporaire impossible ({last_error})"
    )))
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0)
}

fn cleanup(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

#[allow(clippy::too_many_arguments)]
fn report(
    inspection: &Inspection,
    mode: &str,
    destination: String,
    copied_files: usize,
    bytes: u64,
    pruned: usize,
    dry_run: bool,
) -> TransferReport {
    TransferReport {
        playlist_id: inspection.playlist.id.clone(),
        name: inspection.playlist.name.clone(),
        mode: mode.to_string(),
        destination,
        matched: inspection.matched_count(),
        missing: inspection.missing_count(),
        unused_files: inspection.unused_files.len(),
        copied_files,
        bytes,
        pruned,
        dry_run,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Racine des dossiers de test : sous `target/`, toujours inscriptible par
    /// cargo (le dossier temporaire du systeme peut y etre restreint).
    fn test_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/mpacer-music-tests")
    }

    fn temp_dir(name: &str) -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let directory = test_root().join(format!("{name}-{unique}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory)
            .unwrap_or_else(|error| panic!("dossier de test {} : {error}", directory.display()));
        directory
    }

    /// Fichier MP3 minimal avec un tag ID3v2.3 TBPM.
    fn write_tagged_mp3(path: &Path, bpm: &str) {
        let mut body = vec![0_u8];
        body.extend_from_slice(bpm.as_bytes());
        let frame_size = body.len() as u32;
        let total = 10 + frame_size;
        let mut tag = Vec::new();
        tag.extend_from_slice(b"ID3");
        tag.extend_from_slice(&[3, 0, 0]);
        tag.push((total >> 21) as u8 & 0x7F);
        tag.push((total >> 14) as u8 & 0x7F);
        tag.push((total >> 7) as u8 & 0x7F);
        tag.push(total as u8 & 0x7F);
        tag.extend_from_slice(b"TBPM");
        tag.extend_from_slice(&frame_size.to_be_bytes());
        tag.extend_from_slice(&[0, 0]);
        tag.extend_from_slice(&body);
        fs::write(path, tag).expect("ecriture du mp3");
    }

    /// Fichier WAV mono 8000 Hz 16 bits de la duree demandee.
    fn write_wav(path: &Path, seconds: u32) {
        let byte_rate = 8000_u32 * 2;
        let data_size = byte_rate * seconds;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8000_u32.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        wav.resize(wav.len() + data_size as usize, 0);
        fs::write(path, wav).expect("ecriture du wav");
    }

    fn manifest() -> TransferManifest {
        parse_manifest(
            r#"{
                "version": 1,
                "playlist_id": "run-170",
                "name": "Run 170",
                "source": "deezer",
                "target_bpm": 170.0,
                "tracks": [
                    {"id":"t1","position":1,"title":"Wake me up","artist":"Avicii","duration_s":249.0},
                    {"id":"t2","position":2,"title":"Levels","artist":"Avicii","duration_s":200.0}
                ]
            }"#,
        )
        .expect("manifeste de test")
    }

    // ------------------------------------------------------------------- scan

    #[test]
    fn scan_finds_audio_files_recursively() {
        let root = temp_dir("scan");
        fs::create_dir_all(root.join("sous-dossier")).unwrap();
        write_tagged_mp3(&root.join("a.mp3"), "124");
        write_wav(&root.join("sous-dossier").join("b.wav"), 1);
        fs::write(root.join("notes.txt"), b"x").unwrap();

        let files = scan_folder(&root).unwrap();
        assert_eq!(files.len(), 2);
        let mp3 = files.iter().find(|file| file.file_name == "a.mp3").unwrap();
        assert_eq!(mp3.bpm, Some(124.0));
        let wav = files.iter().find(|file| file.file_name == "b.wav").unwrap();
        assert_eq!(wav.duration_s.map(|value| value.round()), Some(1.0));
        assert!(wav.duration_s.is_some());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scanning_a_missing_folder_is_a_usage_error() {
        let error = scan_folder(Path::new("Z:/dossier/qui/n-existe/pas")).unwrap_err();
        assert_eq!(error.exit_code(), 2);
        assert!(error.message().contains("introuvable"));
    }

    // ------------------------------------------------------------- inspection

    #[test]
    fn inspect_reports_matches_missing_and_unused() {
        let root = temp_dir("inspect");
        write_tagged_mp3(&root.join("01 - Avicii - Wake me up.mp3"), "124");
        write_tagged_mp3(&root.join("02 - Avicii - Levels.mp3"), "128");
        write_tagged_mp3(&root.join("inutile.mp3"), "100");

        let inspection = inspect(&manifest(), &root).unwrap();
        assert_eq!(inspection.playlist.id, "run-170");
        assert_eq!(inspection.playlist.track_count, 2);
        assert_eq!(inspection.matched_count(), 2);
        assert_eq!(inspection.missing_count(), 0);
        assert_eq!(inspection.unused_files.len(), 1);
        assert_eq!(inspection.unused_files[0].file_name, "inutile.mp3");
        let first = &inspection.matches[0];
        assert_eq!(first.file.as_deref(), Some("01 - Avicii - Wake me up.mp3"));
        assert_eq!(first.bpm, Some(124.0));
        assert!(inspection.total_bytes > 0);

        // Le manifeste final porte file et size_bytes pour les pistes trouvees.
        let final_manifest = inspection.final_manifest();
        assert_eq!(
            final_manifest.tracks[0].file.as_deref(),
            Some("01 - Avicii - Wake me up.mp3")
        );
        assert_eq!(
            final_manifest.tracks[0].size_bytes,
            Some(
                fs::metadata(root.join("01 - Avicii - Wake me up.mp3"))
                    .unwrap()
                    .len()
            )
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn inspect_marks_missing_titles_without_a_file() {
        let root = temp_dir("inspect-missing");
        write_tagged_mp3(&root.join("01 - Avicii - Wake me up.mp3"), "124");
        let inspection = inspect(&manifest(), &root).unwrap();
        assert_eq!(inspection.matched_count(), 1);
        assert_eq!(inspection.missing_count(), 1);
        assert_eq!(inspection.missing[0].track_id, "t2");
        assert!(inspection.matches[1].file.is_none());
        let _ = fs::remove_dir_all(&root);
    }

    // ------------------------------------------------------------------ copie

    #[test]
    fn materialize_writes_the_tree_and_the_final_manifest() {
        let root = temp_dir("materialize");
        let music = root.join("musique");
        fs::create_dir_all(&music).unwrap();
        write_tagged_mp3(&music.join("01 - Avicii - Wake me up.mp3"), "124");

        let inspection = inspect(&manifest(), &music).unwrap();
        let target = root.join("cible");
        let materialized = materialize(&inspection, &target).unwrap();
        assert!(materialized.playlist_dir.ends_with("run-170"));
        let written = materialized
            .playlist_dir
            .join("01 - Avicii - Wake me up.mp3");
        assert!(written.is_file());
        assert_eq!(materialized.bytes, fs::metadata(&written).unwrap().len());

        let json = fs::read_to_string(materialized.playlist_dir.join("manifest.json")).unwrap();
        let final_manifest = parse_manifest(&json).expect("manifeste final lisible");
        assert_eq!(
            final_manifest.tracks[0].file.as_deref(),
            Some("01 - Avicii - Wake me up.mp3")
        );
        assert!(final_manifest.tracks[0].size_bytes.is_some());
        assert!(final_manifest.tracks[1].file.is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_dry_run_writes_nothing_and_needs_no_adb() {
        let root = temp_dir("dry-run");
        let music = root.join("musique");
        fs::create_dir_all(&music).unwrap();
        write_tagged_mp3(&music.join("01 - Avicii - Wake me up.mp3"), "124");

        let mut request = TransferRequest::new(manifest(), music);
        request.dry_run = true;
        let report = transfer(&request, &SilentSink).unwrap();
        assert!(report.dry_run);
        assert_eq!(report.mode, "simulation");
        assert_eq!(report.copied_files, 0);
        assert_eq!(report.matched, 1);
        assert_eq!(report.missing, 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn transfer_to_a_target_dir_builds_the_watch_tree() {
        let root = temp_dir("target");
        let music = root.join("musique");
        fs::create_dir_all(&music).unwrap();
        write_tagged_mp3(&music.join("01 - Avicii - Wake me up.mp3"), "124");
        write_tagged_mp3(&music.join("02 - Avicii - Levels.mp3"), "128");

        let mut request = TransferRequest::new(manifest(), music);
        request.target_dir = Some(root.join("cible"));
        let report = transfer(&request, &SilentSink).unwrap();
        assert_eq!(report.mode, "dossier local");
        assert_eq!(report.copied_files, 2);
        assert_eq!(report.missing, 0);

        let playlist_dir = root.join("cible").join("run-170");
        assert!(playlist_dir.join("manifest.json").is_file());
        assert!(playlist_dir.join("01 - Avicii - Wake me up.mp3").is_file());
        assert!(playlist_dir.join("02 - Avicii - Levels.mp3").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn strict_stops_when_titles_are_missing() {
        let root = temp_dir("strict");
        let music = root.join("musique");
        fs::create_dir_all(&music).unwrap();
        write_tagged_mp3(&music.join("01 - Avicii - Wake me up.mp3"), "124");

        let mut request = TransferRequest::new(manifest(), music);
        request.target_dir = Some(root.join("cible"));
        request.strict = true;
        let error = transfer(&request, &SilentSink).unwrap_err();
        assert_eq!(error.exit_code(), 6);
        assert!(!root.join("cible").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn without_adb_the_transfer_reports_code_3() {
        let root = temp_dir("no-adb");
        let music = root.join("musique");
        fs::create_dir_all(&music).unwrap();
        write_tagged_mp3(&music.join("01 - Avicii - Wake me up.mp3"), "124");

        let request = TransferRequest::new(manifest(), music);
        let error = transfer(&request, &SilentSink).unwrap_err();
        assert_eq!(error.exit_code(), 3);
        assert!(error.message().contains("adb"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn copy_order_sorts_by_size_then_position() {
        let root = temp_dir("order");
        let music = root.join("musique");
        fs::create_dir_all(&music).unwrap();
        write_wav(&music.join("01 - Avicii - Wake me up.wav"), 3);
        write_wav(&music.join("02 - Avicii - Levels.wav"), 1);
        let inspection = inspect(&manifest(), &music).unwrap();
        let order = copy_order(&inspection);
        assert_eq!(order.len(), 2);
        assert_eq!(order[0].track_id, "t2");
        assert_eq!(order[1].track_id, "t1");
        let _ = fs::remove_dir_all(&root);
    }

    // ------------------------------------------------------- manifeste / codes

    #[test]
    fn exit_codes_follow_the_contract() {
        assert_eq!(ToolError::Usage("x".into()).exit_code(), 2);
        assert_eq!(ToolError::Io("x".into()).exit_code(), 2);
        assert_eq!(ToolError::AdbMissing("x".into()).exit_code(), 3);
        assert_eq!(ToolError::NoDevice("x".into()).exit_code(), 4);
        assert_eq!(
            ToolError::NotEnoughSpace {
                needed: 10,
                free: 1
            }
            .exit_code(),
            5
        );
        assert_eq!(ToolError::MissingTracks(2).exit_code(), 6);
        assert_eq!(ToolError::Cancelled.exit_code(), 0);
    }

    #[test]
    fn load_manifest_reads_a_file_and_inline_json() {
        let root = temp_dir("manifest");
        let path = root.join("run-170.json");
        fs::write(&path, r#"{"playlist_id":"p1","name":"Run","tracks":[]}"#).unwrap();
        let from_file = load_manifest(Some(path.to_str().unwrap()), None).unwrap();
        assert_eq!(from_file.playlist_id, "p1");
        let inline = load_manifest(None, Some(r#"{"playlist_id":"p2","name":"Run"}"#)).unwrap();
        assert_eq!(inline.playlist_id, "p2");
        assert!(load_manifest(None, None).is_err());
        assert!(load_manifest(Some("Z:/absent.json"), None).is_err());
        assert!(load_manifest(None, Some("pas du json")).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
