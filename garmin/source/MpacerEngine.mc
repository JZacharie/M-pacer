//! Orchestrateur : relie GPS, allure, tours, séance, assistant et alertes.
//!
//! Portage de `crates/mpacer-core/src/engine.rs`. C'est le seul objet que le
//! shell Garmin manipule : il reçoit des positions et des commandes, et rend un
//! état complet prêt à afficher.

using Toybox.System;
using Toybox.Time;
using Toybox.Math;

class MpacerEngine {

    var settings;

    var gps;
    var pace;
    var laps;
    var workout;
    var assistant;
    var coach;
    var cardio;
    var track;

    var heartRate;        // [[t_ms, bpm], ...] sous-échantillonné à 5 s
    var pauses;
    var openPause;        // {:at_s, :at_distance_m, :automatic, :started_t_ms}

    var startedAtMs;      // époque en ms (Long)
    var startedTimerMs;   // System.getTimer() au départ
    var lastTMs;
    var lastTrackMs;
    var lastHeartRateMs;
    var lastLap;
    var output;

    function initialize(settings) {
        self.settings = settings;
        self.gps = new MpacerGpsMonitor();
        self.pace = new MpacerPaceEngine();
        self.laps = new MpacerLapTracker();
        self.workout = new MpacerWorkout();
        self.assistant = new MpacerAssistant();
        self.coach = new MpacerCoach();
        self.cardio = new MpacerCardio(settings.maxHeartRate);
        self.track = new MpacerTrack(4000);
        self.heartRate = [];
        self.pauses = [];
        self.openPause = null;
        self.startedAtMs = null;
        self.startedTimerMs = null;
        self.lastTMs = 0;
        self.lastTrackMs = null;
        self.lastHeartRateMs = null;
        self.lastLap = null;
        self.output = null;
        self.applySettings(settings);
    }

    function applySettings(settings) {
        self.settings = settings;
        self.pace.setDetectPaceChange(settings.detectPaceChange);
        self.laps.setUnits(settings.imperial);
        self.workout.autoPause = settings.autoPause;
        self.cardio.maxBpm = settings.maxHeartRate;
        self.assistant.configure(
            self.assistantMode(),
            settings.plannedDistanceM,
            settings.plannedTimeS,
            settings.negativeSplit,
            settings.negativeSplitRatio
        );
        self.coach.configure(
            settings.alertsEnabled,
            self.alertFrequency(),
            settings.imperial,
            settings.extendedLapInfo
        );
    }

    function assistantMode() {
        if (self.settings.assistantMode == "predict_finish") {
            return :predict_finish;
        }
        if (self.settings.assistantMode == "planned_time") {
            return :planned_time;
        }
        return :track_pace;
    }

    function alertFrequency() {
        var value = self.settings.alertFrequency;
        if (value == "off") { return :off; }
        if (value == "1min") { return :every_1min; }
        if (value == "5min") { return :every_5min; }
        if (value == "lap") { return :lap; }
        return :every_2min;
    }

    // ------------------------------------------------------------------ commandes

    function start(tMs, armed) {
        var events;
        if (armed) {
            events = self.workout.arm(tMs);
        } else {
            events = self.workout.start(tMs);
        }
        self.beginRecording(tMs);
        return self.step(tMs, events, false);
    }

    function beginRecording(tMs) {
        self.laps.start(0.0);
        self.track.clear();
        self.pace.clear();
        self.coach.reset(tMs);
        self.heartRate = [];
        self.pauses = [];
        self.openPause = null;
        self.lastLap = null;
        self.startedTimerMs = tMs;
        self.lastTMs = tMs;
        self.lastTrackMs = null;
        self.lastHeartRateMs = null;
        self.startedAtMs = Time.now().value().toLong() * 1000l;
    }

    function pause(tMs) { return self.step(tMs, self.workout.pause(tMs), false); }

    function resume(tMs) { return self.step(tMs, self.workout.resume(tMs), false); }

    function stop(tMs) { return self.step(tMs, self.workout.stop(tMs), false); }

