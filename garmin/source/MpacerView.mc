//! Écran rond : allure lissée, distance, temps, voyant GPS, panneau
//! d'assistant, tours et cardio. Le rendu ne calcule rien : il affiche l'état
//! produit par MpacerEngine.

using Toybox.WatchUi;
using Toybox.Graphics;
using Toybox.System;

class MpacerView extends WatchUi.View {

    var app;

    function initialize(app) {
        View.initialize();
        self.app = app;
    }

    function onShow() {
        self.app.onViewShown();
    }

    function onUpdate(dc) {
        dc.setColor(Graphics.COLOR_WHITE, Graphics.COLOR_BLACK);
        dc.clear();
        if (self.app.screen == :sync) {
            self.drawSync(dc);
        } else if (self.app.screen == :settings) {
            self.drawSettings(dc);
        } else {
            self.drawRun(dc);
        }
    }

    // ------------------------------------------------------------------- course

    function drawRun(dc) {
        var width = dc.getWidth();
        var height = dc.getHeight();
        var center = width / 2;
        var output = self.app.output();

        if (output == null) {
            self.center(dc, center, height / 2, Graphics.FONT_MEDIUM, MpacerText.gpsStatus(:acquiring), Graphics.COLOR_LT_GRAY);
            return;
        }

        self.drawGpsLight(dc, 28, 28, output[:light]);

        // Allure courante : la valeur centrale, comme sur la montre Wear OS.
        var paceText = MpacerUnits.formatPace(output[:current_pace]);
        dc.setColor(Graphics.COLOR_WHITE, Graphics.COLOR_TRANSPARENT);
        dc.drawText(center, (height * 0.26).toNumber(), Graphics.FONT_NUMBER_MILD, paceText, Graphics.TEXT_JUSTIFY_CENTER);
        self.center(dc, center, (height * 0.40).toNumber(), Graphics.FONT_TINY,
            MpacerText.wordPace() + " / " + MpacerUnits.paceLabel(self.app.settings.imperial), Graphics.COLOR_LT_GRAY);

        // Distance et temps de course.
        var distanceText = MpacerUnits.formatDistance(output[:distance_m], self.app.settings.imperial);
        var timeText = MpacerUnits.formatDuration(output[:elapsed_s]);
        self.center(dc, center, (height * 0.50).toNumber(), Graphics.FONT_MEDIUM, distanceText, Graphics.COLOR_WHITE);
        self.center(dc, center, (height * 0.59).toNumber(), Graphics.FONT_MEDIUM, timeText, Graphics.COLOR_WHITE);

        // Tours : distance du tour courant et allure du tour précédent.
        var lapText = MpacerText.wordLap() + " " + MpacerUnits.formatDistanceShort(output[:current_lap_distance_m], self.app.settings.imperial);
        if (output[:current_lap_pace] != null) {
            lapText = lapText + "  " + MpacerUnits.formatPace(output[:current_lap_pace]);
        } else if (output[:previous_lap_pace] != null) {
            lapText = lapText + "  " + MpacerUnits.formatPace(output[:previous_lap_pace]);
        }
        self.center(dc, center, (height * 0.68).toNumber(), Graphics.FONT_TINY, lapText, Graphics.COLOR_LT_GRAY);

        // Cardio et zone.
        if (output[:heart_rate_bpm] != null) {
            var heart = MpacerText.wordHeart() + " " + output[:heart_rate_bpm].toString();
            if (output[:heart_rate_zone] > 0) {
                heart = heart + "  " + MpacerText.wordZone() + " " + output[:heart_rate_zone].toString();
            }
            self.center(dc, center, (height * 0.75).toNumber(), Graphics.FONT_TINY, heart, Graphics.COLOR_RED);
        }

        // Panneau d'assistant, alerte en cours, ou état de la séance.
        var alert = self.app.currentAlert();
        if (alert != null) {
            self.center(dc, center, (height * 0.84).toNumber(), Graphics.FONT_TINY, alert, Graphics.COLOR_YELLOW);
        } else {
            self.drawPanel(dc, center, (height * 0.84).toNumber(), output);
        }

        self.drawState(dc, width, height, output);
    }

