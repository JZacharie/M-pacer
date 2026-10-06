//! Historique des seances et format d'echange `.pac`.
//!
//! L'export/import reprend la logique de Pace Control (sauvegarde des seances
//! pour transfert ou reinstallation), mais avec un format JSON documente et
//! versionne plutot qu'un format binaire proprietaire.

use crate::analysis::Pause;
use crate::best_distances::{BestEffort, TrackPoint};
use crate::cardio::HeartRateSample;
use crate::lap::Lap;
use crate::race_plan::RacePlan;
use crate::units::UnitSystem;
use serde::{Deserialize, Serialize};

/// Version du format d'echange.
///
/// Version 2 : frequence cardiaque, pauses, temps ecoule et plan de course.
/// Les seances version 1 restent lisibles (les champs absents retombent sur
/// leur valeur par defaut).
pub const PAC_FORMAT: &str = "mpacer.pac";
pub const PAC_VERSION: u32 = 2;

/// Resume complet d'une seance enregistree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkoutSummary {
    /// Identifiant stable (horodatage de depart en ms).
    pub id: String,
    pub started_at_ms: i64,
    /// Duree de course, pauses exclues (s).
    pub duration_s: f64,
    pub distance_m: f64,
    /// Allure moyenne (s/km).
    pub average_pace_s_per_km: f64,
    pub laps: Vec<Lap>,
    pub best_efforts: Vec<BestEffort>,
    pub track: Vec<TrackPoint>,
    pub unit_system: UnitSystem,
    /// Temps total ecoule, pauses comprises (s).
    ///
    /// Zero pour une seance enregistree avant cette version : l'affichage
    /// retombe alors sur la duree de course.
    #[serde(default)]
    pub elapsed_s: f64,
    /// Pauses de la seance, dans l'ordre.
    #[serde(default)]
    pub pauses: Vec<Pause>,
    /// Mesures de frequence cardiaque (bpm), si la montre en fournit.
    #[serde(default)]
    pub heart_rate: Vec<HeartRateSample>,
    /// Plan de course suivi pendant la seance (mode "temps cible").
    #[serde(default)]
    pub plan: Option<RacePlan>,
}

impl WorkoutSummary {
    /// Allure moyenne dans le systeme demande.
    pub fn average_pace(&self, units: UnitSystem) -> Option<f64> {
        if self.average_pace_s_per_km <= 0.0 {
            return None;
        }
        let speed = 1000.0 / self.average_pace_s_per_km;
        units.pace_from_speed(speed)
    }

    /// Vitesse moyenne (m/s).
    pub fn average_speed_mps(&self) -> Option<f64> {
        if self.duration_s <= 0.0 {
            return None;
        }
        Some(self.distance_m / self.duration_s)
    }

    /// Temps total ecoule, pauses comprises (s).
    ///
    /// Les seances anterieures a la version 2 du format n'ont pas cette valeur :
    /// elles retombent sur la duree de course plutot que d'afficher zero.
    pub fn total_elapsed_s(&self) -> f64 {
        if self.elapsed_s > 0.0 {
            self.elapsed_s
        } else {
            self.duration_s
        }
    }

    /// Duree cumulee des pauses (s).
    pub fn paused_s(&self) -> f64 {
        crate::analysis::total_pause_s(&self.pauses)
    }

    /// Vrai si la seance porte des mesures de frequence cardiaque.
    pub fn has_heart_rate(&self) -> bool {
        !self.heart_rate.is_empty()
    }
}

/// Fichier d'echange `.pac`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PacFile {
    pub format: String,
    pub version: u32,
    pub workouts: Vec<WorkoutSummary>,
}

/// Serialise des seances au format `.pac` (JSON).
pub fn export_pac(workouts: &[WorkoutSummary]) -> Result<String, serde_json::Error> {
    let file = PacFile {
        format: PAC_FORMAT.to_string(),
        version: PAC_VERSION,
        workouts: workouts.to_vec(),
    };
    serde_json::to_string_pretty(&file)
}

/// Relit un fichier `.pac`.
pub fn import_pac(data: &str) -> Result<Vec<WorkoutSummary>, PacError> {
    let file: PacFile = serde_json::from_str(data).map_err(PacError::Parse)?;
    if file.format != PAC_FORMAT {
        return Err(PacError::UnknownFormat(file.format));
    }
    if file.version > PAC_VERSION {
        return Err(PacError::UnsupportedVersion(file.version));
    }
    Ok(file.workouts)
}

/// Erreurs d'import.
#[derive(Debug)]
pub enum PacError {
    Parse(serde_json::Error),
    UnknownFormat(String),
    UnsupportedVersion(u32),
}

impl std::fmt::Display for PacError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PacError::Parse(e) => write!(f, "fichier .pac illisible : {e}"),
            PacError::UnknownFormat(format) => write!(f, "format inconnu : {format}"),
            PacError::UnsupportedVersion(v) => write!(f, "version .pac non supportee : {v}"),
        }
    }
}

impl std::error::Error for PacError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_workout() -> WorkoutSummary {
        WorkoutSummary {
            id: "1700000000000".into(),
            started_at_ms: 1_700_000_000_000,
            duration_s: 1500.0,
            distance_m: 5000.0,
            average_pace_s_per_km: 300.0,
            laps: vec![Lap {
                index: 1,
                distance_m: 1000.0,
                duration_s: 300.0,
                pace_s_per_km: 300.0,
            }],
            best_efforts: vec![],
            track: vec![TrackPoint {
                t_ms: 0,
                dist_m: 0.0,
                lat: 45.0,
                lon: 3.0,
                elevation_m: Some(300.0),
            }],
            unit_system: UnitSystem::Metric,
            elapsed_s: 1560.0,
            pauses: vec![Pause {
                at_s: 600.0,
                at_distance_m: 2000.0,
                duration_s: 60.0,
                automatic: false,
            }],
            heart_rate: vec![HeartRateSample { t_ms: 0, bpm: 140 }],
            plan: Some(RacePlan::new(5000.0, 1500.0)),
        }
    }

    #[test]
    fn pac_roundtrip_preserves_workouts() {
        let workouts = vec![sample_workout()];
        let text = export_pac(&workouts).unwrap();
        assert!(text.contains("mpacer.pac"));
        let imported = import_pac(&text).unwrap();
        assert_eq!(imported, workouts);
    }

    #[test]
    fn unknown_format_is_rejected() {
        let text = r#"{"format":"other","version":1,"workouts":[]}"#;
        assert!(matches!(import_pac(text), Err(PacError::UnknownFormat(_))));
    }

    #[test]
    fn future_version_is_rejected() {
        let text = r#"{"format":"mpacer.pac","version":99,"workouts":[]}"#;
        assert!(matches!(
            import_pac(text),
            Err(PacError::UnsupportedVersion(99))
        ));
    }

    #[test]
    fn average_pace_is_exposed_in_both_unit_systems() {
        let workout = sample_workout();
        assert!((workout.average_pace(UnitSystem::Metric).unwrap() - 300.0).abs() < 1e-9);
        assert!((workout.average_pace(UnitSystem::Imperial).unwrap() - 482.8).abs() < 0.5);
        assert!((workout.average_speed_mps().unwrap() - 3.3333).abs() < 0.001);
    }
}
