//! Orchestrateur : relie GPS, allure, tours, seance, assistant et voix.
//!
//! `PacerEngine` est le seul objet que le shell Android doit connaitre :
//! il recoit des `GpsSample` et des commandes utilisateur, et renvoie un
//! `EngineOutput` complet (metriques + panneau + annonces vocales) pret a
//! etre affiche.

use crate::analysis::Pause;
use crate::assistant::{Assistant, AssistantConfig, AssistantPanel};
use crate::best_distances::{best_efforts, TrackPoint, STANDARD_DISTANCES};
use crate::cardio::{HeartRateSample, HeartRateZones};
use crate::gps::{GpsMonitor, GpsSample, GpsStatus, GpsThresholds, StatusLight};
use crate::history::WorkoutSummary;
use crate::lap::{Lap, LapTracker};
use crate::music::{MusicConfig, MusicDirector, MusicInput, MusicState, NowPlaying, Playlist};
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
    /// Reglages musique (BPM de reference, playlist, annonces de tempo).
    #[serde(default)]
    pub music: MusicConfig,
    /// Reference des zones de frequence cardiaque (FC max, FC de repos).
    pub heart_rate: HeartRateZones,
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
            music: MusicConfig::default(),
            heart_rate: HeartRateZones::default(),
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
    /// Derniere frequence cardiaque recue (bpm).
    pub heart_rate_bpm: Option<u16>,
    /// Zone de la derniere frequence (1 a 5), absente sous la zone 1.
    pub heart_rate_zone: Option<u8>,
    /// Etat musique du tick (toujours present, jamais null).
    #[serde(default)]
    pub music: MusicState,
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
            heart_rate_bpm: None,
            heart_rate_zone: None,
            music: MusicState::default(),
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
    /// Mesures de frequence cardiaque, horodatees.
    heart_rate: Vec<HeartRateSample>,
    /// Pauses terminees.
    pauses: Vec<Pause>,
    /// Pause en cours : (temps de course, distance, automatique, instant).
    open_pause: Option<(f64, f64, bool, TimestampMs)>,
    /// Instant du depart de la seance.
    started_t_ms: Option<TimestampMs>,
    /// Dernier instant vu par le moteur (borne du temps ecoule).
    last_t_ms: TimestampMs,
    /// Directeur d'orchestre musique.
    music: MusicDirector,
    /// Dernier etat musique publie (sorties sans nouveau tick comprises).
    last_music: MusicState,
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
            heart_rate: Vec::new(),
            pauses: Vec::new(),
            open_pause: None,
            started_t_ms: None,
            last_t_ms: 0,
            music: MusicDirector::new(config.music),
            last_music: MusicState::default(),
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
        self.music.set_config(config.music);
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

    /// Reglages musique (BPM de reference, seuils, annonces).
    pub fn set_music_config(&mut self, music: MusicConfig) {
        self.config.music = music;
        self.music.set_config(music);
    }

    /// Playlist preparee sur la montre (`None` = aucune).
    pub fn set_music_playlist(&mut self, playlist: Option<Playlist>) {
        self.music.set_playlist(playlist);
    }

    /// Instantane de la piste en cours, pousse par le lecteur de la montre.
    pub fn on_now_playing(&mut self, now: Option<NowPlaying>) {
        self.music.on_now_playing(now);
    }

    /// Cadence mesuree par un capteur de pas (pas/minute).
    pub fn on_cadence(&mut self, spm: f64) {
        self.music.on_cadence(spm);
    }

    /// Informe le moteur du pseudo de l'adversaire (course a distance).
    pub fn set_opponent(&mut self, nickname: Option<String>) {
        self.opponent_name = nickname;
    }

    // --------------------------------------------------------------- commandes

    /// Demarre la seance (bouton vert).
    pub fn start(&mut self, t_ms: TimestampMs) -> EngineOutput {
        let events = self.workout.start(t_ms);
        self.begin_recording(t_ms);
        self.step(t_ms, events, false)
    }

    /// Demarrage suspendu (appui long) : le chrono attend le premier mouvement.
    pub fn arm(&mut self, t_ms: TimestampMs) -> EngineOutput {
        let events = self.workout.arm(t_ms);
        self.begin_recording(t_ms);
        self.step(t_ms, events, false)
    }

    /// Remise a zero des enregistrements annexes (trace, cardio, pauses).
    fn begin_recording(&mut self, t_ms: TimestampMs) {
        // Le suivi de tours raisonne en temps de course : le depart vaut zero.
        self.laps.start(0.0);
        self.track.clear();
        self.pace.clear();
        self.voice.reset();
        self.heart_rate.clear();
        self.pauses.clear();
        self.open_pause = None;
        self.started_t_ms = Some(t_ms);
        self.last_t_ms = t_ms;
    }

    /// Enregistre une mesure de frequence cardiaque (montre ou ceinture).
    ///
    /// Aucun recalcul n'est declenche ici : la mesure remonte a l'affichage au
    /// prochain pas (GPS ou tick). Sans cela, le cardio et le GPS arrivant a la
    /// meme seconde feraient avancer deux fois le coach vocal.
    ///
    /// Les mesures sont conservees meme pendant une pause : la frequence de
    /// recupération fait partie de la seance, meme si elle ne compte pas dans
    /// l'allure.
    pub fn on_heart_rate(&mut self, t_ms: TimestampMs, bpm: u16) {
        if bpm > 0 {
            self.heart_rate.push(HeartRateSample { t_ms, bpm });
        }
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
        self.heart_rate.clear();
        self.pauses.clear();
        self.open_pause = None;
        self.started_t_ms = None;
        self.last_t_ms = 0;
        self.music.reset();
        self.last_music = MusicState::default();
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
        self.last_t_ms = t_ms;
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

        self.record_pauses(t_ms, &events);

        // Tours franchis.
        let mut lap_completed = None;
        if self.workout.state().is_active() {
            let laps = self
                .laps
                .update(self.workout.elapsed_s(), self.workout.distance_m());
            if let Some(lap) = laps.last() {
                lap_completed = Some(*lap);
            }
        }

        let panel = self.current_panel();
        // Le directeur d'orchestre est evalue avant la voix : l'annonce de
        // tempo dispose ainsi de la consigne du tick.
        let music = self.music.evaluate(self.music_input(t_ms, &panel));
        self.last_music = music.clone();

        // Le pseudo adverse est clone : l'instantane ne doit pas emprunter self,
        // qui est mute juste apres par le coach vocal.
        let opponent = self.opponent_name.clone();
        let mut snapshot = self.voice_snapshot(lap_completed, opponent.as_deref());
        snapshot.music_bpm = music.target_bpm;

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
        if let Some(message) = self
            .voice
            .on_music(&music, self.config.music.announce, &snapshot)
        {
            messages.push(message);
        }

        self.output(events, messages, lap_completed, panel, t_ms)
    }

    /// Ouvre ou ferme une pause a partir des evenements de la seance.
    ///
    /// Le temps de course est fige au declenchement : c'est lui qui situe la
    /// pause dans la seance (le temps ecoule, lui, continue d'avancer).
    fn record_pauses(&mut self, t_ms: TimestampMs, events: &[WorkoutEvent]) {
        for event in events {
            match event {
                WorkoutEvent::Paused | WorkoutEvent::AutoPaused => {
                    if self.open_pause.is_none() {
                        self.open_pause = Some((
                            self.workout.elapsed_s(),
                            self.workout.distance_m(),
                            *event == WorkoutEvent::AutoPaused,
                            t_ms,
                        ));
                    }
                }
                WorkoutEvent::Resumed | WorkoutEvent::AutoResumed | WorkoutEvent::Stopped => {
                    self.close_pause(t_ms);
                }
                _ => {}
            }
        }
    }

    fn close_pause(&mut self, t_ms: TimestampMs) {
        if let Some((at_s, at_distance_m, automatic, started)) = self.open_pause.take() {
            let duration_s = ((t_ms - started) as f64 / 1000.0).max(0.0);
            if duration_s > 0.0 {
                self.pauses.push(Pause {
                    at_s,
                    at_distance_m,
                    duration_s,
                    automatic,
                });
            }
        }
    }

    fn current_panel(&self) -> AssistantPanel {
        self.assistant.update(
            self.workout.elapsed_s(),
            self.workout.distance_m(),
            self.pace.current_pace(self.config.units),
            self.config.units,
        )
    }

    /// Contexte musique du tick : plan, allure, cardio et vitesse.
    fn music_input(&self, t_ms: TimestampMs, panel: &AssistantPanel) -> MusicInput {
        MusicInput {
            t_ms,
            state: self.workout.state(),
            target_pace_s_per_km: self
                .assistant
                .plan()
                .filter(|plan| plan.is_valid())
                .map(|plan| plan.pace_at_distance_s_per_km(self.workout.distance_m())),
            current_pace_s_per_km: self.pace.current_pace(UnitSystem::Metric),
            shadow_delta_m: panel.shadow.map(|shadow| shadow.distance_delta_m),
            shadow_on_plan: panel.shadow.map(|shadow| shadow.on_plan).unwrap_or(false),
            heart_rate_zone: self
                .heart_rate
                .last()
                .map(|sample| self.config.heart_rate.zone_of(sample.bpm as f64))
                .filter(|zone| *zone > 0),
            speed_mps: self.pace.current_speed_mps(),
        }
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
            // Rempli par le tick depuis la consigne musique ; absent sur une
            // annonce a la demande.
            music_bpm: None,
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
                self.workout.elapsed_s(),
                distance,
            ),
            previous_lap_pace: self.laps.previous_lap_pace(),
            current_lap_distance_m: self.laps.current_lap_distance_m(distance),
            speed_mps: self.pace.current_speed_mps(),
            panel,
            lap_completed,
            events,
            messages,
            heart_rate_bpm: self.heart_rate.last().map(|sample| sample.bpm),
            heart_rate_zone: self
                .heart_rate
                .last()
                .map(|sample| self.config.heart_rate.zone_of(sample.bpm as f64))
                .filter(|zone| *zone > 0),
            music: self.last_music.clone(),
        }
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

    /// Mesures de frequence cardiaque enregistrees.
    pub fn heart_rate(&self) -> &[HeartRateSample] {
        &self.heart_rate
    }

    /// Pauses terminees.
    pub fn pauses(&self) -> &[Pause] {
        &self.pauses
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
        // Temps ecoule : du depart au dernier instant vu par le moteur, pauses
        // comprises. Une montre arretee par le systeme retombe sur la duree de
        // course plutot que d'inventer une valeur.
        let elapsed_s = match self.started_t_ms {
            Some(_) if self.last_t_ms > started_at_ms => {
                (self.last_t_ms - started_at_ms) as f64 / 1000.0
            }
            _ => duration_s,
        };
        // Une pause encore ouverte (seance arretee sans reprendre) compte aussi.
        let mut pauses = self.pauses.clone();
        if let Some((at_s, at_distance_m, automatic, started)) = self.open_pause {
            let duration_s = ((self.last_t_ms - started) as f64 / 1000.0).max(0.0);
            if duration_s > 0.0 {
                pauses.push(Pause {
                    at_s,
                    at_distance_m,
                    duration_s,
                    automatic,
                });
            }
        }
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
            elapsed_s,
            pauses,
            heart_rate: self.heart_rate.clone(),
            plan: self.assistant.plan().copied(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::AssistantMode;
    use crate::geo::Position;
    use crate::music::{DirectiveReason, MusicDirective, Playlist, Track};
    use crate::race_plan::NegativeSplit;
    use crate::voice::{VoiceCue, VoiceFrequency};

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
    fn a_manual_pause_is_recorded_with_its_duration() {
        let mut engine = ready_engine();
        engine.start(10_000);
        run(&mut engine, 11_000, 30, 3.0); // 30 s de course
        engine.pause(41_000);
        engine.resume(71_000); // 30 s de pause

        let summary = engine.summary(10_000);
        assert_eq!(summary.pauses.len(), 1);
        let pause = summary.pauses[0];
        assert!((pause.duration_s - 30.0).abs() < 0.01, "pause = {pause:?}");
        assert!(!pause.automatic);
        assert!((pause.at_s - 31.0).abs() < 0.5, "at_s = {}", pause.at_s);
        // Le temps de course exclut la pause, le temps ecoule non.
        assert!((summary.duration_s - 31.0).abs() < 0.5);
        assert!((summary.total_elapsed_s() - 61.0).abs() < 0.5);
        assert!((summary.paused_s() - 30.0).abs() < 0.01);
    }

    #[test]
    fn an_automatic_pause_is_flagged_and_recorded() {
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
        let mut lat = 45.0 + (30.0 * 3.0) / 111_195.0;
        for i in 0..40 {
            let t = 41_000 + i * 1000;
            engine.on_gps(GpsSample::new(t, Position::new(lat, 3.0), 4.0).with_speed(0.0));
            lat += 1e-7;
        }
        let summary = engine.summary(10_000);
        assert_eq!(summary.pauses.len(), 1);
        assert!(summary.pauses[0].automatic);
        assert!(summary.pauses[0].duration_s > 5.0);
    }

    #[test]
    fn heart_rate_reaches_the_output_and_the_summary() {
        let mut engine = ready_engine();
        engine.start(10_000);
        run(&mut engine, 11_000, 20, 3.0);
        for second in 1..=20_u16 {
            engine.on_heart_rate(11_000 + second as i64 * 1000, 140 + second);
        }
        let output = engine.tick(50_000);
        assert_eq!(output.heart_rate_bpm, Some(160));
        assert_eq!(output.heart_rate_zone, Some(4)); // 160 / 190 = 84 %

        let summary = engine.summary(10_000);
        assert_eq!(summary.heart_rate.len(), 20);
        assert_eq!(summary.heart_rate[0].bpm, 141);
        assert!(summary.has_heart_rate());
    }

    #[test]
    fn the_plan_used_by_the_assistant_is_archived_with_the_summary() {
        let mut engine = ready_engine();
        engine.set_assistant_config(AssistantConfig {
            mode: AssistantMode::AchievePlannedTime,
            race_distance_m: Some(10_000.0),
            planned_time_s: Some(3000.0),
            negative_split: NegativeSplit::with_ratio(0.03),
        });
        engine.start(10_000);
        run(&mut engine, 11_000, 60, 3.3333);

        let plan = engine.summary(10_000).plan.expect("plan archive");
        assert!((plan.target_time_s - 3000.0).abs() < 1e-9);
        assert!((plan.distance_m - 10_000.0).abs() < 1e-9);
        assert!(plan.negative_split.enabled);
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

    // ---------------------------------------------------------------- musique

    fn music_config() -> MusicConfig {
        MusicConfig {
            enabled: true,
            ..Default::default()
        }
    }

    fn music_track(id: &str, bpm: f64, position: u32) -> Track {
        Track {
            id: id.to_string(),
            title: format!("Titre {id}"),
            artist: Some("Artiste".to_string()),
            duration_s: 215.0,
            bpm: Some(bpm),
            position,
        }
    }

    fn music_playlist() -> Playlist {
        Playlist {
            id: "p1".to_string(),
            name: "Run 170".to_string(),
            target_bpm: Some(170.0),
            tracks: vec![music_track("t1", 170.0, 0), music_track("t2", 180.0, 1)],
        }
    }

    fn music_engine() -> PacerEngine {
        let mut engine = ready_engine();
        engine.set_music_config(music_config());
        engine.set_music_playlist(Some(music_playlist()));
        engine
    }

    #[test]
    fn music_state_is_published_in_every_output() {
        let mut engine = PacerEngine::default();
        let output = engine.tick(0);
        // Musique active par defaut, mais aucune playlist : rien a jouer.
        assert!(output.music.enabled);
        assert_eq!(output.music.directive, MusicDirective::None);
        assert_eq!(output.music.reason, DirectiveReason::NoPlaylist);
        assert_eq!(output.music.playlist_id, None);
    }

    #[test]
    fn a_playlist_drives_the_output_and_announces_the_tempo_once() {
        let mut engine = music_engine();
        let started = engine.start(10_000);
        assert_eq!(started.music.directive, MusicDirective::Play);
        assert_eq!(started.music.next_track_id.as_deref(), Some("t1"));
        assert_eq!(started.music.target_bpm, Some(170.0));
        assert_eq!(started.music.playlist_name.as_deref(), Some("Run 170"));
        assert!(started
            .messages
            .iter()
            .any(|message| message.cue == VoiceCue::MusicTempo));

        let output = run(&mut engine, 11_000, 30, 1000.0 / 300.0);
        // Le tempo n'est annonce qu'au demarrage de la seance.
        assert!(!output
            .messages
            .iter()
            .any(|message| message.cue == VoiceCue::MusicTempo));
        assert!(output.music.cadence_spm.unwrap() > 160.0);
    }

    #[test]
    fn a_sensor_cadence_reaches_the_music_state() {
        let mut engine = music_engine();
        engine.start(10_000);
        engine.on_cadence(174.0);
        let output = engine.tick(11_000);
        assert_eq!(output.music.cadence_spm, Some(174.0));
    }

    #[test]
    fn cadence_is_estimated_without_a_sensor() {
        let mut engine = music_engine();
        engine.start(10_000);
        let output = run(&mut engine, 11_000, 20, 1000.0 / 300.0);
        let cadence = output.music.cadence_spm.unwrap();
        assert!((cadence - 173.9).abs() < 1.0, "cadence = {cadence}");
    }

    #[test]
    fn music_pause_and_resume_follow_the_workout() {
        let mut engine = music_engine();
        engine.start(10_000);
        run(&mut engine, 11_000, 10, 3.0);

        let paused = engine.pause(25_000);
        assert_eq!(paused.music.directive, MusicDirective::Pause);
        assert_eq!(paused.music.reason, DirectiveReason::Paused);
        let still_paused = engine.tick(26_000);
        assert_eq!(still_paused.music.directive, MusicDirective::None);
        assert_eq!(still_paused.music.reason, DirectiveReason::Paused);

        let resumed = engine.resume(27_000);
        assert_eq!(resumed.music.directive, MusicDirective::Resume);
        assert_eq!(resumed.music.reason, DirectiveReason::Resumed);
    }

    #[test]
    fn being_behind_the_plan_boosts_the_music_target() {
        let mut engine = music_engine();
        engine.set_assistant_config(AssistantConfig {
            mode: AssistantMode::AchievePlannedTime,
            race_distance_m: Some(10_000.0),
            planned_time_s: Some(3000.0),
            negative_split: NegativeSplit::even_pace(),
        });
        engine.start(10_000);
        // 6:00/km au lieu de 5:00/km : une minute de retard sur le plan.
        let output = run(&mut engine, 11_000, 300, 1000.0 / 360.0);
        assert_eq!(output.music.reason, DirectiveReason::BehindPlan);
        assert_eq!(output.music.target_bpm, Some(176.0));
    }
}
