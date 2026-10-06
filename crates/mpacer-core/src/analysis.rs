//! Analyse d'une seance : temps de passage, comparaison au plan, pauses et acceleration.
//!
//! Ces calculs tournent **hors de la montre** : ils s'appliquent a un
//! WorkoutSummary deja enregistre, donc dans le backend ou l'interface web.
//! Le coeur reste sans E/S, ce qui les rend testables sur PC.

use crate::best_distances::{distance_at_time_m, TrackPoint};
use crate::history::WorkoutSummary;
use serde::{Deserialize, Serialize};

/// Ecart de temps entre deux points de trace au-dela duquel on considere
/// qu'il s'est passe autre chose qu'une simple acquisition (pause, trou GPS).
pub use crate::best_distances::TRACK_GAP_S;

/// Une pause de la seance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pause {
    /// Temps de course (s, pauses exclues) au declenchement.
    pub at_s: f64,
    /// Distance parcourue (m) au declenchement.
    pub at_distance_m: f64,
    /// Duree de la pause (s).
    pub duration_s: f64,
    /// Vrai si la pause a ete declenchee automatiquement (immobilite).
    pub automatic: bool,
}

/// Duree cumulee des pauses (s).
pub fn total_pause_s(pauses: &[Pause]) -> f64 {
    pauses.iter().map(|pause| pause.duration_s).sum()
}

/// Cout energetique de la course a une pente donnee (J/kg/m).
///
/// Modele de Minetti et al. (2002), celui qu'utilisent Runalyze et la plupart des
/// calculateurs d'allure ajustee : la depense par metre augmente fortement en
/// montee, diminue en descente jusqu'a environ -20 %, puis remonte (freinage).
pub fn energy_cost(grade: f64) -> f64 {
    let i = grade.clamp(-0.30, 0.30);
    155.4 * i.powi(5) - 30.4 * i.powi(4) - 43.3 * i.powi(3) + 46.3 * i * i + 19.5 * i + 3.6
}

/// Cout energetique sur le plat, reference du calcul.
const FLAT_ENERGY_COST: f64 = 3.6;

/// Bilan de terrain d'une plage de distance.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Terrain {
    gain_m: f64,
    loss_m: f64,
    /// Distance equivalente sur le plat, a effort egal.
    equivalent_distance_m: f64,
    /// Vrai si au moins une altitude a ete lue : sinon le GAP n'a aucun sens.
    has_elevation: bool,
}

/// Allure ajustee a la pente (GAP) sur l'ensemble d'une seance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradeAdjusted {
    /// Allure equivalente sur terrain plat (s/km).
    pub pace_s_per_km: f64,
    /// Distance equivalente sur le plat parcourue pour le meme effort (m).
    pub equivalent_distance_m: f64,
    pub elevation_gain_m: f64,
    pub elevation_loss_m: f64,
}

/// Calcule l'allure ajustee a la pente de la seance.
///
/// Renvoie `None` sans altitude : mieux vaut ne rien afficher que d'inventer un
/// terrain plat.
pub fn grade_adjusted(summary: &WorkoutSummary) -> Option<GradeAdjusted> {
    let terrain = terrain_in_range(&summary.track, f64::NEG_INFINITY, f64::INFINITY);
    if !terrain.has_elevation || terrain.equivalent_distance_m <= 0.0 || summary.duration_s <= 0.0 {
        return None;
    }
    Some(GradeAdjusted {
        pace_s_per_km: summary.duration_s / (terrain.equivalent_distance_m / 1000.0),
        equivalent_distance_m: terrain.equivalent_distance_m,
        elevation_gain_m: terrain.gain_m,
        elevation_loss_m: terrain.loss_m,
    })
}

/// Bilan d'une plage de distance : denivele et distance equivalente sur le plat.
fn terrain_in_range(track: &[TrackPoint], start_m: f64, end_m: f64) -> Terrain {
    let mut terrain = Terrain::default();
    let mut previous: Option<(f64, f64)> = None;
    for point in track {
        if point.dist_m < start_m || point.dist_m >= end_m {
            previous = None;
            continue;
        }
        let Some(elevation) = point.elevation_m else {
            // Une altitude manquante interrompt la chaine : aucun denivele invente.
            previous = None;
            continue;
        };
        if let Some((previous_dist, previous_elevation)) = previous {
            let dd = point.dist_m - previous_dist;
            if dd > 0.0 {
                let de = elevation - previous_elevation;
                terrain.has_elevation = true;
                if de > 0.0 {
                    terrain.gain_m += de;
                } else {
                    terrain.loss_m -= de;
                }
                let grade = (de / dd).clamp(-0.30, 0.30);
                terrain.equivalent_distance_m += dd * energy_cost(grade) / FLAT_ENERGY_COST;
            }
        }
        previous = Some((point.dist_m, elevation));
    }
    terrain
}

