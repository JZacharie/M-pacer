//! Import d'une course Strava ou Garmin : du fichier televerse a la fiche.
//!
//! Deux points d'entree partagent ce code, avec la meme validation :
//!   * POST /courses/importer (navigateur, cookie de session) ;
//!   * POST /api/v1/races/import (montre, compagnon ou script, jeton d'appareil).
//!
//! Le fichier n'est jamais conserve tel quel : la trace est relue, normalisee et
//! stockee en GPX 1.1 par la couche base, ce qui evite de garder un fichier
//! utilisateur sur le disque du serveur.

use crate::error::{AppError, AppResult};
use crate::models::{ImportedRaceMeta, Race, RaceInput};
use crate::state::AppState;
use axum::extract::Multipart;
use mpacer_core::race_import::{import_race, ImportError, ImportedRace};

/// Importe le fichier televerse et enregistre la course de reference.
pub async fn import_uploaded_race(
    state: &AppState,
    user_id: &str,
    filename: &str,
    bytes: &[u8],
) -> AppResult<Race> {
    if bytes.is_empty() {
        return Err(AppError::InvalidMultipart(
            "le fichier recu est vide".to_string(),
        ));
    }
    let imported = import_race(filename, bytes).map_err(map_import_error)?;
    let input = race_input_from_import(&imported);
    input.validate().map_err(AppError::bad_request)?;

    let meta = ImportedRaceMeta {
        source: imported.source.as_str().to_string(),
        moving_time_s: imported.moving_time_s,
        elapsed_time_s: imported.elapsed_time_s,
        elevation_gain_m: (imported.elevation_gain_m > 0.5).then_some(imported.elevation_gain_m),
        gpx: imported.to_gpx(),
        points: imported.points.len() as i32,
    };
    let race = crate::db::insert_imported_race(&state.pool, user_id, &input, &meta, state.now_ms())
        .await?;
    tracing::info!(
        utilisateur = %user_id,
        course = %race.id,
        origine = %meta.source,
        points = meta.points,
        "course de reference importee"
    );
    Ok(race)
}

/// Champs de la fiche deduits du fichier importe.
///
/// Le reste de la fiche reste vide : rien n'est invente. Le coureur complete
/// (dossard, notes) s'il le souhaite, comme pour n'importe quelle course.
fn race_input_from_import(imported: &ImportedRace) -> RaceInput {
    let name: String = imported.name.trim().chars().take(200).collect();
    let first = imported.points.first();
    RaceInput {
        name: if name.is_empty() {
            "Course importee".to_string()
        } else {
            name
        },
        start_at_ms: imported.started_at_ms,
        distance_m: (imported.distance_m > 0.0).then_some(imported.distance_m),
        latitude: first.map(|point| point.lat),
        longitude: first.map(|point| point.lon),
        ..RaceInput::default()
    }
}

/// Traduit une erreur d'import en refus HTTP explicite.
fn map_import_error(error: ImportError) -> AppError {
    match error {
        ImportError::TooLarge => AppError::PayloadTooLarge(error.to_string()),
        ImportError::UnsupportedFormat(_) => AppError::UnsupportedMediaType(error.to_string()),
        ImportError::NoTrack => AppError::BadRequest(error.to_string()),
    }
}

/// Collecte le fichier d'un corps multipart (champ file, ou premier fichier).
pub async fn collect_race_file(mut multipart: Multipart) -> AppResult<(String, Vec<u8>)> {
    let mut file: Option<(String, Vec<u8>)> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::InvalidMultipart(error.to_string()))?
    {
        let filename = field.file_name().map(str::to_string).unwrap_or_default();
        let bytes = field
            .bytes()
            .await
            .map_err(|error| AppError::InvalidMultipart(error.to_string()))?;
        // Seul le premier fichier non vide compte : un formulaire qui renvoie
        // deux fois le meme champ ne doit pas creer deux courses.
        if file.is_none() && !bytes.is_empty() {
            file = Some((filename, bytes.to_vec()));
        }
    }
    file.ok_or_else(|| AppError::InvalidMultipart("aucun fichier recu".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpacer_core::race_import::ImportSource;

    /// GPX minimal : deux points a 1 Hz, 100 m d'ecart.
    fn sample_gpx() -> &'static str {
        "<?xml version=\"1.0\"?><gpx creator=\"StravaGPX\"><metadata><name>10 km de nuit</name></metadata>\
         <trk><trkseg>\
         <trkpt lat=\"45.0\" lon=\"3.0\"><ele>300</ele><time>2026-05-01T19:00:00Z</time></trkpt>\
         <trkpt lat=\"45.0009\" lon=\"3.0\"><ele>302</ele><time>2026-05-01T19:00:30Z</time></trkpt>\
         </trkseg></trk></gpx>"
    }

    #[test]
    fn imported_fields_fill_the_race_sheet_without_inventing_anything() {
        let imported = import_race("10km.gpx", sample_gpx().as_bytes()).expect("import");
        let input = race_input_from_import(&imported);
        assert_eq!(input.name, "10 km de nuit");
        assert_eq!(input.start_at_ms, imported.started_at_ms);
        assert_eq!(input.distance_m, Some(imported.distance_m));
        assert_eq!(input.latitude, Some(45.0));
        assert_eq!(input.longitude, Some(3.0));
        // Rien d'autre n'est rempli : ni dossard, ni notes, ni objectif.
        assert!(input.bib_number.is_none());
        assert!(input.notes.is_none());
        assert!(input.goal_time_s.is_none());
        assert!(input.validate().is_ok());
        assert_eq!(imported.source, ImportSource::Strava);
    }

    #[test]
    fn long_names_are_truncated_to_what_the_race_sheet_accepts() {
        let gpx = format!(
            "<gpx creator=\"StravaGPX\"><metadata><name>{}</name></metadata>\
             <trk><trkseg><trkpt lat=\"45.0\" lon=\"3.0\"/></trkseg></trk></gpx>",
            "a".repeat(300)
        );
        let imported = import_race("long.gpx", gpx.as_bytes()).expect("import");
        let input = race_input_from_import(&imported);
        assert_eq!(input.name.chars().count(), 200);
        assert!(input.validate().is_ok());
    }

    #[test]
    fn import_errors_keep_their_http_code() {
        assert_eq!(
            map_import_error(ImportError::NoTrack).status(),
            axum::http::StatusCode::BAD_REQUEST
        );
        assert_eq!(
            map_import_error(ImportError::TooLarge).status(),
            axum::http::StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            map_import_error(ImportError::UnsupportedFormat("fit".into())).status(),
            axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
    }
}
