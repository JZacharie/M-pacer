//! Archive locale et état de synchronisation.
//!
//! `Application.Storage` est le seul stockage persistant d'une application
//! Connect IQ : 8 Ko par clé, 128 Ko au total. Chaque séance en attente occupe
//! donc **sa propre clé** et la trace GPS n'y est jamais conservée (elle vit
//! dans le fichier FIT enregistré par la montre). Le résumé reste petit :
//! tours, meilleures distances, pauses, plan, quelques mesures de cardio.

using Toybox.Application;

class MpacerArchive {

    var syncedIds;
    var pendingIds;

    function initialize() {
        self.syncedIds = [];
        self.pendingIds = [];
        self.load();
    }

    function load() {
        var synced = self.readArray("syncedIds");
        if (synced != null) {
            self.syncedIds = synced;
        }
        var pending = self.readArray("pendingIds");
        if (pending != null) {
            self.pendingIds = pending;
        }
    }

    function readArray(key) {
        try {
            var value = Application.Storage.getValue(key);
            if (value instanceof Lang.Array) {
                return value;
            }
        } catch (e) {
        }
        return null;
    }

    function readString(key) {
        try {
            var value = Application.Storage.getValue(key);
            if (value != null && value instanceof Lang.String) {
                return value;
            }
        } catch (e) {
        }
        return null;
    }

    function write(key, value) {
        try {
            Application.Storage.setValue(key, value);
            return true;
        } catch (e) {
            System.println("M-pacer : ecriture Storage impossible (" + key + ")");
            return false;
        }
    }

    // ------------------------------------------------------------------- jeton

    function token() { return self.readString("token"); }

    function setToken(value) { self.write("token", value); }

    function clearToken() {
        try {
            Application.Storage.deleteValue("token");
        } catch (e) {
        }
    }

    function isPaired() { return self.token() != null; }

    function baseUrl() { return self.readString("baseUrl"); }

    function setBaseUrl(value) { self.write("baseUrl", value); }

    // -------------------------------------------------------- séances en attente

    function pendingCount() { return self.pendingIds.size(); }

    function hasPending(id) {
        for (var i = 0; i < self.pendingIds.size(); i++) {
            if (self.pendingIds[i] == id) {
                return true;
            }
        }
        return false;
    }

    function isSynced(id) {
        for (var i = 0; i < self.syncedIds.size(); i++) {
            if (self.syncedIds[i] == id) {
                return true;
            }
        }
        return false;
    }

    function markSynced(id) {
        if (self.isSynced(id)) {
            return;
        }
        self.syncedIds.add(id);
        // L'historique des identifiants envoyés reste borné : au-delà de 200
        // séances, les plus anciennes sont oubliées (l'envoi étant idempotent,
        // un renvoi ne crée jamais de doublon).
        while (self.syncedIds.size() > 200) {
            self.syncedIds = self.syncedIds.slice(1);
        }
        self.write("syncedIds", self.syncedIds);
    }

    function addPending(summary) {
        var id = summary[:id];
        if (self.hasPending(id)) {
            return;
        }
        // La trace n'est jamais persistée (quota de 8 Ko par clé) : le résumé
        // envoyé plus tard reste complet, seule la trace GPS manque.
        var stored = self.compact(summary);
        if (!self.write("pending:" + id, stored)) {
            return;
        }
        self.pendingIds.add(id);
        while (self.pendingIds.size() > 20) {
            var oldest = self.pendingIds[0];
            self.pendingIds = self.pendingIds.slice(1);
            try {
                Application.Storage.deleteValue("pending:" + oldest);
            } catch (e) {
            }
        }
        self.write("pendingIds", self.pendingIds);
    }

    function compact(summary) {
        var copy = {};
        var keys = [
            :id, :started_at_ms, :duration_s, :distance_m, :average_pace_s_per_km,
            :laps, :best_efforts, :unit_system, :elapsed_s, :pauses, :plan
        ];
        for (var i = 0; i < keys.size(); i++) {
            var key = keys[i];
            if (summary[key] != null) {
                copy[key] = summary[key];
            }
        }
        // La cardio est conservée, mais sous-échantillonnée : au plus 200 mesures.
        var heart = summary[:heart_rate];
        if (heart != null && heart instanceof Lang.Array) {
            copy[:heart_rate] = self.decimate(heart, 200);
        }
        return copy;
    }

    function decimate(values, maximum) {
        if (values.size() <= maximum) {
            return values;
        }
        var result = [];
        var step = (values.size() / maximum).toNumber();
        if (step < 1) {
            step = 1;
        }
        var index = 0;
        while (index < values.size()) {
            result.add(values[index]);
            index = index + step;
        }
        return result;
    }

    function pending(id) {
        try {
            var value = Application.Storage.getValue("pending:" + id);
            if (value instanceof Lang.Dictionary) {
                return value;
            }
        } catch (e) {
        }
        return null;
    }

    function removePending(id) {
        try {
            Application.Storage.deleteValue("pending:" + id);
        } catch (e) {
        }
        var kept = [];
        for (var i = 0; i < self.pendingIds.size(); i++) {
            if (self.pendingIds[i] != id) {
                kept.add(self.pendingIds[i]);
            }
        }
        self.pendingIds = kept;
        self.write("pendingIds", self.pendingIds);
        self.markSynced(id);
    }

    function nextPendingId() {
        for (var i = 0; i < self.pendingIds.size(); i++) {
            var id = self.pendingIds[i];
            if (!self.isSynced(id)) {
                return id;
            }
        }
        return null;
    }
}