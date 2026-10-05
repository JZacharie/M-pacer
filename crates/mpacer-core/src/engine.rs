//! Orchestrateur : relie GPS, allure, tours, seance, assistant et voix.
//!
//! `PacerEngine` est le seul objet que le shell Android doit connaitre :
//! il recoit des `GpsSample` et des commandes utilisateur, et renvoie un
//! `EngineOutput` complet (metriques + panneau + annonces vocales) pret a
//! etre affiche.

use crate::assistant::{Assistant, AssistantConfig, AssistantPanel};
use crate::best_distances::{best_efforts, TrackPoint, STANDARD_DISTANCES};
use crate::gps::{GpsMonitor, GpsSample, GpsStatus, GpsThresholds, StatusLight};
use crate::history::WorkoutSummary;
use crate::lap::{Lap, LapTracker};
use crate::pace::{PaceConfig, PaceEngine};
use crate::units::{DistanceDisplay, UnitSystem};
use crate::voice::{VoiceCoach, VoiceConfig, VoiceMessage, VoiceSnapshot};
use crate::workout::{Workout, WorkoutConfig, WorkoutEvent, WorkoutState};
use crate::TimestampMs;
use serde::{Deserialize, Serialize};

/// Reglages complets du moteur (miroir de l'ecran Settings).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EngineConfig {
    pub units: UnitSystem,
    pub distance_display: DistanceDisplay,
    pub pace: PaceConfig,
    pub detect_pace_change: bool,
    pub gps: GpsThresholds,
    pub workout: WorkoutConfig,
    pub voice: VoiceConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            units: UnitSystem::Metric,
            distance_display: DistanceDisplay::default(),
            pace: PaceConfig::default(),
            detect_pace_change: false,
            gps: GpsThresholds::default(),
            workout: WorkoutConfig::default(),
            voice: VoiceConfig::default(),
        }
    }
}

/// Etat complet restitue a l'interface apres chaque mise a jour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineOutput {
    pub status: GpsStatus,
    pub light: StatusLight,
    pub state: WorkoutState,
    pub elapsed_s: f64,
    pub distance_m: f64,
    /// Allure courante (moyennee 2 min) dans l'unite preferee.
    pub current_pace: Option<f64>,
    /// Allure du tour courant (tronçon partiel).
    pub current_lap_pace: Option<f64>,
    /// Allure du tour precedent (km/mile complet).
    pub previous_lap_pace: Option<f64>,
    /// Distance parcourue dans le tour courant.
    pub current_lap_distance_m: f64,
    pub speed_mps: Option<f64>,
    pub panel: AssistantPanel,
    pub lap_completed: Option<Lap>,
    pub events: Vec<WorkoutEvent>,
    pub messages: Vec<VoiceMessage>,
}

impl Default for EngineOutput {
    fn default() -> Self {
        Self {
            status: GpsStatus::Acquiring,
            light: StatusLight::Orange,
            state: WorkoutState::Idle,
            elapsed_s: 0.0,
            distance_m: 0.0,
            current_pace: None,
            current_lap_pace: None,
            previous_lap_pace: None,
            current_lap_distance_m: 0.0,
            speed_mps: None,
            panel: AssistantPanel::default(),
            lap_completed: None,
            events: Vec::new(),
            messages: Vec::new(),
        }
    }
}

/// Moteur de l'application.
#[derive(Debug, Clone)]
pub struct PacerEngine {
    config: EngineConfig,
    gps: GpsMonitor,
    pace: PaceEngine,
    laps: LapTracker,
    workout: Workout,
    assistant: Assistant,
    voice: VoiceCoach,
    track: Vec<TrackPoint>,
    opponent_name: Option<String>,
}

impl Default for PacerEngine {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

impl PacerEngine {
    pub fn new(config: EngineConfig) -> Self {
        let mut pace = PaceEngine::new(config.pace);
        pace.set_detect_pace_change(config.detect_pace_change);
        Self {
            config,
            gps: GpsMonitor::new(config.gps),
            pace,
            laps: LapTracker::new(config.units),
            workout: Workout::new(config.workout),
            assistant: Assistant::default(),
            voice: VoiceCoach::new(config.voice),
            track: Vec::new(),
            opponent_name: None,
        }
    }