    function reset() {
        self.gps = new MpacerGpsMonitor();
        self.pace.clear();
        self.workout.reset();
        self.coach.reset(0);
        self.track.clear();
        self.heartRate = [];
        self.pauses = [];
        self.openPause = null;
        self.startedAtMs = null;
        self.startedTimerMs = null;
        self.lastTMs = 0;
        self.lastTrackMs = null;
        self.lastHeartRateMs = null;
        self.lastLap = null;
        self.output = null;
    }

    // Triple clic (bouton bas) : remise à zéro de la fenêtre d'allure.
    function resetPaceWindow(tMs) {
        self.pace.resetWindow(tMs);
        return self.tick(tMs);
    }

    // Tour manuel (bouton LAP) : le tour est aussi ajouté au fichier FIT.
    function manualLap(tMs) {
        var lap = self.laps.manualLap(self.workout.elapsedS, self.workout.distanceM);
        if (lap == null) {
            return self.tick(tMs);
        }
        self.lastLap = lap;
        var alerts = self.coach.onLap(lap, self.snapshot(lap));
        var panel = self.currentPanel();
        self.output = self.buildOutput([], alerts, lap, panel);
        return self.output;
    }

    // Annonce immédiate (bouton haut) : état courant, sans attendre la période.
    function announceNow(tMs) {
        var alerts = self.coach.manualStatus(self.snapshot(null));
        var panel = self.currentPanel();
        self.output = self.buildOutput([], alerts, null, panel);
        return self.output;
    }

    // ------------------------------------------------------------------ capteurs

    function onGps(tMs, lat, lon, accuracyM, altitudeM) {
        var result = self.gps.push(tMs, lat, lon, accuracyM);
        var accepted = result[1];
        if (accepted) {
            self.workout.onSample(lat, lon);
            if (self.workout.state == :running) {
                self.recordTrack(tMs, lat, lon, altitudeM);
            }
        }
        // Comme le cœur Rust : un rafraîchissement GPS a toujours lieu, même si
        // l'échantillon a été rejeté (l'écran et les alertes restent à jour).
        return self.step(tMs, [], true);
    }

    function recordTrack(tMs, lat, lon, altitudeM) {
        var intervalMs = (self.settings.traceIntervalS * 1000.0).toNumber();
        if (intervalMs < 1000) {
            intervalMs = 1000;
        }
        if (self.lastTrackMs != null && tMs - self.lastTrackMs < intervalMs) {
            return;
        }
        if (self.track.push(tMs, self.workout.distanceM, lat, lon, altitudeM)) {
            self.lastTrackMs = tMs;
        }
    }

    function onHeartRate(tMs, bpm) {
        if (bpm == null || bpm < 20 || bpm > 250) {
            return;
        }
        if (self.lastHeartRateMs != null && tMs - self.lastHeartRateMs < 5000) {
            return;
        }
        self.lastHeartRateMs = tMs;
        self.heartRate.add([tMs, bpm]);
    }

    function currentHeartRate() {
        if (self.heartRate.size() == 0) {
            return null;
        }
        return self.heartRate[self.heartRate.size() - 1][1];
    }

    function tick(tMs) {
        return self.step(tMs, [], false);
    }

    // ------------------------------------------------------------------ moteur

