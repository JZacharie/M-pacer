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
