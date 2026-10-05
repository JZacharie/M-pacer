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
