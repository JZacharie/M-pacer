//! Qualite du signal GPS et machine a etats du feu de statut.
//!
//! Reproduit le voyant de statut de Pace Control (rouge / orange / jaune /
//! vert) et expose une API independante de la plateforme : le shell Wear OS
//! pousse des `GpsSample`, le moteur decide de l'etat affiche.

use crate::geo::{haversine_m, Position};
use crate::TimestampMs;
use serde::{Deserialize, Serialize};

/// Echantillon GPS normalise, produit par le shell (FusedLocationProvider).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GpsSample {
    pub t_ms: TimestampMs,
    pub position: Position,
    /// Precision horizontale estimee (m). 68 % de confiance, comme Android.
    pub accuracy_m: f64,
    pub altitude_m: Option<f64>,
    /// Vitesse Doppler fournie par le GNSS (m/s), plus fiable que la derive.
    pub speed_mps: Option<f64>,
}

impl GpsSample {
    pub const fn new(t_ms: TimestampMs, position: Position, accuracy_m: f64) -> Self {
        Self {
            t_ms,
            position,
            accuracy_m,
            altitude_m: None,
            speed_mps: None,
        }
    }

    pub const fn with_speed(mut self, speed_mps: f64) -> Self {
        self.speed_mps = Some(speed_mps);
        self
    }

    pub const fn with_altitude(mut self, altitude_m: f64) -> Self {
        self.altitude_m = Some(altitude_m);
        self
    }
}

/// Etat logique du GPS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpsStatus {
    /// GPS desactive par l'utilisateur : l'app ne peut pas fonctionner.
    Disabled,
    /// Acquisition en cours : pas encore de position exploitable.
    Acquiring,
    /// Position disponible mais precision insuffisante.
    Poor,
    /// Signal bon.
    Good,
}

impl GpsStatus {
    pub const fn light(self) -> StatusLight {
        match self {
            GpsStatus::Disabled => StatusLight::Red,
            GpsStatus::Acquiring => StatusLight::Orange,
            GpsStatus::Poor => StatusLight::Yellow,
            GpsStatus::Good => StatusLight::Green,
        }
    }

    /// Cle i18n (le texte final est fourni par la ressource Android).
    pub const fn message_key(self) -> &'static str {
        match self {
            GpsStatus::Disabled => "gps_disabled",
            GpsStatus::Acquiring => "gps_acquiring",
            GpsStatus::Poor => "gps_poor",
            GpsStatus::Good => "gps_ok",
        }
    }

    /// L'utilisateur peut-il demarrer une seance ?
    pub const fn can_start(self) -> bool {
        matches!(self, GpsStatus::Good | GpsStatus::Poor)
    }
}

/// Couleur du voyant de statut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusLight {
    Red,
    Orange,
    Yellow,
    Green,
}

/// Seuils de qualite (metres) et duree de confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GpsThresholds {
    /// Sous ce seuil, la precision est consideree bonne.
    pub good_accuracy_m: f64,
    /// Au-dessus, la precision est mauvaise.
    pub poor_accuracy_m: f64,
    /// Nombre d'echantillons corrects consecutifs avant de passer au vert.
    pub good_samples_required: u32,
    /// Vitesse maximale plausible (m/s) pour rejeter les sauts GPS.
    pub max_speed_mps: f64,
}

impl Default for GpsThresholds {
    fn default() -> Self {
        Self {
            good_accuracy_m: 10.0,
            poor_accuracy_m: 25.0,
            good_samples_required: 3,
            max_speed_mps: 12.0,
        }
    }
}

/// Filtre d'entree GPS : ecarte les mesures aberrantes et calcule le statut.
#[derive(Debug, Clone)]
pub struct GpsMonitor {
    thresholds: GpsThresholds,
    enabled: bool,
    last_accepted: Option<GpsSample>,
    consecutive_good: u32,
    status: GpsStatus,
    /// Nombre total d'echantillons rejetes (diagnostic / log de support).
    pub rejected_samples: u64,
}

impl Default for GpsMonitor {
    fn default() -> Self {
        Self::new(GpsThresholds::default())
    }
}

impl GpsMonitor {
    pub fn new(thresholds: GpsThresholds) -> Self {
        Self {
            thresholds,
            enabled: true,
            last_accepted: None,
            consecutive_good: 0,
            status: GpsStatus::Acquiring,
            rejected_samples: 0,
        }
    }