/// Un temps de passage (tour complet, ou dernier tronçon partiel).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Split {
    pub index: u32,
    pub distance_m: f64,
    pub duration_s: f64,
    pub pace_s_per_km: f64,
    /// Temps de course cumule a la fin du tronçon (s).
    pub cumulative_s: f64,
    /// Distance cumulee a la fin du tronçon (m).
    pub cumulative_distance_m: f64,
    /// Frequence cardiaque moyenne sur le tronçon.
    pub heart_rate_avg: Option<f64>,
    pub heart_rate_max: Option<u16>,
    pub elevation_gain_m: f64,
    pub elevation_loss_m: f64,
    /// Pente moyenne du tronçon (0.03 = 3 %), si l'altitude est connue.
    pub grade_percent: Option<f64>,
    /// Allure ajustee a la pente du tronçon (s/km), si l'altitude est connue.
    pub gap_pace_s_per_km: Option<f64>,
    /// Ecart d'allure avec le tronçon precedent (s/km) : positif = plus lent.
    pub pace_delta_s: Option<f64>,
    /// Allure prevue par le plan sur ce tronçon (s/km).
    pub planned_pace_s_per_km: Option<f64>,
    /// Temps planifie cumule a la fin du tronçon (s).
    pub planned_cumulative_s: Option<f64>,
    /// Temps reel cumule moins temps planifie : positif = en retard sur le plan.
    pub plan_delta_s: Option<f64>,
}

/// Un echantillon de vitesse lissee.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpeedSample {
    /// Temps ecoule depuis le premier point de trace (s).
    pub t_s: f64,
    pub speed_mps: f64,
}

/// Courbe de vitesse lissee (moyenne glissante centree).
///
/// Les couples separes par un trou (pause) sont ignores : la vitesse ne doit
/// pas s'effondrer artificiellement au moment ou le coureur s'arrete.
pub fn speed_curve(track: &[TrackPoint], window_s: f64) -> Vec<SpeedSample> {
    let origin = match track.first() {
        Some(point) => point.t_ms,
        None => return Vec::new(),
    };
    let mut raw: Vec<SpeedSample> = Vec::with_capacity(track.len());
    for window in track.windows(2) {
        let (a, b) = (window[0], window[1]);
        let dt = (b.t_ms - a.t_ms) as f64 / 1000.0;
        let dd = b.dist_m - a.dist_m;
        if dt <= 0.0 || dt > TRACK_GAP_S || dd < 0.0 {
            continue;
        }
        raw.push(SpeedSample {
            t_s: (a.t_ms + b.t_ms) as f64 / 2000.0 - origin as f64 / 1000.0,
            speed_mps: dd / dt,
        });
    }
    if raw.is_empty() {
        return raw;
    }
    let half = window_s.max(1.0) / 2.0;
    let times: Vec<f64> = raw.iter().map(|sample| sample.t_s).collect();
    let mut prefix = Vec::with_capacity(raw.len());
    let mut running = 0.0;
    for sample in &raw {
        running += sample.speed_mps;
        prefix.push(running);
    }
    raw.iter()
        .enumerate()
        .map(|(index, sample)| {
            let _ = index;
            let low = times.partition_point(|time| *time < sample.t_s - half);
            let high = times.partition_point(|time| *time <= sample.t_s + half);
            let before = if low == 0 { 0.0 } else { prefix[low - 1] };
            let count = (high - low).max(1) as f64;
            SpeedSample {
                t_s: sample.t_s,
                speed_mps: (prefix[high - 1] - before) / count,
            }
        })
        .collect()
}

/// Denivele positif cumule (m).
pub fn elevation_gain_m(track: &[TrackPoint]) -> f64 {
    elevation_deltas(track).0
}

/// Denivele negatif cumule (m).
pub fn elevation_loss_m(track: &[TrackPoint]) -> f64 {
    elevation_deltas(track).1
}

