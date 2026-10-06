package com.mpacer.companion.ui

import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Formatage d affichage cote telephone.
 *
 * Les valeurs viennent du backend (et donc du coeur Rust) : on ne fait ici que
 * les rendre lisibles, jamais les recalculer.
 */
object Format {

    fun pace(secondsPerKm: Double?): String {
        if (secondsPerKm == null || secondsPerKm <= 0.0 || secondsPerKm.isNaN()) return "--:--"
        val total = Math.round(secondsPerKm)
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

    fun distance(meters: Double): String =
        String.format(Locale.ROOT, "%.2f km", meters / 1000.0)

    fun date(epochMs: Long): String =
        SimpleDateFormat("dd/MM/yyyy HH:mm", Locale.FRANCE).format(Date(epochMs))
}
