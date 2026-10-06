//! Modeles persistes et echanges API.

use serde::{Deserialize, Serialize};

/// Utilisateur authentifie (cree au premier login Google).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub google_sub: Option<String>,
    pub email: String,
    pub name: Option<String>,
    pub picture_url: Option<String>,
    pub created_at_ms: i64,
    pub last_seen_ms: i64,
}

/// Jeton d'appareil (montre) tel qu'expose a l'interface.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ApiToken {
    pub id: String,
    pub label: String,
    pub created_at_ms: i64,
    pub last_used_ms: Option<i64>,
    pub revoked_at_ms: Option<i64>,
}

/// Seance synchronisee, version legere pour les listes.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WorkoutRow {
    pub id: String,
    pub started_at_ms: i64,
    pub duration_s: f64,
    pub distance_m: f64,
    pub average_pace_s_per_km: f64,
    pub unit_system: String,
    pub uploaded_at_ms: i64,
    /// Commentaire du coureur apres la seance, ecrit depuis le navigateur.
    pub comment: Option<String>,
}

/// Longueur maximale d'un commentaire de seance (caracteres).
pub const WORKOUT_COMMENT_MAX_CHARS: usize = 2_000;

/// Nettoie un commentaire saisi : espaces superflus retires, vide converti en
/// `None`. Un commentaire vide n'est pas stocke comme chaine vide.
pub fn clean_comment(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(WORKOUT_COMMENT_MAX_CHARS).collect())
}

/// Corps de requete d'ingestion : le `WorkoutSummary` produit par le coeur.
///
/// Les champs correspondent exactement a `mpacer_core::history::WorkoutSummary`,
/// ce qui permet a la montre d'envoyer sa seance telle qu'elle l'a enregistree
/// (meme structure que le fichier `.pac`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkoutUpload {
    pub id: String,
    pub started_at_ms: i64,
    pub duration_s: f64,
    pub distance_m: f64,
    pub average_pace_s_per_km: f64,
    #[serde(default = "empty_array")]
    pub laps: serde_json::Value,
    #[serde(default = "empty_array")]
    pub best_efforts: serde_json::Value,
    #[serde(default)]
    pub track: Vec<mpacer_core::best_distances::TrackPoint>,
    #[serde(default)]
    pub unit_system: Option<mpacer_core::units::UnitSystem>,
    /// Temps total ecoule, pauses comprises (s) : permet d'afficher le temps
    /// de pause sans le recalculer cote serveur.
    #[serde(default)]
    pub elapsed_s: f64,
    /// Pauses de la seance (manuelles et automatiques).
    #[serde(default)]
    pub pauses: Vec<mpacer_core::analysis::Pause>,
    /// Mesures de frequence cardiaque (bpm).
    #[serde(default)]
    pub heart_rate: Vec<mpacer_core::cardio::HeartRateSample>,
    /// Plan de course suivi (allure cible, negative split).
    #[serde(default)]
    pub plan: Option<mpacer_core::race_plan::RacePlan>,
}

/// Valeur par defaut des listes absentes (tableau vide, jamais `null`).
fn empty_array() -> serde_json::Value {
    serde_json::Value::Array(Vec::new())
}

impl WorkoutUpload {
    /// Valide la coherence minimale d'une seance avant insertion.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("identifiant de seance manquant".into());
        }
        if !self.duration_s.is_finite() || self.duration_s < 0.0 {
            return Err("duree invalide".into());
        }
        if !self.distance_m.is_finite() || self.distance_m < 0.0 {
            return Err("distance invalide".into());
        }
        if self.distance_m > 500_000.0 {
            return Err("distance invraisemblable (> 500 km)".into());
        }
        if self.duration_s > 48.0 * 3600.0 {
            return Err("duree invraisemblable (> 48 h)".into());
        }
        if !self.elapsed_s.is_finite() || self.elapsed_s < 0.0 {
            return Err("temps ecoule invalide".into());
        }
        if self.pauses.iter().any(|pause| {
            !pause.duration_s.is_finite()
                || pause.duration_s < 0.0
                || pause.duration_s > 24.0 * 3600.0
                || !pause.at_s.is_finite()
                || pause.at_s < 0.0
        }) {
            return Err("pause invraisemblable".into());
        }
        if self
            .heart_rate
            .iter()
            .any(|sample| !(20..=250).contains(&sample.bpm))
        {
            return Err("frequence cardiaque invraisemblable".into());
        }
        Ok(())
    }

    /// Systeme d'unites, metrique par defaut.
    pub fn units(&self) -> mpacer_core::units::UnitSystem {
        self.unit_system.unwrap_or_default()
    }
}

