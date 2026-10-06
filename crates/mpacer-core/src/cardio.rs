//! Frequence cardiaque : mesures, zones et derive cardiaque.
//!
//! Les mesures arrivent de la montre ou d'une ceinture (Bluetooth). Comme la
//! trace GPS, elles sont horodatees : c'est la seule facon de les recaler sur
//! la distance parcourue, qui reste la reference de tous les ecrans.

use crate::best_distances::{distance_at_time_m, TrackPoint};
use serde::{Deserialize, Serialize};

/// Une mesure de frequence cardiaque (bpm) a un instant donne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartRateSample {
    pub t_ms: i64,
    pub bpm: u16,
}

/// Bornes des cinq zones, en fraction de la reference.
pub const ZONE_FRACTIONS: [(f64, f64); 5] = [
    (0.50, 0.60),
    (0.60, 0.70),
    (0.70, 0.80),
    (0.80, 0.90),
    (0.90, 1.00),
];

/// Nom court de chaque zone, dans l'ordre.
pub const ZONE_NAMES: [&str; 5] = [
    "Z1 recuperation",
    "Z2 endurance",
    "Z3 tempo",
    "Z4 seuil",
    "Z5 VO2max",
];

/// Methode de calcul des zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ZoneMethod {
    /// Pourcentage de la frequence cardiaque maximale (methode la plus repandue).
    #[default]
    PercentMax,
    /// Reserve de frequence cardiaque (Karvonen) : FC de repos + x % de (FC max - FC repos).
    HeartRateReserve,
}

/// Reference utilisee pour situer une frequence cardiaque.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeartRateZones {
    pub max_bpm: u16,
    #[serde(default)]
    pub resting_bpm: Option<u16>,
    #[serde(default)]
    pub method: ZoneMethod,
}

impl Default for HeartRateZones {
    fn default() -> Self {
        Self {
            max_bpm: 190,
            resting_bpm: None,
            method: ZoneMethod::PercentMax,
        }
    }
}

impl HeartRateZones {
    pub fn new(max_bpm: u16) -> Self {
        Self {
            max_bpm,
            ..Default::default()
        }
    }

    pub fn with_resting(mut self, resting_bpm: u16) -> Self {
        self.resting_bpm = Some(resting_bpm);
        self
    }

    pub fn with_method(mut self, method: ZoneMethod) -> Self {
        self.method = method;
        self
    }

    /// Bornes basses et hautes des cinq zones, en bpm.
    ///
    /// La reserve de frequence cardiaque n'est utilisee que si une FC de repos
    /// plausible est renseignee : sinon la methode retombe sur le pourcentage
    /// de FC max, plutot que de produire des zones incoherentes.
    pub fn boundaries(&self) -> [(f64, f64); 5] {
        let max = self.max_bpm as f64;
        let reserve = match (self.method, self.resting_bpm) {
            (ZoneMethod::HeartRateReserve, Some(resting))
                if (resting as f64) < max && resting > 0 =>
            {
                Some(resting as f64)
            }
            _ => None,
        };
        std::array::from_fn(|index| {
            let (low, high) = ZONE_FRACTIONS[index];
            match reserve {
                Some(resting) => (
                    resting + low * (max - resting),
                    resting + high * (max - resting),
                ),
                None => (low * max, high * max),
            }
        })
    }

    /// Numero de zone (1 a 5) ; 0 si la frequence est sous la zone 1.
    pub fn zone_of(&self, bpm: f64) -> u8 {
        let bounds = self.boundaries();
        if bpm < bounds[0].0 {
            return 0;
        }
        for (index, (_, high)) in bounds.iter().enumerate() {
            if bpm < *high {
                return index as u8 + 1;
            }
        }
        5
    }
}

/// Bilan cardiaque d'une seance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeartRateSummary {
    pub sample_count: usize,
    pub average_bpm: f64,
    pub max_bpm: u16,
    pub min_bpm: u16,
    /// Temps passe dans chaque zone (s), index 0 = Z1.
    pub zone_seconds: [f64; 5],
    /// Temps passe sous la zone 1 (s) : echauffement, recuperation.
    pub below_zone1_s: f64,
    pub zones: HeartRateZones,
}

impl HeartRateSummary {
    /// Temps total couvert par les mesures (s).
    pub fn total_seconds(&self) -> f64 {
        self.zone_seconds.iter().sum::<f64>() + self.below_zone1_s
    }

