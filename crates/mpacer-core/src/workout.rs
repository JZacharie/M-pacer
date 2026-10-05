//! Machine a etats de la seance : demarrage suspendu, pause auto, arret.
//!
//! Etats et transitions (inspires du comportement Pace Control) :
//!
//! ```text
//! Idle --arm()--> Armed --mouvement/start()--> Running
//! Idle --start()--> Running
//! Running --immobile X s (auto_pause)--> AutoPaused --mouvement--> Running
//! Running/AutoPaused --pause()--> Paused --resume()--> Running
//! Paused --start()--> Running
//! (Running|AutoPaused|Paused) --stop()--> Finished --reset()--> Idle
//! ```
//!
//! Le temps de seance ne s'ecoule **que** dans l'etat `Running` : les pauses
//! manuelles et automatiques sont exclues de la duree et de l'allure.

use crate::geo::Position;
use crate::gps::GpsSample;
use crate::TimestampMs;
use serde::{Deserialize, Serialize};

/// Etat de la seance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkoutState {
    Idle,
    /// Demarrage suspendu (appui long sur Start) : le chrono attend le mouvement.
    Armed,
    Running,
    /// Pause declenchee automatiquement apres une immobilisation.
    AutoPaused,
    /// Pause demandee par l'utilisateur.
    Paused,
    Finished,
}

impl WorkoutState {
    pub const fn is_timing(self) -> bool {
        matches!(self, WorkoutState::Running)
    }

    pub const fn is_paused(self) -> bool {
        matches!(self, WorkoutState::Paused | WorkoutState::AutoPaused)
    }

    pub const fn is_active(self) -> bool {
        matches!(
            self,
            WorkoutState::Armed
                | WorkoutState::Running
                | WorkoutState::AutoPaused
                | WorkoutState::Paused
        )
    }
}

/// Evenement emis par la seance (utilise pour la voix et l'interface).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkoutEvent {
    Armed,
    Started,
    Paused,
    Resumed,
    AutoPaused,
    AutoResumed,
    Stopped,
}

/// Reglages de la seance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorkoutConfig {
    pub auto_pause: bool,
    /// Vitesse sous laquelle on considere le coureur a l'arret (m/s).
    pub stop_speed_mps: f64,
    /// Duree d'immobilisation avant la pause automatique (s).
    pub auto_pause_delay_s: f64,
    /// Vitesse au-dessus de laquelle la seance reprend (m/s).
    pub resume_speed_mps: f64,
    /// Duree de mouvement avant la reprise automatique (s).
    pub auto_resume_delay_s: f64,
}

impl Default for WorkoutConfig {
    fn default() -> Self {
        Self {
            auto_pause: false,
            stop_speed_mps: 0.7, // ~2.5 km/h
            auto_pause_delay_s: 10.0,
            resume_speed_mps: 1.4, // ~5 km/h
            auto_resume_delay_s: 3.0,
        }
    }
}

/// Seance en cours.
#[derive(Debug, Clone)]
pub struct Workout {
    config: WorkoutConfig,
    state: WorkoutState,
    elapsed_s: f64,
    distance_m: f64,
    last_tick_ms: Option<TimestampMs>,
    last_position: Option<Position>,
    slow_since_ms: Option<TimestampMs>,
    fast_since_ms: Option<TimestampMs>,
    /// Nombre de points GPS rejetes par le filtre de plausibilite.
    pub rejected_deltas: u64,
}

impl Default for Workout {
    fn default() -> Self {
        Self::new(WorkoutConfig::default())
    }
}

impl Workout {
    pub fn new(config: WorkoutConfig) -> Self {
        Self {
            config,
            state: WorkoutState::Idle,
            elapsed_s: 0.0,
            distance_m: 0.0,
            last_tick_ms: None,
            last_position: None,
            slow_since_ms: None,
            fast_since_ms: None,
            rejected_deltas: 0,
        }
    }

    pub fn config(&self) -> WorkoutConfig {
        self.config
    }

    pub fn set_config(&mut self, config: WorkoutConfig) {
        self.config = config;
    }

    pub fn state(&self) -> WorkoutState {
        self.state
    }

    /// Temps de seance (pauses exclues).
    pub fn elapsed_s(&self) -> f64 {
        self.elapsed_s
    }

