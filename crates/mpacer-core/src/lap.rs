//! Tours automatiques (kilometre ou mile) et allures de tour.
//!
//! Fournit l'"allure du km courant" (tronçon partiel) et l'"allure du km
//! precedent" (tour complet), les deux valeurs affichees par Pace Control.

use crate::units::UnitSystem;
use crate::TimestampMs;
use serde::{Deserialize, Serialize};

/// Tour enregistre.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Lap {
    /// Numero du tour, a partir de 1.
    pub index: u32,
    /// Distance du tour (m).
    pub distance_m: f64,
    /// Duree du tour (s).
    pub duration_s: f64,
    /// Allure moyenne du tour (s/km), base de comparaison inter-unites.
    pub pace_s_per_km: f64,
}

impl Lap {
    /// Allure du tour dans le systeme demande.
    pub fn pace_in(&self, units: UnitSystem) -> f64 {
        units.meters_per_unit() / (1000.0 / self.pace_s_per_km)
    }
}

/// Suivi des tours.
#[derive(Debug, Clone)]
pub struct LapTracker {
    lap_length_m: f64,
    laps: Vec<Lap>,
    lap_start_dist_m: f64,
    lap_start_t_ms: TimestampMs,
}

impl Default for LapTracker {
    fn default() -> Self {
        Self::new(UnitSystem::Metric)
    }
}

impl LapTracker {
    pub fn new(units: UnitSystem) -> Self {
        Self {
            lap_length_m: units.lap_length_m(),
            laps: Vec::new(),
            lap_start_dist_m: 0.0,
            lap_start_t_ms: 0,
        }
    }

    /// Change l'unite de tour (les tours deja enregistres sont conserves).
    pub fn set_units(&mut self, units: UnitSystem) {
        self.lap_length_m = units.lap_length_m();
    }

    /// Nouvelle seance : distance et temps remis a zero.
    pub fn start(&mut self, t_ms: TimestampMs) {
        self.laps.clear();
        self.lap_start_dist_m = 0.0;
        self.lap_start_t_ms = t_ms;
    }

    pub fn lap_length_m(&self) -> f64 {
        self.lap_length_m
    }

    pub fn laps(&self) -> &[Lap] {
        &self.laps
    }

    pub fn lap_count(&self) -> usize {
        self.laps.len()
    }

    /// Allure du tour precedent (tour complet), s'il existe.
    pub fn previous_lap_pace(&self) -> Option<f64> {
        self.laps.last().map(|l| l.pace_s_per_km)
    }

    /// Distance parcourue dans le tour courant.
    pub fn current_lap_distance_m(&self, total_dist_m: f64) -> f64 {
        (total_dist_m - self.lap_start_dist_m).max(0.0)
    }

    /// Allure du tour courant (tronçon partiel), comme Pace Control.
    pub fn current_lap_pace(
        &self,
        units: UnitSystem,
        t_ms: TimestampMs,
        total_dist_m: f64,
    ) -> Option<f64> {
        let dt = (t_ms - self.lap_start_t_ms) as f64 / 1000.0;
        let dd = self.current_lap_distance_m(total_dist_m);
        if dt < 10.0 || dd < 30.0 {
            return None;
        }
        let speed = dd / dt;
        units.pace_from_speed(speed)
    }

    /// Point d'avancee : renvoie les tours franchis depuis le dernier appel.
    pub fn update(&mut self, t_ms: TimestampMs, total_dist_m: f64) -> Vec<Lap> {
        let mut completed = Vec::new();
        while total_dist_m - self.lap_start_dist_m >= self.lap_length_m {
            let boundary = self.lap_start_dist_m + self.lap_length_m;
            let span = total_dist_m - self.lap_start_dist_m;
            let fraction = if span > 0.0 {
                self.lap_length_m / span
            } else {
                1.0
            };
            let boundary_t = self.lap_start_t_ms
                + (((t_ms - self.lap_start_t_ms) as f64) * fraction).round() as i64;
            let duration_s = (boundary_t - self.lap_start_t_ms) as f64 / 1000.0;
            let pace_s_per_km = if duration_s > 0.0 {
                duration_s / (self.lap_length_m / 1000.0)
            } else {
                0.0
            };
            completed.push(Lap {
                index: self.laps.len() as u32 + 1,
                distance_m: self.lap_length_m,
                duration_s,
                pace_s_per_km,
            });
            self.laps.push(*completed.last().unwrap());
            self.lap_start_dist_m = boundary;
            self.lap_start_t_ms = boundary_t;
        }
        completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lap_is_emitted_at_each_kilometer() {
        let mut tracker = LapTracker::new(UnitSystem::Metric);
        tracker.start(0);
        // 5:00 min/km => 1000 m en 300 s
        for i in 1..=100 {
            tracker.update(i * 3_000, i as f64 * 10.0);
        }
        assert_eq!(tracker.lap_count(), 1);
        let lap = tracker.laps()[0];
        assert_eq!(lap.index, 1);
        assert!((lap.duration_s - 300.0).abs() < 0.001);
        assert!((lap.pace_s_per_km - 300.0).abs() < 0.001);
        assert!((lap.pace_in(UnitSystem::Imperial) - 482.8).abs() < 0.5);
    }

    #[test]
    fn partial_lap_pace_and_previous_lap_pace() {
        let mut tracker = LapTracker::new(UnitSystem::Metric);
        tracker.start(0);
        tracker.update(300_000, 1000.0);
        assert!((tracker.previous_lap_pace().unwrap() - 300.0).abs() < 0.001);
        // Tronçon trop court (5 s) : aucune allure affichee
        assert!(tracker
            .current_lap_pace(UnitSystem::Metric, 305_000, 1010.0)
            .is_none());
        // 100 m en 30 s => 5:00/km sur le tour courant
        let pace = tracker
            .current_lap_pace(UnitSystem::Metric, 330_000, 1100.0)
            .unwrap();
        assert!((pace - 300.0).abs() < 20.0, "pace = {pace}");
    }

    #[test]
    fn mile_laps_use_mile_length() {
        let mut tracker = LapTracker::new(UnitSystem::Imperial);
        tracker.start(0);
        tracker.update(600_000, 1609.344);
        assert_eq!(tracker.lap_count(), 1);
        assert!((tracker.laps()[0].pace_in(UnitSystem::Imperial) - 600.0).abs() < 0.001);
    }

    #[test]
    fn large_jump_emits_several_laps() {
        let mut tracker = LapTracker::new(UnitSystem::Metric);
        tracker.start(0);
        let laps = tracker.update(900_000, 3000.0);
        assert_eq!(laps.len(), 3);
        assert_eq!(laps[2].index, 3);
    }
}
