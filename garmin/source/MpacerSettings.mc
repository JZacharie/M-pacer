//! Réglages de l'application.
//!
//! Les valeurs par défaut viennent de `resources/settings/properties.xml` et
//! sont relues au démarrage. Une propriété absente ou illisible ne casse jamais
//! l'application : le repli est codé ici.

using Toybox.Application;

class MpacerSettings {

    var assistantMode;        // "track_pace" | "predict_finish" | "planned_time"
    var imperial;             // false = kilomètres
    var plannedDistanceM;
    var plannedTimeS;
    var negativeSplit;
    var negativeSplitRatio;   // 0.03 = 3 %
    var detectPaceChange;
    var autoPause;
    var alertsEnabled;
    var alertFrequency;       // "off" | "1min" | "2min" | "5min" | "lap"
    var extendedLapInfo;
    var maxHeartRate;
    var backendUrl;
    var syncTrace;
    var traceIntervalS;

    function initialize() {
        self.applyDefaults();
    }

    function applyDefaults() {
        self.assistantMode = "track_pace";
        self.imperial = false;
        self.plannedDistanceM = 10000.0;
        self.plannedTimeS = 3000.0;
        self.negativeSplit = false;
        self.negativeSplitRatio = 0.03;
        self.detectPaceChange = false;
        self.autoPause = false;
        self.alertsEnabled = true;
        self.alertFrequency = "2min";
        self.extendedLapInfo = false;
        self.maxHeartRate = 190;
        self.backendUrl = "";
        self.syncTrace = false;
        self.traceIntervalS = 15;
    }

    function load() {
        self.applyDefaults();
        var mode = self.readNumber("assistantMode", 0);
        if (mode == 1) {
            self.assistantMode = "predict_finish";
        } else if (mode == 2) {
            self.assistantMode = "planned_time";
        } else {
            self.assistantMode = "track_pace";
        }
        self.imperial = (self.readNumber("units", 0) == 1);
        self.detectPaceChange = self.readBoolean("detectPaceChange", self.detectPaceChange);
        self.autoPause = self.readBoolean("autoPause", self.autoPause);
        self.alertsEnabled = self.readBoolean("alertsEnabled", self.alertsEnabled);
        var frequency = self.readNumber("alertFrequency", 2);
        if (frequency == 0) {
            self.alertFrequency = "off";
        } else if (frequency == 1) {
            self.alertFrequency = "1min";
        } else if (frequency == 3) {
            self.alertFrequency = "5min";
        } else if (frequency == 4) {
            self.alertFrequency = "lap";
        } else {
            self.alertFrequency = "2min";
        }
        self.extendedLapInfo = self.readBoolean("extendedLapInfo", self.extendedLapInfo);

        var distanceKm = self.readFloat("plannedDistanceKm", self.plannedDistanceM / 1000.0);
        if (distanceKm > 0.0) {
            self.plannedDistanceM = distanceKm * 1000.0;
        }
        var minutes = self.readNumber("plannedTimeMin", (self.plannedTimeS / 60.0).toNumber());
        if (minutes > 0) {
            self.plannedTimeS = minutes * 60.0;
        }
        self.negativeSplit = self.readBoolean("negativeSplit", self.negativeSplit);
        var ratioPercent = self.readFloat("negativeSplitRatio", self.negativeSplitRatio * 100.0);
        if (ratioPercent >= 0.0) {
            self.negativeSplitRatio = ratioPercent / 100.0;
        }
        self.maxHeartRate = self.readNumber("maxHeartRate", self.maxHeartRate);

        self.backendUrl = self.readString("backendUrl", self.backendUrl);
        self.syncTrace = self.readBoolean("syncTrace", self.syncTrace);
        var interval = self.readNumber("traceIntervalS", self.traceIntervalS);
        if (interval >= 1) {
            self.traceIntervalS = interval;
        }
    }

    // ------------------------------------------------------------- lecture sûre

    function readRaw(key) {
        try {
            return Application.Properties.getValue(key);
        } catch (e) {
            return null;
        }
    }

    function readString(key, fallback) {
        var value = self.readRaw(key);
        if (value == null) {
            return fallback;
        }
        var text = value.toString();
        if (text.length() == 0) {
            return fallback;
        }
        return text;
    }

    function readBoolean(key, fallback) {
        var value = self.readRaw(key);
        if (value == null) {
            return fallback;
        }
        if (value instanceof Lang.Boolean) {
            return value;
        }
        return "true" == value.toString().toLower();
    }

    function readNumber(key, fallback) {
        var value = self.readRaw(key);
        if (value == null) {
            return fallback;
        }
        var number = value.toNumber();
        if (number == null) {
            return fallback;
        }
        return number;
    }

    function readFloat(key, fallback) {
        var value = self.readRaw(key);
        if (value == null) {
            return fallback;
        }
        var number = value.toFloat();
        if (number == null) {
            return fallback;
        }
        return number;
    }
}