    /// Part du temps passe dans une zone (%), index 0 = Z1.
    pub fn zone_percent(&self, index: usize) -> f64 {
        let total = self.total_seconds();
        if total <= 0.0 {
            return 0.0;
        }
        self.zone_seconds[index] / total * 100.0
    }
}

/// Ecart maximal tolere entre deux mesures pour compter le temps ecoule.
///
/// Un trou plus grand (montre retiree, ceinture deconnectee) ne doit pas etre
/// invente : il est simplement ignore.
const MAX_SAMPLE_GAP_S: f64 = 15.0;

/// Calcule le bilan cardiaque d'une serie de mesures.
pub fn summarize(samples: &[HeartRateSample], zones: HeartRateZones) -> Option<HeartRateSummary> {
    if samples.is_empty() {
        return None;
    }
    let count = samples.len();
    let sum: f64 = samples.iter().map(|s| s.bpm as f64).sum();
    let max_bpm = samples.iter().map(|s| s.bpm).max().unwrap_or(0);
    let min_bpm = samples.iter().map(|s| s.bpm).min().unwrap_or(0);
    let mut zone_seconds = [0.0_f64; 5];
    let mut below_zone1_s = 0.0;
    for window in samples.windows(2) {
        let (previous, next) = (window[0], window[1]);
        let seconds = (next.t_ms - previous.t_ms) as f64 / 1000.0;
        if !(0.0..=MAX_SAMPLE_GAP_S).contains(&seconds) || seconds == 0.0 {
            continue;
        }
        let zone = zones.zone_of(previous.bpm as f64);
        if zone == 0 {
            below_zone1_s += seconds;
        } else {
            zone_seconds[zone as usize - 1] += seconds;
        }
    }
    Some(HeartRateSummary {
        sample_count: count,
        average_bpm: sum / count as f64,
        max_bpm,
        min_bpm,
        zone_seconds,
        below_zone1_s,
        zones,
    })
}

/// Derive cardiaque (decouplage aerobie) entre la premiere et la seconde moitie.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CardiacDrift {
    /// Metres parcourus par battement sur la premiere moitie.
    pub first_half_efficiency: f64,
    /// Metres parcourus par battement sur la seconde moitie.
    pub second_half_efficiency: f64,
    /// Perte de rendement en pourcentage : positif = le coeur a derive.
    pub decoupling_percent: f64,
}

