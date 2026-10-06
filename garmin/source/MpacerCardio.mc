//! Fréquence cardiaque : mesures et zones.
//!
//! Portage de la partie « zones » de `crates/mpacer-core/src/cardio.rs` : cinq
//! zones définies en pourcentage de la FC max, calculées sur la montre.

class MpacerCardio {

    var maxBpm;

    function initialize(maxBpm) {
        self.maxBpm = maxBpm;
    }

    // Bornes basses des cinq zones, en fraction de la FC max.
    function fractions() {
        return [
            [0.50, 0.60],
            [0.60, 0.70],
            [0.70, 0.80],
            [0.80, 0.90],
            [0.90, 1.00]
        ];
    }

    // Numéro de zone (1 à 5) ; 0 si la fréquence est sous la zone 1.
    function zoneOf(bpm) {
        if (bpm == null || self.maxBpm <= 0) {
            return 0;
        }
        var bounds = self.fractions();
        var max = self.maxBpm.toFloat();
        if (bpm < bounds[0][0] * max) {
            return 0;
        }
        for (var i = 0; i < bounds.size(); i++) {
            if (bpm < bounds[i][1] * max) {
                return i + 1;
            }
        }
        return 5;
    }
}
