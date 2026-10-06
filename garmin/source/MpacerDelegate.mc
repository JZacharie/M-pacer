//! Boutons : START (démarrer / pause), LAP (tour manuel), haut/bas (écrans),
//! MENU (annonce immédiate, appairage ou envoi), retour (arrêt de la séance,
//! puis sortie).

using Toybox.WatchUi;

class MpacerDelegate extends WatchUi.BehaviorDelegate {

    var app;

    function initialize(app) {
        BehaviorDelegate.initialize();
        self.app = app;
    }

    function onKey(event) {
        var key = event.getKey();
        if (key == WatchUi.KEY_START) {
            return self.app.onStartKey();
        }
        if (key == WatchUi.KEY_LAP) {
            return self.app.onLapKey();
        }
        if (key == WatchUi.KEY_UP) {
            return self.app.onUpKey();
        }
        if (key == WatchUi.KEY_DOWN) {
            return self.app.onDownKey();
        }
        if (key == WatchUi.KEY_MENU) {
            return self.app.onMenuKey();
        }
        return false;
    }

    function onSelect() {
        return self.app.onSelectKey();
    }

    function onBack() {
        return self.app.onBackKey();
    }
}
