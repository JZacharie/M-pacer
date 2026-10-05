//! Plan de course, negative split et "shadow runner".
//!
//! Le shadow runner est un coureur virtuel qui suit exactement le plan :
//! allure moyenne visee, depart plus lent, acceleration progressive jusqu'a
//! l'arrivee (negative split). L'appli se contente de comparer la position
//! reelle du coureur a celle du shadow runner.
//!
//! Modele mathematique (allure en s/km, distance en m) :
//!
//! ```text
//! a(f) = A * (1 + r - 2*r*f)          allure a la fraction f = d/D du parcours
//! T(d) = A * (d/1000) * (1 + r - r*d/D)   temps du shadow runner a la distance d
//! T(D) = A * D/1000 = temps cible         (le plan est exact au finish)
//! ```
//!
//! `A` = allure moyenne, `r` = ratio de negative split (0.03 = 3 %).

use crate::units::UnitSystem;
use serde::{Deserialize, Serialize};

/// Parametres de negative split.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NegativeSplit {
    pub enabled: bool,
    /// 0.02 a 0.04 recommandes par Pace Control.
    pub ratio: f64,
}

impl Default for NegativeSplit {
    fn default() -> Self {
        Self {
            enabled: false,
            ratio: 0.03,
        }
    }
}

impl NegativeSplit {
    pub fn even_pace() -> Self {
        Self {
            enabled: false,
            ratio: 0.0,
        }
    }

    pub fn with_ratio(ratio: f64) -> Self {
        Self {
            enabled: true,
            ratio,
        }
    }

    fn effective_ratio(&self) -> f64 {
        if self.enabled {
            self.ratio.clamp(0.0, 0.20)
        } else {
            0.0
        }
    }
}

/// Ecart au plan, tel qu'affiche dans le panneau d'assistance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShadowRunnerComparison {
    /// Distance du shadow runner au meme instant (m).
    pub planned_distance_m: f64,
    /// `distance reelle - distance planifiee` : positif = en avance.
    pub distance_delta_m: f64,
    /// `temps planifie a ma position - temps reel` : positif = en avance.
    pub time_delta_s: f64,
    pub ahead: bool,
    /// Vrai si l'ecart est dans la tolerance (affiche "sur le plan").
    pub on_plan: bool,
}

impl ShadowRunnerComparison {
    /// Tolerance : 5 m ou 2 s.
    pub fn is_on_plan(distance_delta_m: f64, time_delta_s: f64) -> bool {
        distance_delta_m.abs() < 5.0 && time_delta_s.abs() < 2.0
    }
}

/// Plan de course d'une epreuve ou d'une seance structuree.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RacePlan {
    /// Distance totale (m).
    pub distance_m: f64,
    /// Temps cible au finish (s).
    pub target_time_s: f64,
    pub negative_split: NegativeSplit,
}

impl RacePlan {
    pub fn new(distance_m: f64, target_time_s: f64) -> Self {
        Self {
            distance_m,
            target_time_s,
            negative_split: NegativeSplit::even_pace(),
        }
    }

    pub fn with_negative_split(mut self, negative_split: NegativeSplit) -> Self {
        self.negative_split = negative_split;
        self
    }

    /// Valide les prerequis (mode "achieving planned time").
    pub fn is_valid(&self) -> bool {
        self.distance_m > 0.0 && self.target_time_s > 0.0
    }

    /// Allure moyenne en s/km.
    pub fn average_pace_s_per_km(&self) -> f64 {
        self.target_time_s / (self.distance_m / 1000.0)
    }

    /// Allure moyenne dans le systeme demande.
    pub fn average_pace(&self, units: UnitSystem) -> f64 {
        units.meters_per_unit() / (1000.0 / self.average_pace_s_per_km())
    }

    /// Allure prevue a une distance donnee (s/km).
    pub fn pace_at_distance_s_per_km(&self, distance_m: f64) -> f64 {
        let average = self.average_pace_s_per_km();
        let r = self.negative_split.effective_ratio();
        let f = (distance_m / self.distance_m).clamp(0.0, 1.0);
        average * (1.0 + r - 2.0 * r * f)
    }

    /// Temps du shadow runner pour atteindre `distance_m` (s).
    pub fn time_at_distance_s(&self, distance_m: f64) -> f64 {
        let average = self.average_pace_s_per_km();
        let r = self.negative_split.effective_ratio();
        let d = distance_m.clamp(0.0, self.distance_m);
        average * (d / 1000.0) * (1.0 + r - r * d / self.distance_m)
    }

    /// Position du shadow runner apres `elapsed_s` secondes de course (m).
    pub fn distance_at_time_m(&self, elapsed_s: f64) -> f64 {
        if elapsed_s <= 0.0 {
            return 0.0;
        }
        let average = self.average_pace_s_per_km();
        let r = self.negative_split.effective_ratio();
        let k = average / 1000.0; // s par metre
        let b = k * (1.0 + r);
        let a = k * r / self.distance_m;
        if a <= 1e-12 {
            return (elapsed_s / b).min(self.distance_m);
        }
        // a*d^2 - b*d + t = 0  =>  d = (b - sqrt(b^2 - 4*a*t)) / (2a)
        let discriminant = b * b - 4.0 * a * elapsed_s;
        if discriminant <= 0.0 {
            return self.distance_m;
        }
        ((b - discriminant.sqrt()) / (2.0 * a)).clamp(0.0, self.distance_m)
    }

