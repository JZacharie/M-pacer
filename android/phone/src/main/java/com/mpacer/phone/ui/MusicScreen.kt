package com.mpacer.phone.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.core.music.LocalPlaylist
import com.mpacer.core.music.MusicDownloader
import com.mpacer.core.music.MusicLibrary
import com.mpacer.core.music.MusicPlayer
import com.mpacer.core.music.MusicSession
import com.mpacer.core.ui.Palette
import kotlinx.coroutines.launch

/**
 * Musique locale du telephone (docs/07 v2).
 *
 * Meme contrat que la montre : le moteur Rust decide quelle piste jouer et a quel
 * tempo ; l'application ne joue que des fichiers presents sur son disque, copies
 * par USB dans son dossier Music/ avec leur manifest.json (outil PC mpacer-music).
 * Aucun flux, aucun DRM, aucun appel reseau pendant la course.
 */
@Composable
fun MusicScreen() {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val library by MusicLibrary.state.collectAsState()
    val player by MusicPlayer.state.collectAsState()
    val telechargement by MusicDownloader.state.collectAsState()

    LaunchedEffect(Unit) {
        MusicPlayer.prepare(context)
        MusicLibrary.reload(context)
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("Musique", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, color = Palette.texte)
        Text(
            "Fichiers locaux : " + (MusicLibrary.usbDirectory(context)?.absolutePath ?: "dossier indisponible"),
            color = Palette.muted,
            fontSize = 11.sp,
        )

        Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Text("Lecture", color = Palette.muted, fontSize = 12.sp)
                Text(
                    player.track?.let { (it.title + (it.artist?.let { artiste -> " - " + artiste } ?: "")) }
                        ?: "Aucune piste",
                    color = Palette.texte,
                    fontWeight = FontWeight.Medium,
                )
                Text(
                    player.playlistName?.let { it + "  " + (player.index + 1) + "/" + player.count } ?: "",
                    color = Palette.muted,
                    fontSize = 12.sp,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    TextButton(onClick = { MusicPlayer.previous() }) { Text("Precedent") }
                    TextButton(onClick = { MusicPlayer.toggle() }) {
                        Text(if (player.playing) "Pause" else "Lire")
                    }
                    TextButton(onClick = { MusicPlayer.next() }) { Text("Suivant") }
                }
            }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Button(
                onClick = { scope.launch { MusicLibrary.scan(context) } },
                enabled = !library.busy,
                colors = ButtonDefaults.buttonColors(
                    containerColor = Palette.orange,
                    contentColor = Color.White,
                ),
            ) {
                Text(if (library.busy) "Import..." else "Importer (USB)")
            }
            Text(
                "espace utilise " + (library.usedBytes / (1024 * 1024)) + " Mo / libre " +
                    (library.freeBytes / (1024 * 1024)) + " Mo",
                color = Palette.muted,
                fontSize = 11.sp,
            )
        }

        // Depot Wi-Fi (docs/16) : recupere les MP3 deposes sur la page /music,
        // puis acquitte chaque piste pour liberer le serveur.
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Button(
                onClick = { MusicDownloader.syncInBackground(context) },
                enabled = !telechargement.busy,
                colors = ButtonDefaults.buttonColors(
                    containerColor = Palette.surface2,
                    contentColor = Palette.texte,
                ),
            ) {
                Text(if (telechargement.busy) "Telechargement..." else "Telecharger (serveur)")
            }
            if (telechargement.busy && telechargement.total > 0) {
                Text(
                    telechargement.current.toString() + "/" + telechargement.total + "  " +
                        (telechargement.bytes / (1024 * 1024)) + " Mo",
                    color = Palette.muted,
                    fontSize = 11.sp,
                )
            }
        }
        telechargement.message
            ?.takeIf { telechargement.busy || it != "Aucun fichier a telecharger" }
            ?.let { Text(it, color = Palette.muted, fontSize = 13.sp) }

        library.message?.let { Text(it, color = Palette.muted, fontSize = 13.sp) }

        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(library.playlists) { playlist ->
                CartePlaylist(
                    playlist = playlist,
                    active = MusicSession.local?.id == playlist.id,
                    onPlay = {
                        MusicSession.setPlaylist(playlist)
                        MusicPlayer.play(playlist)
                    },
                    onDelete = {
                        scope.launch {
                            MusicLibrary.delete(context, playlist.id)
                            if (MusicSession.local?.id == playlist.id) MusicSession.setPlaylist(null)
                        }
                    },
                )
            }
        }
    }
}

@Composable
private fun CartePlaylist(
    playlist: LocalPlaylist,
    active: Boolean,
    onPlay: () -> Unit,
    onDelete: () -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = if (active) Palette.surface2 else Palette.surface,
        ),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(playlist.name, color = Palette.texte, fontSize = 15.sp)
                Text(
                    playlist.trackCount.toString() + " titre(s), " + playlist.playable.size +
                        " jouable(s)" + (playlist.targetBpm?.let { "  cible " + it.toInt() + " BPM" } ?: ""),
                    color = Palette.muted,
                    fontSize = 12.sp,
                )
            }
            TextButton(onClick = onPlay) { Text(if (active) "Relire" else "Jouer") }
            TextButton(onClick = onDelete) { Text("Suppr.", color = Palette.danger) }
        }
    }
}
