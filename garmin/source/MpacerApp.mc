//! Application Garmin (Connect IQ) : c'est le seul fichier qui touche à la
//! plateforme. Il pilote les capteurs (GPS 1 Hz, cardio), l'enregistrement FIT,
//! les vibrations et les écrans ; tout le calcul vient de MpacerEngine, portage
//! du cœur Rust de M-pacer.

using Toybox.Application;
using Toybox.System;
using Toybox.Position;
using Toybox.Sensor;
using Toybox.Activity;
using Toybox.ActivityRecording;
using Toybox.WatchUi;
using Toybox.Attention;

class MpacerApp extends Application.AppBase {

    var settings;
    var engine;
    var archive;
    var sync;

    var view;
    var delegate;

    var session;          // ActivityRecording.Session (fichier FIT)
    var recording;
    var sensorsOn;
    var vibrateOk;

    // Écran affiché : :run, :sync, :settings
    var screen;

    // Dernière alerte (annonce remplacée par un texte : Garmin n'expose pas de
    // synthèse vocale aux applications Connect IQ).
    var alertText;
    var alertUntilMs;

    function initialize() {
        AppBase.initialize();
        self.settings = new MpacerSettings();
        self.settings.load();
        self.engine = new MpacerEngine(self.settings);
        self.archive = new MpacerArchive();
        self.sync = new MpacerSync(self.settings, self.archive);
        self.view = null;
        self.delegate = null;
        self.session = null;
        self.recording = false;
        self.sensorsOn = false;
        self.screen = :run;
        self.alertText = null;
        self.alertUntilMs = 0;
        self.vibrateOk = false;
        if (Toybox has :Attention) {
            self.vibrateOk = (Attention has :vibrate);
        }
    }

    function getInitialView() {
        self.view = new MpacerView(self);
        self.delegate = new MpacerDelegate(self);
        return [self.view, self.delegate];
    }

    function onStart(state) {
        self.startSensors();
    }

    function onStop(state) {
        self.stopSensors();
    }

    function onSettingsChanged() {
        self.settings.load();
        self.engine.applySettings(self.settings);
        WatchUi.requestUpdate();
    }

    // ------------------------------------------------------------------ capteurs

    function startSensors() {
        if (self.sensorsOn) {
            return;
        }
        Position.enableLocationEvents(Position.LOCATION_CONTINUOUS, method(:onPosition));
        Sensor.enableSensorEvents(method(:onSensor));
        self.sensorsOn = true;
    }

    function stopSensors() {
        if (!self.sensorsOn) {
            return;
        }
        Position.enableLocationEvents(Position.LOCATION_DISABLE, null);
        Sensor.enableSensorEvents(null);
        self.sensorsOn = false;
    }

    // Position.Info.accuracy est une qualité, pas une précision en mètres : on
    // la traduit vers les seuils du cœur (10 m « bon », 25 m « faible »).
    function accuracyMeters(quality) {
        if (quality == Position.QUALITY_GOOD) {
            return 5.0;
        }
        if (quality == Position.QUALITY_USABLE) {
            return 18.0;
        }
        if (quality == Position.QUALITY_POOR) {
            return 60.0;
        }
        return 999.0;
    }

    function onPosition(info as Position.Info) as Void {
        var location = info.position;
        if (location == null) {
            return;
        }
        var degrees = location.toDegrees();
        var output = self.engine.onGps(
            System.getTimer(),
            degrees[0],
            degrees[1],
            self.accuracyMeters(info.accuracy),
            info.altitude
        );
        self.handleOutput(output);
    }

    function onSensor(info as Sensor.Info) as Void {
        var heartRate = info.heartRate;
        if (heartRate != null) {
            self.engine.onHeartRate(System.getTimer(), heartRate);
        }
    }

    // ------------------------------------------------------------------ séance

    function createSession() {
        if (!(Toybox has :ActivityRecording)) {
            return;
        }
        self.session = ActivityRecording.createSession({
            :name => "M-pacer",
            :sport => Activity.SPORT_RUNNING,
            :subSport => Activity.SUB_SPORT_GENERIC
        });
        if (self.session != null) {
            self.session.start();
        }
    }

    function closeSession() {
        if (self.session != null) {
            self.session.stop();
            self.session.save();
            self.session = null;
        }
    }

    function startWorkout(armed) {
        if (self.recording) {
            return;
        }
        self.createSession();
        self.recording = true;
        self.screen = :run;
        self.handleOutput(self.engine.start(System.getTimer(), armed));
    }

