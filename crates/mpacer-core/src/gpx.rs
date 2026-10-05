//! Export GPX 1.1 des traces (partage vers Strava, Garmin, etc.).
//!
//! Implementation minimale et sans dependance : le GPX produit est valide et
//! suffisant pour les services de partage d'activite.

use crate::history::WorkoutSummary;

/// Convertit un timestamp UNIX (ms) en date ISO 8601 UTC.
pub fn iso8601_utc(ms: i64) -> String {
    let (days, ms_of_day) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    let (year, month, day) = civil_from_days(days);
    let seconds = ms_of_day / 1000;
    let millis = ms_of_day % 1000;
    let (h, m, s) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}.{millis:03}Z")
}

/// Algorithme de Howard Hinnant : jours depuis l'epoque -> date civile.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Echappe les caracteres XML d'un nom de trace.
fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Produit le contenu GPX d'une seance.
pub fn export_gpx(workout: &WorkoutSummary) -> String {
    let mut out = String::with_capacity(256 + workout.track.len() * 96);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(
        "<gpx version=\"1.1\" creator=\"M-pacer\" xmlns=\"http://www.topografix.com/GPX/1/1\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd\">\n",
    );
    out.push_str("  <metadata>\n");
    out.push_str(&format!("    <name>{}</name>\n", escape_xml(&workout.id)));
    out.push_str(&format!(
        "    <time>{}</time>\n",
        iso8601_utc(workout.started_at_ms)
    ));
    out.push_str("  </metadata>\n  <trk>\n");
    out.push_str(&format!("    <name>{}</name>\n", escape_xml(&workout.id)));
    out.push_str("    <type>running</type>\n    <trkseg>\n");
    for point in &workout.track {
        // La trace ne stocke que les temps relatifs : on les replace dans l'absolu.
        let absolute_ms = workout.started_at_ms + point.t_ms;
        out.push_str(&format!(
            "      <trkpt lat=\"{:.7}\" lon=\"{:.7}\">\n",
            point.lat, point.lon
        ));
        if let Some(elevation) = point.elevation_m {
            out.push_str(&format!("        <ele>{elevation:.1}</ele>\n"));
        }
        out.push_str(&format!(
            "        <time>{}</time>\n",
            iso8601_utc(absolute_ms)
        ));
        out.push_str("      </trkpt>\n");
    }
    out.push_str("    </trkseg>\n  </trk>\n</gpx>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::best_distances::TrackPoint;
    use crate::units::UnitSystem;

    #[test]
    fn epoch_conversion_is_correct() {
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso8601_utc(1_700_000_000_000), "2023-11-14T22:13:20.000Z");
        // Annee bissextile
        assert_eq!(iso8601_utc(1_582_934_400_000), "2020-02-29T00:00:00.000Z");
    }

    #[test]
    fn gpx_contains_track_points_and_metadata() {
        let workout = WorkoutSummary {
            id: "1700000000000".into(),
            started_at_ms: 1_700_000_000_000,
            duration_s: 60.0,
            distance_m: 200.0,
            average_pace_s_per_km: 300.0,
            laps: vec![],
            best_efforts: vec![],
            track: vec![
                TrackPoint {
                    t_ms: 0,
                    dist_m: 0.0,
                    lat: 45.0,
                    lon: 3.0,
                    elevation_m: Some(300.0),
                },
                TrackPoint {
                    t_ms: 60_000,
                    dist_m: 200.0,
                    lat: 45.001,
                    lon: 3.001,
                    elevation_m: None,
                },
            ],
            unit_system: UnitSystem::Metric,
        };
        let gpx = export_gpx(&workout);
        assert!(gpx.starts_with("<?xml"));
        assert!(gpx.contains("<trkpt lat=\"45.0000000\" lon=\"3.0000000\">"));
        assert!(gpx.contains("<ele>300.0</ele>"));
        assert!(gpx.contains("<time>2023-11-14T22:14:20.000Z</time>"));
        assert_eq!(gpx.matches("<trkpt").count(), 2);
    }
}
