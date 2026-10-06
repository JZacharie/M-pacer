//! # mpacer-ffi - pont entre le coeur Rust et Wear OS
//!
//! Deux usages :
//!
//! 1. **C ABI JSON** (toujours disponible, teste sur PC) : le shell Kotlin
//!    envoie une commande JSON et recoit un etat JSON.
//!    ```c
//!    Handle* h = mpacer_new();
//!    char* json = mpacer_command(h, "{\"cmd\":\"start\",\"t_ms\":1700000000000}");
//!    mpacer_string_free(json);
//!    mpacer_free(h);
//!    ```
//! 2. **JNI** : le fichier `android/app/src/main/cpp/mpacer_jni.c` expose cette
//!    C ABI a Kotlin via les conventions JNI (`Java_...`), sans dependance
//!    supplementaire cote Rust.
//!
//! Toute erreur est renvoyee sous forme de JSON `{"error": "..."}` : le pont ne
//! panique jamais a travers la frontiere FFI.

// Sur Windows/MSVC, l'editeur de liens signale la creation des fichiers .dll.lib/.exp :
// bruit d'environnement, sans rapport avec le code, qui ferait echouer `-D warnings`.
#![allow(linker_messages)]

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};

use mpacer_core::assistant::{AssistantConfig, AssistantMode};
use mpacer_core::engine::{EngineConfig, EngineOutput, PacerEngine};
use mpacer_core::geo::Position;
use mpacer_core::gps::GpsSample;
use mpacer_core::history::WorkoutSummary;
use mpacer_core::music::{MusicConfig, NowPlaying, Playlist};
use mpacer_core::race_plan::NegativeSplit;
use mpacer_core::voice::VoiceConfig;
use serde::{Deserialize, Serialize};

/// Commande envoyee par le shell.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Remplace tous les reglages d'un coup.
    Configure {
        config: EngineConfig,
    },
    /// Reglages de l'assistant de course.
    SetAssistant {
        mode: AssistantMode,
        race_distance_m: Option<f64>,
        planned_time_s: Option<f64>,
        #[serde(default)]
        negative_split_ratio: f64,
    },
    /// Reglages du retour vocal.
    SetVoice {
        config: VoiceConfig,
    },
    /// Reglages musique (BPM de reference, seuils, annonces).
    SetMusic {
        config: MusicConfig,
    },
    /// Playlist preparee sur la montre (`null` = aucune).
    SetMusicPlaylist {
        playlist: Option<Playlist>,
    },
    /// Instantane de la piste en cours (`null` = lecture arretee).
    MusicNowPlaying {
        now: Option<NowPlaying>,
    },
    /// Cadence mesuree par un capteur de pas.
    OnCadence {
        t_ms: i64,
        spm: f64,
    },
    /// Nouvelle position GPS (1 Hz en general).
    Gps {
        t_ms: i64,
        lat: f64,
        lon: f64,
        accuracy_m: f64,
        #[serde(default)]
        altitude_m: Option<f64>,
        #[serde(default)]
        speed_mps: Option<f64>,
    },
    /// Mesure de frequence cardiaque (capteur de la montre ou ceinture).
    HeartRate {
        t_ms: i64,
        bpm: u16,
    },
    Start {
        t_ms: i64,
    },
    Arm {
        t_ms: i64,
    },
    Pause {
        t_ms: i64,
    },
    Resume {
        t_ms: i64,
    },
    Stop {
        t_ms: i64,
    },
    /// Rafraichissement sans nouvelle position.
    Tick {
        t_ms: i64,
    },
    /// Triple clic casque : reinitialise la fenetre d'allure.
    ResetPaceWindow {
        t_ms: i64,
    },
    /// Double clic casque : annonce immediate.
    AnnounceNow {
        t_ms: i64,
    },
    /// Fin de seance remise a zero.
    Reset,
    /// Resume de la seance courante (historique, export GPX/.pac).
    Summary {
        started_at_ms: i64,
    },
    /// Version du moteur.
    Version,
}

/// Reponse renvoyee au shell.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Response {
    Output(Box<EngineOutput>),
    Summary(Box<WorkoutSummary>),
    Version { version: String },
    Error { error: String },
}

/// Etat persistant cote Rust, detenu par le shell (un par seance).
pub struct Handle {
    engine: PacerEngine,
}

impl Handle {
    pub fn new() -> Self {
        Self {
            engine: PacerEngine::default(),
        }
    }