    // ---------------------------------------------------------------- reglages

    pub fn config(&self) -> EngineConfig {
        self.config
    }

    /// Applique les reglages "generaux" (unites, GPS, seance, allure).
    pub fn set_config(&mut self, config: EngineConfig) {
        self.config = config;
        self.gps = GpsMonitor::new(config.gps);
        self.laps.set_units(config.units);
        self.workout.set_config(config.workout);
        self.voice.set_config(config.voice);
        let mut pace = PaceEngine::new(config.pace);
        pace.set_detect_pace_change(config.detect_pace_change);
        self.pace = pace;
    }

    pub fn set_units(&mut self, units: UnitSystem) {
        self.config.units = units;
        self.laps.set_units(units);
    }

    pub fn set_distance_display(&mut self, display: DistanceDisplay) {
        self.config.distance_display = display;
    }

    pub fn set_detect_pace_change(&mut self, enabled: bool) {
        self.config.detect_pace_change = enabled;
        self.pace.set_detect_pace_change(enabled);
    }

    pub fn set_assistant_config(&mut self, assistant: AssistantConfig) {
        self.assistant.set_config(assistant);
    }

    pub fn set_voice_config(&mut self, voice: VoiceConfig) {
        self.config.voice = voice;
        self.voice.set_config(voice);
    }

    /// Informe le moteur du pseudo de l'adversaire (course a distance).
    pub fn set_opponent(&mut self, nickname: Option<String>) {
        self.opponent_name = nickname;
    }

    // --------------------------------------------------------------- commandes

    /// Demarre la seance (bouton vert).
    pub fn start(&mut self, t_ms: TimestampMs) -> EngineOutput {
        let events = self.workout.start(t_ms);
        self.laps.start(t_ms);
        self.track.clear();
        self.pace.clear();
        self.voice.reset();
        self.step(t_ms, events, false)
    }

    /// Demarrage suspendu (appui long) : le chrono attend le premier mouvement.
    pub fn arm(&mut self, t_ms: TimestampMs) -> EngineOutput {
        let events = self.workout.arm(t_ms);
        self.laps.start(t_ms);
        self.track.clear();
        self.pace.clear();
        self.voice.reset();
        self.step(t_ms, events, false)
    }

    pub fn pause(&mut self, t_ms: TimestampMs) -> EngineOutput {
        let events = self.workout.pause(t_ms);
        self.step(t_ms, events, false)
    }

    pub fn resume(&mut self, t_ms: TimestampMs) -> EngineOutput {
        let events = self.workout.resume(t_ms);
        self.step(t_ms, events, false)
    }

    pub fn stop(&mut self, t_ms: TimestampMs) -> EngineOutput {
        let events = self.workout.stop(t_ms);
        self.step(t_ms, events, false)
    }

    /// Triple clic sur le bouton du casque : remise a zero de l'allure courante.
    pub fn reset_pace_window(&mut self, t_ms: TimestampMs) {
        self.pace.reset_window(t_ms);
    }

    /// Double clic : annonce immediate (renvoie le message dans la sortie suivante).
    pub fn announce_now(&mut self, t_ms: TimestampMs) -> EngineOutput {
        let opponent = self.opponent_name.clone();
        let snapshot = self.voice_snapshot(None, opponent.as_deref());
        let messages: Vec<VoiceMessage> = self.voice.manual_status(&snapshot).into_iter().collect();
        let panel = self.current_panel();
        self.output(Vec::new(), messages, None, panel, t_ms)
    }

    /// Nouvelle seance : remet tout a zero.
    pub fn reset(&mut self) {
        self.gps = GpsMonitor::new(self.config.gps);
        self.pace.clear();
        self.workout.reset();
        self.voice.reset();
        self.track.clear();
        self.opponent_name = None;
    }

    // ------------------------------------------------------------- alimentation