    pub fn status(&self) -> GpsStatus {
        self.status
    }

    pub fn thresholds(&self) -> GpsThresholds {
        self.thresholds
    }

    pub fn last_accepted(&self) -> Option<GpsSample> {
        self.last_accepted
    }

    /// L'utilisateur (ou le systeme) coupe le GPS.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.status = GpsStatus::Disabled;
            self.last_accepted = None;
            self.consecutive_good = 0;
        } else {
            self.status = GpsStatus::Acquiring;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Injecte un echantillon et renvoie le statut resultant.
    ///
    /// Un echantillon est rejete s'il implique un deplacement physiquement
    /// impossible depuis la derniere mesure acceptee (saut GPS), ou si sa
    /// precision est pire que le double du seuil "poor".
    pub fn push(&mut self, sample: GpsSample) -> GpsStatus {
        self.push_checked(sample).0
    }

    /// Comme `push`, mais indique aussi si l'echantillon a ete accepte.
    pub fn push_checked(&mut self, sample: GpsSample) -> (GpsStatus, bool) {
        if !self.enabled {
            self.status = GpsStatus::Disabled;
            return (self.status, false);
        }

        if sample.accuracy_m > self.thresholds.poor_accuracy_m * 2.0 {
            self.rejected_samples += 1;
            return (self.status, false);
        }

        if let Some(prev) = self.last_accepted {
            let dt_s = (sample.t_ms - prev.t_ms) as f64 / 1000.0;
            let moved = haversine_m(prev.position, sample.position);
            let plausible = if dt_s > 0.0 {
                moved <= self.thresholds.max_speed_mps * dt_s + sample.accuracy_m + prev.accuracy_m
            } else {
                moved <= sample.accuracy_m * 2.0
            };
            if !plausible {
                self.rejected_samples += 1;
                return (self.status, false);
            }
        }

        self.last_accepted = Some(sample);
        self.status = if sample.accuracy_m <= self.thresholds.good_accuracy_m {
            self.consecutive_good += 1;
            if self.consecutive_good >= self.thresholds.good_samples_required {
                GpsStatus::Good
            } else {
                GpsStatus::Poor
            }
        } else {
            self.consecutive_good = 0;
            // Precision insuffisante : voyant jaune tant qu'on reste sous le
            // double du seuil "poor" (au-dela l'echantillon est rejete).
            GpsStatus::Poor
        };
        (self.status, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(t: i64, lat: f64, accuracy: f64) -> GpsSample {
        GpsSample::new(t, Position::new(lat, 3.0), accuracy)
    }

    #[test]
    fn status_goes_orange_then_yellow_then_green() {
        let mut monitor = GpsMonitor::default();
        assert_eq!(monitor.status(), GpsStatus::Acquiring);
        monitor.push(sample(0, 45.0, 30.0));
        assert_eq!(monitor.status(), GpsStatus::Poor);
        // 3 echantillons precis consecutifs => vert
        monitor.push(sample(1000, 45.00001, 5.0));
        monitor.push(sample(2000, 45.00002, 5.0));
        assert_eq!(monitor.status(), GpsStatus::Poor);
        monitor.push(sample(3000, 45.00003, 5.0));
        assert_eq!(monitor.status(), GpsStatus::Good);
        assert_eq!(monitor.status().light(), StatusLight::Green);
    }

    #[test]
    fn disabled_gps_is_red() {
        let mut monitor = GpsMonitor::default();
        monitor.set_enabled(false);
        assert_eq!(monitor.status(), GpsStatus::Disabled);
        assert_eq!(monitor.status().light(), StatusLight::Red);
        assert!(!monitor.status().can_start());
    }

    #[test]
    fn gps_jump_is_rejected() {
        let mut monitor = GpsMonitor::default();
        monitor.push(sample(0, 45.0, 5.0));
        let before = monitor.last_accepted;
        monitor.push(sample(1000, 45.05, 5.0)); // ~5.5 km en 1 s
        assert_eq!(monitor.rejected_samples, 1);
        assert_eq!(monitor.last_accepted, before);
    }

    #[test]
    fn very_inaccurate_sample_is_rejected() {
        let mut monitor = GpsMonitor::default();
        monitor.push(sample(0, 45.0, 100.0));
        assert_eq!(monitor.rejected_samples, 1);
        assert!(monitor.last_accepted().is_none());
    }
}
