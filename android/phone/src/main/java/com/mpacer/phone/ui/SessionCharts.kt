package com.mpacer.phone.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.core.MpacerFormat
import com.mpacer.core.ui.Palette

/**
 * Petits graphiques de la fiche de seance, dessines au canevas.
 *
 * Aucune bibliotheque de graphiques : trois formes suffisent (une courbe, des
 * barres de zone, des barres de tour), et elles tiennent en quelques dizaines
 * de lignes. Les donnees viennent toutes du rapport du coeur Rust : ici, on ne
 * fait que les mettre en pixels.
 */

/**
 * Courbe d'une serie, avec son aire et le repere de la moyenne.
 *
 * @param points couples (abscisse, ordonnee) : distance et pouls, ou distance
 *   et altitude.
 * @param moyenne ordonnee de reference tracee en pointille.
 */
@Composable
fun Courbe(
    points: List<AnalyseSeance.Couple>,
    couleur: Color,
    modifier: Modifier = Modifier,
    moyenne: Double? = null,
) {
    Canvas(modifier) {
        if (points.size < 2) return@Canvas
        val minX = points.minOf { it.x }
        val maxX = points.maxOf { it.x }
        var minY = points.minOf { it.y }
        var maxY = points.maxOf { it.y }
        if (maxX <= minX || maxY <= minY) return@Canvas
        // Un peu d'air en haut et en bas : une courbe plate ne doit pas coller aux bords.
        val marge = ((maxY - minY) * 0.15).coerceAtLeast(1.0)
        minY -= marge
        maxY += marge
        val largeur = size.width
        val hauteur = size.height

        fun position(couple: AnalyseSeance.Couple) = Offset(
            ((couple.x - minX) / (maxX - minX) * largeur).toFloat(),
            (hauteur - (couple.y - minY) / (maxY - minY) * hauteur).toFloat(),
        )

        val trace = Path()
        val depart = position(points.first())
        trace.moveTo(depart.x, depart.y)
        for (index in 1 until points.size) {
            val point = position(points[index])
            trace.lineTo(point.x, point.y)
        }

        val aire = Path().apply {
            addPath(trace)
            lineTo(largeur, hauteur)
            lineTo(0f, hauteur)
            close()
        }
        drawPath(
            aire,
            Brush.verticalGradient(
                listOf(couleur.copy(alpha = 0.30f), couleur.copy(alpha = 0f)),
            ),
        )
        drawPath(
            trace,
            couleur,
            style = Stroke(width = 2.5.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
        moyenne?.let { reference ->
            val y = (hauteur - (reference - minY) / (maxY - minY) * hauteur).toFloat()
            drawLine(
                Palette.muted2.copy(alpha = 0.55f),
                Offset(0f, y),
                Offset(largeur, y),
                strokeWidth = 1.dp.toPx(),
            )
        }
    }
}

/** Temps passe dans chacune des cinq zones cardiaques. */
@Composable
fun BarresZones(cardio: AnalyseSeance.Cardio, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.Bottom,
    ) {
        cardio.secondesParZone.forEachIndexed { index, secondes ->
            Column(
                modifier = Modifier.weight(1f),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(MpacerFormat.duration(secondes), color = Palette.muted, fontSize = 10.sp)
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(70.dp),
                    contentAlignment = Alignment.BottomCenter,
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .fillMaxHeight(cardio.partZone(index).toFloat().coerceIn(0.03f, 1f))
                            .clip(RoundedCornerShape(4.dp))
                            .background(Palette.zoneColor(index + 1)),
                    )
                }
                Text("Z" + (index + 1), color = Palette.muted2, fontSize = 10.sp)
            }
        }
    }
}

/**
 * Un temps de passage : une barre proportionnelle a la vitesse, l'allure, et le
 * pouls quand la montre en a fourni.
 *
 * La barre represente la **vitesse** (et non l'allure) : la plus longue est donc
 * la plus rapide, ce qui se lit sans reflechir.
 */
