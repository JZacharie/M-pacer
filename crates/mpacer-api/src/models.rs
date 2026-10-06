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
pub const MUSIC_SOURCES: [&str; 3] = ["spotify", "upload", "manual"];

/// Origines d'une valeur de BPM, alignees sur `mpacer_core::music::BpmSource` :
/// `spotify` (audio-features), `tag` (balise du fichier), `tap` (tap-tempo),
/// `manual` (saisie directe).
pub const BPM_SOURCES: [&str; 4] = ["spotify", "tag", "tap", "manual"];

/// Extension audio acceptee au televersement (filtre volontairement restreint :
/// le serveur ne sert que des formats que la montre sait lire).
pub const AUDIO_EXTENSIONS: [&str; 8] = ["mp3", "ogg", "oga", "opus", "m4a", "mp4", "flac", "wav"];

/// Taille maximale d'un televersement (multipart complet), en octets.
pub const MAX_UPLOAD_BYTES: i64 = 512 * 1024 * 1024;

/// Borne de plausibilite d'un BPM (identique au coeur Rust).
pub const BPM_MIN: f64 = 30.0;
pub const BPM_MAX: f64 = 300.0;

/// Playlist de course enregistree par l'utilisateur.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MusicPlaylist {
    pub id: String,
    pub user_id: String,
    pub name: String,
    /// `spotify` | `upload` | `manual`.
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
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    pub storage_path: Option<String>,
}

/// Plan de telechargement : ce que la montre doit recuperer avant une course.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MusicDownloadPlan {
    pub id: String,
    pub user_id: String,
    pub playlist_id: String,
    pub race_id: Option<String>,
    pub target_bpm: Option<f64>,
    pub requested_at_ms: i64,
    pub acked_at_ms: Option<i64>,
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
    pub total_bytes: i64,
    /// Pistes dont les octets sont presents sur le serveur (une playlist Spotify
    /// n'en a aucun : la montre ne telecharge que la fiche).
    pub ready_track_count: i64,
    pub updated_at_ms: i64,
}

/// Piste publiee a la montre (API appareil et plan de telechargement).
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
    pub size_bytes: Option<i64>,
    pub mime: Option<String>,
    pub spotify_uri: Option<String>,
    /// Chemin relatif (`/api/v1/music/tracks/{id}/file`), `null` sans fichier.
    /// La montre resout ce chemin sur sa propre base : elle joint le serveur par
    /// une autre URL que `MPACER_PUBLIC_URL` (IP du reseau local).
    pub download_url: Option<String>,
}

impl MusicTrackView {
    /// Vue publique d'une piste ; le chemin de telechargement n'est expose que
    /// si les octets sont bien sur le serveur.
    pub fn from_track(track: &MusicTrack) -> Self {
        let download_url = track
            .storage_path
            .as_ref()
            .filter(|path| !path.trim().is_empty())
            .map(|_| format!("/api/v1/music/tracks/{}/file", track.id));
        MusicTrackView {
            id: track.id.clone(),
            position: track.position,
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            duration_s: track.duration_s,
            bpm: track.bpm,
            bpm_source: track.bpm_source.clone(),
            size_bytes: track.size_bytes,
            mime: track.mime.clone(),
            spotify_uri: track.spotify_uri.clone(),
            download_url,
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
    pub tracks: Vec<MusicTrackView>,
}

/// Plan de telechargement public (API appareil).
#[derive(Debug, Clone, Serialize)]
pub struct MusicPlanView {
    pub id: String,
    pub playlist_id: String,
    pub name: String,
    pub target_bpm: Option<f64>,
    pub race_id: Option<String>,
    pub race_name: Option<String>,
    pub requested_at_ms: i64,
    pub tracks: Vec<MusicTrackView>,
}

/// Corps de l'accuse de telechargement d'une playlist.
#[derive(Debug, Clone, Deserialize)]
pub struct MusicAckRequest {
    #[serde(default)]
    pub track_ids: Vec<String>,
}

/// Corps de l'accuse d'un plan de telechargement.
#[derive(Debug, Clone, Deserialize)]
pub struct MusicPlanAckRequest {
    pub plan_id: String,
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

/// Titre lisible deduit d'un nom de fichier televerse.
pub fn title_from_filename(filename: &str) -> String {
    let base = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(filename)
        .trim();
    let without_extension = match base.rfind('.') {
        Some(index) if index > 0 => &base[..index],
        _ => base,
    };
    let title = without_extension.replace(['_', '-'], " ").trim().to_string();
    if title.is_empty() {
        "Titre sans nom".to_string()
    } else {
        title.chars().take(200).collect()
    }
}

/// Extension audio (minuscule) d'un nom de fichier, si elle est acceptee.
pub fn audio_extension(filename: &str) -> Option<String> {
    let extension = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(filename)
        .rsplit_once('.')
        .map(|(_, extension)| extension.trim().to_ascii_lowercase())?;
    AUDIO_EXTENSIONS
        .contains(&extension.as_str())
        .then_some(extension)
}

/// Type MIME d'une extension audio.
pub fn audio_mime(extension: &str) -> &'static str {
    match extension {
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/opus",
        "m4a" | "mp4" => "audio/mp4",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
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

    #[test]
    fn download_url_only_exists_with_a_file() {
        let mut track = MusicTrack {
            id: "t1".into(),
            playlist_id: "p1".into(),
            user_id: "u1".into(),
            position: 0,
            title: "Titre".into(),
            artist: None,
            album: None,
            duration_s: None,
            bpm: None,
            bpm_source: None,
            spotify_uri: Some("spotify:track:t1".into()),
            mime: None,
            size_bytes: None,
            storage_path: None,
            created_at_ms: 0,
            downloaded_at_ms: None,
        };
        assert_eq!(MusicTrackView::from_track(&track).download_url, None);
        track.storage_path = Some("u1/fichier.mp3".into());
        assert_eq!(
            MusicTrackView::from_track(&track).download_url.as_deref(),
            Some("/api/v1/music/tracks/t1/file")
        );
    }

    #[test]
    fn title_and_extension_come_from_the_filename() {
        assert_eq!(title_from_filename("Wake_me-up.mp3"), "Wake me up");
        assert_eq!(title_from_filename("C:\\musique\\titre.ogg"), "titre");
        assert_eq!(title_from_filename("sans-extension"), "sans extension");
        assert_eq!(audio_extension("a/b/TRUC.MP3"), Some("mp3".to_string()));
        assert_eq!(audio_extension("notes.txt"), None);
        assert_eq!(audio_extension("sanspoint"), None);
        assert_eq!(audio_mime("m4a"), "audio/mp4");
    }

    #[test]
    fn only_the_frozen_bpm_sources_are_accepted() {
        assert_eq!(valid_bpm_source(Some("Tap")), Some("tap".to_string()));
        assert_eq!(valid_bpm_source(Some("analyse")), None);
        assert_eq!(valid_bpm_source(None), None);
    }
}