/// Reponse d'ingestion.
#[derive(Debug, Clone, Serialize)]
pub struct UploadResponse {
    pub id: String,
    /// `true` si la seance remplace une version deja synchronisee.
    pub replaced: bool,
}

// ------------------------------------------------------------------ courses

/// Course a venir : fiche complete (dossard, horaires, hebergement, suivi).
///
/// Tous les champs facultatifs sont `NULL` tant que le coureur ne les a pas
/// renseignes : la fiche se remplit progressivement, sans valeur inventee.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Race {
    pub id: String,
    pub user_id: String,
    pub name: String,
    /// Date et heure de depart (ms UNIX), `None` si encore inconnue.
    pub start_at_ms: Option<i64>,
    pub distance_m: Option<f64>,
    pub discipline: Option<String>,
    pub location: Option<String>,
    pub start_location: Option<String>,
    pub bib_number: Option<String>,
    pub bib_pickup_at_ms: Option<i64>,
    pub bib_pickup_location: Option<String>,
    pub live_url: Option<String>,
    pub registration_url: Option<String>,
    pub website_url: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub hotel_name: Option<String>,
    pub hotel_address: Option<String>,
    pub hotel_phone: Option<String>,
    pub hotel_url: Option<String>,
    pub hotel_booked: bool,
    pub hotel_check_in_ms: Option<i64>,
    pub hotel_check_out_ms: Option<i64>,
    pub lodging_notes: Option<String>,
    pub nutrition_notes: Option<String>,
    pub important_info: Option<String>,
    pub notes: Option<String>,
    pub goal_time_s: Option<f64>,
    /// Origine du fichier importe ("strava", "garmin", "gpx", "tcx"), `None`
    /// pour une course saisie a la main.
    pub source: Option<String>,
    /// Vrai pour une ancienne course importee, utilisee comme reference.
    pub is_reference: bool,
    /// Temps en mouvement releve dans l'export (s).
    pub moving_time_s: Option<f64>,
    /// Temps ecoule releve dans l'export (s), pauses comprises.
    pub elapsed_time_s: Option<f64>,
    /// Denivele positif cumule (m).
    pub elevation_gain_m: Option<f64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Champs modifiables d'une course.
///
/// Meme structure pour l'API JSON et pour le formulaire web : une seule
/// normalisation et une seule validation, donc aucun ecart entre les deux.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RaceInput {
    pub name: String,
    #[serde(default)]
    pub start_at_ms: Option<i64>,
    #[serde(default)]
    pub distance_m: Option<f64>,
    #[serde(default)]
    pub discipline: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub start_location: Option<String>,
    #[serde(default)]
    pub bib_number: Option<String>,
    #[serde(default)]
    pub bib_pickup_at_ms: Option<i64>,
    #[serde(default)]
    pub bib_pickup_location: Option<String>,
    #[serde(default)]
    pub live_url: Option<String>,
    #[serde(default)]
    pub registration_url: Option<String>,
    #[serde(default)]
    pub website_url: Option<String>,
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
    #[serde(default)]
    pub hotel_name: Option<String>,
    #[serde(default)]
    pub hotel_address: Option<String>,
    #[serde(default)]
    pub hotel_phone: Option<String>,
    #[serde(default)]
    pub hotel_url: Option<String>,
    #[serde(default)]
    pub hotel_booked: bool,
    #[serde(default)]
    pub hotel_check_in_ms: Option<i64>,
    #[serde(default)]
    pub hotel_check_out_ms: Option<i64>,
    #[serde(default)]
    pub lodging_notes: Option<String>,
    #[serde(default)]
    pub nutrition_notes: Option<String>,
    #[serde(default)]
    pub important_info: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub goal_time_s: Option<f64>,
}