    pub fn distance_m(&self) -> f64 {
        self.distance_m
    }

    /// Demarre la seance immediatement.
    pub fn start(&mut self, t_ms: TimestampMs) -> Vec<WorkoutEvent> {
        self.begin(t_ms, false)
    }

    /// Demarrage suspendu (appui long) : le chrono attend le premier mouvement.
    pub fn arm(&mut self, t_ms: TimestampMs) -> Vec<WorkoutEvent> {
        self.begin(t_ms, true)
    }

    fn begin(&mut self, t_ms: TimestampMs, armed: bool) -> Vec<WorkoutEvent> {
        self.elapsed_s = 0.0;
        self.distance_m = 0.0;
        self.last_position = None;
        self.slow_since_ms = None;
        self.fast_since_ms = None;
        self.last_tick_ms = Some(t_ms);
        self.state = if armed {
            WorkoutState::Armed
        } else {
            WorkoutState::Running
        };
        vec![if armed {
            WorkoutEvent::Armed
        } else {
            WorkoutEvent::Started
        }]
    }

    /// Pause manuelle.
    pub fn pause(&mut self, t_ms: TimestampMs) -> Vec<WorkoutEvent> {
        if !matches!(self.state, WorkoutState::Running | WorkoutState::Armed) {
            return Vec::new();
        }
        self.advance(t_ms);
        self.state = WorkoutState::Paused;
        self.slow_since_ms = None;
        self.fast_since_ms = None;
        vec![WorkoutEvent::Paused]
    }

    /// Reprise manuelle.
    pub fn resume(&mut self, t_ms: TimestampMs) -> Vec<WorkoutEvent> {
        if !self.state.is_paused() {
            return Vec::new();
        }
        self.state = WorkoutState::Running;
        self.last_tick_ms = Some(t_ms);
        self.slow_since_ms = None;
        self.fast_since_ms = None;
        vec![WorkoutEvent::Resumed]
    }

    /// Fin de seance.
    pub fn stop(&mut self, t_ms: TimestampMs) -> Vec<WorkoutEvent> {
        if !self.state.is_active() {
            return Vec::new();
        }
        self.advance(t_ms);
        self.state = WorkoutState::Finished;
        vec![WorkoutEvent::Stopped]
    }

    /// Retour a l'etat initial pour une nouvelle seance.
    pub fn reset(&mut self) {
        *self = Workout::new(self.config);
    }

    /// Fait avancer le chrono jusqu'a `t_ms` si la seance tourne.
    pub fn tick(&mut self, t_ms: TimestampMs) {
        self.advance(t_ms);
    }

    fn advance(&mut self, t_ms: TimestampMs) {
        if let Some(last) = self.last_tick_ms {
            if t_ms > last && self.state.is_timing() {
                self.elapsed_s += (t_ms - last) as f64 / 1000.0;
            }
        }
        if self.last_tick_ms.is_none() || t_ms > self.last_tick_ms.unwrap() {
            self.last_tick_ms = Some(t_ms);
        }
    }

    /// Integre une position GPS validee par le filtre d'entree.
    pub fn on_sample(&mut self, sample: GpsSample) {
        if let Some(previous) = self.last_position {
            let delta = crate::geo::haversine_m(previous, sample.position);
            match self.state {
                WorkoutState::Running => {
                    self.distance_m += delta;
                }
                WorkoutState::Armed => {
                    // Premier mouvement : la seance demarre (relais du chrono).
                    self.state = WorkoutState::Running;
                    self.distance_m += delta;
                }
                _ => {}
            }
        }
        self.last_position = Some(sample.position);
    }

