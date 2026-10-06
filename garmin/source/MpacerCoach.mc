//! Alertes de course : *quand* prévenir le coureur, et *quoi* afficher.
//!
//! Portage du planificateur de `crates/mpacer-core/src/voice.rs`. Garmin
//! n'expose aucune synthèse vocale aux applications Connect IQ : la voix est
//! remplacée par une vibration (et par le texte, visible à l'écran). Les
//! déclencheurs, eux, sont identiques : départ, pause, reprise, arrêt, tour
//! franchi, annonce périodique, écart au shadow runner.

class MpacerCoach {

    var enabled;
    var frequency;         // :off :every_1min :every_2min :every_5min :lap
    var imperial;
    var extendedLapInfo;
    var lastPeriodicMs;
    var announcedLaps;

    function initialize() {
        self.enabled = true;
        self.frequency = :every_2min;
        self.imperial = false;
        self.extendedLapInfo = false;
        self.lastPeriodicMs = null;
        self.announcedLaps = 0;
    }

    function configure(enabled, frequency, imperial, extendedLapInfo) {
        self.enabled = enabled;
        self.frequency = frequency;
        self.imperial = imperial;
        self.extendedLapInfo = extendedLapInfo;
    }

    function reset() {
        self.lastPeriodicMs = null;
        self.announcedLaps = 0;
    }

    // Période des annonces, ou null si aucune n'est périodique.
    function periodMs() {
        if (self.frequency == :every_1min) { return 60000; }
        if (self.frequency == :every_2min) { return 120000; }
        if (self.frequency == :every_5min) { return 300000; }
        return null;
    }

    function alert(cue, text, vibration) {
        return { :cue => cue, :text => text, :vibration => vibration };
    }

    function onEvent(event, tMs, snapshot) {
        if (!self.enabled) {
            return [];
        }
        if (event == :armed) {
            return [self.alert(:armed, MpacerText.armed(), :none)];
        }
        if (event == :started) {
            return [self.alert(:started, MpacerText.alertStarted(), :long)];
        }
        if (event == :paused || event == :auto_paused) {
            return [self.alert(event, MpacerText.alertPaused(), :double)];
        }
        if (event == :resumed || event == :auto_resumed) {
            return [self.alert(event, MpacerText.alertResumed(), :short)];
        }
        if (event == :stopped) {
            return [self.alert(:stopped, self.finishedText(snapshot), :long)];
        }
        return [];
    }

    function finishedText(snapshot) {
        if (snapshot == null) {
            return MpacerText.alertStopped();
        }
        return MpacerText.alertStopped() + " " + MpacerUnits.formatDistance(snapshot[:distance_m], self.imperial)
            + " " + MpacerUnits.formatDuration(snapshot[:elapsed_s]) + ".";
    }

    function onLap(lap, snapshot) {
        if (!self.enabled || lap == null) {
            return [];
        }
        self.announcedLaps = self.announcedLaps + 1;
        var text = MpacerText.wordLap() + " " + lap[:index].toString() + " : "
            + MpacerUnits.formatDuration(lap[:duration_s]) + " ("
            + MpacerUnits.formatPace(lap[:pace_s_per_km] / (1000.0 / MpacerUnits.metersPerUnit(self.imperial))) + ")";
        if (self.extendedLapInfo) {
            text = text + ". " + MpacerText.wordDistance() + " "
                + MpacerUnits.formatDistance(snapshot[:distance_m], self.imperial);
        }
        return [self.alert(:lap, text, :double)];
    }

    // Annonce périodique : allure, distance, temps, écart au plan.
    function onTick(tMs, snapshot) {
        if (!self.enabled || snapshot == null) {
            return [];
        }
        var period = self.periodMs();
        if (period == null) {
            return [];
        }
        if (self.lastPeriodicMs != null && tMs - self.lastPeriodicMs < period) {
            return [];
        }
        self.lastPeriodicMs = tMs;
        return [self.alert(:periodic, self.periodicText(snapshot), :short)];
    }

    function periodicText(snapshot) {
        var text = MpacerText.wordPace() + " " + MpacerUnits.formatPace(snapshot[:current_pace])
            + " " + MpacerText.wordPer() + " " + MpacerUnits.label(self.imperial)
            + ". " + MpacerText.wordDistance() + " " + MpacerUnits.formatDistance(snapshot[:distance_m], self.imperial)
            + ". " + MpacerText.wordTime() + " " + MpacerUnits.formatDuration(snapshot[:elapsed_s]) + ".";
        if (snapshot[:lap] != null) {
            text = text + " " + MpacerText.wordLastLap() + " "
                + MpacerUnits.formatDuration(snapshot[:lap][:duration_s]) + ".";
        }
        var position = self.positionText(snapshot[:shadow]);
        if (position.length() > 0) {
            text = text + " " + position;
        }
        return text;
    }

    function positionText(shadow) {
        if (shadow == null) {
            return "";
        }
        if (shadow[:on_plan]) {
            return MpacerText.onPlan() + ".";
        }
        var prefix = MpacerText.wordBehindBy();
        if (shadow[:ahead]) {
            prefix = MpacerText.wordAheadBy();
        }
        return prefix + " " + MpacerUnits.formatDuration((shadow[:time_delta_s]).abs())
            + ", " + MpacerUnits.formatDistanceShort((shadow[:distance_delta_m]).abs(), self.imperial) + ".";
    }

    // Annonce à la demande (bouton Menu sur l'écran de course).
    function manualStatus(snapshot) {
        if (snapshot == null) {
            return [];
        }
        return [self.alert(:manual, self.periodicText(snapshot), :short)];
    }
}