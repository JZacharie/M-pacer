//! # mpacer-api
//!
//! Backend de M-pacer : synchronisation des seances enregistrees sur la montre,
//! authentification Google et interface web de consultation.
//!
//! ```text
//! montre Wear OS --(Bearer)--> /api/v1/workouts --+
//!                                                 +--> SQLite (WorkoutSummary JSON)
//! navigateur ------(cookie)--> /  (interface web) -+
//! ```
//!
//! Points cles :
//! * la montre ne stocke aucun secret Google : elle s'appaire par code
//!   (`device authorization grant`) et recoit un jeton opaque ;
//! * la seance est conservee telle que le coeur Rust l'a produite (meme structure
//!   que le fichier `.pac`), ce qui rend l'export GPX et la relecture triviaux ;
//! * le service fonctionne hors ligne cote montre : la synchronisation est un
//!   confort, jamais un prerequis pour courir.

pub mod assets;
pub mod auth;
pub mod avatar;
pub mod bpm;
pub mod config;
pub mod dashboards;
pub mod db;
pub mod deemix;
pub mod deezer;
pub mod error;
pub mod friends;
pub mod live;
pub mod models;
pub mod mqtt;
pub mod race_import;
pub mod routes;
pub mod spotify;
pub mod state;
