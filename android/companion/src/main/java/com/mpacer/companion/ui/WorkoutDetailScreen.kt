package com.mpacer.companion.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.companion.UiState

/**
 * Detail d une seance : allure moyenne, distance, duree, tours, et les deux actions
 * utiles (renvoyer vers la montre, ouvrir la page web du backend).
 */
@Composable
fun WorkoutDetailScreen(
    state: UiState,
    onBack: () -> Unit,
    onSendToWatch: () -> Unit,
    onOpenWeb: (String) -> Unit,
) {
    val detail = state.detail
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text("Detail de la seance", fontSize = 22.sp, fontWeight = FontWeight.Bold)

        if (detail == null) {
            if (state.busy) {
                CircularProgressIndicator()
            } else {
                Text("Seance indisponible", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        } else {
            Card(modifier = Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier.padding(12.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Text(Format.date(detail.workout.startedAtMs), fontWeight = FontWeight.Bold)
                    Text("Distance : " + Format.distance(detail.workout.distanceM))
                    Text("Duree : " + Format.duration(detail.workout.durationS))
                    Text("Allure moyenne : " + Format.pace(detail.workout.averagePaceSPerKm) + " /km")
                    detail.summary?.let { summary ->
                        Text("Tours : " + summary.laps.size + "   Points GPS : " + summary.track.size)
                    }
                }
            }

            detail.summary?.laps?.takeIf { laps -> laps.isNotEmpty() }?.let { laps ->
                Text("Tours", fontWeight = FontWeight.Bold)
                laps.forEach { lap ->
                    Text(
                        "Tour " + lap.index + " : " + Format.distance(lap.distanceM) +
                            "   " + Format.pace(lap.paceSPerKm) + " /km"
                    )
                }
            }

            Button(onClick = onSendToWatch, modifier = Modifier.fillMaxWidth()) {
                Text("Envoyer vers la montre")
            }
            OutlinedButton(
                onClick = { onOpenWeb(detail.workout.id) },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Ouvrir sur le site")
            }
        }

        state.message?.let { message ->
            Text(message, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }

        OutlinedButton(onClick = onBack, modifier = Modifier.fillMaxWidth()) {
            Text("Retour")
        }
    }
}
