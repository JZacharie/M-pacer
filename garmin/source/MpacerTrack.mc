//! Trace GPS en mémoire et meilleures distances.
//!
//! Portage de `crates/mpacer-core/src/best_distances.rs`. Sur une montre,
//! chaque point coûte de la mémoire : la trace est donc stockée en **tableaux
//! parallèles** (nombres et flottants bruts) et non en dictionnaires, et le
//! nombre de points est plafonné.
//!
//! Le meilleur temps sur une distance est calculé par une fenêtre glissante en
//! O(n) par distance cible : la version Rust, en O(n²), serait trop lente sur
//! une montre pour une séance de trois heures.

class MpacerTrack {

    var ts;
    var dists;
    var lats;
    var lons;
    var alts;
    var maxPoints;
    var dropped;

    function initialize(maxPoints) {
        self.maxPoints = maxPoints;
        self.dropped = 0;
        self.clear();
    }

    function clear() {
        self.ts = [];
        self.dists = [];
        self.lats = [];
        self.lons = [];
        self.alts = [];
    }

    function size() {
        return self.ts.size();
    }

    function isFull() {
        return self.ts.size() >= self.maxPoints;
    }

    function push(tMs, distM, lat, lon, altM) {
        if (self.isFull()) {
            self.dropped = self.dropped + 1;
            return false;
        }
        self.ts.add(tMs);
        self.dists.add(distM);
        self.lats.add(lat);
        self.lons.add(lon);
        self.alts.add(altM);
        return true;
    }

    // Interpolation linéaire du temps (ms) à une distance donnée, à partir de
    // l'indice `fromIndex` : le pointeur n'avance que vers l'avant.
    function interpolateTimeMs(fromIndex, distM) {
        var count = self.ts.size();
        if (count < 2) {
            return null;
        }
        var index = fromIndex;
        while (index + 1 < count && self.dists[index + 1] <= distM) {
            index = index + 1;
        }
        if (index + 1 >= count) {
            return null;
        }
        var span = self.dists[index + 1] - self.dists[index];
        if (span <= 0.0) {
            return self.ts[index];
        }
        var ratio = (distM - self.dists[index]) / span;
        return self.ts[index] + (self.ts[index + 1] - self.ts[index]) * ratio;
    }

    // Meilleur temps réalisé sur chaque distance cible.
    // `targets` : tableau de `[distance_m, libellé]`.
    function bestEfforts(targets) {
        var count = self.ts.size();
        var results = [];
        if (count < 2) {
            return results;
        }
        var total = self.dists[count - 1];
        for (var t = 0; t < targets.size(); t++) {
            var target = targets[t][0];
            var label = targets[t][1];
            if (total < target) {
                continue;
            }
            var bestTimeS = null;
            var bestStartM = 0.0;
            var pointer = 0;
            for (var i = 1; i < count; i++) {
                if (self.dists[i] < target) {
                    continue;
                }
                var startDist = self.dists[i] - target;
                while (pointer + 1 < i && self.dists[pointer + 1] <= startDist) {
                    pointer = pointer + 1;
                }
                var startTimeMs = self.interpolateTimeMs(pointer, startDist);
                if (startTimeMs == null) {
                    continue;
                }
                var timeS = (self.ts[i] - startTimeMs) / 1000.0;
                if (timeS > 0.0 && (bestTimeS == null || timeS < bestTimeS)) {
                    bestTimeS = timeS;
                    bestStartM = startDist;
                }
            }
            if (bestTimeS != null) {
                results.add({
                    :label => label,
                    :distance_m => target,
                    :time_s => bestTimeS,
                    :start_dist_m => bestStartM
                });
            }
        }
        return results;
    }
}
