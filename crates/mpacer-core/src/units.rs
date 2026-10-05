//! Unites metriques / imperiales et formatage des valeurs affichees.

use serde::{Deserialize, Serialize};

/// Metres dans un mile terrestre.
pub const METERS_PER_MILE: f64 = 1609.344;
/// Metres dans un kilometre.
pub const METERS_PER_KM: f64 = 1000.0;

/// Systeme d'unites choisi par l'utilisateur (reglage "Units").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UnitSystem {
    #[default]
    Metric,
    Imperial,
}

impl UnitSystem {
    /// Metres pour une unite de distance de ce systeme.
    pub const fn meters_per_unit(self) -> f64 {
        match self {
            UnitSystem::Metric => METERS_PER_KM,
            UnitSystem::Imperial => METERS_PER_MILE,
        }
    }

    /// Suffixe court de distance ("km" / "mi").
    pub const fn label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "km",
            UnitSystem::Imperial => "mi",
        }
    }

    /// Nom complet de l'unite de distance.
    pub const fn long_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "kilometers",
            UnitSystem::Imperial => "miles",
        }
    }

    /// Libelle d'allure ("min/km" / "min/mi").
    pub const fn pace_label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "min/km",
            UnitSystem::Imperial => "min/mi",
        }
    }

    /// Longueur d'un tour dans ce systeme.
    pub const fn lap_length_m(self) -> f64 {
        self.meters_per_unit()
    }

    /// Convertit une distance (m) en unites de ce systeme.
    pub fn distance_in_units(self, meters: f64) -> f64 {
        meters / self.meters_per_unit()
    }

    /// Convertit une vitesse (m/s) en allure (secondes par unite).
    pub fn pace_from_speed(self, speed_mps: f64) -> Option<f64> {
        if !speed_mps.is_finite() || speed_mps <= 0.05 {
            return None;
        }
        Some(self.meters_per_unit() / speed_mps)
    }

    /// Convertit une allure (s/unite) en vitesse (m/s).
    pub fn speed_from_pace(self, pace_s_per_unit: f64) -> Option<f64> {
        if !pace_s_per_unit.is_finite() || pace_s_per_unit <= 0.0 {
            return None;
        }
        Some(self.meters_per_unit() / pace_s_per_unit)
    }

    /// Distance formatee, ex. `"10.05 km"`.
    pub fn format_distance(self, meters: f64) -> String {
        format!("{:.2} {}", self.distance_in_units(meters), self.label())
    }

    /// Distance courte (1 decimale), pour les ecarts au shadow runner.
    pub fn format_distance_short(self, meters: f64) -> String {
        if self == UnitSystem::Metric && meters.abs() < 1000.0 {
            format!("{:.0} m", meters)
        } else {
            format!("{:.2} {}", self.distance_in_units(meters), self.label())
        }
    }
}

/// Configuration d'affichage des distances.
///
/// Reproduit le reglage Pace Control "respect race distance units" : l'allure
/// suit le systeme prefere, mais la distance de la course reste exprimee dans
/// l'unite de la course (ex. 5 km) pour que le coureur mesure facilement ce
/// qu'il lui reste a parcourir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistanceDisplay {
    pub preferred: UnitSystem,
    pub respect_race_units: bool,
    pub race_units: UnitSystem,
}

impl Default for DistanceDisplay {
    fn default() -> Self {
        Self {
            preferred: UnitSystem::Metric,
            respect_race_units: false,
            race_units: UnitSystem::Metric,
        }
    }
}

impl DistanceDisplay {
    /// Systeme utilise pour les distances affichees.
    pub fn distance_system(&self) -> UnitSystem {
        if self.respect_race_units {
            self.race_units
        } else {
            self.preferred
        }
    }
}

/// Formate une duree en secondes : `"42:15"` ou `"1:23:45"`.
pub fn format_duration(total_seconds: f64) -> String {
    if !total_seconds.is_finite() {
        return "--:--".to_string();
    }
    let negative = total_seconds < 0.0;
    let total = total_seconds.abs().round() as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    let body = if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    };
    if negative {
        format!("-{body}")
    } else {
        body
    }
}

/// Formate une allure en secondes par unite : `"5:41"`.
pub fn format_pace(pace_s_per_unit: Option<f64>) -> String {
    match pace_s_per_unit {
        None => "--:--".to_string(),
        Some(p) if !p.is_finite() || p < 0.0 => "--:--".to_string(),
        Some(p) => {
            let total = p.round() as u64;
            let (m, s) = (total / 60, total % 60);
            format!("{m}:{s:02}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(0.0), "0:00");
        assert_eq!(format_duration(62.4), "1:02");
        assert_eq!(format_duration(2535.0), "42:15");
        assert_eq!(format_duration(5025.0), "1:23:45");
        assert_eq!(format_duration(f64::INFINITY), "--:--");
    }

    #[test]
    fn pace_formatting_rolls_over_seconds() {
        assert_eq!(format_pace(Some(341.4)), "5:41");
        assert_eq!(format_pace(Some(59.6)), "1:00");
        assert_eq!(format_pace(None), "--:--");
    }

    #[test]
    fn pace_speed_roundtrip() {
        // 5:41 min/km ~ 2.93 m/s
        let speed = UnitSystem::Metric.speed_from_pace(341.0).unwrap();
        let pace = UnitSystem::Metric.pace_from_speed(speed).unwrap();
        assert!((pace - 341.0).abs() < 1e-9);
        // 1 mile = 1609.344 m : une allure de 9:00 min/mi => 2.98 m/s
        let speed_mi = UnitSystem::Imperial.speed_from_pace(540.0).unwrap();
        assert!((speed_mi - 2.9803).abs() < 0.001);
    }

    #[test]
    fn race_units_are_respected_for_distance_only() {
        let display = DistanceDisplay {
            preferred: UnitSystem::Imperial,
            respect_race_units: true,
            race_units: UnitSystem::Metric,
        };
        assert_eq!(display.distance_system(), UnitSystem::Metric);
        assert_eq!(display.preferred.pace_label(), "min/mi");
    }

    #[test]
    fn distance_formatting() {
        assert_eq!(UnitSystem::Metric.format_distance(10050.0), "10.05 km");
        assert_eq!(UnitSystem::Imperial.format_distance(1609.344), "1.00 mi");
        assert_eq!(UnitSystem::Metric.format_distance_short(350.0), "350 m");
    }
}
