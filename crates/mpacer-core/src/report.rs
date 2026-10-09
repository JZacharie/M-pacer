//! Rapport d'analyse d'une seance terminee.
//!
//! Une fiche de seance (application telephone, service web) a besoin de la meme
//! matiere : totaux, cardio, terrain, temps de passage et regularite. Ce module
//! l'assemble une fois pour toutes, en une structure serialisable.
//!
//! Aucun algorithme n'est reecrit ici : les calculs vivent dans [crate::analysis]
//! et [crate::cardio], deja testes. Ce module les appelle, met en forme et
//! echantillonne les courbes pour qu'une trace d'une heure reste affichable.

use serde::{Deserialize, Serialize};

use crate::analysis::{
    acceleration, elevation_profile, elevation_summary, grade_adjusted, speed_extremes, splits,
    AccelerationAnalysis, ElevationOptions, ElevationSummary, GradeAdjusted, SpeedExtremes, Split,
};
use crate::best_distances::distance_at_time_m;
use crate::cardio::{self, CardiacDrift, HeartRateSummary, HeartRateZones};
use crate::history::WorkoutSummary;

/// Nombre de points gardes pour les courbes (cardio, altitude).
///
/// Une seance d'une heure a 1 Hz compte 3 600 points : inutile d'en transporter
/// autant jusqu'a l'ecran, qui en affiche quelques centaines au plus.
const CURVE_POINTS: usize = 180;

/// Tout ce qu'une fiche de seance affiche, en une seule structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionReport {
    pub distance_m: f64,
    /// Temps en mouvement, pauses exclues (s).
    pub duration_s: f64,
    /// Temps total ecoule, pauses comprises (s).
    pub elapsed_s: f64,
    pub paused_s: f64,
    pub pause_count: usize,
    pub average_pace_s_per_km: f64,
    pub average_speed_mps: f64,
    /// Allure ajustee a la pente (GAP) et distance equivalente sur le plat.
    pub grade_adjusted: Option<GradeAdjusted>,
    pub elevation: Option<ElevationSummary>,
    /// Profil altimetrique echantillonne : couples (distance_m, altitude_m).
    pub elevation_profile: Vec<(f64, f64)>,
    pub splits: Vec<Split>,
    pub heart_rate: Option<HeartRateSummary>,
    /// Derive cardiaque (decouplage aerobie) entre les deux moities.
    pub cardiac_drift: Option<CardiacDrift>,
    /// Courbe cardiaque echantillonnee : couples (distance_m, bpm).
    pub heart_rate_curve: Vec<(f64, f64)>,
    pub speed_extremes: Option<SpeedExtremes>,
    pub acceleration: Option<AccelerationAnalysis>,
    pub regularity: Option<Regularity>,
    /// Observations pretes a lire : ce qui, dans la seance, merite d'etre retenu.
    pub highlights: Vec<Highlight>,
    /// Vrai si la seance porte une trace GPS exploitable.
    pub has_track: bool,
}

/// Ton d'une observation : ce qui va bien, ce qui merite attention, ou un repere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HighlightLevel {
    Good,
    Watch,
    Info,
}

/// Une observation tiree du rapport, prete a afficher.
///
/// Les regles vivent ici, pas dans les interfaces : un seuil de derive cardiaque
/// est un avis d'entraineur, il se teste comme le reste.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Highlight {
    pub level: HighlightLevel,
    pub text: String,
}

/// Regularite de l'allure : ce qui distingue une course tenue d'une course subie.
///
/// Toutes les valeurs sont calculees sur les tours **complets** : le dernier
/// troncon, souvent court, fausserait l'ecart-type sans rien dire de la course.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Regularity {
    pub split_count: usize,
    /// Ecart-type des allures de tour (s/km).
    pub pace_spread_s: f64,
    /// Le meme ecart rapporte a l'allure moyenne (%).
    pub pace_spread_percent: f64,
    pub fastest_pace_s_per_km: f64,
    pub slowest_pace_s_per_km: f64,
    /// Allure de la premiere moitie, puis de la seconde (s/km).
    pub first_half_pace_s_per_km: f64,
    pub second_half_pace_s_per_km: f64,
    /// Positif quand la seconde moitie est la plus rapide (negative split, %).
    pub negative_split_percent: f64,
}

