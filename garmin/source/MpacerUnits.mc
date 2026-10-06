//! Unités, conversions et formatage des valeurs affichées.
//!
//! Portage fidèle de `crates/mpacer-core/src/units.rs`. Le shell Monkey C ne
//! calcule rien de plus : ce fichier ne dépend que de Toybox.Math et n'utilise
//! aucune API de montre.

using Toybox.Math;

class MpacerUnits {

    static function metersPerKm() { return 1000.0; }

    static function metersPerMile() { return 1609.344; }

    static function metersPerUnit(imperial) {
        if (imperial) {
            return metersPerMile();
        }
        return metersPerKm();
    }

    static function label(imperial) {
        if (imperial) {
            return "mi";
        }
        return "km";
    }

    static function paceLabel(imperial) {
        if (imperial) {
            return "min/mi";
        }
        return "min/km";
    }

    // Distance (m) -> unités du système choisi.
    static function distanceInUnits(meters, imperial) {
        return meters / metersPerUnit(imperial);
    }

    // Vitesse (m/s) -> allure (s / unité). `null` sous 0,05 m/s, comme le cœur Rust.
    static function paceFromSpeed(speedMps, imperial) {
        if (speedMps == null) {
            return null;
        }
        if (speedMps <= 0.05) {
            return null;
        }
        return metersPerUnit(imperial) / speedMps;
    }

    // Allure (s / unité) -> vitesse (m/s).
    static function speedFromPace(paceSPerUnit, imperial) {
        if (paceSPerUnit == null || paceSPerUnit <= 0.0) {
            return null;
        }
        return metersPerUnit(imperial) / paceSPerUnit;
    }

    // ---------------------------------------------------------------- formatage

    static function pad2(value) {
        var n = (Math.round(value)).abs().toNumber();
        if (n < 10) {
            return "0" + n.toString();
        }
        return n.toString();
    }

    static function padN(value, width) {
        var text = value.toString();
        while (text.length() < width) {
            text = "0" + text;
        }
        return text;
    }

    // Durée en secondes : "42:15" (ou "1:23:45" au-delà d'une heure).
    static function formatDuration(seconds) {
        if (seconds == null) {
            return "--:--";
        }
        var negative = seconds < 0.0;
        var total = Math.round((seconds).abs()).toNumber();
        var hours = total / 3600;
        var minutes = (total % 3600) / 60;
        var secs = total % 60;
        var body = minutes.toString() + ":" + pad2(secs);
        if (hours > 0) {
            body = hours.toString() + ":" + pad2(minutes) + ":" + pad2(secs);
        }
        if (negative) {
            return "-" + body;
        }
        return body;
    }

    // Allure en s / unité : "5:41", ou "--:--" si inexploitable.
    static function formatPace(secondsPerUnit) {
        if (secondsPerUnit == null) {
            return "--:--";
        }
        if (secondsPerUnit < 0.0) {
            return "--:--";
        }
        var total = Math.round(secondsPerUnit).toNumber();
        return (total / 60).toString() + ":" + pad2(total % 60);
    }

    // Nombre à N décimales, sans dépendre des spécifications de Lang.format.
    static function formatDecimal(value, decimals) {
        var factor = 1;
        for (var i = 0; i < decimals; i++) {
            factor = factor * 10;
        }
        var scaled = Math.round(value * factor.toFloat()).toNumber();
        var negative = scaled < 0;
        if (negative) {
            scaled = -scaled;
        }
        var text = (scaled / factor).toString() + "." + padN(scaled % factor, decimals);
        if (negative) {
            return "-" + text;
        }
        return text;
    }

    // Distance affichée : "10.05 km".
    static function formatDistance(meters, imperial) {
        return formatDecimal(distanceInUnits(meters, imperial), 2) + " " + label(imperial);
    }

    // Distance courte : "350 m" en métrique sous le kilomètre, sinon deux décimales.
    static function formatDistanceShort(meters, imperial) {
        if (!imperial && (meters).abs() < 1000.0) {
            return Math.round(meters).toString() + " m";
        }
        return formatDecimal(distanceInUnits(meters, imperial), 2) + " " + label(imperial);
    }
}