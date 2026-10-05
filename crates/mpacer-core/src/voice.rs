//! Retour vocal : planification des annonces et rendu multilingue.
//!
//! Le coeur produit des `VoiceCue` structures ; le rendu textuel est fait ici
//! (FR/EN) puis transmis au moteur TTS Android. Cela permet de tester les
//! messages sans dependre du systeme, et d'ajouter des langues sans toucher a
//! la logique de course.

use crate::lap::Lap;
use crate::race_plan::ShadowRunnerComparison;
use crate::units::{format_duration, format_pace, UnitSystem};
use crate::workout::WorkoutEvent;
use serde::{Deserialize, Serialize};

/// Langue du retour vocal (moteur TTS Google).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    Fr,
    En,
}

impl Language {
    pub const fn code(self) -> &'static str {
        match self {
            Language::Fr => "fr-FR",
            Language::En => "en-US",
        }
    }
}

/// Frequence des annonces periodiques.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum VoiceFrequency {
    Off,
    EveryMinute,
    #[default]
    Every2Minutes,
    Every5Minutes,
    EveryLap,
    /// Uniquement sur demande (double clic sur le bouton du casque).
    ManualOnly,
}

impl VoiceFrequency {
    /// Periode d'annonce en millisecondes (`None` = pas d'annonce periodique).
    pub const fn period_ms(self) -> Option<i64> {
        match self {
            VoiceFrequency::Off | VoiceFrequency::EveryLap | VoiceFrequency::ManualOnly => None,
            VoiceFrequency::EveryMinute => Some(60_000),
            VoiceFrequency::Every2Minutes => Some(120_000),
            VoiceFrequency::Every5Minutes => Some(300_000),
        }
    }
}

/// Comportement vis-a-vis de la musique jouee par une autre application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MusicPolicy {
    #[default]
    Duck,
    Pause,
    IgnoreAndSpeak,
}

/// Reglages du retour vocal.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VoiceConfig {
    pub enabled: bool,
    pub frequency: VoiceFrequency,
    pub language: Language,
    /// Message enrichi (distance, temps total, temps du tour) en fin de tour.
    pub extended_lap_info: bool,
    /// Formulations courtes et informelles.
    pub short_forms: bool,
    pub music_policy: MusicPolicy,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            frequency: VoiceFrequency::Every2Minutes,
            language: Language::Fr,
            extended_lap_info: false,
            short_forms: false,
            music_policy: MusicPolicy::Duck,
        }
    }
}

/// Nature de l'annonce.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum VoiceCue {
    Armed,
    Started,
    Paused,
    Resumed,
    AutoPaused,
    AutoResumed,
    Stopped,
    /// Annonce periodique (allure, distance, temps).
    Periodic,
    /// Fin de tour (km ou mile).
    LapCompleted,
    /// Annonce a la demande (double clic casque).
    ManualStatus,
    /// Decompte avant le depart (course a distance).
    Countdown(u8),
}

/// Donnees disponibles au moment de l'annonce.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VoiceSnapshot<'a> {
    pub units: UnitSystem,
    pub distance_m: f64,
    pub elapsed_s: f64,
    /// Allure courante en s/unite.
    pub current_pace: Option<f64>,
    /// Tour qui vient d'etre termine.
    pub lap: Option<Lap>,
    /// Ecart au shadow runner (mode "achieving planned time").
    pub shadow: Option<ShadowRunnerComparison>,
    /// Course a distance : pseudo de l'adversaire et ecart de distance.
    pub opponent: Option<&'a str>,
    pub opponent_delta_m: Option<f64>,
}

/// Annonce prete a etre envoyee au TTS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceMessage {
    pub cue: VoiceCue,
    pub text: String,
}

