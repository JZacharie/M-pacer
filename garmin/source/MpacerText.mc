//! Textes de l'interface.
//!
//! Toutes les chaînes visibles passent par ici : les ressources du projet
//! (resources/strings/strings.xml) sont en UTF-8 et portent les accents, ce qui
//! évite d'écrire des caractères accentués dans le code Monkey C.

using Toybox.WatchUi;

class MpacerText {

    static function appName() { return WatchUi.loadResource(Rez.Strings.AppName); }

    static function gpsStatus(status) {
        if (status == :disabled) { return WatchUi.loadResource(Rez.Strings.GpsDisabled); }
        if (status == :acquiring) { return WatchUi.loadResource(Rez.Strings.GpsAcquiring); }
        if (status == :poor) { return WatchUi.loadResource(Rez.Strings.GpsPoor); }
        return WatchUi.loadResource(Rez.Strings.GpsOk);
    }

    static function start() { return WatchUi.loadResource(Rez.Strings.Start); }
    static function pause() { return WatchUi.loadResource(Rez.Strings.Pause); }
    static function resume() { return WatchUi.loadResource(Rez.Strings.Resume); }
    static function stop() { return WatchUi.loadResource(Rez.Strings.Stop); }
    static function armed() { return WatchUi.loadResource(Rez.Strings.Armed); }
    static function noSignal() { return WatchUi.loadResource(Rez.Strings.NoSignal); }

    static function finish() { return WatchUi.loadResource(Rez.Strings.Finish); }
    static function remaining() { return WatchUi.loadResource(Rez.Strings.Remaining); }
    static function onPlan() { return WatchUi.loadResource(Rez.Strings.OnPlan); }
    static function ahead() { return WatchUi.loadResource(Rez.Strings.Ahead); }
    static function behind() { return WatchUi.loadResource(Rez.Strings.Behind); }

    static function sync() { return WatchUi.loadResource(Rez.Strings.Sync); }
    static function settings() { return WatchUi.loadResource(Rez.Strings.SettingsScreen); }
    static function versionLabel() { return WatchUi.loadResource(Rez.Strings.WordVersion); }
    static function notPaired() { return WatchUi.loadResource(Rez.Strings.NotPaired); }
    static function paired() { return WatchUi.loadResource(Rez.Strings.Paired); }
    static function pending() { return WatchUi.loadResource(Rez.Strings.Pending); }
    static function pairingCode() { return WatchUi.loadResource(Rez.Strings.PairingCode); }
    static function pairingHint() { return WatchUi.loadResource(Rez.Strings.PairingHint); }
    static function pairFailed() { return WatchUi.loadResource(Rez.Strings.PairFailed); }
    static function syncOk() { return WatchUi.loadResource(Rez.Strings.SyncOk); }
    static function syncFailed() { return WatchUi.loadResource(Rez.Strings.SyncFailed); }
    static function noBackend() { return WatchUi.loadResource(Rez.Strings.NoBackend); }

    static function alertStarted() { return WatchUi.loadResource(Rez.Strings.AlertStarted); }
    static function alertPaused() { return WatchUi.loadResource(Rez.Strings.AlertPaused); }
    static function alertResumed() { return WatchUi.loadResource(Rez.Strings.AlertResumed); }
    static function alertStopped() { return WatchUi.loadResource(Rez.Strings.AlertStopped); }

    static function wordPace() { return WatchUi.loadResource(Rez.Strings.WordPace); }
    static function wordDistance() { return WatchUi.loadResource(Rez.Strings.WordDistance); }
    static function wordTime() { return WatchUi.loadResource(Rez.Strings.WordTime); }
    static function wordLastLap() { return WatchUi.loadResource(Rez.Strings.WordLastLap); }
    static function wordPer() { return WatchUi.loadResource(Rez.Strings.WordPer); }
    static function wordRemaining() { return WatchUi.loadResource(Rez.Strings.WordRemaining); }
    static function wordAheadBy() { return WatchUi.loadResource(Rez.Strings.WordAheadBy); }
    static function wordBehindBy() { return WatchUi.loadResource(Rez.Strings.WordBehindBy); }
    static function wordAverage() { return WatchUi.loadResource(Rez.Strings.WordAverage); }
    static function wordLap() { return WatchUi.loadResource(Rez.Strings.WordLap); }
    static function wordHeart() { return WatchUi.loadResource(Rez.Strings.WordHeart); }
    static function wordZone() { return WatchUi.loadResource(Rez.Strings.WordZone); }
    static function wordTarget() { return WatchUi.loadResource(Rez.Strings.WordTarget); }
    static function wordPlan() { return WatchUi.loadResource(Rez.Strings.WordPlan); }
    static function wordGps() { return WatchUi.loadResource(Rez.Strings.WordGps); }
    static function wordSync() { return WatchUi.loadResource(Rez.Strings.WordSync); }
    static function wordSettings() { return WatchUi.loadResource(Rez.Strings.WordSettings); }

    static function wordAutoPause() { return WatchUi.loadResource(Rez.Strings.WordAutoPause); }
    static function finished() { return WatchUi.loadResource(Rez.Strings.Finished); }
    static function newWorkoutHint() { return WatchUi.loadResource(Rez.Strings.NewWorkoutHint); }

    static function lapLabel() { return WatchUi.loadResource(Rez.Strings.LapLabel); }
    static function lapCurrent() { return WatchUi.loadResource(Rez.Strings.LapCurrent); }
    static function configMissingDistance() { return WatchUi.loadResource(Rez.Strings.ConfigMissingDistance); }
    static function configMissingTime() { return WatchUi.loadResource(Rez.Strings.ConfigMissingTime); }
}