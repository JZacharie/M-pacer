package com.mpacer.companion.ui

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.companion.MusicPlaylistRow
import com.mpacer.companion.UiState

/**
 * Onglet Musique du telephone (docs/07 section 7.3).
 *
 * Deux actions, dans l'ordre du parcours reel :
 *  1. « Envoyer sur la montre » : met un plan de preparation en file par le
 *     Data Layer (/mpacer/music) et reveille la montre, qui telecharge en Wi-Fi ;
 *  2. « Televerser des fichiers » : endpoint appareil
 *     POST /api/v1/music/playlists (multipart name + files, jeton Bearer).
 *
 * Aucun octet audio ne transite par le telephone dans le premier cas : il ne
 * transporte qu'une fiche de quelques centaines d'octets.
 */
@Composable
fun MusicScreen(
    state: UiState,
    onRefresh: () -> Unit,
    onSendPlan: (MusicPlaylistRow) -> Unit,
    onUpload: (String, List<Uri>) -> Unit,
    onOpenPage: () -> Unit,
    onOpenWorkouts: () -> Unit,
) {
    var playlistName by remember { mutableStateOf("") }
    var picked by remember { mutableStateOf<List<Uri>>(emptyList()) }

    val picker = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenMultipleDocuments()
    ) { uris -> picked = uris }

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

        Text("Playlists preparees sur le serveur", fontWeight = FontWeight.Bold)
        if (state.musicPlaylists.isEmpty()) {
            Text(
                "Aucune playlist. Importez-en une depuis la page Musique du site.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            state.musicPlaylists.forEach { row ->
                MusicCard(row = row, onSend = { onSendPlan(row) })
            }
        }

        Card(modifier = Modifier.fillMaxWidth()) {
            Column(
                modifier = Modifier.padding(12.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text("Televerser des fichiers", fontWeight = FontWeight.Bold)
                Text(
                    "Les fichiers restent sur le serveur ; la montre ne telecharge que ce",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    "dont elle a besoin, a la demande.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                OutlinedTextField(
                    value = playlistName,
                    onValueChange = { playlistName = it },
                    label = { Text("Nom de la playlist") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedButton(
                    onClick = { picker.launch(arrayOf("audio/*")) },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        if (picked.isEmpty()) {
                            "Choisir des fichiers MP3/OGG/M4A"
                        } else {
                            picked.size.toString() + " fichier(s) choisi(s)"
                        }
                    )
                }
                Button(
                    onClick = { onUpload(playlistName, picked) },
                    enabled = picked.isNotEmpty(),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Televerser sur le serveur")
                }
            }
        }
    }
}

@Composable
private fun MusicCard(row: MusicPlaylistRow, onSend: () -> Unit) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(row.name, fontWeight = FontWeight.Bold)
            Text(
                text = row.source + "  " + row.trackCount + " titre(s)  " +
                    Format.bytes(row.totalBytes) +
                    "  BPM cible " + (row.targetBpm?.toInt()?.toString() ?: "auto"),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Button(onClick = onSend, modifier = Modifier.fillMaxWidth()) {
                Text("Envoyer sur la montre")
            }
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