    function step(tMs, events, freshGps) {
        self.lastTMs = tMs;
        self.workout.tick(tMs);

        if (freshGps) {
            var speed = self.pace.shortSpeedMps();
            var motion = self.workout.updateMotion(tMs, speed);
            for (var i = 0; i < motion.size(); i++) {
                events.add(motion[i]);
            }
            if (self.workout.state == :running) {
                self.pace.push(tMs, self.workout.distanceM);
            }
        }

        self.recordPauses(tMs, events);

        var lapCompleted = null;
        if (self.workout.isActive()) {
            var completed = self.laps.update(self.workout.elapsedS, self.workout.distanceM);
            if (completed.size() > 0) {
                lapCompleted = completed[completed.size() - 1];
                self.lastLap = lapCompleted;
            }
        }

        var panel = self.currentPanel();
        var snapshot = self.snapshot(lapCompleted);

        var alerts = [];
        for (var e = 0; e < events.size(); e++) {
            var eventAlerts = self.coach.onEvent(events[e], tMs, snapshot);
            for (var a = 0; a < eventAlerts.size(); a++) {
                alerts.add(eventAlerts[a]);
            }
        }
        if (lapCompleted != null) {
            var lapAlerts = self.coach.onLap(lapCompleted, snapshot);
            for (var l = 0; l < lapAlerts.size(); l++) {
                alerts.add(lapAlerts[l]);
            }
        }
        if (self.workout.state == :running) {
            var tickAlerts = self.coach.onTick(tMs, snapshot);
            for (var t = 0; t < tickAlerts.size(); t++) {
                alerts.add(tickAlerts[t]);
            }
        }

        // Les tours franchis alimentent aussi la montre : le FIT contient alors
        // les mêmes tours que l'écran.
        self.output = self.buildOutput(events, alerts, lapCompleted, panel);
        return self.output;
    }

    function recordPauses(tMs, events) {
        for (var i = 0; i < events.size(); i++) {
            var event = events[i];
            if (event == :paused || event == :auto_paused) {
                if (self.openPause == null) {
                    self.openPause = {
                        :at_s => self.workout.elapsedS,
                        :at_distance_m => self.workout.distanceM,
                        :automatic => event == :auto_paused,
                        :started_t_ms => tMs
                    };
                }
            } else if (event == :resumed || event == :auto_resumed || event == :stopped) {
                self.closePause(tMs);
            }
        }
    }

    function closePause(tMs) {
        if (self.openPause == null) {
            return;
        }
        var durationS = (tMs - self.openPause[:started_t_ms]) / 1000.0;
        if (durationS > 0.0) {
            self.pauses.add({
                :at_s => self.openPause[:at_s],
                :at_distance_m => self.openPause[:at_distance_m],
                :duration_s => durationS,
                :automatic => self.openPause[:automatic]
            });
        }
        self.openPause = null;
    }

    function currentPanel() {
        return self.assistant.update(
            self.workout.elapsedS,
            self.workout.distanceM,
            self.pace.currentPace(self.settings.imperial),
            self.settings.imperial
        );
    }

    function snapshot(lap) {
        var panel = self.assistant.update(
            self.workout.elapsedS,
            self.workout.distanceM,
            self.pace.currentPace(self.settings.imperial),
            self.settings.imperial
        );
        return {
            :distance_m => self.workout.distanceM,
            :elapsed_s => self.workout.elapsedS,
            :current_pace => self.pace.currentPace(self.settings.imperial),
            :lap => lap,
            :shadow => panel[:shadow]
        };
    }

    function buildOutput(events, alerts, lapCompleted, panel) {
        var heartRate = self.currentHeartRate();
        var zone = 0;
        if (heartRate != null) {
            zone = self.cardio.zoneOf(heartRate);
        }
        return {
            :gps_status => self.gps.status,
            :light => self.gps.light(),
            :state => self.workout.state,
            :elapsed_s => self.workout.elapsedS,
            :distance_m => self.workout.distanceM,
            :current_pace => self.pace.currentPace(self.settings.imperial),
            :current_lap_pace => self.laps.currentLapPace(
                self.settings.imperial, self.workout.elapsedS, self.workout.distanceM),
            :previous_lap_pace => self.laps.previousLapPaceSPerKm(),
            :current_lap_distance_m => self.laps.currentLapDistanceM(self.workout.distanceM),
            :speed_mps => self.pace.currentSpeedMps(),
            :panel => panel,
            :lap_completed => lapCompleted,
            :last_lap => self.lastLap,
            :events => events,
            :alerts => alerts,
            :heart_rate_bpm => heartRate,
            :heart_rate_zone => zone
        };
    }

    // ------------------------------------------------------------- résumé final

