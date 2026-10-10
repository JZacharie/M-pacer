package com.mpacer.companion.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.companion.Stats
import com.mpacer.companion.UiState
import com.mpacer.companion.WorkoutRow

/**
 * Liste des seances synchronisees (GET /api/v1/workouts) et statistiques 30 jours.
 * Un appui sur une carte ouvre le detail ; les actions en haut couvrent le
 * rafraichissement, l import d un fichier et l installation de l application montre.
 */
@Composable
fun WorkoutListScreen(
    state: UiState,
    onRefresh: () -> Unit,
    onOpen: (String) -> Unit,
    onImport: () -> Unit,
    onInstallWatch: () -> Unit,
    onDisconnect: () -> Unit,
    onOpenMusic: () -> Unit,
) {
    val context = LocalContext.current
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Tabs(actif = Onglet.Seances, onOpenWorkouts = {}, onOpenMusic = onOpenMusic)

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column {
                Text("M-pacer", fontSize = 24.sp, fontWeight = FontWeight.Bold)
                Text(
                    state.me?.email ?: state.baseUrl,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                // Version et jour de compilation : l'application d'appoint n'a
                // pas d'ecran Reglages, l'en-tete de la liste fait office.
                Text(
                    versionAffichee(context),
                    fontSize = 11.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (state.busy) {
                CircularProgressIndicator(modifier = Modifier.size(24.dp))
            }
        }

        state.stats?.let { stats -> StatsCard(stats) }

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onRefresh) { Text("Rafraichir") }
            OutlinedButton(onClick = onImport) { Text("Envoyer un fichier") }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onInstallWatch) { Text("Installer la montre") }
            OutlinedButton(onClick = onDisconnect) { Text("Deconnexion") }
        }

        state.message?.let { message ->
            Text(message, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }

        if (state.workouts.isEmpty()) {
            Text(
                "Aucune seance synchronisee pour le moment.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(state.workouts) { workout ->
                    WorkoutCard(workout, onClick = { onOpen(workout.id) })
                }
            }
        }
    }
}

@Composable
private fun StatsCard(stats: Stats) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(stats.workoutCount.toString() + " seances")
            Text(Format.distance(stats.totalDistanceM))
            Text(Format.duration(stats.totalDurationS))
        }
    }
}

@Composable
private fun WorkoutCard(workout: WorkoutRow, onClick: () -> Unit) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick),
    ) {
        Column(
            modifier = Modifier.padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(Format.date(workout.startedAtMs), fontWeight = FontWeight.Bold)
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(Format.distance(workout.distanceM))
                Text(Format.duration(workout.durationS))
                Text(Format.pace(workout.averagePaceSPerKm) + " /km")
            }
        }
    }
}
