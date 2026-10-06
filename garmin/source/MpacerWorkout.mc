//! Machine à états de la séance : démarrage suspendu, pause, reprise, arrêt.
//!
//! Portage de `crates/mpacer-core/src/workout.rs`. Le temps de séance ne
//! s'écoule que dans l'état `:running` : pauses manuelles et automatiques sont
//! exclues de la durée comme de l'allure moyenne.

using Toybox.Math;

class MpacerWorkout {

    // États : :idle :armed :running :auto_paused :paused :finished
    var state;

    var elapsedS;
    var distanceM;

    var lastTickMs;
    var lastLat;
    var lastLon;

    var slowSinceMs;
    var fastSinceMs;

    var autoPause;
    var stopSpeedMps;
    var autoPauseDelayS;
    var resumeSpeedMps;
    var autoResumeDelayS;

    var rejectedDeltas;

    function initialize() {
        self.autoPause = false;
        self.stopSpeedMps = 0.7;      // ~2,5 km/h
        self.autoPauseDelayS = 10.0;
        self.resumeSpeedMps = 1.4;    // ~5 km/h
        self.autoResumeDelayS = 3.0;
        self.rejectedDeltas = 0;
        self.reset();
    }

    function reset() {
        self.state = :idle;
        self.elapsedS = 0.0;
        self.distanceM = 0.0;
        self.lastTickMs = null;
        self.lastLat = null;
        self.lastLon = null;
        self.slowSinceMs = null;
        self.fastSinceMs = null;
    }

    function isTiming() { return self.state == :running; }

    function isPaused() { return self.state == :paused || self.state == :auto_paused; }

    function isActive() {
        return self.state == :armed || self.state == :running
            || self.state == :auto_paused || self.state == :paused;
    }

    function start(tMs) { return self.begin(tMs, false); }

    // Démarrage suspendu : le chrono attend le premier mouvement.
    function arm(tMs) { return self.begin(tMs, true); }

    function begin(tMs, armed) {
        self.elapsedS = 0.0;
        self.distanceM = 0.0;
        self.lastLat = null;
        self.lastLon = null;
        self.slowSinceMs = null;
        self.fastSinceMs = null;
        self.lastTickMs = tMs;
        if (armed) {
            self.state = :armed;
            return [:armed];
        }
        self.state = :running;
        return [:started];
    }

    function pause(tMs) {
        if (self.state != :running && self.state != :armed) {
            return [];
        }
        self.advance(tMs);
        self.state = :paused;
        self.slowSinceMs = null;
        self.fastSinceMs = null;
        return [:paused];
    }

    function resume(tMs) {
        if (!self.isPaused()) {
            return [];
        }
        self.state = :running;
        self.lastTickMs = tMs;
        self.slowSinceMs = null;
        self.fastSinceMs = null;
        return [:resumed];
    }

    function stop(tMs) {
        if (!self.isActive()) {
            return [];
        }
        self.advance(tMs);
        self.state = :finished;
        return [:stopped];
    }

    function tick(tMs) {
        self.advance(tMs);
    }

    function advance(tMs) {
        if (self.lastTickMs != null) {
            if (tMs > self.lastTickMs && self.isTiming()) {
                self.elapsedS = self.elapsedS + (tMs - self.lastTickMs) / 1000.0;
            }
            if (tMs > self.lastTickMs) {
                self.lastTickMs = tMs;
            }
        } else {
            self.lastTickMs = tMs;
        }
    }

    // Position validée par le filtre GPS : seule source de distance.
    function onSample(lat, lon) {
        if (self.lastLat != null) {
            var delta = MpacerGeo.haversineM(self.lastLat, self.lastLon, lat, lon);
            if (self.state == :running) {
                self.distanceM = self.distanceM + delta;
            } else if (self.state == :armed) {
                self.state = :running;   // premier mouvement : le chrono démarre
                self.distanceM = self.distanceM + delta;
            }
        }
        self.lastLat = lat;
        self.lastLon = lon;
    }

    // Pause / reprise automatiques à partir de la vitesse lissée.
    function updateMotion(tMs, speedMps) {
        if (!self.autoPause) {
            return [];
        }
        var speed = 0.0;
        if (speedMps != null) {
            speed = speedMps;
        }
        var events = [];
        if (self.state == :running) {
            if (speed < self.stopSpeedMps) {
                if (self.slowSinceMs == null) {
                    self.slowSinceMs = tMs;
                }
                if (tMs - self.slowSinceMs >= (self.autoPauseDelayS * 1000.0).toNumber()) {
                    self.state = :auto_paused;
                    self.slowSinceMs = null;
                    self.lastTickMs = tMs;
                    events.add(:auto_paused);
                }
            } else {
                self.slowSinceMs = null;
            }
        } else if (self.state == :auto_paused || self.state == :armed) {
            var required = self.autoResumeDelayS;
            if (self.state == :armed) {
                required = 0.0;
            }
            if (speed >= self.resumeSpeedMps) {
                if (self.fastSinceMs == null) {
                    self.fastSinceMs = tMs;
                }
                if (tMs - self.fastSinceMs >= (required * 1000.0).toNumber()) {
                    var wasAuto = self.state == :auto_paused;
                    self.state = :running;
                    self.fastSinceMs = null;
                    self.slowSinceMs = null;
                    self.lastTickMs = tMs;
                    if (wasAuto) {
                        events.add(:auto_resumed);
                    } else {
                        events.add(:started);
                    }
                }
            } else {
                self.fastSinceMs = null;
            }
        }
        return events;
    }
}
