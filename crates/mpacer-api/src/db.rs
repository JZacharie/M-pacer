//! Acces base de donnees (PostgreSQL) et initialisation du schema.
//!
//! La couche de persistance est organisee en sous-modules par domaine :
//! - [`users`] : utilisateurs, jetons d'appareil (`api_tokens`), recherche
//! - [`sessions`] : seances de course (`workouts`), statistiques et cumuls
//! - [`races`] : courses (`races`), traces GPX associees et taches
//! - [`music`] : playlists, morceaux, quotas et comptes Deezer
//! - [`dashboards`] : tableaux de bord personnalises et leurs widgets
//! - [`friends`] : amities, demandes d'amis, invitations et appareils live
//!
//! Toutes les fonctions restent egalement re-exportees a la racine de `db`
//! pour assurer une compatibilite ascendante transparente avec le reste du projet.

pub mod dashboards;
pub mod friends;
pub mod music;
pub mod races;
pub mod sessions;
pub mod users;

// Re-exports directs de chaque sous-domaine
pub use dashboards::*;
pub use friends::*;
pub use music::*;
pub use races::*;
pub use sessions::*;
pub use users::*;

use crate::config::Config;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::str::FromStr;
use std::time::Duration;

/// Schema initial, embarque dans le binaire (aucun fichier a deplacer).
const SCHEMA: &str = include_str!("../migrations/0001_init.sql");
/// Courses a venir et suivi : meme mecanisme, rejoue juste apres le schema initial.
const SCHEMA_RACES: &str = include_str!("../migrations/0002_races.sql");
/// Musique : playlists, titres, plans de telechargement et compte Deezer.
const SCHEMA_MUSIC: &str = include_str!("../migrations/0003_music.sql");
/// Courses de reference importees (Strava/Garmin) et commentaires de seance.
const SCHEMA_REFERENCES: &str = include_str!("../migrations/0004-references-et-commentaires.sql");
/// Tableaux de bord : ecrans composes par l'utilisateur (widgets ordonnes).
const SCHEMA_DASHBOARDS: &str = include_str!("../migrations/0005-tableaux-de-bord.sql");
/// Sources musicales multiples : compte Deezer et identifiant Deezer des playlists.
const SCHEMA_MUSIC_SOURCES: &str = include_str!("../migrations/0006-sources-musique.sql");
/// Amis, invitations et appareils revendiques pour le partage en direct.
const SCHEMA_FRIENDS: &str = include_str!("../migrations/0007-amis.sql");
/// Deezer par cookie `arl` : identifiant Deezer des pistes (liens Deemix).
const SCHEMA_DEEZER_ARL: &str = include_str!("../migrations/0008-deezer-arl.sql");
/// Deezer seule source externe : retrait des tables et colonnes de l'ancienne source.
const SCHEMA_DEEZER_ONLY: &str = include_str!("../migrations/0009-source-unique-deezer.sql");
/// Demandes d'amitie : l'amitie nait quand la personne visee accepte.
const SCHEMA_FRIEND_REQUESTS: &str = include_str!("../migrations/0010-demandes-amis.sql");

/// Ouvre le pool et applique le schema (idempotent).
pub async fn connect(config: &Config) -> anyhow::Result<PgPool> {
    let options = PgConnectOptions::from_str(&config.database.url())?;
    connect_with_options(options).await
}

/// Meme chose avec des options de connexion explicites (tests, reglages fins).
pub async fn connect_with_options(options: PgConnectOptions) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await?;
    sqlx::raw_sql(SCHEMA).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_RACES).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_MUSIC).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_REFERENCES).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_DASHBOARDS).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_MUSIC_SOURCES).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_FRIENDS).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_DEEZER_ARL).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_DEEZER_ONLY).execute(&pool).await?;
    sqlx::raw_sql(SCHEMA_FRIEND_REQUESTS).execute(&pool).await?;
    Ok(pool)
}