impl RaceInput {
    /// Supprime les espaces superflus et transforme les champs vides en `None`.
    ///
    /// Un formulaire HTML renvoie toujours des chaines : sans ce nettoyage, la
    /// base stockerait `""` au lieu de `NULL` et l'affichage ne saurait plus
    /// distinguer « non renseigne » de « renseigne vide ».
    pub fn normalized(&self) -> RaceInput {
        RaceInput {
            name: self.name.trim().to_string(),
            start_at_ms: self.start_at_ms,
            distance_m: self.distance_m,
            discipline: clean(&self.discipline),
            location: clean(&self.location),
            start_location: clean(&self.start_location),
            bib_number: clean(&self.bib_number),
            bib_pickup_at_ms: self.bib_pickup_at_ms,
            bib_pickup_location: clean(&self.bib_pickup_location),
            live_url: clean(&self.live_url),
            registration_url: clean(&self.registration_url),
            website_url: clean(&self.website_url),
            latitude: self.latitude,
            longitude: self.longitude,
            hotel_name: clean(&self.hotel_name),
            hotel_address: clean(&self.hotel_address),
            hotel_phone: clean(&self.hotel_phone),
            hotel_url: clean(&self.hotel_url),
            hotel_booked: self.hotel_booked,
            hotel_check_in_ms: self.hotel_check_in_ms,
            hotel_check_out_ms: self.hotel_check_out_ms,
            lodging_notes: clean(&self.lodging_notes),
            nutrition_notes: clean(&self.nutrition_notes),
            important_info: clean(&self.important_info),
            notes: clean(&self.notes),
            goal_time_s: self.goal_time_s,
        }
    }

    /// Verifie la coherence de la fiche avant enregistrement.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("le nom de la course est obligatoire".into());
        }
        if self.name.chars().count() > 200 {
            return Err("le nom de la course est trop long (200 caracteres maximum)".into());
        }
        if let Some(distance) = self.distance_m {
            if !distance.is_finite() || distance < 0.0 {
                return Err("distance invalide".into());
            }
            if distance > 500_000.0 {
                return Err("distance invraisemblable (> 500 km)".into());
            }
        }
        if let Some(goal) = self.goal_time_s {
            if !goal.is_finite() || goal <= 0.0 || goal > 48.0 * 3600.0 {
                return Err("objectif de temps invalide (entre 0 et 48 h)".into());
            }
        }
        match (self.latitude, self.longitude) {
            (Some(latitude), Some(longitude)) => {
                if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
                    return Err("coordonnees hors limites".into());
                }
            }
            (None, None) => {}
            _ => return Err("latitude et longitude vont ensemble".into()),
        }
        check_link("le lien du live", &self.live_url)?;
        check_link("le lien d'inscription", &self.registration_url)?;
        check_link("le lien du site de la course", &self.website_url)?;
        check_link("le lien de l'hotel", &self.hotel_url)?;
        match (self.hotel_check_in_ms, self.hotel_check_out_ms) {
            (Some(check_in), Some(check_out)) if check_out < check_in => {
                return Err("la date de depart de l'hotel precede l'arrivee".into());
            }
            _ => {}
        }
        Ok(())
    }
}

/// Metadonnees d'une course importee depuis un export Strava ou Garmin.
///
/// Elles ne passent pas par `RaceInput` : le formulaire web ne les saisit pas,
/// et une modification ulterieure de la fiche ne doit pas les effacer.
#[derive(Debug, Clone)]
pub struct ImportedRaceMeta {
    /// Code stable de l'origine ("strava", "garmin", "gpx", "tcx").
    pub source: String,
    pub moving_time_s: Option<f64>,
    pub elapsed_time_s: Option<f64>,
    pub elevation_gain_m: Option<f64>,
    /// Trace normalisee (GPX 1.1) conservee avec la course.
    pub gpx: String,
    /// Nombre de points de la trace conservee.
    pub points: i32,
}