fn elevation_deltas(track: &[TrackPoint]) -> (f64, f64) {
    let (mut gain, mut loss) = (0.0, 0.0);
    for window in track.windows(2) {
        let (Some(previous), Some(next)) = (window[0].elevation_m, window[1].elevation_m) else {
            continue;
        };
        let delta = next - previous;
        if delta > 0.0 {
            gain += delta;
        } else {
            loss -= delta;
        }
    }
    (gain, loss)
}

/// Temps de passage : un tour par kilometre (ou mile), plus le dernier tronçon.
///
/// Les durees viennent des tours enregistres (temps de course, pauses exclues) ;
/// la frequence cardiaque et le denivele sont releves sur la trace, dans la
/// plage de distance de chaque tronçon.
pub fn splits(summary: &WorkoutSummary) -> Vec<Split> {
    let mut boundaries: Vec<(f64, f64, f64)> = Vec::new(); // (debut, fin, duree)
    let mut cumulative_distance = 0.0;
    let mut moving_s = 0.0;
    for lap in &summary.laps {
        boundaries.push((
            cumulative_distance,
            cumulative_distance + lap.distance_m,
            lap.duration_s,
        ));
        cumulative_distance += lap.distance_m;
        moving_s += lap.duration_s;
    }
    let remainder = summary.distance_m - cumulative_distance;
    if remainder > 1.0 {
        boundaries.push((
            cumulative_distance,
            summary.distance_m,
            (summary.duration_s - moving_s).max(0.0),
        ));
    }
    if boundaries.is_empty() {
        return Vec::new();
    }

    let plan = summary.plan.as_ref();
    let mut splits = Vec::with_capacity(boundaries.len());
    let mut cumulative_s = 0.0;
    let mut previous_pace: Option<f64> = None;
    for (index, (start, end, duration_s)) in boundaries.iter().enumerate() {
        cumulative_s += duration_s;
        let distance = end - start;
        let pace_s_per_km = if distance > 0.0 {
            duration_s / (distance / 1000.0)
        } else {
            0.0
        };
        let (heart_rate_avg, heart_rate_max) = heart_rate_in_range(summary, *start, *end);
        let terrain = terrain_in_range(&summary.track, *start, *end);
        // L'allure ajustee n'a de sens qu'avec de l'altitude : sur un profil
        // inconnu, elle serait identique a l'allure reelle et n'apprendrait rien.
        let gap_pace_s_per_km = if terrain.has_elevation && terrain.equivalent_distance_m > 0.0 {
            Some(duration_s / (terrain.equivalent_distance_m / 1000.0))
        } else {
            None
        };
        let grade_percent = if terrain.has_elevation && distance > 0.0 {
            Some((terrain.gain_m - terrain.loss_m) / distance)
        } else {
            None
        };
        let planned_pace_s_per_km =
            plan.map(|plan| plan.pace_at_distance_s_per_km((start + end) / 2.0));
        let planned_cumulative_s = plan.map(|plan| plan.time_at_distance_s(*end));
        splits.push(Split {
            index: index as u32 + 1,
            distance_m: distance,
            duration_s: *duration_s,
            pace_s_per_km,
            cumulative_s,
            cumulative_distance_m: *end,
            heart_rate_avg,
            heart_rate_max,
            elevation_gain_m: terrain.gain_m,
            elevation_loss_m: terrain.loss_m,
            grade_percent,
            gap_pace_s_per_km,
            pace_delta_s: previous_pace.map(|previous| pace_s_per_km - previous),
            planned_pace_s_per_km,
            planned_cumulative_s,
            plan_delta_s: planned_cumulative_s.map(|planned| cumulative_s - planned),
        });
        previous_pace = Some(pace_s_per_km);
    }
    splits
}

/// Frequence cardiaque moyenne et maximale dans une plage de distance.
fn heart_rate_in_range(
    summary: &WorkoutSummary,
    start_m: f64,
    end_m: f64,
) -> (Option<f64>, Option<u16>) {
    let mut sum = 0.0;
    let mut count = 0_u32;
    let mut max = 0_u16;
    for sample in &summary.heart_rate {
        let Some(distance) = distance_at_time_m(&summary.track, sample.t_ms) else {
            continue;
        };
        if distance < start_m || distance >= end_m {
            continue;
        }
        sum += sample.bpm as f64;
        count += 1;
        max = max.max(sample.bpm);
    }
    if count == 0 {
        (None, None)
    } else {
        (Some(sum / count as f64), Some(max))
    }
}