    function drawPanel(dc, center, y, output) {
        var panel = output[:panel];
        if (panel == null || !panel[:visible]) {
            return;
        }
        if (panel[:issue] == :missing_distance) {
            self.center(dc, center, y, Graphics.FONT_TINY, MpacerText.configMissingDistance(), Graphics.COLOR_ORANGE);
            return;
        }
        if (panel[:issue] == :missing_time) {
            self.center(dc, center, y, Graphics.FONT_TINY, MpacerText.configMissingTime(), Graphics.COLOR_ORANGE);
            return;
        }
        var text = null;
        var color = Graphics.COLOR_LT_GRAY;
        if (panel[:shadow] != null) {
            var shadow = panel[:shadow];
            if (shadow[:on_plan]) {
                text = MpacerText.onPlan();
                color = Graphics.COLOR_GREEN;
            } else if (shadow[:ahead]) {
                text = MpacerText.ahead() + " " + MpacerUnits.formatDuration((shadow[:time_delta_s]).abs())
                    + "  " + MpacerUnits.formatDistanceShort((shadow[:distance_delta_m]).abs(), self.app.settings.imperial);
                color = Graphics.COLOR_GREEN;
            } else {
                text = MpacerText.behind() + " " + MpacerUnits.formatDuration((shadow[:time_delta_s]).abs())
                    + "  " + MpacerUnits.formatDistanceShort((shadow[:distance_delta_m]).abs(), self.app.settings.imperial);
                color = Graphics.COLOR_ORANGE;
            }
        } else if (panel[:estimated_finish_s] != null) {
            text = MpacerText.finish() + " " + MpacerUnits.formatDuration(panel[:estimated_finish_s]);
        } else if (panel[:remaining_m] != null) {
            text = MpacerText.remaining() + " " + MpacerUnits.formatDistance(panel[:remaining_m], self.app.settings.imperial);
        }
        if (text != null) {
            self.center(dc, center, y, Graphics.FONT_TINY, text, color);
        }
    }

    function drawState(dc, width, height, output) {
        var state = output[:state];
        if (state == :finished) {
            self.center(dc, width / 2, (height * 0.14).toNumber(), Graphics.FONT_SMALL, MpacerText.finished(), Graphics.COLOR_GREEN);
            self.center(dc, width / 2, (height * 0.92).toNumber(), Graphics.FONT_XTINY, MpacerText.newWorkoutHint(), Graphics.COLOR_LT_GRAY);
            return;
        }
        var text = null;
        if (state == :armed) {
            text = MpacerText.armed();
        } else if (state == :paused) {
            text = MpacerText.pause();
        } else if (state == :auto_paused) {
            text = MpacerText.wordAutoPause();
        }
        if (text == null) {
            return;
        }
        self.center(dc, width / 2, (height * 0.14).toNumber(), Graphics.FONT_SMALL, text, Graphics.COLOR_YELLOW);
    }

    function drawGpsLight(dc, x, y, light) {
        var color = Graphics.COLOR_RED;
        if (light == :orange) {
            color = Graphics.COLOR_ORANGE;
        } else if (light == :yellow) {
            color = Graphics.COLOR_YELLOW;
        } else if (light == :green) {
            color = Graphics.COLOR_GREEN;
        }
        dc.setColor(color, Graphics.COLOR_TRANSPARENT);
        dc.fillCircle(x, y, 7);
        dc.setColor(Graphics.COLOR_DK_GRAY, Graphics.COLOR_TRANSPARENT);
        dc.drawCircle(x, y, 7);
    }

    // ------------------------------------------------------------ synchronisation