/// Chaine nettoyee : `None` si vide apres suppression des espaces.
fn clean(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// Refuse les liens qui ne sont pas http(s).
///
/// Sans ce controle, un lien `javascript:` enregistre dans la fiche serait
/// rendu tel quel dans un `href` et deviendrait un script executable au clic.
fn check_link(label: &str, value: &Option<String>) -> Result<(), String> {
    let Some(url) = value else { return Ok(()) };
    let lowered = url.trim().to_ascii_lowercase();
    if lowered.starts_with("https://") || lowered.starts_with("http://") {
        return Ok(());
    }
    Err(format!("{label} doit commencer par http:// ou https://"))
}

/// Element du suivi d'une course (« dossard retire », « hotel reserve »...).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RaceTask {
    pub id: String,
    pub race_id: String,
    pub user_id: String,
    pub label: String,
    pub due_at_ms: Option<i64>,
    pub done: bool,
    pub done_at_ms: Option<i64>,
    pub position: i32,
    pub created_at_ms: i64,
}

/// Elements de suivi crees automatiquement avec une nouvelle course.
///
/// Ils couvrent la preparation courante d'une course sur route ou en trail ;
/// chacun peut etre coche, complete par d'autres elements, ou supprime.
pub const DEFAULT_RACE_TASKS: [&str; 8] = [
    "Inscription confirmee",
    "Certificat medical / PPS a jour",
    "Dossard retire",
    "Hotel reserve",
    "Transport reserve",
    "Sac de course prepare",
    "Nutrition et ravitaillement prevus",
    "Lien du live partage avec les proches",
];

// ------------------------------------------------------------------ musique

/// Origines d'une playlist, telles que stockees dans `music_playlists.source`.
///
/// En v2 il n'y a plus de televersement : une playlist vient de Spotify ou de la
/// saisie manuelle.
pub const MUSIC_SOURCES: [&str; 2] = ["spotify", "manual"];

/// Origines d'une valeur de BPM, alignees sur `mpacer_core::music::BpmSource` :
/// `spotify` (audio-features), `tag` (balise du fichier), `tap` (tap-tempo),
/// `manual` (saisie directe).
pub const BPM_SOURCES: [&str; 4] = ["spotify", "tag", "tap", "manual"];

/// Borne de plausibilite d'un BPM (identique au coeur Rust).
pub const BPM_MIN: f64 = 30.0;
pub const BPM_MAX: f64 = 300.0;

/// Playlist de course enregistree par l'utilisateur.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MusicPlaylist {
    pub id: String,
    pub user_id: String,
    pub name: String,
    /// `spotify` | `manual`.
    pub source: String,
    pub spotify_id: Option<String>,
    pub cover_url: Option<String>,
    /// Consigne fixe ; `None` = tempo automatique (calcule par la montre).
    pub target_bpm: Option<f64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Champs d'une playlist a la creation.
#[derive(Debug, Clone, Default)]
pub struct MusicPlaylistInput {
    pub name: String,
    pub source: String,
    pub spotify_id: Option<String>,
    pub cover_url: Option<String>,
    pub target_bpm: Option<f64>,
}

/// Piste d'une playlist.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MusicTrack {
    pub id: String,
    pub playlist_id: String,
    pub user_id: String,
    pub position: i32,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_s: Option<f64>,
    pub bpm: Option<f64>,
    pub bpm_source: Option<String>,
    pub spotify_uri: Option<String>,
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    /// Chemin relatif a `MPACER_MEDIA_DIR` ; `None` = aucun octet sur le serveur.
    pub storage_path: Option<String>,
    pub created_at_ms: i64,
    /// Accuse de telechargement par la montre.
    pub downloaded_at_ms: Option<i64>,
}

