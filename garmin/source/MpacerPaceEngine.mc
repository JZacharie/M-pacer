//! Moteur d'allure : moyenne glissante de 2 minutes et détection de changement.
//!
//! Portage de `crates/mpacer-core/src/pace.rs`. Un échantillon est un couple
//! `[t_ms, distance_cumulée_m]` conservé dans un tableau : sur une montre, un
//! dictionnaire par point coûterait beaucoup trop de mémoire.

using Toybox.Math;

class MpacerPaceEngine {

    var samples;
    var windowS;
    var probeS;
    var minProbeDistanceM;
    var changeThreshold;
    var maxPaceSPerKm;
    var detectPaceChange;
    var segmentStartMs;
    var lastChangeMs;

    function initialize() {
        self.samples = [];
        self.windowS = 120.0;      // fenêtre de moyennage, comme Pace Control
        self.probeS = 20.0;        // fenêtres comparées par la détection
        self.minProbeDistanceM = 25.0;
        self.changeThreshold = 0.15;
        self.maxPaceSPerKm = 1800.0;
        self.detectPaceChange = false;
        self.segmentStartMs = null;
        self.lastChangeMs = null;
    }

    function setDetectPaceChange(enabled) {
        self.detectPaceChange = enabled;
    }

    function clear() {
        self.samples = [];
        self.segmentStartMs = null;
        self.lastChangeMs = null;
    }

    function sampleCount() {
        return self.samples.size();
    }

    // Ajoute un point (temps absolu en ms, distance cumulée en m).
    function push(tMs, distM) {
        if (self.samples.size() > 0) {
            var last = self.samples[self.samples.size() - 1];
            if (tMs <= last[0]) {
                return; // série non monotone : ignorée, comme le cœur Rust
            }
        }
        self.samples.add([tMs, distM]);
        if (self.segmentStartMs == null) {
            self.segmentStartMs = tMs;
        }
        self.trim(tMs);
        if (self.detectPaceChange) {
            self.maybeDetectChange(tMs);
        }
        self.trim(tMs);
    }

    // Remise à zéro manuelle de la fenêtre (équivalent du triple clic Wear OS).
    function resetWindow(tMs) {
        self.segmentStartMs = tMs;
        self.lastChangeMs = tMs;
    }

    function currentSpeedMps() {
        if (self.samples.size() == 0) {
            return null;
        }
        var last = self.samples[self.samples.size() - 1];
        var windowStart = last[0] - (self.windowS * 1000.0).toNumber();
        var startMs = windowStart;
        if (self.segmentStartMs != null && self.segmentStartMs > windowStart) {
            startMs = self.segmentStartMs;
        }
        return self.speedBetween(startMs, last[0]);
    }

    // Allure moyennée, dans le système d'unités demandé (s / unité).
    function currentPace(imperial) {
        var speed = self.currentSpeedMps();
        if (speed == null) {
            return null;
        }
        if (1000.0 / speed > self.maxPaceSPerKm) {
            return null;
        }
        return MpacerUnits.paceFromSpeed(speed, imperial);
    }

    // Vitesse sur les 20 dernières secondes (auto-pause, détection).
    function shortSpeedMps() {
        if (self.samples.size() == 0) {
            return null;
        }
        var last = self.samples[self.samples.size() - 1];
        return self.speedBetween(last[0] - (self.probeS * 1000.0).toNumber(), last[0]);
    }

    function speedBetween(fromMs, toMs) {
        if (self.samples.size() < 2 || toMs <= fromMs) {
            return null;
        }
        var from = null;
        for (var i = 0; i < self.samples.size(); i++) {
            if (self.samples[i][0] >= fromMs) {
                from = self.samples[i];
                break;
            }
        }
        if (from == null) {
            return null;
        }
        var last = self.samples[self.samples.size() - 1];
        var dt = (last[0] - from[0]) / 1000.0;
        var dd = last[1] - from[1];
        if (dt < 5.0 || dd < 5.0) {
            return null;
        }
        return dd / dt;
    }

    function maybeDetectChange(tMs) {
        var probeMs = (self.probeS * 1000.0).toNumber();
        if (self.lastChangeMs != null && tMs - self.lastChangeMs < probeMs) {
            return;
        }
        var recent = self.speedBetween(tMs - probeMs, tMs);
        var previous = self.speedBetween(tMs - 2 * probeMs, tMs - probeMs);
        if (recent == null || previous == null) {
            return;
        }
        var reference = recent;
        if (previous > recent) {
            reference = previous;
        }
        var delta = (recent - previous).abs();
        if (reference > 0.0 && delta / reference >= self.changeThreshold) {
            // La fenêtre redémarre au début de la sonde : c'est ce qui rend le
            // fractionné lisible (l'allure affichée suit le nouveau rythme).
            self.segmentStartMs = tMs - probeMs;
            self.lastChangeMs = tMs;
        }
    }

    function trim(nowMs) {
        var horizonMs = ((self.windowS + 2.0 * self.probeS + 30.0) * 1000.0).toNumber();
        var cutoff = nowMs - horizonMs;
        var drop = 0;
        while (drop < self.samples.size() && self.samples[drop][0] < cutoff) {
            drop = drop + 1;
        }
        if (drop > 0) {
            self.samples = self.samples.slice(drop);
        }
    }
}