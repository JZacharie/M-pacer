//! Assistant de course : les quatre modes de Pace Control.
//!
//! | Mode | Entrees requises | Sortie |
//! |------|------------------|--------|
//! | `TrackPace` | aucune | panneau masque (allure + tours) |
//! | `PredictFinishTime` | distance | temps de finish estime |
//! | `AchievePlannedTime` | distance + temps cible (+ negative split) | shadow runner, ecart temps/distance |
//! | `RemoteRace` | nom de course + distance + serveur | classement temps reel |

use crate::race_plan::{NegativeSplit, RacePlan, ShadowRunnerComparison};
use crate::units::{format_duration, UnitSystem};
use serde::{Deserialize, Serialize};

/// Mode de l'assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AssistantMode {
    #[default]
    TrackPace,
    PredictFinishTime,
    AchievePlannedTime,
    RemoteRace,
}

impl AssistantMode {
    /// Le panneau d'assistance est-il affiche ?
    pub const fn shows_panel(self) -> bool {
        !matches!(self, AssistantMode::TrackPace)
    }
}

/// Reglages de l'assistant (ecran Settings).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantConfig {
    pub mode: AssistantMode,
    /// Distance de la course / seance (m).
    pub race_distance_m: Option<f64>,
    /// Temps cible (s), mode "achieving planned time".
    pub planned_time_s: Option<f64>,
    pub negative_split: NegativeSplit,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            mode: AssistantMode::TrackPace,
            race_distance_m: None,
            planned_time_s: None,
            negative_split: NegativeSplit::default(),
        }
    }
}

/// Contenu du panneau affiche sous les metriques principales.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AssistantPanel {
    pub mode: AssistantMode,
    pub visible: bool,
    /// Temps de finish estime (s) : modes Predire / Planifie.
    pub estimated_finish_s: Option<f64>,
    /// Comparaison au shadow runner (mode planifie).
    pub shadow: Option<ShadowRunnerComparison>,
    /// Distance restante (m).
    pub remaining_m: Option<f64>,
}

impl Default for AssistantPanel {
    fn default() -> Self {
        Self {
            mode: AssistantMode::TrackPace,
            visible: false,
            estimated_finish_s: None,
            shadow: None,
            remaining_m: None,
        }
    }
}

/// Erreur de configuration a afficher dans l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigIssue {
    /// Distance de course manquante.
    MissingDistance,
    /// Temps cible manquant.
    MissingTargetTime,
}

impl ConfigIssue {
    pub const fn message_key(self) -> &'static str {
        match self {
            ConfigIssue::MissingDistance => "config_missing_distance",
            ConfigIssue::MissingTargetTime => "config_missing_target_time",
        }
    }
}

/// Assistant : conserve la configuration et produit le panneau a chaque tick.
#[derive(Debug, Clone)]
pub struct Assistant {
    config: AssistantConfig,
    plan: Option<RacePlan>,
}

impl Default for Assistant {
    fn default() -> Self {
        Self::new(AssistantConfig::default())
    }
}

impl Assistant {
    pub fn new(config: AssistantConfig) -> Self {
        let plan = Self::build_plan(&config);
        Self { config, plan }
    }

    fn build_plan(config: &AssistantConfig) -> Option<RacePlan> {
        match (config.mode, config.race_distance_m, config.planned_time_s) {
            (AssistantMode::AchievePlannedTime, Some(distance_m), Some(target_s))
                if distance_m > 0.0 && target_s > 0.0 =>
            {
                Some(RacePlan::new(distance_m, target_s).with_negative_split(config.negative_split))
            }
            (AssistantMode::PredictFinishTime, Some(distance_m), _) if distance_m > 0.0 => {
                Some(RacePlan::new(distance_m, 0.0))
            }
            _ => None,
        }
    }

    pub fn config(&self) -> &AssistantConfig {
        &self.config
    }

    /// Applique de nouveaux reglages ; le plan est reconstruit.
    pub fn set_config(&mut self, config: AssistantConfig) {
        self.plan = Self::build_plan(&config);
        self.config = config;
    }

    pub fn mode(&self) -> AssistantMode {
        self.config.mode
    }

    pub fn plan(&self) -> Option<&RacePlan> {
        self.plan.as_ref()
    }

    /// Configuration incomplete pour le mode courant.
    pub fn issue(&self) -> Option<ConfigIssue> {
        match self.config.mode {
            AssistantMode::TrackPace | AssistantMode::RemoteRace => None,
            AssistantMode::PredictFinishTime => {
                if self.config.race_distance_m.is_none() {
                    Some(ConfigIssue::MissingDistance)
                } else {
                    None
                }
            }
            AssistantMode::AchievePlannedTime => {
                if self.config.race_distance_m.is_none() {
                    Some(ConfigIssue::MissingDistance)
                } else if self.config.planned_time_s.is_none() {
                    Some(ConfigIssue::MissingTargetTime)
                } else {
                    None
                }
            }
        }
    }

    /// Distance de course effective (celle du plan ou celle configuree).
    pub fn race_distance_m(&self) -> Option<f64> {
        self.config
            .race_distance_m
            .or_else(|| self.plan.map(|p| p.distance_m))
    }