/// Une phase d'acceleration : le depart, ou une reprise apres une pause.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AccelerationPhase {
    /// Temps ecoule depuis le debut de la trace (s).
    pub at_s: f64,
    /// Distance parcourue au moment de la phase (m).
    pub at_distance_m: f64,
    /// Vrai pour une reprise apres une pause, faux pour le depart.
    pub after_pause: bool,
    /// Temps mis pour atteindre l'allure de croisiere (s), si elle est atteinte.
    pub seconds: Option<f64>,
    /// Distance parcourue pendant la phase (m).
    pub distance_m: f64,
}

/// Analyse de l'acceleration et des phases de la seance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccelerationAnalysis {
    /// Allure de croisiere de reference (m/s) : 80 % de la vitesse moyenne.
    pub cruise_speed_mps: f64,
    pub phases: Vec<AccelerationPhase>,
    /// Temps passe a accelerer (s).
    pub accelerating_s: f64,
    /// Temps passe a allure stable (s).
    pub steady_s: f64,
    /// Temps passe a ralentir (s).
    pub decelerating_s: f64,
}

/// Pente de vitesse (m/s par seconde) au-dela de laquelle on parle d'acceleration.
const ACCELERATION_THRESHOLD_MPS2: f64 = 0.05;
/// Duree maximale donnee a une phase pour atteindre l'allure de croisiere (s).
const PHASE_TIMEOUT_S: f64 = 300.0;

/// Analyse les phases d'acceleration et la repartition du temps.
pub fn acceleration(summary: &WorkoutSummary) -> Option<AccelerationAnalysis> {
    let curve = speed_curve(&summary.track, 5.0);
    if curve.len() < 5 || summary.duration_s <= 0.0 || summary.distance_m <= 0.0 {
        return None;
    }
    let average_speed = summary.distance_m / summary.duration_s;
    let cruise = average_speed * 0.8;

    // Debuts de phase : le depart, puis chaque reprise apres un trou de trace.
    let origin = summary.track.first()?.t_ms;
    let mut starts: Vec<(f64, bool)> = vec![(0.0, false)];
    for window in summary.track.windows(2) {
        let (a, b) = (window[0], window[1]);
        let gap = (b.t_ms - a.t_ms) as f64 / 1000.0;
        if gap > TRACK_GAP_S {
            starts.push(((b.t_ms - origin) as f64 / 1000.0, true));
        }
    }

    let mut phases = Vec::with_capacity(starts.len());
    for (start_s, after_pause) in starts {
        let reached = curve
            .iter()
            .filter(|sample| sample.t_s >= start_s && sample.t_s <= start_s + PHASE_TIMEOUT_S)
            .find(|sample| sample.speed_mps >= cruise);
        let seconds = reached.map(|sample| sample.t_s - start_s);
        let at_distance_m = distance_at_elapsed_s(&summary.track, start_s).unwrap_or(0.0);
        let end_s = reached.map(|sample| sample.t_s).unwrap_or(start_s);
        let end_distance = distance_at_elapsed_s(&summary.track, end_s).unwrap_or(at_distance_m);
        phases.push(AccelerationPhase {
            at_s: start_s,
            at_distance_m,
            after_pause,
            seconds,
            distance_m: (end_distance - at_distance_m).max(0.0),
        });
    }

    // Repartition du temps : acceleration, allure stable, ralentissement.
    let (mut accelerating_s, mut steady_s, mut decelerating_s) = (0.0, 0.0, 0.0);
    for window in curve.windows(2) {
        let (a, b) = (window[0], window[1]);
        let dt = b.t_s - a.t_s;
        if dt <= 0.0 || dt > TRACK_GAP_S {
            continue;
        }
        let slope = (b.speed_mps - a.speed_mps) / dt;
        if slope > ACCELERATION_THRESHOLD_MPS2 {
            accelerating_s += dt;
        } else if slope < -ACCELERATION_THRESHOLD_MPS2 {
            decelerating_s += dt;
        } else {
            steady_s += dt;
        }
    }

    Some(AccelerationAnalysis {
        cruise_speed_mps: cruise,
        phases,
        accelerating_s,
        steady_s,
        decelerating_s,
    })
}