    function drawSync(dc) {
        var width = dc.getWidth();
        var center = width / 2;
        var height = dc.getHeight();
        self.center(dc, center, (height * 0.16).toNumber(), Graphics.FONT_SMALL, MpacerText.sync(), Graphics.COLOR_WHITE);

        var code = self.app.sync.userCode;
        if (code != null) {
            self.center(dc, center, (height * 0.30).toNumber(), Graphics.FONT_NUMBER_MEDIUM, code, Graphics.COLOR_YELLOW);
            self.center(dc, center, (height * 0.44).toNumber(), Graphics.FONT_TINY, MpacerText.pairingHint(), Graphics.COLOR_LT_GRAY);
            if (self.app.sync.verificationUri != null) {
                self.center(dc, center, (height * 0.52).toNumber(), Graphics.FONT_XTINY, self.app.sync.verificationUri, Graphics.COLOR_LT_GRAY);
            }
        } else {
            var status = self.app.sync.stateText();
            self.center(dc, center, (height * 0.32).toNumber(), Graphics.FONT_SMALL, status, Graphics.COLOR_WHITE);
            var pending = MpacerText.pending() + " : " + self.app.archive.pendingCount().toString();
            self.center(dc, center, (height * 0.46).toNumber(), Graphics.FONT_TINY, pending, Graphics.COLOR_LT_GRAY);
            self.center(dc, center, (height * 0.58).toNumber(), Graphics.FONT_XTINY,
                "MENU : " + (self.app.sync.isPaired() ? "envoyer" : "appairer"), Graphics.COLOR_LT_GRAY);
        }
        var url = self.app.sync.baseUrl();
        if (url.length() == 0) {
            self.center(dc, center, (height * 0.66).toNumber(), Graphics.FONT_XTINY, MpacerText.noBackend(), Graphics.COLOR_ORANGE);
        }
    }

    // ---------------------------------------------------------------- réglages

    function drawSettings(dc) {
        var width = dc.getWidth();
        var center = width / 2;
        var height = dc.getHeight();
        var settings = self.app.settings;
        self.center(dc, center, (height * 0.10).toNumber(), Graphics.FONT_SMALL, MpacerText.settings(), Graphics.COLOR_WHITE);
        // Version de l'application et jour de compilation : une seule ligne,
        // car l'ecran est deja bien rempli. Les deux valeurs viennent du fichier
        // genere par garmin/build.ps1 (voir monkey.jungle).
        var ligneVersion = MpacerText.versionLabel() + " " + MpacerBuildInfo.version()
            + " (" + MpacerBuildInfo.buildDate() + ")";
        var lines = [
            "Assistant : " + settings.assistantMode,
            "Unites : " + (settings.imperial ? "mi" : "km"),
            "Pause auto : " + self.onOff(settings.autoPause),
            "Alertes : " + settings.alertFrequency,
            "Distance : " + MpacerUnits.formatDistance(settings.plannedDistanceM, settings.imperial),
            "Temps : " + MpacerUnits.formatDuration(settings.plannedTimeS),
            "Negative split : " + self.onOff(settings.negativeSplit),
            "FC max : " + settings.maxHeartRate.toString(),
            "Trace GPS : " + self.onOff(settings.syncTrace),
            ligneVersion
        ];
        var y = (height * 0.20).toNumber();
        var step = ((height * 0.60) / lines.size()).toNumber();
        for (var i = 0; i < lines.size(); i++) {
            self.center(dc, center, y, Graphics.FONT_XTINY, lines[i], Graphics.COLOR_LT_GRAY);
            y = y + step;
        }
        self.center(dc, center, (height * 0.87).toNumber(), Graphics.FONT_XTINY,
            "Reglages : Garmin Connect > M-pacer", Graphics.COLOR_DK_GRAY);
    }

    function onOff(value) {
        if (value) {
            return "oui";
        }
        return "non";
    }

    function center(dc, x, y, font, text, color) {
        if (text == null) {
            return;
        }
        dc.setColor(color, Graphics.COLOR_TRANSPARENT);
        dc.drawText(x, y, font, text, Graphics.TEXT_JUSTIFY_CENTER);
    }
}