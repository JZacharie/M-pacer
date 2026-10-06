package com.mpacer.companion.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.companion.MusicPlaylistRow
import com.mpacer.companion.UiState

/**
 * Onglet Musique du telephone (docs/07 v2, section 6.5).
 *
 * Onglet **informatif** : il liste les playlists du backend et rappelle la
 * procedure de transfert. Aucun fichier audio ne passe par le telephone : les MP3
 * restent sur le disque de l'ordinateur et sont copies sur la montre par USB avec
 * l'outil `mpacer-music`.
 */
@Composable
fun MusicScreen(
    state: UiState,
    onRefresh: () -> Unit,
    onOpenPage: () -> Unit,
    onOpenWorkouts: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Tabs(actif = Onglet.Musique, onOpenWorkouts = onOpenWorkouts, onOpenMusic = {})

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Musique", fontSize = 22.sp, fontWeight = FontWeight.Bold)
            if (state.busy) {
                CircularProgressIndicator()
            }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = onRefresh) { Text("Rafraichir") }
            OutlinedButton(onClick = onOpenPage) { Text("Ouvrir la page Musique") }
        }

        state.message?.let { message ->
            Text(message, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }

        TransferCard()

        Text("Playlists du serveur", fontWeight = FontWeight.Bold)
        if (state.musicPlaylists.isEmpty()) {
            Text(
                "Aucune playlist. Importez-en une depuis la page Musique du site.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            state.musicPlaylists.forEach { row -> MusicCard(row) }
        }
    }
}

/** Rappel de la procedure USB : c'est elle qui amene la musique sur la montre. */
@Composable
private fun TransferCard() {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text("Transfert vers la montre (USB)", fontWeight = FontWeight.Bold)
            Text(
                "1. Sur la page Musique du site, telechargez le manifeste de la playlist.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                "2. Sur l'ordinateur : mpacer-music transfer --manifest run-170.json",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                "    --folder \"D:\\Musique\\Course\"",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                "3. Branchez la montre en USB, puis lancez la commande.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                "4. Sur la montre : Musique > Importer (USB).",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                "Les fichiers audio restent sur votre disque : le serveur ne stocke que les fiches.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun MusicCard(row: MusicPlaylistRow) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(row.name, fontWeight = FontWeight.Bold)
            val duree = row.durationS?.takeIf { it > 0.0 }?.let { "  " + Format.duration(it) } ?: ""
            Text(
                text = row.source + "  " + row.trackCount + " titre(s)" + duree +
                    "  BPM cible " + (row.targetBpm?.toInt()?.toString() ?: "auto"),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** Onglets partages entre la liste des seances et la musique. */
internal enum class Onglet { Seances, Musique }

@Composable
internal fun Tabs(actif: Onglet, onOpenWorkouts: () -> Unit, onOpenMusic: () -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (actif == Onglet.Seances) {
            Button(onClick = onOpenWorkouts) { Text("Seances") }
        } else {
            OutlinedButton(onClick = onOpenWorkouts) { Text("Seances") }
        }
        if (actif == Onglet.Musique) {
            Button(onClick = onOpenMusic) { Text("Musique") }
        } else {
            OutlinedButton(onClick = onOpenMusic) { Text("Musique") }
        }
    }
}