/// Distance atteinte au dernier point de trace connu a cet instant (s).
///
/// On ne traverse volontairement pas un trou : une reprise apres pause part de
/// la distance atteinte avant la pause, et non de zero.
pub fn distance_at_elapsed_s(track: &[TrackPoint], elapsed_s: f64) -> Option<f64> {
    let origin = track.first()?.t_ms;
    let target = origin + (elapsed_s * 1000.0).round() as i64;
    let index = track.partition_point(|point| point.t_ms <= target);
    if index == 0 {
        return Some(track[0].dist_m);
    }
    track.get(index - 1).map(|point| point.dist_m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cardio::HeartRateSample;
    use crate::lap::Lap;
    use crate::race_plan::{NegativeSplit, RacePlan};
    use crate::units::UnitSystem;

    /// Seance de 3 km a 3 m/s (5:33/km) en 1000 s, 1 point par seconde,
    /// avec 10 m de denivele positif par kilometre.
    fn steady_summary() -> WorkoutSummary {
        let track: Vec<TrackPoint> = (0..=1000)
            .map(|i| TrackPoint {
                t_ms: i * 1000,
                dist_m: i as f64 * 3.0,
                lat: 45.0,
                lon: 3.0,
                elevation_m: Some(300.0 + i as f64 * 0.03),
            })
            .collect();
        WorkoutSummary {
            id: "test".into(),
            started_at_ms: 0,
            duration_s: 1000.0,
            distance_m: 3000.0,
            average_pace_s_per_km: 333.3,
            laps: (1..=3)
                .map(|index| Lap {
                    index,
                    distance_m: 1000.0,
                    duration_s: 333.33,
                    pace_s_per_km: 333.33,
                })
                .collect(),
            best_efforts: vec![],
            track,
            unit_system: UnitSystem::Metric,
            elapsed_s: 1000.0,
            pauses: vec![],
            heart_rate: vec![],
            plan: None,
        }
    }

    #[test]
    fn splits_follow_the_kilometre_boundaries() {
        let summary = steady_summary();
        let splits = splits(&summary);
        assert_eq!(splits.len(), 3);
        assert!((splits[0].pace_s_per_km - 333.33).abs() < 0.01);
        assert!((splits[2].cumulative_distance_m - 3000.0).abs() < 1e-9);
        assert!(splits[0].pace_delta_s.is_none());
        // 10 m de denivele positif par kilometre.
        assert!((splits[0].elevation_gain_m - 10.0).abs() < 0.5);
    }

    #[test]
    fn a_partial_last_split_is_reported() {
        let mut summary = steady_summary();
        summary.distance_m = 3400.0;
        summary.duration_s = 1133.0;
        let splits = splits(&summary);
        assert_eq!(splits.len(), 4);
        assert!((splits[3].distance_m - 400.0).abs() < 1e-9);
        assert!((splits[3].duration_s - 133.0).abs() < 0.01);
    }

    #[test]
    fn plan_comparison_reports_a_late_runner() {
        let mut summary = steady_summary();
        // Plan : 3 km en 900 s (5:00/km), plus rapide que le coureur (1000 s).
        summary.plan = Some(RacePlan::new(3000.0, 900.0));
        let splits = splits(&summary);
        let last = splits.last().unwrap();
        assert!((last.planned_cumulative_s.unwrap() - 900.0).abs() < 1e-6);
        assert!(last.plan_delta_s.unwrap() > 90.0);
        assert!(last.planned_pace_s_per_km.unwrap() < 305.0);
    }

    #[test]
    fn negative_split_plan_slows_down_at_the_start() {
        let plan =
            RacePlan::new(10_000.0, 3000.0).with_negative_split(NegativeSplit::with_ratio(0.03));
        let start = plan.pace_at_distance_s_per_km(500.0);
        let finish = plan.pace_at_distance_s_per_km(9500.0);
        assert!(start > finish);
    }

    #[test]
    fn heart_rate_is_averaged_per_split() {
        let mut summary = steady_summary();
        summary.heart_rate = (0..=400)
            .map(|i| HeartRateSample {
                t_ms: i * 2500,
                bpm: 150,
            })
            .collect();
        let splits = splits(&summary);
        assert!((splits[0].heart_rate_avg.unwrap() - 150.0).abs() < 1e-9);
        assert_eq!(splits[0].heart_rate_max, Some(150));
    }

    #[test]
    fn speed_curve_is_flat_on_a_steady_run() {
        let summary = steady_summary();
        let curve = speed_curve(&summary.track, 5.0);
        assert!(curve.len() > 100);
        let average = curve.iter().map(|s| s.speed_mps).sum::<f64>() / curve.len() as f64;
        assert!((average - 3.0).abs() < 0.05, "average = {average}");
    }

    #[test]
    fn acceleration_analysis_reports_the_start_phase() {
        let summary = steady_summary();
        let analysis = acceleration(&summary).unwrap();
        assert_eq!(analysis.phases.len(), 1);
        assert!(!analysis.phases[0].after_pause);
        // Le coureur est deja a l'allure de croisiere au premier echantillon.
        assert!(analysis.phases[0].seconds.unwrap() < 5.0);
        assert!(analysis.steady_s > analysis.accelerating_s);
    }

    #[test]
    fn a_pause_creates_a_second_acceleration_phase() {
        let mut summary = steady_summary();
        // Pause de 40 s a 1200 m : la trace reprend 41 s plus tard, meme distance.
        let origin = summary.track[0].t_ms;
        summary.track = summary
            .track
            .iter()
            .filter(|point| point.dist_m <= 1200.0 || point.dist_m > 1203.0)
            .map(|point| {
                let mut point = *point;
                if point.dist_m > 1203.0 {
                    point.t_ms += 40_000;
                }
                let _ = origin;
                point
            })
            .collect();
        summary.pauses = vec![Pause {
            at_s: 400.0,
            at_distance_m: 1200.0,
            duration_s: 40.0,
            automatic: false,
        }];
        summary.elapsed_s = 1040.0;
        let analysis = acceleration(&summary).unwrap();
        assert_eq!(analysis.phases.len(), 2);
        assert!(analysis.phases[1].after_pause);
        // La reprise a lieu vers 440 s : la pause de 40 s a commence vers 400 s.
        assert!(
            (440.0..460.0).contains(&analysis.phases[1].at_s),
            "at_s = {}",
            analysis.phases[1].at_s
        );
    }

    /// Meme seance, avec une pente constante (0.05 = 5 %).
    fn with_grade(grade: f64) -> WorkoutSummary {
        let mut summary = steady_summary();
        for point in &mut summary.track {
            point.elevation_m = Some(300.0 + point.dist_m * grade);
        }
        summary
    }

    #[test]
    fn energy_cost_is_minimal_on_the_flat() {
        // Le modele de Minetti passe par un minimum en descente (vers -20 %),
        // mais des -5 % il est deja plus econome qu'a plat, et couteux en montee.
        assert!((energy_cost(0.0) - 3.6).abs() < 1e-9);
        assert!(energy_cost(0.05) > energy_cost(0.0));
        assert!(energy_cost(-0.05) < energy_cost(0.0));
    }

    #[test]
    fn gap_equals_pace_on_the_flat() {
        let summary = with_grade(0.0);
        let gap = grade_adjusted(&summary).unwrap();
        assert!((gap.pace_s_per_km - 333.33).abs() < 0.5, "{gap:?}");
        assert!((gap.equivalent_distance_m - 3000.0).abs() < 1.0);
    }

    #[test]
    fn gap_is_faster_uphill_and_slower_downhill() {
        let actual = 333.33;
        let uphill = grade_adjusted(&with_grade(0.05)).unwrap();
        assert!(uphill.pace_s_per_km < actual - 40.0, "montee : {uphill:?}");
        assert!(uphill.equivalent_distance_m > 3000.0);

        let downhill = grade_adjusted(&with_grade(-0.05)).unwrap();
        assert!(
            downhill.pace_s_per_km > actual + 40.0,
            "descente : {downhill:?}"
        );
        assert!(downhill.equivalent_distance_m < 3000.0);
    }

    #[test]
    fn gap_needs_elevation() {
        let mut summary = steady_summary();
        for point in &mut summary.track {
            point.elevation_m = None;
        }
        assert!(grade_adjusted(&summary).is_none());
        // Et sans altitude, les tronçons n'annoncent pas de GAP.
        assert!(splits(&summary)
            .iter()
            .all(|split| split.gap_pace_s_per_km.is_none()));
    }

    #[test]
    fn splits_report_grade_and_gap() {
        let summary = with_grade(0.02);
        let splits = splits(&summary);
        let first = &splits[0];
        assert!(
            (first.grade_percent.unwrap() - 0.02).abs() < 1e-3,
            "{first:?}"
        );
        assert!(first.gap_pace_s_per_km.unwrap() < first.pace_s_per_km);
    }

    #[test]
    fn total_pause_time_is_summed() {
        let pauses = vec![
            Pause {
                at_s: 100.0,
                at_distance_m: 300.0,
                duration_s: 20.0,
                automatic: true,
            },
            Pause {
                at_s: 400.0,
                at_distance_m: 1200.0,
                duration_s: 35.5,
                automatic: false,
            },
        ];
        assert!((total_pause_s(&pauses) - 55.5).abs() < 1e-9);
    }
}