    /// Compare la position reelle a celle du plan.
    pub fn compare(&self, elapsed_s: f64, distance_m: f64) -> ShadowRunnerComparison {
        let planned_distance_m = self.distance_at_time_m(elapsed_s);
        let distance_delta_m = distance_m - planned_distance_m;
        let time_delta_s = self.time_at_distance_s(distance_m) - elapsed_s;
        ShadowRunnerComparison {
            planned_distance_m,
            distance_delta_m,
            time_delta_s,
            ahead: time_delta_s >= 0.0,
            on_plan: ShadowRunnerComparison::is_on_plan(distance_delta_m, time_delta_s),
        }
    }

    /// Projection du temps final si l'allure courante est maintenue.
    ///
    /// `current_pace_s_per_unit` provient de `PaceEngine::current_pace`.
    pub fn predict_finish_time_s(
        &self,
        elapsed_s: f64,
        distance_m: f64,
        current_pace_s_per_unit: Option<f64>,
        units: UnitSystem,
    ) -> Option<f64> {
        let pace = current_pace_s_per_unit?;
        let remaining_m = self.distance_m - distance_m;
        if remaining_m <= 0.0 {
            return Some(elapsed_s);
        }
        let remaining_units = remaining_m / units.meters_per_unit();
        Some(elapsed_s + pace * remaining_units)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exemple du manuel : marathon en 4 h, negative split 3 %.
    #[test]
    fn manual_example_start_5_52_finish_5_30() {
        let plan = RacePlan::new(42_195.0, 4.0 * 3600.0)
            .with_negative_split(NegativeSplit::with_ratio(0.03));
        let start = plan.pace_at_distance_s_per_km(0.0);
        let finish = plan.pace_at_distance_s_per_km(42_195.0);
        assert!(
            (start / 60.0 - 5.85).abs() < 0.02,
            "start = {}",
            start / 60.0
        ); // 5:51
        assert!(
            (finish / 60.0 - 5.50).abs() < 0.02,
            "finish = {}",
            finish / 60.0
        ); // 5:30
        assert!((plan.average_pace_s_per_km() / 60.0 - 5.686).abs() < 0.01); // 5:41
    }

    #[test]
    fn plan_is_exact_at_the_finish_line() {
        for ratio in [0.0, 0.02, 0.04] {
            let split = if ratio == 0.0 {
                NegativeSplit::even_pace()
            } else {
                NegativeSplit::with_ratio(ratio)
            };
            let plan = RacePlan::new(10_000.0, 3000.0).with_negative_split(split);
            let t = plan.time_at_distance_s(10_000.0);
            assert!((t - 3000.0).abs() < 1e-6, "ratio {ratio} => {t}");
        }
    }

    #[test]
    fn time_and_distance_are_inverse_functions() {
        let plan =
            RacePlan::new(21_097.5, 6300.0).with_negative_split(NegativeSplit::with_ratio(0.025));
        for d in [0.0, 1000.0, 5000.0, 10_000.0, 21_097.5] {
            let t = plan.time_at_distance_s(d);
            let back = plan.distance_at_time_m(t);
            assert!((back - d).abs() < 1.0, "d = {d}, back = {back}");
        }
    }

    #[test]
    fn even_pace_plan_is_linear() {
        let plan = RacePlan::new(5000.0, 1500.0); // 5:00/km
        assert!((plan.distance_at_time_m(300.0) - 1000.0).abs() < 0.001);
        assert!((plan.time_at_distance_s(1000.0) - 300.0).abs() < 0.001);
    }

    #[test]
    fn comparison_reports_ahead_and_behind() {
        let plan = RacePlan::new(10_000.0, 3000.0); // 5:00/km
        let ahead = plan.compare(600.0, 2100.0); // 2100 m au lieu de 2000 m
        assert!(ahead.ahead);
        assert!((ahead.distance_delta_m - 100.0).abs() < 1.0);
        assert!((ahead.time_delta_s - 30.0).abs() < 1.0);

        let behind = plan.compare(600.0, 1900.0);
        assert!(!behind.ahead);
        assert!(behind.distance_delta_m < 0.0);

        let on_plan = plan.compare(600.0, 2001.0);
        assert!(on_plan.on_plan);
    }

    #[test]
    fn shadow_runner_never_exceeds_distance() {
        let plan = RacePlan::new(5000.0, 1500.0);
        assert!((plan.distance_at_time_m(10_000.0) - 5000.0).abs() < 1e-9);
    }

    #[test]
    fn finish_time_prediction_uses_current_pace() {
        let plan = RacePlan::new(10_000.0, 3000.0);
        // 4 km en 20 min, allure courante 5:00/km => 6 km restants = 30 min
        let predicted = plan
            .predict_finish_time_s(1200.0, 4000.0, Some(300.0), UnitSystem::Metric)
            .unwrap();
        assert!(
            (predicted - 3000.0).abs() < 0.001,
            "predicted = {predicted}"
        );
    }
}