    function pauseWorkout() {
        if (!self.recording) {
            return;
        }
        self.handleOutput(self.engine.pause(System.getTimer()));
    }

    function resumeWorkout() {
        if (!self.recording) {
            return;
        }
        self.handleOutput(self.engine.resume(System.getTimer()));
    }

    function stopWorkout() {
        if (!self.recording) {
            return;
        }
        self.handleOutput(self.engine.stop(System.getTimer()));
        self.recording = false;
        var summary = self.engine.summary();
        self.closeSession();
        // Envoi immédiat si la montre est appairée ; sinon la séance reste dans
        // l'archive locale (courir ne dépend jamais du réseau).
        self.sync.uploadSummary(summary);
        self.screen = :sync;
        WatchUi.requestUpdate();
    }

    function manualLap() {
        if (!self.recording) {
            return;
        }
        var output = self.engine.manualLap(System.getTimer());
        if (output[:lap_completed] != null && self.session != null) {
            self.session.addLap();
        }
        self.handleOutput(output);
    }

    // -------------------------------------------------------------- alertes/UI

    function handleOutput(output) {
        if (output == null) {
            return;
        }
        var alerts = output[:alerts];
        if (alerts != null) {
            for (var i = 0; i < alerts.size(); i++) {
                var alert = alerts[i];
                self.vibrate(alert[:vibration]);
                if (alert[:text] != null) {
                    self.alertText = alert[:text];
                    self.alertUntilMs = System.getTimer() + 8000;
                }
            }
        }
        if (output[:lap_completed] != null && self.session != null) {
            // Le FIT porte les mêmes tours que l'écran.
            self.session.addLap();
        }
        if (self.recording) {
            WatchUi.requestUpdate();
        }
    }

    function vibrate(pattern) {
        if (!self.vibrateOk) {
            return;
        }
        var profiles;
        if (pattern == :long) {
            profiles = [new Attention.VibeProfile(100, 700)];
        } else if (pattern == :double) {
            profiles = [new Attention.VibeProfile(80, 200), new Attention.VibeProfile(80, 200)];
        } else if (pattern == :short) {
            profiles = [new Attention.VibeProfile(60, 150)];
        } else {
            return;
        }
        Attention.vibrate(profiles);
    }

    function currentAlert() {
        if (self.alertText == null) {
            return null;
        }
        if (System.getTimer() > self.alertUntilMs) {
            self.alertText = null;
            return null;
        }
        return self.alertText;
    }

    // ------------------------------------------------------------------ boutons

    function onStartKey() {
        var state = self.engine.workout.state;
        if (!self.recording) {
            self.startWorkout(false);
        } else if (state == :running || state == :armed) {
            self.pauseWorkout();
        } else if (self.engine.workout.isPaused()) {
            self.resumeWorkout();
        }
        return true;
    }

    function onLapKey() {
        self.manualLap();
        return true;
    }

    function onUpKey() {
        self.nextScreen(-1);
        return true;
    }

    function onDownKey() {
        self.nextScreen(1);
        return true;
    }

    function onMenuKey() {
        if (self.screen == :run) {
            self.handleOutput(self.engine.announceNow(System.getTimer()));
        } else if (self.screen == :sync) {
            if (self.sync.isPaired()) {
                self.sync.syncPending();
            } else {
                self.sync.startPairing();
            }
        }
        WatchUi.requestUpdate();
        return true;
    }

    function onSelectKey() {
        return self.onMenuKey();
    }

    function onBackKey() {
        if (self.recording) {
            self.stopWorkout();
            return true;
        }
        if (self.screen != :run) {
            self.screen = :run;
            WatchUi.requestUpdate();
            return true;
        }
        return false;
    }

    function nextScreen(direction) {
        if (direction > 0) {
            if (self.screen == :run) {
                self.screen = :sync;
            } else if (self.screen == :sync) {
                self.screen = :settings;
            } else {
                self.screen = :run;
            }
        } else {
            if (self.screen == :run) {
                self.screen = :settings;
            } else if (self.screen == :settings) {
                self.screen = :sync;
            } else {
                self.screen = :run;
            }
        }
        WatchUi.requestUpdate();
    }

    function onViewShown() {
        self.engine.tick(System.getTimer());
        WatchUi.requestUpdate();
    }

    function output() {
        return self.engine.output;
    }
}