    function elapsedWallS() {
        if (self.startedTimerMs == null || self.lastTMs <= self.startedTimerMs) {
            return self.workout.elapsedS;
        }
        return (self.lastTMs - self.startedTimerMs) / 1000.0;
    }

    function summary() {
        var distanceM = self.workout.distanceM;
        var durationS = self.workout.elapsedS;
        var averagePace = 0.0;
        if (distanceM > 0.0) {
            averagePace = durationS / (distanceM / 1000.0);
        }
        var startedAt = self.startedAtMs;
        if (startedAt == null) {
            startedAt = Time.now().value().toLong() * 1000l;
        }
        var pauses = [];
        for (var i = 0; i < self.pauses.size(); i++) {
            pauses.add(self.pauses[i]);
        }
        // Une pause encore ouverte compte aussi (séance arrêtée sans reprise).
        if (self.openPause != null) {
            var duration = (self.lastTMs - self.openPause[:started_t_ms]) / 1000.0;
            if (duration > 0.0) {
                pauses.add({
                    :at_s => self.openPause[:at_s],
                    :at_distance_m => self.openPause[:at_distance_m],
                    :duration_s => duration,
                    :automatic => self.openPause[:automatic]
                });
            }
        }
        return {
            :id => startedAt.toString(),
            :started_at_ms => startedAt,
            :duration_s => durationS,
            :distance_m => distanceM,
            :average_pace_s_per_km => averagePace,
            :laps => self.laps.laps,
            :best_efforts => self.track.bestEfforts(self.standardDistances()),
            :track => self.payloadTrack(),
            :unit_system => self.unitSystemName(),
            :elapsed_s => self.elapsedWallS(),
            :pauses => pauses,
            :heart_rate => self.payloadHeartRate(),
            :plan => self.payloadPlan()
        };
    }

    function unitSystemName() {
        if (self.settings.imperial) {
            return "Imperial";
        }
        return "Metric";
    }

    // Distances de référence : 1 km, 1 mi, 5 km, 5 mi, 10 km, semi.
    function standardDistances() {
        return [
            [1000.0, "1 km"],
            [1609.344, "1 mi"],
            [5000.0, "5 km"],
            [8046.72, "5 mi"],
            [10000.0, "10 km"],
            [21097.5, "Half marathon"]
        ];
    }

    // Trace envoyée au backend : bornée (le BLE plafonne la taille des requêtes)
    // et sous-échantillonnée selon le réglage « traceIntervalS ».
    function payloadTrack() {
        var points = [];
        if (!self.settings.syncTrace || self.track.size() == 0) {
            return points;
        }
        var count = self.track.size();
        var maximum = 300;
        var step = 1;
        if (count > maximum) {
            step = (count / maximum).toNumber();
            if (step < 1) {
                step = 1;
            }
        }
        var index = 0;
        while (index < count) {
            var altitude = self.track.alts[index];
            points.add({
                :t_ms => self.track.ts[index],
                :dist_m => self.track.dists[index],
                :lat => self.track.lats[index],
                :lon => self.track.lons[index],
                :elevation_m => altitude
            });
            index = index + step;
        }
        return points;
    }

    function payloadHeartRate() {
        var samples = [];
        var maximum = 600;
        var step = 1;
        if (self.heartRate.size() > maximum) {
            step = (self.heartRate.size() / maximum).toNumber();
            if (step < 1) {
                step = 1;
            }
        }
        var index = 0;
        while (index < self.heartRate.size()) {
            samples.add({
                :t_ms => self.heartRate[index][0],
                :bpm => self.heartRate[index][1]
            });
            index = index + step;
        }
        return samples;
    }

    function payloadPlan() {
        if (self.assistant.plan == null) {
            return null;
        }
        var plan = self.assistant.plan;
        return {
            :distance_m => plan.distanceM,
            :target_time_s => plan.targetTimeS,
            :negative_split => {
                :enabled => plan.negativeSplitEnabled,
                :ratio => plan.negativeSplitRatio
            }
        };
    }
}