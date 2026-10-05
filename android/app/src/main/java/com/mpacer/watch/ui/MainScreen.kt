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
            .background(Color.Black),
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
                color = Color.White,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(MpacerFormat.distance(output?.distanceM ?: 0.0), color = Color.LightGray)
                Text(MpacerFormat.duration(output?.elapsedS ?: 0.0), color = Color.LightGray)
            }
            AssistantPanel(output)
            Controls(output, onStart, onPause, onResume, onStop, onSettings, onSync)
        }
    }
}

@Composable
private fun StatusLight(state: WatchState) {
    val color = when (state.output?.light) {
        "Green" -> Color(0xFF2ECC71)
        "Yellow" -> Color(0xFFF1C40F)
        "Red" -> Color(0xFFE74C3C)
        else -> Color(0xFFE67E22)
    }
    Box(
        modifier = Modifier
            .size(10.dp)
            .clip(CircleShape)
            .background(color)
    )
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
                shadow.onPlan -> Color.LightGray
                shadow.ahead -> Color(0xFF2ECC71)
                else -> Color(0xFFE67E22)
            },
            textAlign = TextAlign.Center,
        )
    } else {
        output?.estimatedFinishS?.let {
            Text("finish ${MpacerFormat.duration(it)}", color = Color.LightGray)
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
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        when (state) {
            "Idle", "Finished" -> {
                Button(onClick = onStart) { Text("Start") }
                Button(onClick = onSync) { Text("Sync") }
                Button(onClick = onSettings) { Text("Reglages") }
            }
            "Running" -> {
                Button(onClick = onPause) { Text("Pause") }
                Button(onClick = onStop, colors = ButtonDefaults.secondaryButtonColors()) { Text("Stop") }
            }
            else -> {
                Button(onClick = onResume) { Text("Reprendre") }
                Button(onClick = onStop, colors = ButtonDefaults.secondaryButtonColors()) { Text("Stop") }
            }
        }
    }
    MaterialTheme { /* theme sombre par defaut sur Wear OS */ }
}