/// Compare le rendement (vitesse / frequence cardiaque) des deux moities.
///
/// Un decouplage faible (< 5 %) signifie que la seance est restee aerobie ;
/// au-dela, la fatigue ou la chaleur se sont fait sentir.
pub fn cardiac_drift(track: &[TrackPoint], samples: &[HeartRateSample]) -> Option<CardiacDrift> {
    let total_distance = track.last()?.dist_m;
    if total_distance <= 0.0 || samples.len() < 4 {
        return None;
    }
    let half = total_distance / 2.0;

    // Chaque intervalle entre deux mesures est rattache a la moitie ou il tombe.
    let mut first = (0.0_f64, 0.0_f64, 0.0_f64); // (distance, temps, temps x bpm)
    let mut second = (0.0_f64, 0.0_f64, 0.0_f64);
    for window in samples.windows(2) {
        let (previous, next) = (window[0], window[1]);
        let seconds = (next.t_ms - previous.t_ms) as f64 / 1000.0;
        if !(0.0..=MAX_SAMPLE_GAP_S).contains(&seconds) || seconds == 0.0 {
            continue;
        }
        let (Some(start), Some(end)) = (
            distance_at_time_m(track, previous.t_ms),
            distance_at_time_m(track, next.t_ms),
        ) else {
            continue;
        };
        let distance = (end - start).max(0.0);
        let bpm = (previous.bpm as f64 + next.bpm as f64) / 2.0;
        let bucket = if (start + end) / 2.0 < half {
            &mut first
        } else {
            &mut second
        };
        bucket.0 += distance;
        bucket.1 += seconds;
        bucket.2 += seconds * bpm;
    }
    let (first_distance, first_seconds, first_bpm_sum) = first;
    let (second_distance, second_seconds, second_bpm_sum) = second;
    if first_seconds <= 0.0 || second_seconds <= 0.0 {
        return None;
    }
    let first_hr = first_bpm_sum / first_seconds;
    let second_hr = second_bpm_sum / second_seconds;
    if first_hr <= 0.0 || second_hr <= 0.0 {
        return None;
    }
    let first_efficiency = first_distance / first_seconds / first_hr;
    let second_efficiency = second_distance / second_seconds / second_hr;
    if first_efficiency <= 0.0 {
        return None;
    }
    Some(CardiacDrift {
        first_half_efficiency: first_efficiency,
        second_half_efficiency: second_efficiency,
        decoupling_percent: (first_efficiency - second_efficiency) / first_efficiency * 100.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(pairs: &[(i64, u16)]) -> Vec<HeartRateSample> {
        pairs
            .iter()
            .map(|(t_ms, bpm)| HeartRateSample {
                t_ms: *t_ms,
                bpm: *bpm,
            })
            .collect()
    }

    #[test]
    fn percent_of_max_zones_are_ordered() {
        let zones = HeartRateZones::new(200);
        assert_eq!(zones.zone_of(90.0), 0); // sous 50 %
        assert_eq!(zones.zone_of(110.0), 1); // 55 %
        assert_eq!(zones.zone_of(130.0), 2); // 65 %
        assert_eq!(zones.zone_of(150.0), 3); // 75 %
        assert_eq!(zones.zone_of(170.0), 4); // 85 %
        assert_eq!(zones.zone_of(185.0), 5); // 92 %
    }

    #[test]
    fn karvonen_zones_use_the_resting_rate() {
        let zones = HeartRateZones::new(200)
            .with_resting(50)
            .with_method(ZoneMethod::HeartRateReserve);
        let bounds = zones.boundaries();
        // Z1 : 50 + 50 % de 150 = 125 bpm
        assert!((bounds[0].0 - 125.0).abs() < 1e-9);
        assert!((bounds[4].1 - 200.0).abs() < 1e-9);
        assert_eq!(zones.zone_of(130.0), 1);
    }

    #[test]
    fn reserve_falls_back_to_max_when_resting_is_missing() {
        let zones = HeartRateZones::new(200).with_method(ZoneMethod::HeartRateReserve);
        assert!((zones.boundaries()[0].0 - 100.0).abs() < 1e-9);
    }

    #[test]
    fn summary_times_each_zone_and_ignores_gaps() {
        // 10 s a 110 bpm (Z1 avec FC max 200), puis un trou de 60 s, puis 10 s a 185 (Z5).
        let samples = samples(&[(0, 110), (10_000, 110), (70_000, 185), (80_000, 185)]);
        let summary = summarize(&samples, HeartRateZones::new(200)).unwrap();
        assert_eq!(summary.sample_count, 4);
        assert!((summary.zone_seconds[0] - 10.0).abs() < 1e-9);
        assert!((summary.zone_seconds[4] - 10.0).abs() < 1e-9);
        assert!((summary.average_bpm - 147.5).abs() < 1e-9);
        assert_eq!(summary.max_bpm, 185);
        assert_eq!(summary.min_bpm, 110);
    }

    #[test]
    fn a_trace_without_heart_rate_has_no_summary() {
        assert!(summarize(&[], HeartRateZones::default()).is_none());
    }

    /// Trace de 4 km a 3 m/s, 1 point par seconde.
    fn steady_track() -> Vec<TrackPoint> {
        (0..=4000)
            .map(|i| TrackPoint {
                t_ms: i * 1000,
                dist_m: i as f64 * 3.0,
                lat: 45.0,
                lon: 3.0,
                elevation_m: None,
            })
            .collect()
    }

    /// Une mesure toutes les 10 s.
    fn every_ten_seconds(bpm: impl Fn(i64) -> u16) -> Vec<HeartRateSample> {
        (0..=400)
            .map(|i| HeartRateSample {
                t_ms: i * 10_000,
                bpm: bpm(i),
            })
            .collect()
    }

    #[test]
    fn steady_run_shows_no_cardiac_drift() {
        let drift = cardiac_drift(&steady_track(), &every_ten_seconds(|_| 150)).unwrap();
        assert!(drift.decoupling_percent.abs() < 0.5, "{drift:?}");
    }

    #[test]
    fn rising_heart_rate_at_constant_speed_is_detected() {
        // 140 bpm au debut, 160 bpm a la fin, a vitesse constante.
        let samples = every_ten_seconds(|i| 140 + (i as u16) / 10);
        let drift = cardiac_drift(&steady_track(), &samples).unwrap();
        assert!(drift.decoupling_percent > 8.0, "{drift:?}");
    }
}