/// Assemble le rapport d'une seance.
///
/// Les zones cardiaques sont passees en parametre : elles dependent de la
/// frequence maximale de l'utilisateur, que le moteur ne connait pas.
pub fn session_report(summary: &WorkoutSummary, zones: HeartRateZones) -> SessionReport {
    let splits = splits(summary);
    let mut report = SessionReport {
        distance_m: summary.distance_m,
        duration_s: summary.duration_s,
        elapsed_s: summary.total_elapsed_s(),
        paused_s: summary.paused_s(),
        pause_count: summary.pauses.len(),
        average_pace_s_per_km: summary.average_pace_s_per_km,
        average_speed_mps: summary.average_speed_mps().unwrap_or(0.0),
        grade_adjusted: grade_adjusted(summary),
        elevation: elevation_summary(&summary.track, ElevationOptions::default()),
        elevation_profile: decimate(
            &elevation_profile(&summary.track, ElevationOptions::default()),
            CURVE_POINTS,
        ),
        heart_rate: cardio::summarize(&summary.heart_rate, zones),
        cardiac_drift: cardio::cardiac_drift(&summary.track, &summary.heart_rate),
        heart_rate_curve: heart_rate_curve(summary),
        speed_extremes: speed_extremes(&summary.track, 5.0),
        acceleration: acceleration(summary),
        regularity: regularity(&splits),
        highlights: Vec::new(),
        has_track: !summary.track.is_empty(),
        splits,
    };
    report.highlights = highlights(&report);
    report
}

/// Regularite de l'allure, sur les tours complets.
pub fn regularity(splits: &[Split]) -> Option<Regularity> {
    let nominal = splits
        .iter()
        .map(|split| split.distance_m)
        .fold(0.0_f64, f64::max);
    if nominal <= 0.0 {
        return None;
    }
    let paces: Vec<f64> = splits
        .iter()
        .filter(|split| split.distance_m >= nominal * 0.8)
        .map(|split| split.pace_s_per_km)
        .filter(|pace| *pace > 0.0)
        .collect();
    if paces.len() < 2 {
        return None;
    }
    let average = paces.iter().sum::<f64>() / paces.len() as f64;
    let variance = paces
        .iter()
        .map(|pace| (pace - average).powi(2))
        .sum::<f64>()
        / paces.len() as f64;
    let spread = variance.sqrt();
    let (first_half, second_half) = half_paces(splits)?;
    Some(Regularity {
        split_count: paces.len(),
        pace_spread_s: spread,
        pace_spread_percent: if average > 0.0 {
            spread / average * 100.0
        } else {
            0.0
        },
        fastest_pace_s_per_km: paces.iter().cloned().fold(f64::INFINITY, f64::min),
        slowest_pace_s_per_km: paces.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        first_half_pace_s_per_km: first_half,
        second_half_pace_s_per_km: second_half,
        negative_split_percent: if first_half > 0.0 {
            (first_half - second_half) / first_half * 100.0
        } else {
            0.0
        },
    })
}

// --------------------------------------------------------------- observations

/// Derive cardiaque en deca de laquelle la seance est restee aerobie (%).
const DRIFT_GOOD_PERCENT: f64 = 3.0;
/// Derive cardiaque au-dela de laquelle la fatigue ou la chaleur se sont fait sentir (%).
const DRIFT_WATCH_PERCENT: f64 = 5.0;
/// Ecart-type d'allure tenu pour regulier (% de l'allure moyenne).
const SPREAD_GOOD_PERCENT: f64 = 2.0;
/// Ecart-type d'allure au-dela duquel la course part dans tous les sens (%).
const SPREAD_WATCH_PERCENT: f64 = 5.0;
/// Gain d'allure sur la seconde moitie a partir duquel on parle de negative split (%).
const SPLIT_GAIN_GOOD_PERCENT: f64 = 1.0;
/// Perte d'allure sur la seconde moitie a partir de laquelle on parle de depart trop rapide (%).
const SPLIT_LOSS_WATCH_PERCENT: f64 = 2.0;
/// Part du temps en Z4-Z5 a partir de laquelle la seance est jugee soutenue (%).
const HARD_ZONE_SHARE_PERCENT: f64 = 25.0;
/// Part du temps de pause dans le temps ecoule a partir de laquelle on la signale (%).
const PAUSE_SHARE_PERCENT: f64 = 5.0;
/// Ecart entre allure ajustee et allure reelle a partir duquel le relief compte (s/km).
const GAP_GAP_S: f64 = 5.0;

