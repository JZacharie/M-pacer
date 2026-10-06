//! Géométrie : distance orthodromique (Haversine).
//!
//! Portage de `crates/mpacer-core/src/geo.rs`. Le filtre de plausibilité vit
//! dans MpacerGpsMonitor, comme côté Rust.

using Toybox.Math;

class MpacerGeo {

    static function pi() { return 3.141592653589793; }

    static function earthRadiusM() { return 6371008.8; }

    static function toRadians(degrees) {
        return degrees * pi() / 180.0;
    }

    // Distance en mètres entre deux positions en degrés décimaux.
    static function haversineM(lat1, lon1, lat2, lon2) {
        var rlat1 = toRadians(lat1);
        var rlat2 = toRadians(lat2);
        var dlat = rlat2 - rlat1;
        var dlon = toRadians(lon2 - lon1);
        var sinLat = Math.sin(dlat / 2.0);
        var sinLon = Math.sin(dlon / 2.0);
        var h = sinLat * sinLat + Math.cos(rlat1) * Math.cos(rlat2) * sinLon * sinLon;
        var c = Math.sqrt(h);
        if (c > 1.0) { c = 1.0; }
        if (c < 0.0) { c = 0.0; }
        return 2.0 * earthRadiusM() * Math.asin(c);
    }
}
