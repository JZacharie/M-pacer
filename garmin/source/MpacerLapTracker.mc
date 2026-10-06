//! Tours automatiques (kilomètre ou mile) et allures de tour.
//!
//! Portage de `crates/mpacer-core/src/lap.rs`. Le temps utilisé est le temps de
//! course (pauses exclues) : une pause ne rallonge jamais la durée d'un tour.

class MpacerLapTracker {

    var lapLengthM;
    var laps;
    var lapStartDistM;
    var lapStartElapsedS;

    function initialize() {
        self.lapLengthM = MpacerUnits.metersPerKm();
        self.laps = [];
        self.lapStartDistM = 0.0;
        self.lapStartElapsedS = 0.0;
    }

    function setUnits(imperial) {
        self.lapLengthM = MpacerUnits.metersPerUnit(imperial);
    }

    function start(elapsedS) {
        self.laps = [];
        self.lapStartDistM = 0.0;
        self.lapStartElapsedS = elapsedS;
    }

    function lapCount() {
        return self.laps.size();
    }

    function previousLapPaceSPerKm() {
        if (self.laps.size() == 0) {
            return null;
        }
        return self.laps[self.laps.size() - 1][:pace_s_per_km];
    }

    function currentLapDistanceM(totalDistM) {
        var distance = totalDistM - self.lapStartDistM;
        if (distance < 0.0) {
            return 0.0;
        }
        return distance;
    }

    // Allure du tour courant (tronçon partiel) : rien avant 10 s ou 30 m.
    function currentLapPace(imperial, elapsedS, totalDistM) {
        var dt = elapsedS - self.lapStartElapsedS;
        var dd = self.currentLapDistanceM(totalDistM);
        if (dt < 10.0 || dd < 30.0) {
            return null;
        }
        return MpacerUnits.paceFromSpeed(dd / dt, imperial);
    }

    // Tour manuel (bouton LAP) : clôt le tronçon courant, quelle que soit sa
    // distance, et repart de la position atteinte.
    function manualLap(elapsedS, totalDistM) {
        var dd = self.currentLapDistanceM(totalDistM);
        if (dd < 10.0) {
            return null;
        }
        var dt = elapsedS - self.lapStartElapsedS;
        if (dt < 0.0) {
            dt = 0.0;
        }
        var paceSPerKm = 0.0;
        if (dt > 0.0) {
            paceSPerKm = dt / (dd / 1000.0);
        }
        var lap = {
            :index => self.laps.size() + 1,
            :distance_m => dd,
            :duration_s => dt,
            :pace_s_per_km => paceSPerKm,
            :manual => true
        };
        self.laps.add(lap);
        self.lapStartDistM = totalDistM;
        self.lapStartElapsedS = elapsedS;
        return lap;
    }

    // Avance la distance et renvoie les tours franchis depuis le dernier appel.
    function update(elapsedS, totalDistM) {
        var completed = [];
        while (totalDistM - self.lapStartDistM >= self.lapLengthM) {
            var boundary = self.lapStartDistM + self.lapLengthM;
            var span = totalDistM - self.lapStartDistM;
            var fraction = 1.0;
            if (span > 0.0) {
                fraction = self.lapLengthM / span;
            }
            var boundaryS = self.lapStartElapsedS + (elapsedS - self.lapStartElapsedS) * fraction;
            var durationS = boundaryS - self.lapStartElapsedS;
            var paceSPerKm = 0.0;
            if (durationS > 0.0) {
                paceSPerKm = durationS / (self.lapLengthM / 1000.0);
            }
            var lap = {
                :index => self.laps.size() + 1,
                :distance_m => self.lapLengthM,
                :duration_s => durationS,
                :pace_s_per_km => paceSPerKm
            };
            self.laps.add(lap);
            completed.add(lap);
            self.lapStartDistM = boundary;
            self.lapStartElapsedS = boundaryS;
        }
        return completed;
    }
}