@Composable
fun LigneTour(
    tour: AnalyseSeance.Tour,
    allureReference: Double,
    modifier: Modifier = Modifier,
) {
    val allure = tour.allureSPerKm.takeIf { it > 0.0 }
    // Une barre garde toujours une amorce visible, meme a l'allure la plus lente.
    val part = if (allure != null && allureReference > 0.0) {
        (allureReference / allure).toFloat().coerceIn(0.05f, 1f)
    } else {
        0.05f
    }
    // Trois teintes plutot que deux : un tour pile dans l'allure moyenne n'est
    // ni un bon point ni un avertissement.
    val ecart = allure?.minus(allureReference)
    val couleur = when {
        ecart == null -> Palette.muted
        ecart <= -2.0 -> Palette.ok
        ecart >= 2.0 -> Palette.attention
        else -> Palette.muted
    }

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            text = tour.index.toString(),
            color = Palette.muted2,
            fontSize = 12.sp,
            modifier = Modifier.width(18.dp),
        )
        Box(
            modifier = Modifier
                .weight(1f)
                .height(10.dp)
                .clip(RoundedCornerShape(5.dp))
                .background(Palette.surface2),
        ) {
            Box(
                modifier = Modifier
                    .fillMaxWidth(part)
                    .fillMaxHeight()
                    .clip(RoundedCornerShape(5.dp))
                    .background(couleur),
            )
        }
        Text(
            text = if (allure != null) MpacerFormat.pace(allure) else "--:--",
            color = Palette.texte,
            fontSize = 14.sp,
            fontWeight = FontWeight.Medium,
            modifier = Modifier.width(48.dp),
        )
        Text(
            text = tour.cardioMoyen?.let { it.toInt().toString() } ?: "",
            color = Palette.zoneColor(null),
            fontSize = 12.sp,
            modifier = Modifier.width(34.dp),
        )
    }
}

/**
 * Miniature du tracé GPS (silhouette vectorielle inspirée de Strava).
 * Dessinée au Canvas sans charger de WebView : rapide, léger et autonome.
 */
@Composable
fun MiniTraceGPX(
    points: List<PointCarte>,
    modifier: Modifier = Modifier,
    couleur: Color = Palette.orange,
) {
    Canvas(modifier = modifier) {
        if (points.size < 2) return@Canvas
        val minLat = points.minOf { it.lat }
        val maxLat = points.maxOf { it.lat }
        val minLon = points.minOf { it.lon }
        val maxLon = points.maxOf { it.lon }

        val deltaLat = (maxLat - minLat).coerceAtLeast(0.0001)
        val deltaLon = (maxLon - minLon).coerceAtLeast(0.0001)

        val padding = 8.dp.toPx()
        val availW = size.width - padding * 2
        val availH = size.height - padding * 2

        val scale = minOf(availW / deltaLon.toFloat(), availH / deltaLat.toFloat())
        val offsetX = padding + (availW - deltaLon.toFloat() * scale) / 2f
        val offsetY = padding + (availH - deltaLat.toFloat() * scale) / 2f

        fun proj(p: PointCarte): Offset {
            val x = offsetX + ((p.lon - minLon).toFloat() * scale)
            // Latitude inversée pour l'axe Y de l'écran
            val y = offsetY + ((maxLat - p.lat).toFloat() * scale)
            return Offset(x, y)
        }

        val path = Path()
        val start = proj(points.first())
        path.moveTo(start.x, start.y)
        for (i in 1 until points.size) {
            val pt = proj(points[i])
            path.lineTo(pt.x, pt.y)
        }

        // Tracé principal lumineux avec dégradé subtil
        drawPath(
            path = path,
            color = couleur,
            style = Stroke(
                width = 3.dp.toPx(),
                cap = StrokeCap.Round,
                join = StrokeJoin.Round,
            ),
        )

        // Point de départ (vert clair)
        drawCircle(
            color = Palette.ok,
            radius = 3.5.dp.toPx(),
            center = start,
        )

        // Point d'arrivée (orange ou rouge)
        val end = proj(points.last())
        drawCircle(
            color = Color.White,
            radius = 3.5.dp.toPx(),
            center = end,
        )
    }
}

