//! # mpacer-core
//!
//! Coeur metier de **M-pacer**, application de controle d'allure pour montre
//! Android (Wear OS), inspiree de Pace Control (PBkSoft).
//!
//! Le crate est **pur** : aucune E/S, aucun acces reseau, aucun appel Android.
//! Il est donc testable sur PC, embarquable dans la montre et reutilisable tel
//! quel par le pont JNI/UniFFI (`mpacer-ffi`) et le simulateur (`mpacer-sim`).
//!
//! ## Pipeline
//!
//! ```text
//! GPS brut --> [gps::GpsMonitor] --> qualite du signal
//!          --> [pace::PaceEngine] --> allure courante lissee (2 min)
//!          --> [workout::Workout] --> distance / temps / tours / auto-pause
//!          --> [race_plan::RacePlan] --> shadow runner, ecart au plan
//!          --> [assistant::Assistant] --> panneau d'information + [voice::VoiceCoach]
//! ```
//!
//! Point d'entree recommande : `engine::PacerEngine`.

pub mod assistant;
pub mod best_distances;
pub mod engine;
pub mod geo;
pub mod gps;
pub mod gpx;
pub mod history;
pub mod lap;
pub mod pace;
pub mod race_plan;
pub mod remote_race;
pub mod units;
pub mod voice;
pub mod workout;

pub use assistant::{Assistant, AssistantMode, AssistantPanel};
pub use engine::{EngineConfig, EngineOutput, PacerEngine};
pub use gps::{GpsMonitor, GpsSample, GpsStatus, StatusLight};
pub use pace::PaceEngine;
pub use race_plan::{NegativeSplit, RacePlan, ShadowRunnerComparison};
pub use units::UnitSystem;
pub use workout::{Workout, WorkoutEvent, WorkoutState};

/// Timestamp UNIX en millisecondes.
pub type TimestampMs = i64;

/// Version du moteur, exposee au shell Android (ecran "A propos").
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