/// Rend une annonce en texte, dans la langue configuree.
pub fn render(cue: VoiceCue, snapshot: &VoiceSnapshot, config: &VoiceConfig) -> String {
    let units = snapshot.units;
    let unit_name = if config.short_forms {
        if units == UnitSystem::Metric {
            "klick"
        } else {
            "mile"
        }
    } else {
        units.long_label()
    };
    let pace = format_pace(snapshot.current_pace);
    let distance = units.format_distance(snapshot.distance_m);
    let time = format_duration(snapshot.elapsed_s);

    let position = |shadow: &ShadowRunnerComparison, fr: bool| -> String {
        let delta = format_duration(shadow.time_delta_s.abs());
        let gap = units.format_distance_short(shadow.distance_delta_m.abs());
        if shadow.on_plan {
            if fr {
                "Vous etes sur le plan.".into()
            } else {
                "You are on plan.".into()
            }
        } else if shadow.ahead {
            if fr {
                format!("Vous etes en avance de {delta}, soit {gap}.")
            } else {
                format!("You are ahead by {delta}, that is {gap}.")
            }
        } else if fr {
            format!("Vous etes en retard de {delta}, soit {gap}.")
        } else {
            format!("You are behind by {delta}, that is {gap}.")
        }
    };

    match config.language {
        Language::Fr => match cue {
            VoiceCue::Armed => "Seance prete. Le chrono demarrera au premier mouvement.".into(),
            VoiceCue::Started => "C'est parti.".into(),
            VoiceCue::Paused => "Seance en pause.".into(),
            VoiceCue::Resumed => "Reprise.".into(),
            VoiceCue::AutoPaused => "Pause automatique.".into(),
            VoiceCue::AutoResumed => "Reprise automatique.".into(),
            VoiceCue::Stopped => format!("Seance terminee. {distance} en {time}."),
            VoiceCue::Countdown(seconds) => format!("Depart dans {seconds}."),
            VoiceCue::Periodic | VoiceCue::ManualStatus | VoiceCue::LapCompleted => {
                let mut message = if config.short_forms {
                    format!("{pace} par {unit_name}. {distance}. {time}.")
                } else {
                    format!("Allure {pace} par {unit_name}. Distance {distance}. Temps {time}.")
                };
                if let Some(lap) = snapshot.lap {
                    message.push_str(&format!(
                        " Dernier tour en {}.",
                        format_duration(lap.duration_s)
                    ));
                    if config.extended_lap_info {
                        message.push_str(&format!(
                            " Allure du tour {}.",
                            format_pace(Some(lap.pace_in(units)))
                        ));
                    }
                }
                if let Some(shadow) = &snapshot.shadow {
                    message.push(' ');
                    message.push_str(&position(shadow, true));
                }
                if let (Some(opponent), Some(delta)) =
                    (snapshot.opponent, snapshot.opponent_delta_m)
                {
                    let gap = units.format_distance_short(delta.abs());
                    message.push(' ');
                    let relative = if delta >= 0.0 {
                        format!("Vous menez de {gap} sur {opponent}.")
                    } else {
                        format!("{opponent} est devant de {gap}.")
                    };
                    message.push_str(&relative);
                }
                message
            }
        },
        Language::En => match cue {
            VoiceCue::Armed => "Workout ready. The timer will start on your first move.".into(),
            VoiceCue::Started => "Here we go.".into(),
            VoiceCue::Paused => "Workout paused.".into(),
            VoiceCue::Resumed => "Resumed.".into(),
            VoiceCue::AutoPaused => "Auto pause.".into(),
            VoiceCue::AutoResumed => "Auto resume.".into(),
            VoiceCue::Stopped => format!("Workout finished. {distance} in {time}."),
            VoiceCue::Countdown(seconds) => format!("Starting in {seconds}."),
            VoiceCue::Periodic | VoiceCue::ManualStatus | VoiceCue::LapCompleted => {
                let mut message = if config.short_forms {
                    format!("{pace} per {unit_name}. {distance}. {time}.")
                } else {
                    format!("Pace {pace} per {unit_name}. Distance {distance}. Time {time}.")
                };
                if let Some(lap) = snapshot.lap {
                    message.push_str(&format!(" Last lap {}.", format_duration(lap.duration_s)));
                    if config.extended_lap_info {
                        message.push_str(&format!(
                            " Lap pace {}.",
                            format_pace(Some(lap.pace_in(units)))
                        ));
                    }
                }
                if let Some(shadow) = &snapshot.shadow {
                    message.push(' ');
                    message.push_str(&position(shadow, false));
                }
                message
            }
        },
    }
}

