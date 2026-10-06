//! Synchronisation avec le backend auto-hébergé.
//!
//! Deux responsabilités, volontairement séparées du moteur :
//!   1. appairer la montre (device flow RFC 8628 : code utilisateur, puis
//!      sondage du jeton jusqu'à approbation sur la page /link du site) ;
//!   2. envoyer les séances locales sur POST /api/v1/workouts.
//!
//! Le jeton est un jeton opaque propre à l'appareil. Connect IQ n'offre ni
//! Keystore ni stockage chiffré : il vit dans Application.Storage, et se révoque
//! depuis la page « Jetons » du site.

using Toybox.Lang;
using Toybox.Communications;
using Toybox.PersistedContent;
using Toybox.System;
using Toybox.Timer;
using Toybox.WatchUi;

class MpacerSync {

    var settings;
    var archive;

    var phase;            // :idle :requesting :waiting :syncing :done :error
    var message;
    var userCode;
    var verificationUri;
    var deviceCode;
    var intervalS;
    var expiresAtMs;
    var pollTimer;
    var pollInFlight;
    var queue;            // identifiants de séances restant à envoyer
    var currentId;
    var currentSummary;
    var uploading;

    function initialize(settings, archive) {
        self.settings = settings;
        self.archive = archive;
        self.phase = :idle;
        self.message = null;
        self.userCode = null;
        self.verificationUri = null;
        self.deviceCode = null;
        self.intervalS = 5;
        self.expiresAtMs = 0;
        self.pollTimer = null;
        self.pollInFlight = false;
        self.queue = [];
        self.currentId = null;
        self.currentSummary = null;
        self.uploading = false;
    }

    function isPaired() {
        return self.archive.isPaired();
    }

    // Adresse du backend : réglage de la montre, sinon valeur de properties.xml.
    function baseUrl() {
        var url = self.archive.baseUrl();
        if (url == null || url.length() == 0) {
            url = self.settings.backendUrl;
        }
        if (url == null) {
            return "";
        }
        return self.normalize(url);
    }

    function normalize(url) {
        var text = url;
        // Espaces de tête et de queue.
        while (text.length() > 0 && text.substring(0, 1) == " ") {
            text = text.substring(1, text.length());
        }
        while (text.length() > 0 && text.substring(text.length() - 1, text.length()) == " ") {
            text = text.substring(0, text.length() - 1);
        }
        while (text.length() > 0 && text.substring(text.length() - 1, text.length()) == "/") {
            text = text.substring(0, text.length() - 1);
        }
        return text;
    }

    function setBaseUrl(url) {
        self.archive.setBaseUrl(self.normalize(url));
    }

    function jsonOptions() {
        return {
            :method => Communications.HTTP_REQUEST_METHOD_POST,
            :headers => { "Content-Type" => Communications.REQUEST_CONTENT_TYPE_JSON },
            :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
        };
    }

    function authOptions() {
        return {
            :method => Communications.HTTP_REQUEST_METHOD_POST,
            :headers => {
                "Content-Type" => Communications.REQUEST_CONTENT_TYPE_JSON,
                "Authorization" => "Bearer " + self.archive.token()
            },
            :responseType => Communications.HTTP_RESPONSE_CONTENT_TYPE_JSON
        };
    }

    // ------------------------------------------------------------------ appairage

    function startPairing() {
        if (self.baseUrl().length() == 0) {
            self.fail(MpacerText.noBackend());
            return;
        }
        self.phase = :requesting;
        self.message = null;
        self.userCode = null;
        Communications.makeWebRequest(
            self.baseUrl() + "/api/v1/device/code",
            { "label" => "Montre Garmin" },
            self.jsonOptions(),
            method(:onDeviceCode)
        );
        WatchUi.requestUpdate();
    }

    function onDeviceCode(responseCode as Lang.Number, data as Lang.Dictionary or Lang.String or PersistedContent.Iterator or Null) as Void {
        if (responseCode != 200 || data == null || !(data instanceof Lang.Dictionary)) {
            self.fail(MpacerText.pairFailed() + " (" + responseCode + ")");
            return;
        }
        self.deviceCode = data["device_code"];
        self.userCode = data["user_code"];
        self.verificationUri = data["verification_uri"];
        var interval = data["interval"];
        if (interval != null && interval >= 2) {
            self.intervalS = interval;
        }
        var expires = data["expires_in"];
        if (expires == null || expires <= 0) {
            expires = 600;
        }
        self.expiresAtMs = System.getTimer() + (expires * 1000).toNumber();
        self.phase = :waiting;
        self.message = MpacerText.pairingHint();
        self.startPolling();
        WatchUi.requestUpdate();
    }

