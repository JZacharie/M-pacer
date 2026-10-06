package com.mpacer.watch.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.Button
import androidx.wear.compose.material.ButtonDefaults
import androidx.wear.compose.material.MaterialTheme
import androidx.wear.compose.material.Text
import com.mpacer.watch.EngineOutput
import com.mpacer.watch.MpacerFormat
import com.mpacer.watch.WatchState
import com.mpacer.watch.music.MusicDirective
import com.mpacer.watch.music.MusicState
import kotlin.math.roundToInt

/**
 * Ecran principal, pense pour un ecran rond :
 * 1. l'allure courante, tres grande (seule information vraiment lue en courant),
 * 2. distance et temps,
 * 3. panneau d'assistant (temps de finish estime ou ecart au shadow runner),
 * 4. feu de statut GPS.
 */
@Composable
fun MainScreen(
    state: WatchState,
    onStart: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onStop: () -> Unit,
    onSettings: () -> Unit,
    onSync: () -> Unit,
    onMusic: () -> Unit,
) {
    val output = state.output
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Palette.encre),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center,
            modifier = Modifier.padding(12.dp),
        ) {
            StatusLight(state)
            Text(
                text = MpacerFormat.pace(output?.currentPace),
                fontSize = 54.sp,
                fontWeight = FontWeight.Bold,
                color = Palette.texte,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(MpacerFormat.distance(output?.distanceM ?: 0.0), color = Palette.muted)
                Text(MpacerFormat.duration(output?.elapsedS ?: 0.0), color = Palette.muted)
            }
            HeartRateLine(output)
            AssistantPanel(output)
            MusicPill(output?.music)
            Controls(output, onStart, onPause, onResume, onStop, onSettings, onSync, onMusic)
        }
    }
}

@Composable
private fun StatusLight(state: WatchState) {
    GpsLight(state.output?.light)
}

@Composable
private fun AssistantPanel(output: EngineOutput?) {
    val shadow = output?.shadow
    if (shadow != null) {
        val text = if (shadow.onPlan) {
            "sur le plan"
        } else {
            val sign = if (shadow.ahead) "+" else "-"
            // Ecart en distance, plus parlant que l'ecart en temps pendant l'effort
            "$sign${Math.abs(shadow.distanceDeltaM).toInt()} m"
        }
        Text(
            text = text,
            color = when {
                shadow.onPlan -> Palette.muted
                shadow.ahead -> Palette.ok
                else -> Palette.orange
            },
            textAlign = TextAlign.Center,
        )
    } else {
        output?.estimatedFinishS?.let {
            Text("finish ${MpacerFormat.duration(it)}", color = Palette.muted)
        }
    }
}

/**
 * Frequence cardiaque et zone, quand la montre en fournit une.
 *
 * La couleur suit la zone (bleu en endurance, rouge au seuil) : c'est la
 * convention des montres, et celle de la page d'analyse du backend. Rien n'est
 * affiche si la montre n'a pas de capteur, ou si l'utilisateur a refuse la
 * permission : la seance reste complete.
 */
@Composable
private fun HeartRateLine(output: EngineOutput?) {
    val current = output ?: return
    val bpm = current.heartRateBpm ?: return
    val zone = current.heartRateZone
    Text(
        text = if (zone == null) bpm.toString() + " bpm" else bpm.toString() + " bpm  Z" + zone,
        color = Palette.zoneColor(zone),
        fontSize = 13.sp,
    )
}

/**
 * Pastille musique (docs/07 section 7.2) : BPM consigne par le moteur, avec la
 * fleche de la directive en cours. Rien n'est affiche si la musique est coupee.
 */
@Composable
private fun MusicPill(music: MusicState?) {
    val target = music?.targetBpm ?: return
    if (!music.enabled) return
    val fleche = when (music.directive) {
        MusicDirective.BOOST -> "  ^"
        MusicDirective.RELAX -> "  v"
        MusicDirective.SKIP_TO -> "  >>"
        else -> ""
    }
    Text(
        text = target.roundToInt().toString() + " BPM" + fleche,
        color = when (music.directive) {
            MusicDirective.BOOST -> Palette.orange
            MusicDirective.RELAX -> Palette.ok
            else -> Palette.muted
        },
        fontSize = 13.sp,
    )
}

@Composable
private fun Controls(
    output: EngineOutput?,
    onStart: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onStop: () -> Unit,
    onSettings: () -> Unit,
    onSync: () -> Unit,
    onMusic: () -> Unit,
) {
    val state = output?.state ?: "Idle"
    // Un seul bouton porte l'accent orange : l'action principale de l'ecran.
    val principal = ButtonDefaults.buttonColors(
        backgroundColor = Palette.orange,
        contentColor = Color.White,
    )
    val secondaire = ButtonDefaults.secondaryButtonColors(
        backgroundColor = Palette.surface2,
        contentColor = Palette.texte,
    )
    val arret = ButtonDefaults.secondaryButtonColors(
        backgroundColor = Palette.surface2,
        contentColor = Palette.danger,
    )
    // Les libelles sont raccourcis et reduits : sur un ecran rond de 450 px,
    // quatre boutons ne laissent pas la place a un mot long.
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
        when (state) {
            "Idle", "Finished" -> {
                Button(onClick = onStart, colors = principal) {
                    Text("Demarrer", fontSize = 12.sp, maxLines = 1)
                }
                Button(onClick = onMusic, colors = secondaire) {
                    Text("Musique", fontSize = 10.sp, maxLines = 1)
                }
                Button(onClick = onSync, colors = secondaire) {
                    Text("Sync", fontSize = 12.sp, maxLines = 1)
                }
                Button(onClick = onSettings, colors = secondaire) {
                    Text("Reglages", fontSize = 9.sp, maxLines = 1)
                }
            }
            "Running" -> {
                Button(onClick = onPause, colors = secondaire) {
                    Text("Pause", fontSize = 13.sp, maxLines = 1)
                }
                Button(onClick = onStop, colors = arret) {
                    Text("Stop", fontSize = 13.sp, maxLines = 1)
                }
            }
            else -> {
                Button(onClick = onResume, colors = principal) {
                    Text("Reprendre", fontSize = 11.sp, maxLines = 1)
                }
                Button(onClick = onStop, colors = arret) {
                    Text("Stop", fontSize = 13.sp, maxLines = 1)
                }
            }
        }
    }
}