/// Planificateur d'annonces : decide *quand* parler, pas *quoi* dire.
#[derive(Debug, Clone)]
pub struct VoiceCoach {
    config: VoiceConfig,
    last_periodic_ms: Option<i64>,
    announced_laps: u32,
}

impl Default for VoiceCoach {
    fn default() -> Self {
        Self::new(VoiceConfig::default())
    }
}

impl VoiceCoach {
    pub fn new(config: VoiceConfig) -> Self {
        Self {
            config,
            last_periodic_ms: None,
            announced_laps: 0,
        }
    }

    pub fn config(&self) -> VoiceConfig {
        self.config
    }

    pub fn set_config(&mut self, config: VoiceConfig) {
        self.config = config;
    }

    pub fn reset(&mut self) {
        self.last_periodic_ms = None;
        self.announced_laps = 0;
    }

    fn enabled(&self) -> bool {
        self.config.enabled && self.config.frequency != VoiceFrequency::Off
    }

    /// Annonce liee a un evenement de seance.
    pub fn on_event(
        &mut self,
        event: WorkoutEvent,
        t_ms: i64,
        snapshot: &VoiceSnapshot,
    ) -> Option<VoiceMessage> {
        let cue = match event {
            WorkoutEvent::Armed => VoiceCue::Armed,
            WorkoutEvent::Started => VoiceCue::Started,
            WorkoutEvent::Paused => VoiceCue::Paused,
            WorkoutEvent::Resumed => VoiceCue::Resumed,
            WorkoutEvent::AutoPaused => VoiceCue::AutoPaused,
            WorkoutEvent::AutoResumed => VoiceCue::AutoResumed,
            WorkoutEvent::Stopped => VoiceCue::Stopped,
        };
        if matches!(cue, VoiceCue::Started) {
            self.last_periodic_ms = Some(t_ms);
        }
        if !self.config.enabled {
            return None;
        }
        Some(VoiceMessage {
            cue,
            text: render(cue, snapshot, &self.config),
        })
    }

    /// Annonce periodique (a chaque tick du moteur).
    pub fn on_tick(&mut self, t_ms: i64, snapshot: &VoiceSnapshot) -> Option<VoiceMessage> {
        if !self.enabled() {
            return None;
        }
        let period = self.config.frequency.period_ms()?;
        let elapsed_since = self
            .last_periodic_ms
            .map(|last| t_ms - last)
            .unwrap_or(i64::MAX);
        if elapsed_since >= period && snapshot.elapsed_s > 0.0 {
            self.last_periodic_ms = Some(t_ms);
            return Some(VoiceMessage {
                cue: VoiceCue::Periodic,
                text: render(VoiceCue::Periodic, snapshot, &self.config),
            });
        }
        None
    }

    /// Annonce de fin de tour (frequence "chaque tour" ou info etendue active).
    pub fn on_lap(&mut self, lap: &Lap, snapshot: &VoiceSnapshot) -> Option<VoiceMessage> {
        if !self.config.enabled {
            return None;
        }
        let wants =
            self.config.frequency == VoiceFrequency::EveryLap || self.config.extended_lap_info;
        if !wants || lap.index <= self.announced_laps {
            return None;
        }
        self.announced_laps = lap.index;
        let mut snapshot = *snapshot;
        snapshot.lap = Some(*lap);
        Some(VoiceMessage {
            cue: VoiceCue::LapCompleted,
            text: render(VoiceCue::LapCompleted, &snapshot, &self.config),
        })
    }

