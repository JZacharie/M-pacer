//! Geometrie : distance orthodromique et lissage de trace.

use serde::{Deserialize, Serialize};

/// Rayon moyen terrestre (m).
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// Position geographique.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Position {
    pub lat: f64,
    pub lon: f64,
}

impl Position {
    pub const fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}

/// Distance orthodromique (Haversine) en metres.
///
/// Precision ~0.5 % : suffisant pour un compteur de distance de course, y
/// compris sur les traces GPS bruitees.
pub fn haversine_m(a: Position, b: Position) -> f64 {
    let lat1 = a.lat.to_radians();
    let lat2 = b.lat.to_radians();
    let dlat = lat2 - lat1;
    let dlon = (b.lon - a.lon).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * h.sqrt().clamp(0.0, 1.0).asin()
}

/// Distance orthodromique signee sur un axe est-ouest (utile aux tests).
pub fn is_plausible_jump(
    previous: Position,
    next: Position,
    dt_s: f64,
    max_speed_mps: f64,
) -> bool {
    if dt_s <= 0.0 {
        return true;
    }
    haversine_m(previous, next) <= max_speed_mps * dt_s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_distance_paris_london() {
        let d = haversine_m(
            Position::new(48.8566, 2.3522),
            Position::new(51.5074, -0.1278),
        );
        // ~343.5 km
        assert!((d - 343_500.0).abs() < 2_000.0, "d = {d}");
    }

    #[test]
    fn one_degree_of_latitude_is_about_111_km() {
        let d = haversine_m(Position::new(45.0, 3.0), Position::new(46.0, 3.0));
        assert!((d - 111_195.0).abs() < 500.0, "d = {d}");
    }

    #[test]
    fn implausible_jump_is_detected() {
        let a = Position::new(45.0, 3.0);
        let b = Position::new(45.01, 3.0); // ~1.1 km
        assert!(!is_plausible_jump(a, b, 1.0, 12.0));
        assert!(is_plausible_jump(a, b, 120.0, 12.0));
    }
}