    /// Recalcule le panneau a partir de l'etat de la seance.
    pub fn update(
        &self,
        elapsed_s: f64,
        distance_m: f64,
        current_pace_s_per_unit: Option<f64>,
        units: UnitSystem,
    ) -> AssistantPanel {
        let mut panel = AssistantPanel {
            mode: self.config.mode,
            visible: self.config.mode.shows_panel() && self.plan.is_some(),
            ..Default::default()
        };
        let Some(plan) = self.plan else {
            return panel;
        };
        panel.remaining_m = Some((plan.distance_m - distance_m).max(0.0));

        match self.config.mode {
            AssistantMode::TrackPace | AssistantMode::RemoteRace => {
                panel.visible = false;
            }
            AssistantMode::PredictFinishTime => {
                // Pas de temps cible : on projette avec l'allure courante.
                let predicted = if plan.target_time_s > 0.0 {
                    plan.predict_finish_time_s(
                        elapsed_s,
                        distance_m,
                        current_pace_s_per_unit,
                        units,
                    )
                } else {
                    match current_pace_s_per_unit {
                        Some(pace) => {
                            let remaining =
                                (plan.distance_m - distance_m).max(0.0) / units.meters_per_unit();
                            Some(elapsed_s + pace * remaining)
                        }
                        None => None,
                    }
                };
                panel.estimated_finish_s = predicted;
            }
            AssistantMode::AchievePlannedTime => {
                panel.shadow = Some(plan.compare(elapsed_s, distance_m));
                panel.estimated_finish_s = plan.predict_finish_time_s(
                    elapsed_s,
                    distance_m,
                    current_pace_s_per_unit,
                    units,
                );
            }
        }
        panel
    }

    /// Resume textuel du panneau (utilise par la voix et les tests).
    pub fn describe(&self, panel: &AssistantPanel, units: UnitSystem) -> Option<String> {
        if !panel.visible {
            return None;
        }
        if let Some(shadow) = panel.shadow {
            let direction = if shadow.on_plan {
                "on plan".to_string()
            } else if shadow.ahead {
                format!("ahead by {}", format_duration(shadow.time_delta_s.abs()))
            } else {
                format!("behind by {}", format_duration(shadow.time_delta_s.abs()))
            };
            return Some(format!(
                "{} / {}",
                direction,
                units.format_distance_short(shadow.distance_delta_m.abs())
            ));
        }
        panel
            .estimated_finish_s
            .map(|t| format!("finish {}", format_duration(t)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planned_config() -> AssistantConfig {
        AssistantConfig {
            mode: AssistantMode::AchievePlannedTime,
            race_distance_m: Some(10_000.0),
            planned_time_s: Some(3000.0),
            negative_split: NegativeSplit::even_pace(),
        }
    }

    #[test]
    fn track_pace_mode_has_no_panel() {
        let assistant = Assistant::default();
        let panel = assistant.update(600.0, 2000.0, Some(300.0), UnitSystem::Metric);
        assert!(!panel.visible);
        assert!(panel.estimated_finish_s.is_none());
    }

    #[test]
    fn predict_mode_reports_finish_time() {
        let assistant = Assistant::new(AssistantConfig {
            mode: AssistantMode::PredictFinishTime,
            race_distance_m: Some(10_000.0),
            ..Default::default()
        });
        let panel = assistant.update(600.0, 2000.0, Some(300.0), UnitSystem::Metric);
        assert!(panel.visible);
        // 8 km restants a 5:00/km => 40 min + 10 min ecoulees = 50 min
        assert!((panel.estimated_finish_s.unwrap() - 3000.0).abs() < 0.001);
    }

    #[test]
    fn planned_mode_tracks_shadow_runner() {
        let assistant = Assistant::new(planned_config());
        let ahead = assistant.update(600.0, 2100.0, Some(285.0), UnitSystem::Metric);
        let shadow = ahead.shadow.unwrap();
        assert!(shadow.ahead);
        assert!((shadow.distance_delta_m - 100.0).abs() < 1.0);
        assert!(ahead.visible);

        let behind = assistant.update(600.0, 1900.0, Some(315.0), UnitSystem::Metric);
        assert!(!behind.shadow.unwrap().ahead);
    }

    #[test]
    fn missing_configuration_is_reported() {
        let assistant = Assistant::new(AssistantConfig {
            mode: AssistantMode::AchievePlannedTime,
            race_distance_m: Some(10_000.0),
            planned_time_s: None,
            negative_split: NegativeSplit::default(),
        });
        assert_eq!(assistant.issue(), Some(ConfigIssue::MissingTargetTime));
        assert!(assistant.plan().is_none());
    }

    #[test]
    fn changing_mode_rebuilds_the_plan() {
        let mut assistant = Assistant::default();
        assert!(assistant.plan().is_none());
        assistant.set_config(planned_config());
        assert!(assistant.plan().is_some());
        let description = assistant.describe(
            &assistant.update(300.0, 1000.0, Some(300.0), UnitSystem::Metric),
            UnitSystem::Metric,
        );
        assert!(description.is_some());
    }
}