/// Observations tirees du rapport : ce qu'un coureur doit retenir de sa seance.
///
/// Trois niveaux seulement, et jamais plus de quelques lignes : au-dela, plus
/// personne ne lit. L'ordre va du plus actionnable au plus anecdotique.
pub fn highlights(report: &SessionReport) -> Vec<Highlight> {
    let mut lignes = Vec::new();

    if let Some(regularity) = &report.regularity {
        let ecart = regularity.pace_spread_s;
        if regularity.pace_spread_percent <= SPREAD_GOOD_PERCENT {
            lignes.push(Highlight {
                level: HighlightLevel::Good,
                text: format!("Allure reguliere : {ecart:.0} s/km d'ecart entre les tours"),
            });
        } else if regularity.pace_spread_percent >= SPREAD_WATCH_PERCENT {
            lignes.push(Highlight {
                level: HighlightLevel::Watch,
                text: format!("Allure en dents de scie : {ecart:.0} s/km d'ecart entre les tours"),
            });
        }

        let split = regularity.negative_split_percent;
        if split >= SPLIT_GAIN_GOOD_PERCENT {
            lignes.push(Highlight {
                level: HighlightLevel::Good,
                text: format!("Negative split de {split:.1} % : seconde moitie plus rapide"),
            });
        } else if split <= -SPLIT_LOSS_WATCH_PERCENT {
            lignes.push(Highlight {
                level: HighlightLevel::Watch,
                text: format!(
                    "Seconde moitie plus lente de {:.1} % : depart trop rapide",
                    -split
                ),
            });
        }
    }

    if let Some(drift) = &report.cardiac_drift {
        let taux = drift.decoupling_percent;
        if taux >= DRIFT_WATCH_PERCENT {
            lignes.push(Highlight {
                level: HighlightLevel::Watch,
                text: format!(
                    "Derive cardiaque de {taux:.1} % : le coeur a derive, fatigue ou chaleur"
                ),
            });
        } else if taux <= DRIFT_GOOD_PERCENT {
            lignes.push(Highlight {
                level: HighlightLevel::Good,
                text: format!("Derive cardiaque faible ({taux:.1} %) : seance restee aerobie"),
            });
        }
    }

    if let Some(heart) = &report.heart_rate {
        let total = heart.total_seconds();
        if total > 0.0 {
            let dures = (heart.zone_seconds[3] + heart.zone_seconds[4]) / total * 100.0;
            if dures >= HARD_ZONE_SHARE_PERCENT {
                lignes.push(Highlight {
                    level: HighlightLevel::Info,
                    text: format!("{dures:.0} % du temps en Z4-Z5 : seance soutenue"),
                });
            }
        }
    }

    if let Some(gap) = &report.grade_adjusted {
        let ecart = report.average_pace_s_per_km - gap.pace_s_per_km;
        if ecart >= GAP_GAP_S {
            lignes.push(Highlight {
                level: HighlightLevel::Info,
                text: format!(
                    "Parcours valonne : {:.0} m de D+ pour {:.2} km, allure ajustee {} /km",
                    gap.elevation_gain_m,
                    report.distance_m / 1000.0,
                    format_pace(gap.pace_s_per_km)
                ),
            });
        }
    }

    if report.elapsed_s > 0.0 && report.paused_s / report.elapsed_s * 100.0 >= PAUSE_SHARE_PERCENT {
        lignes.push(Highlight {
            level: HighlightLevel::Info,
            text: format!(
                "{} de pause en {} {}",
                format_short_duration(report.paused_s),
                report.pause_count,
                if report.pause_count > 1 {
                    "arrets"
                } else {
                    "arret"
                }
            ),
        });
    }

    lignes
}

