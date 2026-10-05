//! Moteur d'allure : moyenne glissante, detection de changement d'allure.
//!
//! Reprend l'algorithme decrit dans le manuel Pace Control :
//!
//! * l'allure courante est **moyennee sur deux minutes** ; un calcul base sur
//!   quelques secondes de GPS n'est pas exploitable ;
//! * l'option *Detect pace change* surveille la vitesse recente et, si elle
//!   differe significativement de la precedente, **redemarre la fenetre de
//!   moyennage** : indispensable pour les seances de fractionne ;
//! * l'utilisateur peut aussi reinitialiser manuellement l'allure (triple clic
//!   sur le bouton du casque audio).

use crate::units::UnitSystem;
use crate::TimestampMs;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Point (temps, distance cumulee) injecte dans le moteur.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PaceSample {
    pub t_ms: TimestampMs,
    pub dist_m: f64,
}

/// Reglages du moteur d'allure.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PaceConfig {
    /// Duree de la fenetre de moyennage (120 s par defaut, comme Pace Control).
    pub window_s: f64,
    /// Duree des fenetres comparees par la detection de changement d'allure.
    pub probe_s: f64,
    /// Distance minimale couverte par une sonde pour etre exploitable.
    pub min_probe_distance_m: f64,
    /// Ecart relatif de vitesse declenchant une remise a zero de la fenetre.
    pub change_threshold: f64,
    /// Allure maximale affichee (au-dela : `--:--`), en secondes par km.
    pub max_pace_s_per_km: f64,
}

impl Default for PaceConfig {
    fn default() -> Self {
        Self {
            window_s: 120.0,
            probe_s: 20.0,
            min_probe_distance_m: 25.0,
            change_threshold: 0.15,
            max_pace_s_per_km: 1800.0, // 30 min/km
        }
    }
}

/// Moteur d'allure.
#[derive(Debug, Clone)]
pub struct PaceEngine {
    config: PaceConfig,
    detect_pace_change: bool,
    samples: VecDeque<PaceSample>,
    segment_start_ms: Option<TimestampMs>,
    last_change_ms: Option<TimestampMs>,
}

impl Default for PaceEngine {
    fn default() -> Self {
        Self::new(PaceConfig::default())
    }
}

impl PaceEngine {
    pub fn new(config: PaceConfig) -> Self {
        Self {
            config,
            detect_pace_change: false,
            samples: VecDeque::new(),
            segment_start_ms: None,
            last_change_ms: None,
        }
    }

    pub fn config(&self) -> PaceConfig {
        self.config
    }

    pub fn detect_pace_change(&self) -> bool {
        self.detect_pace_change
    }

    /// Active/desactive la detection automatique de changement d'allure.
    pub fn set_detect_pace_change(&mut self, enabled: bool) {
        self.detect_pace_change = enabled;
    }

    /// Debut de la fenetre de moyennage courante.
    pub fn segment_start_ms(&self) -> Option<TimestampMs> {
        self.segment_start_ms
    }

    /// Nombre de points conserves (diagnostic).
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Ajoute un point (temps absolu, distance cumulee).
    pub fn push(&mut self, t_ms: TimestampMs, dist_m: f64) {
        if let Some(last) = self.samples.back() {
            if t_ms <= last.t_ms {
                return; // serie temporelle non monotone : ignore
            }
        }
        self.samples.push_back(PaceSample { t_ms, dist_m });
        if self.segment_start_ms.is_none() {
            self.segment_start_ms = Some(t_ms);
        }
        self.trim(t_ms);
        if self.detect_pace_change {
            self.maybe_detect_change(t_ms);
        }
        self.trim(t_ms);
    }

    /// Reinitialise la fenetre de moyennage (bouton casque ou changement manuel).
    pub fn reset_window(&mut self, t_ms: TimestampMs) {
        self.segment_start_ms = Some(t_ms);
        self.last_change_ms = Some(t_ms);
    }

    /// Efface tout (nouvelle seance).
    pub fn clear(&mut self) {
        self.samples.clear();
        self.segment_start_ms = None;
        self.last_change_ms = None;
    }

    /// Vitesse moyennee sur la fenetre courante (m/s).
    pub fn current_speed_mps(&self) -> Option<f64> {
        let last = *self.samples.back()?;
        let window_start = last.t_ms - (self.config.window_s * 1000.0) as i64;
        let start_ms = match self.segment_start_ms {
            Some(s) => s.max(window_start),
            None => window_start,
        };
        self.speed_between(start_ms, last.t_ms)
    }

    /// Allure moyennee sur la fenetre courante, dans le systeme demande.
    pub fn current_pace(&self, units: UnitSystem) -> Option<f64> {
        let speed = self.current_speed_mps()?;
        let pace_s_per_km = 1000.0 / speed;
        if pace_s_per_km > self.config.max_pace_s_per_km {
            return None;
        }
        units.pace_from_speed(speed)
    }

    /// Vitesse sur les `probe_s` dernieres secondes (auto-pause, detection).
    pub fn short_speed_mps(&self) -> Option<f64> {
        let last = *self.samples.back()?;
        self.speed_between(last.t_ms - (self.config.probe_s * 1000.0) as i64, last.t_ms)
    }

    /// Vrai si un changement d'allure a ete detecte lors du dernier push.
    pub fn pace_change_detected(&self) -> bool {
        match (self.last_change_ms, self.samples.back()) {
            (Some(change), Some(last)) => (last.t_ms - change).abs() < 1,
            _ => false,
        }
    }

