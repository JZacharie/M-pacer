//! Plan de course, negative split et « shadow runner ».
//!
//! Portage de `crates/mpacer-core/src/race_plan.rs` :
//!
//!   a(f) = A * (1 + r - 2*r*f)              allure à la fraction f = d/D
//!   T(d) = A * (d/1000) * (1 + r - r*d/D)   temps du shadow runner à d
//!   T(D) = A * D/1000 = temps visé          plan exact à l'arrivée

using Toybox.Math;

class MpacerRacePlan {

    var distanceM;
    var targetTimeS;
    var negativeSplitEnabled;
    var negativeSplitRatio;

    function initialize(distanceM, targetTimeS) {
        self.distanceM = distanceM;
        self.targetTimeS = targetTimeS;
        self.negativeSplitEnabled = false;
        self.negativeSplitRatio = 0.03;
    }

    function setNegativeSplit(enabled, ratio) {
        self.negativeSplitEnabled = enabled;
        self.negativeSplitRatio = ratio;
    }

    function effectiveRatio() {
        if (!self.negativeSplitEnabled) {
            return 0.0;
        }
        var ratio = self.negativeSplitRatio;
        if (ratio < 0.0) { ratio = 0.0; }
        if (ratio > 0.20) { ratio = 0.20; }
        return ratio;
    }

    function isValid() {
        return self.distanceM > 0.0 && self.targetTimeS > 0.0;
    }

    function averagePaceSPerKm() {
        return self.targetTimeS / (self.distanceM / 1000.0);
    }

    function averagePace(imperial) {
        return MpacerUnits.metersPerUnit(imperial) / (1000.0 / self.averagePaceSPerKm());
    }

    function paceAtDistanceSPerKm(distanceM) {
        var fraction = distanceM / self.distanceM;
        if (fraction < 0.0) { fraction = 0.0; }
        if (fraction > 1.0) { fraction = 1.0; }
        var r = self.effectiveRatio();
        return self.averagePaceSPerKm() * (1.0 + r - 2.0 * r * fraction);
    }

    function timeAtDistanceS(distanceM) {
        var d = distanceM;
        if (d < 0.0) { d = 0.0; }
        if (d > self.distanceM) { d = self.distanceM; }
        var r = self.effectiveRatio();
        return self.averagePaceSPerKm() * (d / 1000.0) * (1.0 + r - r * d / self.distanceM);
    }

    // Position du shadow runner après `elapsedS` secondes de course.
    function distanceAtTimeM(elapsedS) {
        if (elapsedS <= 0.0) {
            return 0.0;
        }
        var r = self.effectiveRatio();
        var k = self.averagePaceSPerKm() / 1000.0;   // s par mètre
        var b = k * (1.0 + r);
        var a = k * r / self.distanceM;
        if (a <= 0.000000000001) {
            var value = elapsedS / b;
            if (value > self.distanceM) { return self.distanceM; }
            return value;
        }
        // a*d^2 - b*d + t = 0  =>  d = (b - sqrt(b^2 - 4*a*t)) / (2a)
        var discriminant = b * b - 4.0 * a * elapsedS;
        if (discriminant <= 0.0) {
            return self.distanceM;
        }
        var d = (b - Math.sqrt(discriminant)) / (2.0 * a);
        if (d < 0.0) { d = 0.0; }
        if (d > self.distanceM) { d = self.distanceM; }
        return d;
    }

    // Écart au plan : positif = en avance.
    function compare(elapsedS, distanceM) {
        var planned = self.distanceAtTimeM(elapsedS);
        var distanceDeltaM = distanceM - planned;
        var timeDeltaS = self.timeAtDistanceS(distanceM) - elapsedS;
        var onPlan = (distanceDeltaM).abs() < 5.0 && (timeDeltaS).abs() < 2.0;
        return {
            :planned_distance_m => planned,
            :distance_delta_m => distanceDeltaM,
            :time_delta_s => timeDeltaS,
            :ahead => timeDeltaS >= 0.0,
            :on_plan => onPlan
        };
    }

    // Projection du temps final si l'allure courante est maintenue.
    function predictFinishTimeS(elapsedS, distanceM, currentPaceSPerUnit, imperial) {
        if (currentPaceSPerUnit == null) {
            return null;
        }
        var remainingM = self.distanceM - distanceM;
        if (remainingM <= 0.0) {
            return elapsedS;
        }
        var remainingUnits = remainingM / MpacerUnits.metersPerUnit(imperial);
        return elapsedS + currentPaceSPerUnit * remainingUnits;
    }
}