/// Allure en m:ss (s/km), la seule unite lue d'un coup d'oeil.
fn format_pace(seconds_per_km: f64) -> String {
    let total = seconds_per_km.round().max(0.0) as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Duree courte : "12 min" au-dela de la minute, "45 s" sinon.
fn format_short_duration(seconds: f64) -> String {
    let total = seconds.round().max(0.0);
    if total >= 60.0 {
        format!("{:.0} min", total / 60.0)
    } else {
        format!("{total:.0} s")
    }
}

/// Allure des deux moities, coupees a la moitie de la distance.
///
/// Le temps au passage de la moitie est interpole dans le troncon qui la
/// contient : sur une seance courte, la coupure peut tomber au milieu d'un tour.
fn half_paces(splits: &[Split]) -> Option<(f64, f64)> {
    let last = splits.last()?;
    let total_distance = last.cumulative_distance_m;
    let total_time = last.cumulative_s;
    if total_distance <= 0.0 || total_time <= 0.0 {
        return None;
    }
    let half = total_distance / 2.0;
    let mut start_distance = 0.0;
    let mut start_time = 0.0;
    let mut time_at_half = None;
    for split in splits {
        let end_distance = split.cumulative_distance_m;
        if half <= end_distance {
            let fraction = if end_distance > start_distance {
                (half - start_distance) / (end_distance - start_distance)
            } else {
                0.0
            };
            time_at_half = Some(start_time + fraction * split.duration_s);
            break;
        }
        start_distance = end_distance;
        start_time = split.cumulative_s;
    }
    let time_at_half = time_at_half?;
    if time_at_half <= 0.0 || total_time <= time_at_half {
        return None;
    }
    let first = time_at_half / (half / 1000.0);
    let second = (total_time - time_at_half) / ((total_distance - half) / 1000.0);
    Some((first, second))
}

/// Courbe cardiaque ramenee sur la distance : (distance_m, bpm).
///
/// Sans trace GPS, la distance n'existe pas : plutot que d'inventer un axe,
/// on renvoie une courbe vide et l'ecran n'affiche que les moyennes.
fn heart_rate_curve(summary: &WorkoutSummary) -> Vec<(f64, f64)> {
    if summary.track.is_empty() {
        return Vec::new();
    }
    let points: Vec<(f64, f64)> = summary
        .heart_rate
        .iter()
        .filter_map(|sample| {
            distance_at_time_m(&summary.track, sample.t_ms)
                .map(|distance| (distance, sample.bpm as f64))
        })
        .collect();
    decimate(&points, CURVE_POINTS)
}

/// Reduit une serie a un maximum de points, en gardant toujours les extremites.
fn decimate<T: Copy>(values: &[T], max: usize) -> Vec<T> {
    if values.len() <= max || max < 2 {
        return values.to_vec();
    }
    let step = (values.len() - 1) as f64 / (max - 1) as f64;
    (0..max)
        .map(|index| values[(index as f64 * step).round() as usize])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::best_distances::TrackPoint;
    use crate::cardio::HeartRateSample;
    use crate::lap::Lap;
    use crate::units::UnitSystem;

    /// Une trace plate de n points, un point tous les 10 m.
    fn flat_track(count: usize, elevation: Option<f64>) -> Vec<TrackPoint> {
        (0..count)
            .map(|index| TrackPoint {
                t_ms: index as i64 * 3000,
                dist_m: index as f64 * 10.0,
                lat: 45.0 + index as f64 * 0.0001,
                lon: 3.0,
                elevation_m: elevation,
            })
            .collect()
    }

    fn laps(distance_m: f64, count: usize, duration_s: f64) -> Vec<Lap> {
        (0..count)
            .map(|index| Lap {
                index: index as u32 + 1,
                distance_m,
                duration_s,
                pace_s_per_km: duration_s / (distance_m / 1000.0),
            })
            .collect()
    }

    fn workout(laps: Vec<Lap>) -> WorkoutSummary {
        let distance: f64 = laps.iter().map(|lap| lap.distance_m).sum();
        let duration: f64 = laps.iter().map(|lap| lap.duration_s).sum();
        WorkoutSummary {
            id: "1700000000000".into(),
            started_at_ms: 1_700_000_000_000,
            duration_s: duration,
            distance_m: distance,
            average_pace_s_per_km: duration / (distance / 1000.0),
            laps,
            best_efforts: vec![],
            track: vec![],
            unit_system: UnitSystem::Metric,
            elapsed_s: duration,
            pauses: vec![],
            heart_rate: vec![],
            plan: None,
        }
    }

    #[test]
    fn a_session_without_sensors_only_reports_totals() {
        let summary = workout(laps(1000.0, 5, 300.0));
        let report = session_report(&summary, HeartRateZones::default());
        assert_eq!(report.distance_m, 5000.0);
        assert_eq!(report.splits.len(), 5);
        assert!(!report.has_track);
        assert!(report.heart_rate.is_none());
        assert!(report.cardiac_drift.is_none());
        assert!(report.grade_adjusted.is_none());
        assert!(report.elevation.is_none());
        assert!(report.heart_rate_curve.is_empty());
        assert!(report.elevation_profile.is_empty());
    }

    #[test]
    fn an_even_run_has_almost_no_spread() {
        let summary = workout(laps(1000.0, 6, 300.0));
        let report = session_report(&summary, HeartRateZones::default());
        let regularity = report.regularity.expect("regularite");
        assert_eq!(regularity.split_count, 6);
        assert!(regularity.pace_spread_s < 1e-6);
        assert!(regularity.negative_split_percent.abs() < 1e-6);
    }

    #[test]
    fn a_negative_split_names_the_second_half() {
        // Six tours : trois a 6:00/km, trois a 5:30/km.
        let mut tous = laps(1000.0, 3, 360.0);
        tous.extend(laps(1000.0, 3, 330.0));
        for (index, lap) in tous.iter_mut().enumerate() {
            lap.index = index as u32 + 1;
        }
        let summary = workout(tous);
        let report = session_report(&summary, HeartRateZones::default());
        let regularity = report.regularity.expect("regularite");
        assert!((regularity.first_half_pace_s_per_km - 360.0).abs() < 1e-6);
        assert!((regularity.second_half_pace_s_per_km - 330.0).abs() < 1e-6);
        // 30 s de gain sur 360 : un negative split de 8,3 %.
        assert!((regularity.negative_split_percent - 8.333).abs() < 0.01);
        assert!(regularity.pace_spread_s > 10.0);
    }

    #[test]
    fn a_short_last_split_does_not_count_in_the_spread() {
        // Cinq tours complets reguliers, plus 200 m courus vite.
        let mut tous = laps(1000.0, 5, 300.0);
        tous.push(Lap {
            index: 6,
            distance_m: 200.0,
            duration_s: 40.0,
            pace_s_per_km: 200.0,
        });
        let summary = workout(tous);
        let report = session_report(&summary, HeartRateZones::default());
        // Le rapport garde le troncon partiel dans les temps de passage...
        assert_eq!(report.splits.len(), 6);
        // ...mais la regularite ne juge que les cinq tours complets.
        let regularity = report.regularity.expect("regularite");
        assert_eq!(regularity.split_count, 5);
        assert!(regularity.pace_spread_s < 1e-6);
    }

    #[test]
    fn the_heart_rate_curve_follows_the_distance() {
        let mut summary = workout(laps(1000.0, 1, 300.0));
        summary.track = flat_track(101, None);
        summary.heart_rate = (0..=10)
            .map(|index| HeartRateSample {
                t_ms: index * 30_000,
                bpm: 140 + index as u16,
            })
            .collect();
        let report = session_report(&summary, HeartRateZones::default());
        assert_eq!(report.heart_rate_curve.len(), 11);
        assert_eq!(report.heart_rate_curve[0], (0.0, 140.0));
        assert_eq!(report.heart_rate_curve[10].1, 150.0);
        assert!(report.heart_rate_curve[10].0 > report.heart_rate_curve[0].0);
        let heart = report.heart_rate.expect("cardio");
        assert_eq!(heart.max_bpm, 150);
        assert_eq!(heart.min_bpm, 140);
    }

    #[test]
    fn an_altitude_profile_is_kept_and_used_for_the_gap() {
        let mut summary = workout(laps(1000.0, 1, 300.0));
        summary.track = (0..=100)
            .map(|index| TrackPoint {
                t_ms: index as i64 * 3000,
                dist_m: index as f64 * 10.0,
                lat: 45.0 + index as f64 * 0.0001,
                lon: 3.0,
                elevation_m: Some(300.0 + index as f64 * 0.5),
            })
            .collect();
        let report = session_report(&summary, HeartRateZones::default());
        assert!(report.elevation.is_some());
        assert!(!report.elevation_profile.is_empty());
        let gap = report.grade_adjusted.expect("allure ajustee");
        assert!(gap.elevation_gain_m > 40.0);
        // La montee coute plus cher : l'allure equivalente sur le plat est plus rapide.
        assert!(gap.pace_s_per_km < summary.average_pace_s_per_km);
    }

    #[test]
    fn curves_are_decimated_but_keep_their_ends() {
        let values: Vec<f64> = (0..1000).map(|index| index as f64).collect();
        let reduced = decimate(&values, 180);
        assert_eq!(reduced.len(), 180);
        assert_eq!(reduced[0], 0.0);
        assert_eq!(reduced[179], 999.0);
        // Une serie plus courte que la limite n'est pas touchee.
        assert_eq!(decimate(&values[..10], 180).len(), 10);
    }

    #[test]
    fn a_regular_even_run_is_congratulated() {
        let summary = workout(laps(1000.0, 6, 300.0));
        let report = session_report(&summary, HeartRateZones::default());
        assert!(
            report
                .highlights
                .iter()
                .any(|ligne| ligne.level == HighlightLevel::Good
                    && ligne.text.contains("Allure reguliere")),
            "{:?}",
            report.highlights
        );
        assert!(!report
            .highlights
            .iter()
            .any(|ligne| ligne.level == HighlightLevel::Watch));
    }

    #[test]
    fn a_second_half_slower_names_a_start_too_fast() {
        // Trois tours a 5:00/km, trois a 5:30/km : la seconde moitie lache.
        let mut tous = laps(1000.0, 3, 300.0);
        tous.extend(laps(1000.0, 3, 330.0));
        for (index, lap) in tous.iter_mut().enumerate() {
            lap.index = index as u32 + 1;
        }
        let report = session_report(&workout(tous), HeartRateZones::default());
        assert!(
            report
                .highlights
                .iter()
                .any(|ligne| ligne.level == HighlightLevel::Watch
                    && ligne.text.contains("depart trop rapide")),
            "{:?}",
            report.highlights
        );
    }

    #[test]
    fn a_drifting_heart_rate_is_flagged() {
        let mut summary = workout(laps(1000.0, 2, 300.0));
        summary.track = flat_track(201, None);
        // Un point cardiaque toutes les 10 s : au-dela de 15 s, le coeur
        // considere qu'il y a un trou et refuse de conclure (MAX_SAMPLE_GAP_S).
        summary.heart_rate = (0..=60)
            .map(|index| HeartRateSample {
                t_ms: index * 10_000,
                bpm: if index <= 30 { 140 } else { 160 },
            })
            .collect();
        let report = session_report(&summary, HeartRateZones::default());
        let drift = report.cardiac_drift.expect("derive cardiaque");
        assert!(drift.decoupling_percent > 5.0, "{drift:?}");
        assert!(
            report
                .highlights
                .iter()
                .any(|ligne| ligne.level == HighlightLevel::Watch
                    && ligne.text.contains("Derive cardiaque")),
            "{:?}",
            report.highlights
        );
    }

    #[test]
    fn time_spent_at_threshold_is_named() {
        let mut summary = workout(laps(1000.0, 5, 300.0));
        summary.track = flat_track(501, None);
        // Dix minutes de mesures : la moitie en Z3, la moitie en Z5 (max 190).
        summary.heart_rate = (0..=60)
            .map(|index| HeartRateSample {
                t_ms: index * 10_000,
                bpm: if index < 30 { 150 } else { 175 },
            })
            .collect();
        let report = session_report(&summary, HeartRateZones::default());
        assert!(
            report
                .highlights
                .iter()
                .any(|ligne| ligne.text.contains("Z4-Z5")),
            "{:?}",
            report.highlights
        );
    }

    #[test]
    fn a_long_pause_is_reported() {
        let mut summary = workout(laps(1000.0, 5, 300.0));
        summary.elapsed_s = 1600.0;
        summary.pauses = vec![crate::analysis::Pause {
            at_s: 600.0,
            at_distance_m: 2000.0,
            duration_s: 100.0,
            automatic: false,
        }];
        let report = session_report(&summary, HeartRateZones::default());
        assert!(
            report
                .highlights
                .iter()
                .any(|ligne| ligne.text.contains("de pause en 1 arret")),
            "{:?}",
            report.highlights
        );
    }

    #[test]
    fn pacing_helpers_use_running_units() {
        assert_eq!(format_pace(300.0), "5:00");
        assert_eq!(format_pace(272.6), "4:33");
        assert_eq!(format_short_duration(45.0), "45 s");
        assert_eq!(format_short_duration(725.0), "12 min");
    }

    #[test]
    fn the_report_carries_the_same_splits_as_the_analysis() {
        let summary = workout(laps(1000.0, 3, 300.0));
        let report = session_report(&summary, HeartRateZones::default());
        assert_eq!(report.splits, splits(&summary));
    }
}