    /// Logique de pause/reprise automatique a partir de la vitesse lissee.
    pub fn update_motion(
        &mut self,
        t_ms: TimestampMs,
        speed_mps: Option<f64>,
    ) -> Vec<WorkoutEvent> {
        if !self.config.auto_pause {
            return Vec::new();
        }
        let speed = speed_mps.unwrap_or(0.0);
        let mut events = Vec::new();

        if self.state == WorkoutState::Running {
            if speed < self.config.stop_speed_mps {
                let since = *self.slow_since_ms.get_or_insert(t_ms);
                if (t_ms - since) as f64 / 1000.0 >= self.config.auto_pause_delay_s {
                    self.state = WorkoutState::AutoPaused;
                    self.slow_since_ms = None;
                    self.last_tick_ms = Some(t_ms);
                    events.push(WorkoutEvent::AutoPaused);
                }
            } else {
                self.slow_since_ms = None;
            }
        } else if self.state == WorkoutState::AutoPaused || self.state == WorkoutState::Armed {
            // Reaction directe pour l'etat Armed (demarrage suspendu), delai pour AutoPaused.
            let required = if self.state == WorkoutState::Armed {
                0.0
            } else {
                self.config.auto_resume_delay_s
            };
            if speed >= self.config.resume_speed_mps {
                let since = *self.fast_since_ms.get_or_insert(t_ms);
                if (t_ms - since) as f64 / 1000.0 >= required {
                    let was_auto = self.state == WorkoutState::AutoPaused;
                    self.state = WorkoutState::Running;
                    self.fast_since_ms = None;
                    self.slow_since_ms = None;
                    self.last_tick_ms = Some(t_ms);
                    events.push(if was_auto {
                        WorkoutEvent::AutoResumed
                    } else {
                        WorkoutEvent::Started
                    });
                }
            } else {
                self.fast_since_ms = None;
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(t: i64, lat: f64) -> GpsSample {
        GpsSample::new(t, Position::new(lat, 3.0), 5.0)
    }

    #[test]
    fn elapsed_time_only_accumulates_while_running() {
        let mut workout = Workout::default();
        workout.start(0);
        workout.tick(60_000);
        assert!((workout.elapsed_s() - 60.0).abs() < 1e-9);
        workout.pause(60_000);
        workout.tick(180_000);
        assert!((workout.elapsed_s() - 60.0).abs() < 1e-9);
        workout.resume(180_000);
        workout.tick(240_000);
        assert!((workout.elapsed_s() - 120.0).abs() < 1e-9);
    }

    #[test]
    fn distance_is_not_accumulated_while_paused() {
        let mut workout = Workout::default();
        workout.start(0);
        workout.on_sample(sample(0, 45.0));
        workout.on_sample(sample(30_000, 45.001)); // ~111 m
        let running_distance = workout.distance_m();
        assert!(running_distance > 100.0 && running_distance < 120.0);
        workout.pause(30_000);
        workout.on_sample(sample(60_000, 45.01)); // le coureur se deplace en pause
        assert!((workout.distance_m() - running_distance).abs() < 1e-9);
    }

    #[test]
    fn auto_pause_then_auto_resume() {
        let mut workout = Workout::new(WorkoutConfig {
            auto_pause: true,
            ..Default::default()
        });
        workout.start(0);
        assert!(workout.update_motion(1000, Some(3.0)).is_empty());
        // Immobile : pause apres 10 s
        assert!(workout.update_motion(2000, Some(0.1)).is_empty());
        let events = workout.update_motion(13_000, Some(0.1));
        assert_eq!(events, vec![WorkoutEvent::AutoPaused]);
        // Reprise apres 3 s de mouvement
        workout.update_motion(14_000, Some(2.5));
        assert!(workout.update_motion(15_000, Some(2.5)).is_empty());
        let events = workout.update_motion(17_500, Some(2.5));
        assert_eq!(events, vec![WorkoutEvent::AutoResumed]);
        assert_eq!(workout.state(), WorkoutState::Running);
    }

    #[test]
    fn armed_workout_starts_on_first_movement() {
        let mut workout = Workout::default();
        assert_eq!(workout.arm(0), vec![WorkoutEvent::Armed]);
        assert_eq!(workout.state(), WorkoutState::Armed);
        workout.tick(10_000);
        assert!((workout.elapsed_s()).abs() < 1e-9);
        workout.on_sample(sample(10_000, 45.0));
        workout.on_sample(sample(11_000, 45.0001));
        assert_eq!(workout.state(), WorkoutState::Running);
        workout.tick(20_000);
        assert!((workout.elapsed_s() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn stop_requires_an_active_workout() {
        let mut workout = Workout::default();
        assert!(workout.stop(0).is_empty());
        workout.start(0);
        assert_eq!(workout.stop(1000), vec![WorkoutEvent::Stopped]);
        assert_eq!(workout.state(), WorkoutState::Finished);
    }
}