/// Champs d'une piste a l'insertion : un seul endroit a corriger si une colonne
/// s'ajoute, plutot qu'une fonction a douze parametres.
///
/// Le backend ne stocke aucun audio depuis la v2 : ni chemin, ni type MIME, ni
/// taille (les colonnes existent encore en base, mais restent `NULL`).
#[derive(Debug, Clone, Default)]
pub struct MusicTrackInput {
    pub position: i32,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_s: Option<f64>,
    pub bpm: Option<f64>,
    pub bpm_source: Option<String>,
    pub spotify_uri: Option<String>,
}

/// Compte Spotify lie (jetons OAuth). Jamais expose tel quel a l'interface.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SpotifyAccount {
    pub user_id: String,
    pub spotify_user_id: Option<String>,
    pub display_name: Option<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_ms: i64,
    pub scope: Option<String>,
    pub connected_at_ms: i64,
}

/// Resume d'une playlist pour la liste de l'API appareil.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MusicPlaylistSummary {
    pub id: String,
    pub name: String,
    pub source: String,
    pub target_bpm: Option<f64>,
    pub track_count: i64,
    /// Somme des durees connues des titres (0 si aucune duree).
    pub duration_s: f64,
    pub updated_at_ms: i64,
}

/// Piste publiee dans la fiche d'une playlist (metadonnees seules).
#[derive(Debug, Clone, Serialize)]
pub struct MusicTrackView {
    pub id: String,
    pub position: i32,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_s: Option<f64>,
    pub bpm: Option<f64>,
    pub bpm_source: Option<String>,
}

impl MusicTrackView {
    pub fn from_track(track: &MusicTrack) -> Self {
        MusicTrackView {
            id: track.id.clone(),
            position: track.position,
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            duration_s: track.duration_s,
            bpm: track.bpm,
            bpm_source: track.bpm_source.clone(),
        }
    }
}

/// Fiche complete d'une playlist (API appareil).
#[derive(Debug, Clone, Serialize)]
pub struct MusicPlaylistDetail {
    pub id: String,
    pub name: String,
    pub source: String,
    pub target_bpm: Option<f64>,
    /// Chemin relatif du manifeste de transfert, que `mpacer-music` consomme.
    pub manifest_url: String,
    pub tracks: Vec<MusicTrackView>,
}

/// Chemin du manifeste de transfert d'une playlist.
pub fn manifest_url(playlist_id: &str) -> String {
    format!("/api/v1/music/playlists/{playlist_id}/manifest")
}

/// Version du manifeste de transfert produite par le backend.
pub const TRANSFER_MANIFEST_VERSION: u32 = 1;

/// Piste attendue par un manifeste de transfert (docs/07 section 3.1).
///
/// `file` et `size_bytes` ne sont renseignes qu'a l'ecriture sur la montre par
/// `mpacer-music` : le backend les omet, comme le prevoit le contrat
/// (« absents si le titre n'a pas ete trouve »).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestTrack {
    pub id: String,
    pub position: u32,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_s: Option<f64>,
    pub bpm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

/// Manifeste de transfert : ce que `mpacer-music` copie sur la montre par USB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferManifest {
    pub version: u32,
    pub playlist_id: String,
    pub name: String,
    /// `spotify` | `manual`.
    pub source: String,
    pub target_bpm: Option<f64>,
    pub tracks: Vec<ManifestTrack>,
}

impl TransferManifest {
    /// Manifeste d'une playlist et de ses titres, tel qu'exporte par le backend.
    pub fn from_playlist(playlist: &MusicPlaylist, tracks: &[MusicTrack]) -> Self {
        TransferManifest {
            version: TRANSFER_MANIFEST_VERSION,
            playlist_id: playlist.id.clone(),
            name: playlist.name.clone(),
            source: playlist.source.clone(),
            target_bpm: playlist.target_bpm,
            tracks: tracks
                .iter()
                .map(|track| ManifestTrack {
                    id: track.id.clone(),
                    position: track.position.max(0) as u32,
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    duration_s: track.duration_s,
                    bpm: track.bpm,
                    file: None,
                    size_bytes: None,
                })
                .collect(),
        }
    }
}