    function startPolling() {
        if (self.pollTimer == null) {
            self.pollTimer = new Timer.Timer();
        }
        self.pollTimer.start(method(:pollToken), self.intervalS * 1000, true);
    }

    function stopPolling() {
        if (self.pollTimer != null) {
            self.pollTimer.stop();
        }
    }

    function pollToken() {
        if (self.pollInFlight || self.deviceCode == null) {
            return;
        }
        if (System.getTimer() > self.expiresAtMs) {
            self.stopPolling();
            self.fail(MpacerText.pairFailed() + " : code expire");
            return;
        }
        self.pollInFlight = true;
        Communications.makeWebRequest(
            self.baseUrl() + "/api/v1/device/token",
            { "device_code" => self.deviceCode },
            self.jsonOptions(),
            method(:onDeviceToken)
        );
    }

    function onDeviceToken(responseCode as Lang.Number, data as Lang.Dictionary or Lang.String or PersistedContent.Iterator or Null) as Void {
        self.pollInFlight = false;
        if (data != null && data instanceof Lang.Dictionary && data["access_token"] != null) {
            self.archive.setToken(data["access_token"]);
            self.stopPolling();
            self.deviceCode = null;
            self.userCode = null;
            self.phase = :done;
            self.message = MpacerText.paired();
            WatchUi.requestUpdate();
            return;
        }
        if (data != null && data instanceof Lang.Dictionary && data["error"] == "authorization_pending") {
            return;   // fonctionnement normal : on attend l'approbation
        }
        self.stopPolling();
        self.fail(MpacerText.pairFailed() + " (" + responseCode + ")");
    }

    function disconnect() {
        self.archive.clearToken();
        self.phase = :idle;
        self.message = null;
        WatchUi.requestUpdate();
    }

    // ---------------------------------------------------------------- envoi

    // Envoie une séance immédiatement (fin de séance) ; en cas d'échec elle est
    // archivée et repartira au prochain envoi.
    function uploadSummary(summary) {
        if (self.baseUrl().length() == 0) {
            self.archive.addPending(summary);
            self.fail(MpacerText.noBackend());
            return;
        }
        if (!self.isPaired()) {
            self.archive.addPending(summary);
            self.phase = :idle;
            self.message = MpacerText.notPaired();
            return;
        }
        self.currentId = summary[:id];
        self.currentSummary = summary;
        self.phase = :syncing;
        self.message = null;
        self.upload(summary);
    }

    // Envoie toutes les séances en attente, l'une après l'autre.
    function syncPending() {
        if (self.baseUrl().length() == 0) {
            self.fail(MpacerText.noBackend());
            return;
        }
        if (!self.isPaired()) {
            self.fail(MpacerText.notPaired());
            return;
        }
        self.queue = [];
        var ids = self.archive.pendingIds;
        for (var i = 0; i < ids.size(); i++) {
            self.queue.add(ids[i]);
        }
        if (self.queue.size() == 0) {
            self.phase = :done;
            self.message = MpacerText.syncOk();
            WatchUi.requestUpdate();
            return;
        }
        self.phase = :syncing;
        self.message = null;
        self.nextUpload();
    }

    function nextUpload() {
        if (self.queue.size() == 0) {
            self.currentId = null;
            self.currentSummary = null;
            self.phase = :done;
            self.message = MpacerText.syncOk();
            WatchUi.requestUpdate();
            return;
        }
        var id = self.queue[0];
        var stored = self.archive.pending(id);
        if (stored == null) {
            self.queue = self.queue.slice(1);
            self.nextUpload();
            return;
        }
        self.currentId = id;
        self.currentSummary = stored;
        self.upload(stored);
    }

    function upload(summary) {
        self.uploading = true;
        Communications.makeWebRequest(
            self.baseUrl() + "/api/v1/workouts",
            self.payload(summary),
            self.authOptions(),
            method(:onUpload)
        );
    }

