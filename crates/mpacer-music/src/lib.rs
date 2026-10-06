//! Application locale `mpacer-music`.
//!
//! Elle apparie les fichiers audio d'un dossier du PC aux pistes d'un manifeste
//! de transfert, puis les copie sur la montre M-pacer par USB (`adb push`).
//!
//! La CLI (`cli`) et l'interface web locale (`server`, page `web`) partagent
//! exactement le meme planificateur (`planner`) : un seul endroit pour
//! l'appariement, la lecture des balises, l'espace libre, la copie et
//! l'ecriture du `manifest.json` final.

pub mod adb;
pub mod cli;
pub mod planner;
pub mod server;
pub mod web;

pub use mpacer_core::music::TransferManifest;
pub use planner::{
    inspect, materialize, probe_bpm, probe_duration_s, scan_folder, Inspection, Progress,
    ProgressSink, SilentSink, ToolError, TransferReport, TransferRequest, AUDIO_EXTENSIONS,
    TAG_PROBE_BYTES, WATCH_MUSIC_DIR,
};