    /// Injecte une position GPS et recalcule tout l'etat.
    pub fn on_gps(&mut self, sample: GpsSample) -> EngineOutput {
        let (_status, accepted) = self.gps.push_checked(sample);
        if accepted {
            self.workout.on_sample(sample);
            if self.workout.state() == WorkoutState::Running {
                self.track.push(TrackPoint {
                    t_ms: sample.t_ms,
                    dist_m: self.workout.distance_m(),
                    lat: sample.position.lat,
                    lon: sample.position.lon,
                    elevation_m: sample.altitude_m,
                });
            }
        }
        self.step(sample.t_ms, Vec::new(), true)
    }

    /// Rafraichissement periodique sans nouvelle position (affichage, animations).
    ///
    /// Aucune position n'entre dans le moteur d'allure et l'auto-pause n'est pas
    /// reevaluee : seuls de vrais echantillons GPS peuvent faire evoluer
    /// l'allure ou mettre la seance en pause.
    pub fn tick(&mut self, t_ms: TimestampMs) -> EngineOutput {
        self.step(t_ms, Vec::new(), false)
    }

    // -------------------------------------------------------------- etat interne

    fn step(
        &mut self,
        t_ms: TimestampMs,
        mut events: Vec<WorkoutEvent>,
        fresh_gps: bool,
    ) -> EngineOutput {
        self.workout.tick(t_ms);

        if fresh_gps {
            // Auto-pause / reprise automatique sur la vitesse lissee.
            let smoothed_speed = self.pace.short_speed_mps();
            events.extend(self.workout.update_motion(t_ms, smoothed_speed));

            // Alimentation du moteur d'allure : uniquement en course et
            // uniquement sur une position reellement recue.
            if self.workout.state() == WorkoutState::Running {
                self.pace.push(t_ms, self.workout.distance_m());
            }
        }

        // Tours franchis.
        let mut lap_completed = None;
        if self.workout.state().is_active() {
            let laps = self.laps.update(t_ms, self.workout.distance_m());
            if let Some(lap) = laps.last() {
                lap_completed = Some(*lap);
            }
        }

        let panel = self.current_panel();
        // Le pseudo adverse est clone : l'instantane ne doit pas emprunter self,
        // qui est mute juste apres par le coach vocal.
        let opponent = self.opponent_name.clone();
        let snapshot = self.voice_snapshot(lap_completed, opponent.as_deref());

        let mut messages = Vec::new();
        for event in &events {
            if let Some(message) = self.voice.on_event(*event, t_ms, &snapshot) {
                messages.push(message);
            }
        }
        if let Some(lap) = &lap_completed {
            if let Some(message) = self.voice.on_lap(lap, &snapshot) {
                messages.push(message);
            }
        }
        if self.workout.state() == WorkoutState::Running {
            if let Some(message) = self.voice.on_tick(t_ms, &snapshot) {
                messages.push(message);
            }
        }

        self.output(events, messages, lap_completed, panel, t_ms)
    }

    fn current_panel(&self) -> AssistantPanel {
        self.assistant.update(
            self.workout.elapsed_s(),
            self.workout.distance_m(),
            self.pace.current_pace(self.config.units),
            self.config.units,
        )
    }