/// Nom de fichier du manifeste d'une playlist : "Run 170" -> "run-170.json".
pub fn manifest_file_name(playlist_name: &str) -> String {
    let mut slug = String::with_capacity(playlist_name.len());
    let mut separator = false;
    for character in playlist_name.chars() {
        let folded = fold_ascii(character);
        if folded.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            separator = false;
            slug.push(folded);
        } else {
            separator = true;
        }
    }
    let slug: String = slug.chars().take(60).collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "playlist.json".to_string()
    } else {
        format!("{slug}.json")
    }
}

/// Replie les lettres accentuees latines sur leur equivalent ASCII.
fn fold_ascii(character: char) -> char {
    match character {
        'a'..='z' | 'A'..='Z' | '0'..='9' => character.to_ascii_lowercase(),
        'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' | 'À' | 'Á' | 'Â' | 'Ä' | 'Ã' | 'Å' => 'a',
        'ç' | 'Ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
        'ñ' | 'Ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'ö' | 'õ' | 'Ò' | 'Ó' | 'Ô' | 'Ö' | 'Õ' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
        'ý' | 'ÿ' | 'Ý' | 'Ÿ' => 'y',
        other => other.to_ascii_lowercase(),
    }
}

/// Normalise un BPM : `None` si absent ou hors bornes plausibles.
///
/// Une valeur aberrante (0, negative, 1000) est traitee comme inconnue plutot
/// que stockee : la montre ne doit jamais caler son allure sur un tempo faux.
pub fn clean_bpm(value: Option<f64>) -> Option<f64> {
    value.filter(|bpm| bpm.is_finite() && (BPM_MIN..=BPM_MAX).contains(bpm))
}

/// Verifie qu'une source de BPM fait partie du vocabulaire gele.
pub fn valid_bpm_source(source: Option<&str>) -> Option<String> {
    source
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| BPM_SOURCES.contains(&value.as_str()))
}

/// Tableau de bord : un ecran compose par l'utilisateur.
///
/// Le nom est libre ; les widgets vivent dans `dashboard_widgets`, une ligne par
/// widget, ce qui rend l'ordre d'affichage explicite (colonne `position`).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Dashboard {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Un widget place dans un tableau de bord.
///
/// `kind` est une cle du catalogue (`crate::dashboards::WidgetKind`) : une cle
/// inconnue est ignoree au rendu, jamais remplacee par un widget par defaut.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DashboardWidget {
    pub id: String,
    pub dashboard_id: String,
    pub user_id: String,
    pub kind: String,
    pub position: i32,
    pub created_at_ms: i64,
}

/// Longueur maximale du nom d'un tableau de bord (caracteres).
pub const DASHBOARD_NAME_MAX_CHARS: usize = 80;

/// Nettoie un nom de tableau de bord : espaces superflus retires, chaine vide
/// convertie en `None`. Un nom n'est pas un identifiant : il peut etre reutilise.
pub fn clean_dashboard_name(value: &str) -> Option<String> {
    let trimmed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(DASHBOARD_NAME_MAX_CHARS).collect())
}

#[cfg(test)]
mod dashboard_tests {
    use super::*;

    #[test]
    fn dashboard_names_are_cleaned_and_bounded() {
        assert_eq!(
            clean_dashboard_name("  Pace   Control "),
            Some("Pace Control".to_string())
        );
        assert_eq!(clean_dashboard_name("   "), None);
        assert_eq!(clean_dashboard_name("\n\t"), None);
        let long = "a".repeat(DASHBOARD_NAME_MAX_CHARS + 50);
        assert_eq!(
            clean_dashboard_name(&long).unwrap().chars().count(),
            DASHBOARD_NAME_MAX_CHARS
        );
    }
}

#[cfg(test)]
mod music_tests {
    use super::*;

