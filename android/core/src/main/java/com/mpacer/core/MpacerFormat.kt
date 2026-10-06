package com.mpacer.core

import java.util.Locale

/** Formatage d'affichage. Les valeurs viennent du coeur, seul le rendu est local. */
object MpacerFormat {

    fun pace(secondsPerUnit: Double?): String {
        if (secondsPerUnit == null || secondsPerUnit <= 0.0 || secondsPerUnit.isNaN()) return "--:--"
        val total = Math.round(secondsPerUnit)
        return String.format(Locale.ROOT, "%d:%02d", total / 60, total % 60)
    }

    fun duration(seconds: Double): String {
        val total = Math.round(seconds)
        val hours = total / 3600
        val minutes = (total % 3600) / 60
        val secs = total % 60
        return if (hours > 0) {
            String.format(Locale.ROOT, "%d:%02d:%02d", hours, minutes, secs)
        } else {
            String.format(Locale.ROOT, "%d:%02d", minutes, secs)
        }
    }

    fun distance(meters: Double, imperial: Boolean = false): String =
        if (imperial) {
            String.format(Locale.ROOT, "%.2f mi", meters / 1609.344)
        } else {
            String.format(Locale.ROOT, "%.2f km", meters / 1000.0)
        }

    /**
     * Ligne unique pour la notification systeme.
     *
     * La frequence cardiaque n'est ajoutee que si la montre en fournit une : sans
     * capteur, la ligne reste exactement celle d'avant.
     */
    fun summaryLine(output: EngineOutput): String = buildString {
        append(pace(output.currentPace))
        append("  ")
        append(distance(output.distanceM))
        append("  ")
        append(duration(output.elapsedS))
        output.heartRateBpm?.let {
            append("  ")
            append(it)
        }
    }
}