    function onUpload(responseCode as Lang.Number, data as Lang.Dictionary or Lang.String or PersistedContent.Iterator or Null) as Void {
        self.uploading = false;
        if (responseCode >= 200 && responseCode < 300) {
            if (self.currentId != null) {
                self.archive.removePending(self.currentId);
            }
            self.currentId = null;
            self.currentSummary = null;
            self.phase = :done;
            self.message = MpacerText.syncOk();
            WatchUi.requestUpdate();
            if (self.queue.size() > 0) {
                self.queue = self.queue.slice(1);
                self.nextUpload();
            }
            return;
        }
        // Échec : la séance reste dans l'archive locale, elle repartira plus tard.
        if (self.currentSummary != null && self.currentId != null && !self.archive.hasPending(self.currentId)) {
            self.archive.addPending(self.currentSummary);
        }
        self.currentId = null;
        self.currentSummary = null;
        self.queue = [];
        self.fail(MpacerText.syncFailed() + " (" + responseCode + ")");
    }

    // Corps JSON : exactement le WorkoutSummary produit par le moteur, donc le
    // même format que le fichier .pac et que l'application Wear OS.
    function payload(summary) {
        return {
            "id" => summary[:id],
            "started_at_ms" => summary[:started_at_ms],
            "duration_s" => summary[:duration_s],
            "distance_m" => summary[:distance_m],
            "average_pace_s_per_km" => summary[:average_pace_s_per_km],
            "laps" => self.lapsJson(summary[:laps]),
            "best_efforts" => self.effortsJson(summary[:best_efforts]),
            "track" => self.trackJson(summary[:track]),
            "unit_system" => summary[:unit_system],
            "elapsed_s" => summary[:elapsed_s],
            "pauses" => self.pausesJson(summary[:pauses]),
            "heart_rate" => self.heartRateJson(summary[:heart_rate]),
            "plan" => self.planJson(summary[:plan])
        };
    }

    // Les dictionnaires du moteur utilisent des clés Symbol (pratique en Monkey C,
    // mais le JSON doit porter des noms de champs explicites) : la conversion est
    // faite ici, une fois, à l'envoi.

    function lapsJson(laps) {
        var out = [];
        if (laps == null) { return out; }
        for (var i = 0; i < laps.size(); i++) {
            var lap = laps[i];
            out.add({
                "index" => lap[:index],
                "distance_m" => lap[:distance_m],
                "duration_s" => lap[:duration_s],
                "pace_s_per_km" => lap[:pace_s_per_km]
            });
        }
        return out;
    }

    function effortsJson(efforts) {
        var out = [];
        if (efforts == null) { return out; }
        for (var i = 0; i < efforts.size(); i++) {
            var effort = efforts[i];
            out.add({
                "label" => effort[:label],
                "distance_m" => effort[:distance_m],
                "time_s" => effort[:time_s],
                "start_dist_m" => effort[:start_dist_m]
            });
        }
        return out;
    }

    function trackJson(points) {
        var out = [];
        if (points == null) { return out; }
        for (var i = 0; i < points.size(); i++) {
            var point = points[i];
            out.add({
                "t_ms" => point[:t_ms],
                "dist_m" => point[:dist_m],
                "lat" => point[:lat],
                "lon" => point[:lon],
                "elevation_m" => point[:elevation_m]
            });
        }
        return out;
    }

    function pausesJson(pauses) {
        var out = [];
        if (pauses == null) { return out; }
        for (var i = 0; i < pauses.size(); i++) {
            var pause = pauses[i];
            out.add({
                "at_s" => pause[:at_s],
                "at_distance_m" => pause[:at_distance_m],
                "duration_s" => pause[:duration_s],
                "automatic" => pause[:automatic]
            });
        }
        return out;
    }

    function heartRateJson(samples) {
        var out = [];
        if (samples == null) { return out; }
        for (var i = 0; i < samples.size(); i++) {
            var sample = samples[i];
            out.add({ "t_ms" => sample[:t_ms], "bpm" => sample[:bpm] });
        }
        return out;
    }

    function planJson(plan) {
        if (plan == null) {
            return null;
        }
        return {
            "distance_m" => plan[:distance_m],
            "target_time_s" => plan[:target_time_s],
            "negative_split" => {
                "enabled" => plan[:negative_split][:enabled],
                "ratio" => plan[:negative_split][:ratio]
            }
        };
    }

    function fail(text) {
        self.phase = :error;
        self.message = text;
        self.uploading = false;
        System.println("M-pacer sync : " + text);
        WatchUi.requestUpdate();
    }

    function stateText() {
        if (self.phase == :requesting) { return "..."; }
        if (self.message != null) { return self.message; }
        if (self.isPaired()) { return MpacerText.paired(); }
        return MpacerText.notPaired();
    }
}