    #[test]
    fn bpm_out_of_range_is_treated_as_unknown() {
        assert_eq!(clean_bpm(Some(170.0)), Some(170.0));
        assert_eq!(clean_bpm(Some(0.0)), None);
        assert_eq!(clean_bpm(Some(f64::NAN)), None);
        assert_eq!(clean_bpm(Some(1000.0)), None);
        assert_eq!(clean_bpm(None), None);
    }

    /// Cles d'un objet JSON, triees (serde_json trie deja : on le rend explicite).
    fn sorted_keys(value: &serde_json::Value) -> Vec<&str> {
        let mut keys: Vec<&str> = value
            .as_object()
            .expect("objet JSON")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        keys
    }

    /// Piste minimale, telle que la base la renvoie.
    fn sample_track(id: &str, position: i32) -> MusicTrack {
        MusicTrack {
            id: id.into(),
            playlist_id: "p1".into(),
            user_id: "u1".into(),
            position,
            title: "Wake me up".into(),
            artist: Some("Avicii".into()),
            album: Some("True".into()),
            duration_s: Some(249.0),
            bpm: Some(124.0),
            bpm_source: Some("tag".into()),
            spotify_uri: Some("spotify:track:t1".into()),
            // Colonnes conservees en base mais inutilisees depuis la v2.
            mime: None,
            size_bytes: None,
            storage_path: None,
            created_at_ms: 0,
            downloaded_at_ms: None,
        }
    }

    #[test]
    fn the_manifest_follows_the_frozen_schema() {
        let playlist = MusicPlaylist {
            id: "p1".into(),
            user_id: "u1".into(),
            name: "Run 170".into(),
            source: "spotify".into(),
            spotify_id: Some("8f".into()),
            cover_url: None,
            target_bpm: Some(170.0),
            created_at_ms: 0,
            updated_at_ms: 0,
        };
        let manifest = TransferManifest::from_playlist(
            &playlist,
            &[sample_track("t1", 0), sample_track("t2", 1)],
        );
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.playlist_id, "p1");
        assert_eq!(manifest.source, "spotify");
        assert_eq!(manifest.target_bpm, Some(170.0));

        // L'ordre des champs du JSON suit la declaration de la structure.
        let text = serde_json::to_string(&manifest).expect("manifeste serialisable");
        assert!(
            text.starts_with(
                r#"{"version":1,"playlist_id":"p1","name":"Run 170","source":"spotify","target_bpm":170.0,"tracks":["#
            ),
            "{text}"
        );

        // serde_json trie les cles d'un Value : le contenu se verifie donc comme
        // un ensemble (l'ordre est deja couvert ci-dessus).
        let value = serde_json::to_value(&manifest).expect("manifeste serialisable");
        assert_eq!(
            sorted_keys(&value),
            vec![
                "name",
                "playlist_id",
                "source",
                "target_bpm",
                "tracks",
                "version"
            ]
        );
        assert_eq!(
            sorted_keys(&value["tracks"][0]),
            vec![
                "album",
                "artist",
                "bpm",
                "duration_s",
                "id",
                "position",
                "title"
            ],
            "file et size_bytes ne sont ecrits que par mpacer-music"
        );
        assert_eq!(value["tracks"][1]["position"], 1);
    }

    #[test]
    fn manifest_file_name_is_a_readable_slug() {
        assert_eq!(manifest_file_name("Run 170"), "run-170.json");
        assert_eq!(
            manifest_file_name("Ma course 10 km"),
            "ma-course-10-km.json"
        );
        assert_eq!(manifest_file_name("Ete - 10 km"), "ete-10-km.json");
        assert_eq!(manifest_file_name("  "), "playlist.json");
        assert_eq!(manifest_file_name("A/B: C?"), "a-b-c.json");
    }

    #[test]
    fn only_the_frozen_bpm_sources_are_accepted() {
        assert_eq!(valid_bpm_source(Some("Tap")), Some("tap".to_string()));
        assert_eq!(valid_bpm_source(Some("analyse")), None);
        assert_eq!(valid_bpm_source(None), None);
    }
}
