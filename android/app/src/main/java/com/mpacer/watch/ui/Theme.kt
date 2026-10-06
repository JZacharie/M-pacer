package com.mpacer.watch.ui

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/**
 * Palette M-pacer, inspiree de Strava : un seul accent fort (l'orange signature)
 * sur une encre profonde. Sobriete volontaire, dans l'esprit des interfaces iOS :
 * peu de teintes, hierarchie portee par le poids typographique.
 */
object Palette {
    val orange = Color(0xFFFC4C02)
    val orangeProfond = Color(0xFFC93C00)

    val encre = Color(0xFF0B0D10)
    val surface = Color(0xFF16191F)
    val surface2 = Color(0xFF1D2129)

    val texte = Color(0xFFF3F5F8)
    val muted = Color(0xFF98A2B3)
    val muted2 = Color(0xFF6B7480)

    val ok = Color(0xFF2FBF71)
    val attention = Color(0xFFF7B955)
    val danger = Color(0xFFFF5A5F)

    // Zones de frequence cardiaque : bleu en endurance, rouge au seuil. Meme
    // convention que les montres et que la page d'analyse du backend.
    private val zone1 = Color(0xFF38BDF8)
    private val zone2 = Color(0xFF4ADE80)
    private val zone3 = Color(0xFFFACC15)
    private val zone4 = Color(0xFFFB923C)
    private val zone5 = Color(0xFFFF5A5F)

    /** Couleur d'une zone cardiaque (1 a 5) ; gris sans mesure. */
    fun zoneColor(zone: Int?): Color = when (zone) {
        1 -> zone1
        2 -> zone2
        3 -> zone3
        4 -> zone4
        5 -> zone5
        else -> muted
    }
}

/**
 * Voyant d'etat GPS : vert quand la position est fiable, orange pendant la
 * recherche (il pulse alors doucement), rouge si le signal est perdu.
 *
 * @param lumiere valeur renvoyee par le moteur ("Green", "Yellow", "Red" ou null).
 */
@Composable
fun GpsLight(lumiere: String?) {
    val couleur = when (lumiere) {
        "Green" -> Palette.ok
        "Yellow" -> Palette.attention
        "Red" -> Palette.danger
        else -> Palette.orange
    }
    val fixe = lumiere == "Green"
    val transition = rememberInfiniteTransition(label = "gps")
    val pulsation by transition.animateFloat(
        initialValue = 1f,
        targetValue = 0.35f,
        animationSpec = infiniteRepeatable(tween(900), RepeatMode.Reverse),
        label = "pulsation",
    )
    Box(
        modifier = Modifier
            .size(11.dp)
            .clip(CircleShape)
            .background(couleur.copy(alpha = if (fixe) 1f else pulsation)),
    )
}
