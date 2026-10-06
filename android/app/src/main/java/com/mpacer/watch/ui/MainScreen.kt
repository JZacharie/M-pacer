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
            AssistantPanel(output)
            Controls(output, onStart, onPause, onResume, onStop, onSettings, onSync)
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

@Composable
private fun Controls(
    output: EngineOutput?,
    onStart: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onStop: () -> Unit,
    onSettings: () -> Unit,
    onSync: () -> Unit,
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
    // trois boutons ne laissent pas la place a un mot long.
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
        when (state) {
            "Idle", "Finished" -> {
                Button(onClick = onStart, colors = principal) {
                    Text("Demarrer", fontSize = 12.sp, maxLines = 1)
                }
                Button(onClick = onSync, colors = secondaire) {
                    Text("Sync", fontSize = 13.sp, maxLines = 1)
                }
                Button(onClick = onSettings, colors = secondaire) {
                    Text("Reglages", fontSize = 10.sp, maxLines = 1)
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
