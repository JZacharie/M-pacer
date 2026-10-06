//! Qualité du signal GPS et filtre d'entrée.
//!
//! Portage de `crates/mpacer-core/src/gps.rs`. Le shell Garmin traduit
//! `Position.Info.accuracy` (une qualité, pas des mètres) en une précision
//! estimée avant d'appeler `push` : le cœur, lui, ne connaît que des mètres.

class MpacerGpsMonitor {

    // États, sur le même modèle que l'énumération Rust `GpsStatus`.
    //   :disabled  GPS coupé
    //   :acquiring aucune position exploitable
    //   :poor      position disponible mais imprécise
    //   :good      signal bon
    var status;

    var enabled;
    var lastTMs;
    var lastLat;
    var lastLon;
    var lastAccuracyM;
    var consecutiveGood;
    var rejectedSamples;

    var goodAccuracyM;
    var poorAccuracyM;
    var goodSamplesRequired;
    var maxSpeedMps;

    function initialize() {
        self.goodAccuracyM = 10.0;
        self.poorAccuracyM = 25.0;
        self.goodSamplesRequired = 3;
        self.maxSpeedMps = 12.0;
        self.enabled = true;
        self.status = :acquiring;
        self.lastAccuracyM = null;
        self.consecutiveGood = 0;
        self.rejectedSamples = 0;
    }

    function setEnabled(value) {
        self.enabled = value;
        if (!value) {
            self.status = :disabled;
            self.lastAccuracyM = null;
            self.consecutiveGood = 0;
        } else {
            self.status = :acquiring;
        }
    }

    // Le voyant : même correspondance que `GpsStatus::light` (rouge / orange / jaune / vert).
    function light() {
        if (self.status == :disabled) { return :red; }
        if (self.status == :acquiring) { return :orange; }
        if (self.status == :poor) { return :yellow; }
        return :green;
    }

    function canStart() {
        return self.status == :good || self.status == :poor;
    }

    // Injecte un échantillon ; renvoie [statut, accepté].
    function push(tMs, lat, lon, accuracyM) {
        if (!self.enabled) {
            return [:disabled, false];
        }
        // Précision pire que le double du seuil « poor » : mesure jetée.
        if (accuracyM > self.poorAccuracyM * 2.0) {
            self.rejectedSamples = self.rejectedSamples + 1;
            return [self.status, false];
        }
        if (self.lastAccuracyM != null) {
            var dtS = (tMs - self.lastTMs) / 1000.0;
            var moved = MpacerGeo.haversineM(self.lastLat, self.lastLon, lat, lon);
            var plausible;
            if (dtS > 0.0) {
                plausible = moved <= self.maxSpeedMps * dtS + accuracyM + self.lastAccuracyM;
            } else {
                plausible = moved <= accuracyM * 2.0;
            }
            if (!plausible) {
                self.rejectedSamples = self.rejectedSamples + 1;
                return [self.status, false];
            }
        }
        self.lastTMs = tMs;
        self.lastLat = lat;
        self.lastLon = lon;
        self.lastAccuracyM = accuracyM;
        if (accuracyM <= self.goodAccuracyM) {
            self.consecutiveGood = self.consecutiveGood + 1;
            if (self.consecutiveGood >= self.goodSamplesRequired) {
                self.status = :good;
            } else {
                self.status = :poor;
            }
        } else {
            self.consecutiveGood = 0;
            self.status = :poor;
        }
        return [self.status, true];
    }
}