    /// Execute une commande et renvoie la reponse correspondante.
    pub fn execute(&mut self, command: Command) -> Response {
        match command {
            Command::Configure { config } => {
                self.engine.set_config(config);
                Response::Output(Box::new(self.engine.tick(0)))
            }
            Command::SetAssistant {
                mode,
                race_distance_m,
                planned_time_s,
                negative_split_ratio,
            } => {
                let negative_split = if negative_split_ratio > 0.0 {
                    NegativeSplit::with_ratio(negative_split_ratio)
                } else {
                    NegativeSplit::even_pace()
                };
                self.engine.set_assistant_config(AssistantConfig {
                    mode,
                    race_distance_m,
                    planned_time_s,
                    negative_split,
                });
                Response::Output(Box::new(self.engine.tick(0)))
            }
            Command::SetVoice { config } => {
                self.engine.set_voice_config(config);
                Response::Output(Box::new(self.engine.tick(0)))
            }
            Command::SetMusic { config } => {
                self.engine.set_music_config(config);
                Response::Output(Box::new(self.engine.tick(0)))
            }
            Command::SetMusicPlaylist { playlist } => {
                self.engine.set_music_playlist(playlist);
                Response::Output(Box::new(self.engine.tick(0)))
            }
            Command::MusicNowPlaying { now } => {
                self.engine.on_now_playing(now);
                Response::Output(Box::new(self.engine.tick(0)))
            }
            Command::OnCadence { t_ms, spm } => {
                self.engine.on_cadence(spm);
                Response::Output(Box::new(self.engine.tick(t_ms)))
            }
            Command::Gps {
                t_ms,
                lat,
                lon,
                accuracy_m,
                altitude_m,
                speed_mps,
            } => {
                let mut sample = GpsSample::new(t_ms, Position::new(lat, lon), accuracy_m);
                if let Some(altitude) = altitude_m {
                    sample = sample.with_altitude(altitude);
                }
                if let Some(speed) = speed_mps {
                    sample = sample.with_speed(speed);
                }
                Response::Output(Box::new(self.engine.on_gps(sample)))
            }
            Command::HeartRate { t_ms, bpm } => {
                self.engine.on_heart_rate(t_ms, bpm);
                Response::Output(Box::new(self.engine.tick(t_ms)))
            }
            Command::Start { t_ms } => Response::Output(Box::new(self.engine.start(t_ms))),
            Command::Arm { t_ms } => Response::Output(Box::new(self.engine.arm(t_ms))),
            Command::Pause { t_ms } => Response::Output(Box::new(self.engine.pause(t_ms))),
            Command::Resume { t_ms } => Response::Output(Box::new(self.engine.resume(t_ms))),
            Command::Stop { t_ms } => Response::Output(Box::new(self.engine.stop(t_ms))),
            Command::Tick { t_ms } => Response::Output(Box::new(self.engine.tick(t_ms))),
            Command::ResetPaceWindow { t_ms } => {
                self.engine.reset_pace_window(t_ms);
                Response::Output(Box::new(self.engine.tick(t_ms)))
            }
            Command::AnnounceNow { t_ms } => {
                Response::Output(Box::new(self.engine.announce_now(t_ms)))
            }
            Command::Reset => {
                self.engine.reset();
                Response::Output(Box::new(self.engine.tick(0)))
            }
            Command::Summary { started_at_ms } => {
                Response::Summary(Box::new(self.engine.summary(started_at_ms)))
            }
            Command::Version => Response::Version {
                version: mpacer_core::VERSION.to_string(),
            },
        }
    }

    pub fn engine(&self) -> &PacerEngine {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &mut PacerEngine {
        &mut self.engine
    }
}

impl Default for Handle {
    fn default() -> Self {
        Self::new()
    }
}

/// Analyse et execute une commande JSON ; renvoie une reponse JSON.
pub fn dispatch(handle: &mut Handle, command_json: &str) -> String {
    let response = match serde_json::from_str::<Command>(command_json) {
        Ok(command) => handle.execute(command),
        Err(error) => Response::Error {
            error: format!("commande invalide : {error}"),
        },
    };
    match serde_json::to_string(&response) {
        Ok(json) => json,
        Err(error) => format!("{{\"error\":\"serialisation impossible : {error}\"}}"),
    }
}

// ------------------------------------------------------------------ C ABI

/// Cree un moteur. A liberer avec `mpacer_free`.
#[no_mangle]
pub extern "C" fn mpacer_new() -> *mut Handle {
    Box::into_raw(Box::new(Handle::new()))
}

/// Libere un moteur cree par `mpacer_new`.
///
/// # Safety
/// `handle` doit provenir de `mpacer_new` et ne pas avoir deja ete libere.
#[no_mangle]
pub unsafe extern "C" fn mpacer_free(handle: *mut Handle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Execute une commande JSON et renvoie une chaine JSON a liberer avec
/// `mpacer_string_free`.
///
/// # Safety
/// `handle` doit etre valide et `command` une chaine C terminee par NUL.
#[no_mangle]
pub unsafe extern "C" fn mpacer_command(
    handle: *mut Handle,
    command: *const c_char,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if handle.is_null() || command.is_null() {
            return "{\"error\":\"handle ou commande nul\"}".to_string();
        }
        let handle = &mut *handle;
        match CStr::from_ptr(command).to_str() {
            Ok(text) => dispatch(handle, text),
            Err(_) => "{\"error\":\"commande non UTF-8\"}".to_string(),
        }
    }));

    let json =
        result.unwrap_or_else(|_| "{\"error\":\"panique interceptee dans le moteur\"}".to_string());
    match CString::new(json) {
        Ok(text) => text.into_raw(),
        Err(_) => CString::new("{\"error\":\"reponse non transmissible\"}")
            .unwrap()
            .into_raw(),
    }
}

