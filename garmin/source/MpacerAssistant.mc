//! Assistant de course : les modes de Pace Control, sans le mode « course à
//! distance » (il suppose un serveur et un adversaire en ligne : hors périmètre
//! de la montre Garmin dans cette version).
//!
//! Portage de `crates/mpacer-core/src/assistant.rs`.

class MpacerAssistant {

    // Modes : :track_pace :predict_finish :planned_time
    var mode;
    var raceDistanceM;
    var plannedTimeS;
    var negativeSplit;
    var negativeSplitRatio;
    var plan;

    function initialize() {
        self.mode = :track_pace;
        self.raceDistanceM = null;
        self.plannedTimeS = null;
        self.negativeSplit = false;
        self.negativeSplitRatio = 0.03;
        self.plan = null;
    }

    function configure(mode, raceDistanceM, plannedTimeS, negativeSplit, negativeSplitRatio) {
        self.mode = mode;
        self.raceDistanceM = raceDistanceM;
        self.plannedTimeS = plannedTimeS;
        self.negativeSplit = negativeSplit;
        self.negativeSplitRatio = negativeSplitRatio;
        self.rebuildPlan();
    }

    function rebuildPlan() {
        self.plan = null;
        if (self.mode == :planned_time) {
            if (self.raceDistanceM != null && self.raceDistanceM > 0.0
                && self.plannedTimeS != null && self.plannedTimeS > 0.0) {
                var racePlan = new MpacerRacePlan(self.raceDistanceM, self.plannedTimeS);
                racePlan.setNegativeSplit(self.negativeSplit, self.negativeSplitRatio);
                self.plan = racePlan;
            }
        } else if (self.mode == :predict_finish) {
            if (self.raceDistanceM != null && self.raceDistanceM > 0.0) {
                self.plan = new MpacerRacePlan(self.raceDistanceM, 0.0);
            }
        }
    }

    function showsPanel() {
        return self.mode != :track_pace;
    }

    // Configuration incomplète pour le mode courant.
    function issue() {
        if (self.mode == :track_pace) {
            return null;
        }
        if (self.raceDistanceM == null || self.raceDistanceM <= 0.0) {
            return :missing_distance;
        }
        if (self.mode == :planned_time && (self.plannedTimeS == null || self.plannedTimeS <= 0.0)) {
            return :missing_time;
        }
        return null;
    }

    function raceDistanceOrNull() {
        if (self.raceDistanceM != null) {
            return self.raceDistanceM;
        }
        if (self.plan != null) {
            return self.plan.distanceM;
        }
        return null;
    }

    // Panneau affiché sous les métriques principales.
    function update(elapsedS, distanceM, currentPaceSPerUnit, imperial) {
        var panel = {
            :mode => self.mode,
            :visible => false,
            :estimated_finish_s => null,
            :shadow => null,
            :remaining_m => null,
            :issue => self.issue()
        };
        if (self.plan == null) {
            return panel;
        }
        panel[:visible] = self.showsPanel();
        var remaining = self.plan.distanceM - distanceM;
        if (remaining < 0.0) { remaining = 0.0; }
        panel[:remaining_m] = remaining;

        if (self.mode == :predict_finish) {
            var predicted = null;
            if (self.plan.targetTimeS > 0.0) {
                predicted = self.plan.predictFinishTimeS(elapsedS, distanceM, currentPaceSPerUnit, imperial);
            } else if (currentPaceSPerUnit != null) {
                var units = remaining / MpacerUnits.metersPerUnit(imperial);
                predicted = elapsedS + currentPaceSPerUnit * units;
            }
            panel[:estimated_finish_s] = predicted;
        } else if (self.mode == :planned_time) {
            panel[:shadow] = self.plan.compare(elapsedS, distanceM);
            panel[:estimated_finish_s] = self.plan.predictFinishTimeS(elapsedS, distanceM, currentPaceSPerUnit, imperial);
        }
        return panel;
    }
}