    fn voice_snapshot<'a>(&self, lap: Option<Lap>, opponent: Option<&'a str>) -> VoiceSnapshot<'a> {
        VoiceSnapshot {
            units: self.config.units,
            distance_m: self.workout.distance_m(),
            elapsed_s: self.workout.elapsed_s(),
            current_pace: self.pace.current_pace(self.config.units),
            lap,
            shadow: self
                .assistant
                .update(
                    self.workout.elapsed_s(),
                    self.workout.distance_m(),
                    self.pace.current_pace(self.config.units),
                    self.config.units,
                )
                .shadow,
            opponent,
            opponent_delta_m: None,
        }
    }

    fn output(
        &self,
        events: Vec<WorkoutEvent>,
        messages: Vec<VoiceMessage>,
        lap_completed: Option<Lap>,
        panel: AssistantPanel,
        _t_ms: TimestampMs,
    ) -> EngineOutput {
        let distance = self.workout.distance_m();
        let current_pace = self.pace.current_pace(self.config.units);
        EngineOutput {
            status: self.gps.status(),
            light: self.gps.status().light(),
            state: self.workout.state(),
            elapsed_s: self.workout.elapsed_s(),
            distance_m: distance,
            current_pace,
            current_lap_pace: self.laps.current_lap_pace(
                self.config.units,
                self.last_seen_ms(),
                distance,
            ),
            previous_lap_pace: self.laps.previous_lap_pace(),
            current_lap_distance_m: self.laps.current_lap_distance_m(distance),
            speed_mps: self.pace.current_speed_mps(),
            panel,
            lap_completed,
            events,
            messages,
        }
    }

    fn last_seen_ms(&self) -> TimestampMs {
        self.track.last().map(|p| p.t_ms).unwrap_or(0)
    }

    // ---------------------------------------------------------------- accesseurs

    pub fn gps_status(&self) -> GpsStatus {
        self.gps.status()
    }

    pub fn workout(&self) -> &Workout {
        &self.workout
    }

    pub fn laps(&self) -> &[Lap] {
        self.laps.laps()
    }

    pub fn assistant(&self) -> &Assistant {
        &self.assistant
    }

    pub fn track(&self) -> &[TrackPoint] {
        &self.track
    }

    /// Construit le resume final de la seance (historique + export).
    pub fn summary(&self, started_at_ms: TimestampMs) -> WorkoutSummary {
        let distance_m = self.workout.distance_m();
        let duration_s = self.workout.elapsed_s();
        let average_pace_s_per_km = if distance_m > 0.0 {
            duration_s / (distance_m / 1000.0)
        } else {
            0.0
        };
        WorkoutSummary {
            id: format!("{started_at_ms}"),
            started_at_ms,
            duration_s,
            distance_m,
            average_pace_s_per_km,
            laps: self.laps.laps().to_vec(),
            best_efforts: best_efforts(&self.track, STANDARD_DISTANCES),
            track: self.track.clone(),
            unit_system: self.config.units,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::AssistantMode;
    use crate::geo::Position;
    use crate::race_plan::NegativeSplit;
    use crate::voice::VoiceFrequency;

    /// Genere un parcours en ligne droite a la vitesse demandee.
    ///
    /// Les champs scalaires proviennent du dernier echantillon, mais les
    /// annonces vocales et le dernier tour franchi sont accumules : c'est ce
    /// que l'application observerait sur toute la duree de la seance.
    fn run(engine: &mut PacerEngine, start_t: i64, seconds: u32, speed_mps: f64) -> EngineOutput {
        let mut last = EngineOutput::default();
        let mut messages = Vec::new();
        let mut last_lap = None;
        let mut lat = 45.0;
        for i in 0..seconds {
            let t = start_t + (i as i64) * 1000;
            lat += speed_mps / 111_195.0; // degres de latitude par seconde
            let output = engine
                .on_gps(GpsSample::new(t, Position::new(lat, 3.0), 4.0).with_speed(speed_mps));
            messages.extend(output.messages.iter().cloned());
            if output.lap_completed.is_some() {
                last_lap = output.lap_completed;
            }
            last = output;
        }
        last.messages = messages;
        last.lap_completed = last_lap.or(last.lap_completed);
        last
    }

    fn ready_engine() -> PacerEngine {
        let mut engine = PacerEngine::default();
        // 3 points precis pour passer au vert
        for i in 0..3 {
            engine.on_gps(GpsSample::new(
                i * 1000,
                Position::new(45.0 + i as f64 * 1e-6, 3.0),
                4.0,
            ));
        }
        engine
    }

    #[test]
    fn gps_status_is_exposed_in_the_output() {
        let mut engine = ready_engine();
        assert_eq!(engine.gps_status(), GpsStatus::Good);
        let output = engine.tick(10_000);
        assert_eq!(output.light, StatusLight::Green);
    }

    #[test]
    fn full_workout_produces_metrics_and_laps() {
        let mut engine = ready_engine();
        engine.start(10_000);
        // ~10 km/h (2.78 m/s) pendant 370 s => ~1025 m (le premier point GPS
        // ne compte pas : il ne fait que fixer l'origine).
        let output = run(&mut engine, 11_000, 370, 2.7778);
        assert_eq!(output.state, WorkoutState::Running);
        assert!(
            output.distance_m > 1000.0 && output.distance_m < 1060.0,
            "distance = {}",
            output.distance_m
        );
        assert!((output.elapsed_s - 370.0).abs() < 1.0);
        let pace = output.current_pace.unwrap();
        assert!((pace - 360.0).abs() < 8.0, "pace = {pace}");
        assert_eq!(engine.laps().len(), 1);
        assert!(output.lap_completed.is_some());
    }

    #[test]
    fn start_emits_a_voice_announcement() {
        let mut engine = ready_engine();
        let output = engine.start(10_000);
        assert!(output
            .messages
            .iter()
            .any(|m| m.text.contains("C'est parti")));
    }

    #[test]
    fn periodic_announcement_fires_during_the_run() {
        let mut engine = ready_engine();
        engine.set_voice_config(VoiceConfig {
            frequency: VoiceFrequency::EveryMinute,
            ..Default::default()
        });
        engine.start(10_000);
        let output = run(&mut engine, 11_000, 90, 3.0);
        assert!(output
            .messages
            .iter()
            .any(|m| m.cue == crate::voice::VoiceCue::Periodic));
    }

    #[test]
    fn planned_race_shows_shadow_runner_gap() {
        let mut engine = ready_engine();
        engine.set_assistant_config(AssistantConfig {
            mode: AssistantMode::AchievePlannedTime,
            race_distance_m: Some(10_000.0),
            planned_time_s: Some(3000.0),
            negative_split: NegativeSplit::even_pace(),
        });
        engine.start(10_000);
        // 5 min a 5:00/km => 1000 m, exactement sur le plan
        let output = run(&mut engine, 11_000, 300, 3.3333);
        let shadow = output.panel.shadow.unwrap();
        assert!(shadow.on_plan, "delta = {} m", shadow.distance_delta_m);
        assert!(output.panel.estimated_finish_s.is_some());
    }

    #[test]
    fn pause_excludes_time_and_auto_pause_works() {
        let mut engine = ready_engine();
        engine.set_config(EngineConfig {
            workout: WorkoutConfig {
                auto_pause: true,
                ..Default::default()
            },
            ..Default::default()
        });
        engine.start(10_000);
        run(&mut engine, 11_000, 30, 3.0);
        // Immobilite : la pause automatique doit se declencher.
        // On repart de la latitude reellement atteinte pour ne pas etre rejete
        // par le filtre anti-saut GPS.
        let mut lat = 45.0 + (30.0 * 3.0) / 111_195.0;
        let mut output = EngineOutput::default();
        let mut saw_auto_pause = false;
        for i in 0..40 {
            let t = 41_000 + i * 1000;
            output = engine.on_gps(GpsSample::new(t, Position::new(lat, 3.0), 4.0).with_speed(0.0));
            lat += 1e-7; // derive GPS negligeable
            if output.events.contains(&WorkoutEvent::AutoPaused) {
                saw_auto_pause = true;
            }
        }
        assert_eq!(output.state, WorkoutState::AutoPaused);
        assert!(saw_auto_pause);
    }

    #[test]
    fn summary_contains_laps_and_best_efforts() {
        let mut engine = ready_engine();
        engine.start(10_000);
        run(&mut engine, 11_000, 400, 2.7778); // ~1.1 km
        let summary = engine.summary(10_000);
        assert_eq!(summary.laps.len(), 1);
        assert_eq!(summary.best_efforts.len(), 1); // 1 km seulement
        assert!((summary.best_efforts[0].time_s - 360.0).abs() < 15.0);
        assert_eq!(summary.track.len(), 400);
    }

    #[test]
    fn unit_switch_changes_pace_reporting() {
        let mut engine = ready_engine();
        engine.start(10_000);
        run(&mut engine, 11_000, 200, 2.7778);
        let metric = engine.tick(300_000).current_pace.unwrap();
        engine.set_units(UnitSystem::Imperial);
        let imperial = engine.tick(301_000).current_pace.unwrap();
        assert!(
            (imperial / metric - 1.609344).abs() < 0.01,
            "metric = {metric}, imperial = {imperial}"
        );
    }
}