    fn speed_between(&self, from_ms: TimestampMs, to_ms: TimestampMs) -> Option<f64> {
        if self.samples.len() < 2 || to_ms <= from_ms {
            return None;
        }
        let from = self.samples.iter().find(|s| s.t_ms >= from_ms)?;
        let to = self.samples.back()?;
        let dt = (to.t_ms - from.t_ms) as f64 / 1000.0;
        let dd = to.dist_m - from.dist_m;
        if dt < 5.0 || dd < 5.0 {
            return None;
        }
        Some(dd / dt)
    }

    fn maybe_detect_change(&mut self, t_ms: TimestampMs) {
        let probe_ms = (self.config.probe_s * 1000.0) as i64;
        if let Some(last_change) = self.last_change_ms {
            if t_ms - last_change < probe_ms {
                return;
            }
        }
        let recent = self.speed_between(t_ms - probe_ms, t_ms);
        let previous = self.speed_between(t_ms - 2 * probe_ms, t_ms - probe_ms);
        if let (Some(recent), Some(previous)) = (recent, previous) {
            let reference = recent.max(previous);
            let delta = (recent - previous).abs();
            if reference > 0.0 && delta / reference >= self.config.change_threshold {
                self.segment_start_ms = Some(t_ms - probe_ms);
                self.last_change_ms = Some(t_ms);
            }
        }
    }

    fn trim(&mut self, now_ms: TimestampMs) {
        let horizon_ms =
            ((self.config.window_s + 2.0 * self.config.probe_s + 30.0) * 1000.0) as i64;
        let cutoff = now_ms - horizon_ms;
        while let Some(front) = self.samples.front() {
            if front.t_ms < cutoff {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Alimente le moteur avec une vitesse constante, 1 point par seconde.
    fn feed(engine: &mut PaceEngine, start_t: i64, seconds: u32, speed_mps: f64, dist: &mut f64) {
        for i in 0..seconds {
            let t = start_t + (i as i64) * 1000;
            *dist += speed_mps;
            engine.push(t, *dist);
        }
    }

    #[test]
    fn steady_pace_is_3_min_per_km_at_5_55_mps() {
        let mut engine = PaceEngine::default();
        let mut dist = 0.0;
        feed(&mut engine, 0, 180, 5.5556, &mut dist);
        let pace = engine.current_pace(UnitSystem::Metric).unwrap();
        assert!((pace - 180.0).abs() < 1.0, "pace = {pace}");
    }

    #[test]
    fn window_is_limited_to_two_minutes() {
        let mut engine = PaceEngine::default();
        let mut dist = 0.0;
        // 1 km à 10 km/h, puis 1 km à 20 km/h : l'allure affichee doit refléter
        // globalement la fenetre de 2 minutes, pas tout l'historique.
        feed(&mut engine, 0, 360, 2.7778, &mut dist);
        feed(&mut engine, 360_000, 360, 5.5556, &mut dist);
        let pace = engine.current_pace(UnitSystem::Metric).unwrap();
        assert!(pace < 200.0, "l'allure doit avoir diminue, pace = {pace}");
    }

    #[test]
    fn no_pace_without_movement() {
        let mut engine = PaceEngine::default();
        engine.push(0, 0.0);
        engine.push(60_000, 0.0);
        assert!(engine.current_pace(UnitSystem::Metric).is_none());
    }

    #[test]
    fn non_monotonic_timestamps_are_ignored() {
        let mut engine = PaceEngine::default();
        engine.push(1000, 10.0);
        engine.push(500, 20.0);
        assert_eq!(engine.sample_count(), 1);
    }

    #[test]
    fn manual_reset_discards_previous_pace_history() {
        let mut engine = PaceEngine::default();
        let mut dist = 0.0;
        feed(&mut engine, 0, 180, 2.7778, &mut dist); // 10 km/h => 6:00/km
        assert!((engine.current_pace(UnitSystem::Metric).unwrap() - 360.0).abs() < 2.0);
        engine.reset_window(180_000);
        feed(&mut engine, 181_000, 60, 5.5556, &mut dist); // 20 km/h => 3:00/km
        let pace = engine.current_pace(UnitSystem::Metric).unwrap();
        assert!((pace - 180.0).abs() < 5.0, "pace = {pace}");
    }

    #[test]
    fn pace_change_detection_restarts_window() {
        let mut engine = PaceEngine::default();
        engine.set_detect_pace_change(true);
        let mut dist = 0.0;
        feed(&mut engine, 0, 180, 3.0, &mut dist); // 5:33/km
        let before = engine.current_pace(UnitSystem::Metric).unwrap();
        feed(&mut engine, 181_000, 120, 6.0, &mut dist); // passage à 2:47/km
        let after = engine.current_pace(UnitSystem::Metric).unwrap();
        assert!(after < before - 20.0, "before = {before}, after = {after}");
        // La fenetre a bien redemarre apres le changement d'allure : son debut
        // est posterieur au debut de l'historique (au lieu de conserver les
        // 2 minutes completes).
        let start = engine.segment_start_ms().unwrap();
        assert!(start > 160_000, "debut de fenetre = {start}");
    }

    #[test]
    fn without_detection_window_keeps_full_history() {
        let mut engine = PaceEngine::default();
        let mut dist = 0.0;
        feed(&mut engine, 0, 180, 3.0, &mut dist);
        feed(&mut engine, 181_000, 60, 6.0, &mut dist);
        let pace = engine.current_pace(UnitSystem::Metric).unwrap();
        // Toujours proche de la moyenne des 2 dernieres minutes (majoritairement lente)
        assert!(pace > 220.0, "pace = {pace}");
    }
}