    /// Annonce a la demande (double clic sur le bouton du casque).
    pub fn manual_status(&self, snapshot: &VoiceSnapshot) -> Option<VoiceMessage> {
        if !self.config.enabled {
            return None;
        }
        Some(VoiceMessage {
            cue: VoiceCue::ManualStatus,
            text: render(VoiceCue::ManualStatus, snapshot, &self.config),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> VoiceSnapshot<'static> {
        VoiceSnapshot {
            units: UnitSystem::Metric,
            distance_m: 3200.0,
            elapsed_s: 1080.0,
            current_pace: Some(341.0),
            ..Default::default()
        }
    }

    #[test]
    fn periodic_message_contains_pace_distance_and_time() {
        let config = VoiceConfig::default();
        let text = render(VoiceCue::Periodic, &snapshot(), &config);
        assert!(text.contains("5:41"), "{text}");
        assert!(text.contains("3.20 km"), "{text}");
        assert!(text.contains("18:00"), "{text}");
    }

    #[test]
    fn english_rendering_works() {
        let config = VoiceConfig {
            language: Language::En,
            ..Default::default()
        };
        let text = render(VoiceCue::Periodic, &snapshot(), &config);
        assert!(text.starts_with("Pace 5:41 per kilometers"), "{text}");
    }

    #[test]
    fn short_forms_are_used_when_enabled() {
        let config = VoiceConfig {
            short_forms: true,
            ..Default::default()
        };
        let text = render(VoiceCue::Periodic, &snapshot(), &config);
        assert!(text.contains("klick"), "{text}");
    }

    #[test]
    fn shadow_runner_delta_is_spoken() {
        let mut snapshot = snapshot();
        snapshot.shadow = Some(ShadowRunnerComparison {
            planned_distance_m: 3100.0,
            distance_delta_m: 100.0,
            time_delta_s: 30.0,
            ahead: true,
            on_plan: false,
        });
        let text = render(VoiceCue::Periodic, &snapshot, &VoiceConfig::default());
        assert!(text.contains("en avance de 0:30"), "{text}");
        assert!(text.contains("100 m"), "{text}");
    }

    #[test]
    fn periodic_interval_is_respected() {
        let mut coach = VoiceCoach::new(VoiceConfig {
            frequency: VoiceFrequency::EveryMinute,
            ..Default::default()
        });
        let snapshot = snapshot();
        // Le demarrage initialise l'horloge des annonces (sinon la premiere
        // annonce partirait immediatement).
        assert!(coach
            .on_event(WorkoutEvent::Started, 0, &snapshot)
            .is_some());
        assert!(coach.on_tick(0, &snapshot).is_none());
        assert!(coach.on_tick(30_000, &snapshot).is_none());
        assert!(coach.on_tick(60_000, &snapshot).is_some());
        assert!(coach.on_tick(90_000, &snapshot).is_none());
        assert!(coach.on_tick(120_000, &snapshot).is_some());
    }

    #[test]
    fn off_frequency_is_silent() {
        let mut coach = VoiceCoach::new(VoiceConfig {
            frequency: VoiceFrequency::Off,
            ..Default::default()
        });
        assert!(coach.on_tick(600_000, &snapshot()).is_none());
    }

    #[test]
    fn lap_announcement_happens_once_per_lap() {
        let mut coach = VoiceCoach::new(VoiceConfig {
            frequency: VoiceFrequency::EveryLap,
            ..Default::default()
        });
        let lap = Lap {
            index: 1,
            distance_m: 1000.0,
            duration_s: 300.0,
            pace_s_per_km: 300.0,
        };
        assert!(coach.on_lap(&lap, &snapshot()).is_some());
        assert!(coach.on_lap(&lap, &snapshot()).is_none());
    }

    #[test]
    fn manual_status_is_always_available_when_enabled() {
        let coach = VoiceCoach::default();
        assert!(coach.manual_status(&snapshot()).is_some());
        let coach = VoiceCoach::new(VoiceConfig {
            enabled: false,
            ..Default::default()
        });
        assert!(coach.manual_status(&snapshot()).is_none());
    }
}
