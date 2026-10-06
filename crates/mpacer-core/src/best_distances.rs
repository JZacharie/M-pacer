//! Meilleurs temps sur distances de reference (1/5/10 km, 1/5 mi, semi).

use serde::{Deserialize, Serialize};

/// Point de trace avec distance cumulee.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackPoint {
    pub t_ms: i64,
    pub dist_m: f64,
    pub lat: f64,
    pub lon: f64,
    pub elevation_m: Option<f64>,
}

/// Meilleur temps realise sur une distance de reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BestEffort {
    pub label: String,
    pub distance_m: f64,
    pub time_s: f64,
    /// Distance cumulee au debut de l'effort.
    pub start_dist_m: f64,
}

/// Distances de reference proposees par defaut.
pub const STANDARD_DISTANCES: &[(f64, &str)] = &[
    (1000.0, "1 km"),
    (1609.344, "1 mi"),
    (5000.0, "5 km"),
    (8046.72, "5 mi"),
    (10_000.0, "10 km"),
    (21_097.5, "Half marathon"),
];

/// Ecart entre deux points de trace au-dela duquel on ne sait plus rien :
/// pause, perte de signal. Aucune valeur n'est inventee a travers un tel trou.
pub const TRACK_GAP_S: f64 = 5.0;

/// Interpole la distance cumulee (m) a un instant donne.
///
/// Renvoie `None` en dehors de la trace ou a travers un trou : c'est ce qui
/// permet de ne pas attribuer a une course le temps passe a l'arret.
pub fn distance_at_time_m(points: &[TrackPoint], t_ms: i64) -> Option<f64> {
    let first = points.first()?;
    let last = points.last()?;
    if t_ms < first.t_ms || t_ms > last.t_ms {
        return None;
    }
    let index = points.partition_point(|point| point.t_ms < t_ms);
    if index == 0 {
        return Some(first.dist_m);
    }
    if index >= points.len() {
        return Some(last.dist_m);
    }
    let (a, b) = (points[index - 1], points[index]);
    let span = (b.t_ms - a.t_ms) as f64 / 1000.0;
    if span <= 0.0 {
        return Some(b.dist_m);
    }
    if span > TRACK_GAP_S {
        return None;
    }
    let ratio = (t_ms - a.t_ms) as f64 / 1000.0 / span;
    Some(a.dist_m + (b.dist_m - a.dist_m) * ratio)
}

/// Interpole le temps (ms) a une distance cumulee donnee.
pub fn time_at_distance_ms(points: &[TrackPoint], dist_m: f64) -> Option<i64> {
    if points.len() < 2 {
        return None;
    }
    if dist_m <= points[0].dist_m {
        return Some(points[0].t_ms);
    }
    for window in points.windows(2) {
        let (a, b) = (window[0], window[1]);
        if dist_m >= a.dist_m && dist_m <= b.dist_m {
            let span = b.dist_m - a.dist_m;
            if span <= f64::EPSILON {
                return Some(b.t_ms);
            }
            let ratio = (dist_m - a.dist_m) / span;
            return Some(a.t_ms + ((b.t_ms - a.t_ms) as f64 * ratio).round() as i64);
        }
    }
    None
}

/// Calcule les meilleurs temps pour chaque distance cible.
pub fn best_efforts(points: &[TrackPoint], targets: &[(f64, &str)]) -> Vec<BestEffort> {
    let total = points.last().map(|p| p.dist_m).unwrap_or(0.0);
    let mut results = Vec::new();
    for (target, label) in targets {
        if total < *target {
            continue;
        }
        let mut best: Option<f64> = None;
        let mut best_start = 0.0;
        for point in points.iter() {
            let start_dist = point.dist_m - target;
            if start_dist < 0.0 {
                continue;
            }
            let (Some(t_end), Some(t_start)) = (
                time_at_distance_ms(points, point.dist_m),
                time_at_distance_ms(points, start_dist),
            ) else {
                continue;
            };
            let time_s = (t_end - t_start) as f64 / 1000.0;
            if time_s > 0.0 && best.is_none_or(|b| time_s < b) {
                best = Some(time_s);
                best_start = start_dist;
            }
        }
        if let Some(time_s) = best {
            results.push(BestEffort {
                label: (*label).to_string(),
                distance_m: *target,
                time_s,
                start_dist_m: best_start,
            });
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trace reguliere : 1 point toutes les 10 s a 3 m/s (5:33/km).
    fn synthetic_track(total_m: f64, speed_mps: f64) -> Vec<TrackPoint> {
        let mut points = Vec::new();
        let step_m = speed_mps * 10.0;
        let mut dist = 0.0;
        let mut t = 0i64;
        points.push(TrackPoint {
            t_ms: 0,
            dist_m: 0.0,
            lat: 45.0,
            lon: 3.0,
            elevation_m: None,
        });
        while dist < total_m {
            dist += step_m;
            t += 10_000;
            points.push(TrackPoint {
                t_ms: t,
                dist_m: dist,
                lat: 45.0,
                lon: 3.0,
                elevation_m: None,
            });
        }
        points
    }

    #[test]
    fn best_effort_of_a_steady_run_matches_theoretical_time() {
        let points = synthetic_track(12_000.0, 3.0); // 1000 m en 333 s
        let efforts = best_efforts(&points, STANDARD_DISTANCES);
        let km = efforts.iter().find(|e| e.label == "1 km").unwrap();
        assert!((km.time_s - 333.3).abs() < 1.0, "time = {}", km.time_s);
        let ten = efforts.iter().find(|e| e.label == "10 km").unwrap();
        assert!((ten.time_s - 3333.0).abs() < 10.0);
    }

    #[test]
    fn missing_distances_are_skipped() {
        let points = synthetic_track(2000.0, 3.0);
        let efforts = best_efforts(&points, STANDARD_DISTANCES);
        assert!(efforts.iter().all(|e| e.distance_m <= 2000.0));
    }

    #[test]
    fn fastest_segment_is_found_in_a_negative_split_run() {
        // 2 km à 4 m/s puis 2 km à 6 m/s : le meilleur 1 km est le dernier.
        let mut points = vec![TrackPoint {
            t_ms: 0,
            dist_m: 0.0,
            lat: 45.0,
            lon: 3.0,
            elevation_m: None,
        }];
        let mut dist = 0.0;
        let mut t = 0i64;
        for _ in 0..200 {
            dist += 40.0;
            t += 10_000;
            points.push(TrackPoint {
                t_ms: t,
                dist_m: dist,
                lat: 45.0,
                lon: 3.0,
                elevation_m: None,
            });
        }
        for _ in 0..200 {
            dist += 60.0;
            t += 10_000;
            points.push(TrackPoint {
                t_ms: t,
                dist_m: dist,
                lat: 45.0,
                lon: 3.0,
                elevation_m: None,
            });
        }
        let efforts = best_efforts(&points, STANDARD_DISTANCES);
        let km = efforts.iter().find(|e| e.label == "1 km").unwrap();
        assert!((km.time_s - 166.7).abs() < 2.0, "time = {}", km.time_s);
        assert!(km.start_dist_m > 2000.0);
    }

    #[test]
    fn interpolation_is_linear() {
        let points = vec![
            TrackPoint {
                t_ms: 0,
                dist_m: 0.0,
                lat: 0.0,
                lon: 0.0,
                elevation_m: None,
            },
            TrackPoint {
                t_ms: 100_000,
                dist_m: 1000.0,
                lat: 0.0,
                lon: 0.0,
                elevation_m: None,
            },
        ];
        assert_eq!(time_at_distance_ms(&points, 250.0), Some(25_000));
    }
}