/// Libere une chaine renvoyee par `mpacer_command` ou `mpacer_version`.
///
/// # Safety
/// `text` doit provenir de ce module et ne pas avoir deja ete libere.
#[no_mangle]
pub unsafe extern "C" fn mpacer_string_free(text: *mut c_char) {
    if !text.is_null() {
        drop(CString::from_raw(text));
    }
}

/// Version du moteur (chaine a liberer avec `mpacer_string_free`).
#[no_mangle]
pub extern "C" fn mpacer_version() -> *mut c_char {
    CString::new(mpacer_core::VERSION).unwrap().into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(handle: &mut Handle, json: &str) -> serde_json::Value {
        serde_json::from_str(&dispatch(handle, json)).unwrap()
    }

    #[test]
    fn start_and_gps_produce_metrics() {
        let mut handle = Handle::new();
        run(
            &mut handle,
            r#"{"cmd":"configure","config":{"units":"Metric","distance_display":{"preferred":"Metric","respect_race_units":false,"race_units":"Metric"},"pace":{"window_s":120.0,"probe_s":20.0,"min_probe_distance_m":25.0,"change_threshold":0.15,"max_pace_s_per_km":1800.0},"detect_pace_change":false,"gps":{"good_accuracy_m":10.0,"poor_accuracy_m":25.0,"good_samples_required":3,"max_speed_mps":12.0},"workout":{"auto_pause":false,"stop_speed_mps":0.7,"auto_pause_delay_s":10.0,"resume_speed_mps":1.4,"auto_resume_delay_s":3.0},"voice":{"enabled":true,"frequency":"Every2Minutes","language":"Fr","extended_lap_info":false,"short_forms":false,"music_policy":"Duck"}}"#,
        );
        let started = run(&mut handle, r#"{"cmd":"start","t_ms":1000}"#);
        assert_eq!(started["state"], "Running");

        let mut lat = 45.0;
        for i in 0..=180 {
            lat += 5.0 / 111_195.0;
            let json = format!(
                r#"{{"cmd":"gps","t_ms":{},"lat":{},"lon":3.0,"accuracy_m":4.0,"speed_mps":5.0}}"#,
                1000 + (i + 1) * 1000,
                lat
            );
            let output = run(&mut handle, &json);
            if i == 180 {
                // 181 echantillons, le premier ne servant qu'a fixer l'origine.
                assert!((output["elapsed_s"].as_f64().unwrap() - 181.0).abs() < 1.0);
                assert!((output["distance_m"].as_f64().unwrap() - 900.0).abs() < 20.0);
                assert!(output["current_pace"].as_f64().unwrap() > 150.0);
            }
        }
    }

    #[test]
    fn assistant_plan_is_applied() {
        let mut handle = Handle::new();
        let output = run(
            &mut handle,
            r#"{"cmd":"set_assistant","mode":"AchievePlannedTime","race_distance_m":10000.0,"planned_time_s":3000.0,"negative_split_ratio":0.03}"#,
        );
        assert!(output.is_object());
        run(&mut handle, r#"{"cmd":"start","t_ms":0}"#);
        let output = run(&mut handle, r#"{"cmd":"tick","t_ms":60000}"#);
        assert!(output["panel"]["visible"].as_bool().unwrap());
    }

    #[test]
    fn invalid_command_returns_an_error_object() {
        let mut handle = Handle::new();
        let value = run(&mut handle, r#"{"cmd":"nope"}"#);
        assert!(value["error"]
            .as_str()
            .unwrap()
            .contains("commande invalide"));
    }

    #[test]
    fn version_command_works() {
        let mut handle = Handle::new();
        let value = run(&mut handle, r#"{"cmd":"version"}"#);
        assert_eq!(value["version"], mpacer_core::VERSION);
    }

    #[test]
    fn c_abi_roundtrip_works_and_frees_memory() {
        unsafe {
            let handle = mpacer_new();
            assert!(!handle.is_null());
            let command = CString::new(r#"{"cmd":"version"}"#).unwrap();
            let response = mpacer_command(handle, command.as_ptr());
            assert!(!response.is_null());
            let text = CStr::from_ptr(response).to_str().unwrap().to_string();
            assert!(text.contains("version"));
            mpacer_string_free(response);
            mpacer_command(handle, std::ptr::null());
            mpacer_free(handle);
            let version = mpacer_version();
            assert!(!CStr::from_ptr(version).to_str().unwrap().is_empty());
            mpacer_string_free(version);
        }
    }

    #[test]
    fn music_commands_round_trip() {
        let mut handle = Handle::new();
        let configured = run(
            &mut handle,
            r#"{"cmd":"set_music","config":{"enabled":true,"reference_bpm":170.0,"reference_pace_s_per_km":300.0,"pace_elasticity":0.35,"min_bpm":100.0,"max_bpm":200.0,"switch_threshold_bpm":8.0,"boost_bpm":6.0,"relax_bpm":6.0,"announce":true,"avoid_last":3}}"#,
        );
        assert!(configured["music"]["enabled"].as_bool().unwrap());

        let empty = run(&mut handle, r#"{"cmd":"set_music_playlist","playlist":null}"#);
        assert_eq!(empty["music"]["directive"], "None");
        assert_eq!(empty["music"]["reason"], "NoPlaylist");

        let playlist = run(
            &mut handle,
            r#"{"cmd":"set_music_playlist","playlist":{"id":"p1","name":"Run 170","target_bpm":170.0,"tracks":[{"id":"t1","title":"Wake me up","artist":"Avicii","duration_s":215.0,"bpm":172.0,"position":0},{"id":"t2","title":"Autre","artist":null,"duration_s":200.0,"bpm":168.0,"position":1}]}}"#,
        );
        assert_eq!(playlist["music"]["playlist_id"], "p1");
        assert_eq!(playlist["music"]["playlist_name"], "Run 170");
        assert_eq!(playlist["music"]["directive"], "Play");
        assert_eq!(playlist["music"]["next_track_id"], "t1");

        let playing = run(
            &mut handle,
            r#"{"cmd":"music_now_playing","now":{"track_id":"t1","title":"Wake me up","artist":"Avicii","bpm":172.0,"position_s":42.5}}"#,
        );
        assert_eq!(playing["music"]["directive"], "Keep");
        assert_eq!(playing["music"]["current"]["position_s"], 42.5);

        let cadence = run(&mut handle, r#"{"cmd":"on_cadence","t_ms":1700000000000,"spm":174.0}"#);
        assert_eq!(cadence["music"]["cadence_spm"], 174.0);

        let cleared = run(&mut handle, r#"{"cmd":"music_now_playing","now":null}"#);
        assert!(cleared["music"]["current"].is_null());
    }

    #[test]
    fn an_old_configure_payload_without_music_still_parses() {
        let mut handle = Handle::new();
        let output = run(
            &mut handle,
            r#"{"cmd":"configure","config":{"units":"Metric","distance_display":{"preferred":"Metric","respect_race_units":false,"race_units":"Metric"},"pace":{"window_s":120.0,"probe_s":20.0,"min_probe_distance_m":25.0,"change_threshold":0.15,"max_pace_s_per_km":1800.0},"detect_pace_change":false,"gps":{"good_accuracy_m":10.0,"poor_accuracy_m":25.0,"good_samples_required":3,"max_speed_mps":12.0},"workout":{"auto_pause":false,"stop_speed_mps":0.7,"auto_pause_delay_s":10.0,"resume_speed_mps":1.4,"auto_resume_delay_s":3.0},"voice":{"enabled":true,"frequency":"Every2Minutes","language":"Fr","extended_lap_info":false,"short_forms":false,"music_policy":"Duck"},"heart_rate":{"max_bpm":190}}}"#,
        );
        assert!(output["music"].is_object());
        // Le champ absent retombe sur MusicConfig::default() : musique active,
        // mais aucune playlist donc aucune consigne.
        assert!(output["music"]["enabled"].as_bool().unwrap());
        assert_eq!(output["music"]["reason"], "NoPlaylist");
    }

    #[test]
    fn summary_command_returns_a_workout() {
        let mut handle = Handle::new();
        run(&mut handle, r#"{"cmd":"start","t_ms":0}"#);
        let summary = run(&mut handle, r#"{"cmd":"summary","started_at_ms":0}"#);
        assert_eq!(summary["started_at_ms"], 0);
        assert!(summary["laps"].is_array());
    }
}
