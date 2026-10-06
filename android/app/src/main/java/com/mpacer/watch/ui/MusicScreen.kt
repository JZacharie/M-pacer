package com.mpacer.watch.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.Button
import androidx.wear.compose.material.ButtonDefaults
import androidx.wear.compose.material.Chip
import androidx.wear.compose.material.ChipDefaults
import androidx.wear.compose.material.Text
import com.mpacer.watch.TrackingService
import com.mpacer.watch.music.LocalPlaylist
import com.mpacer.watch.music.MusicLibrary
import com.mpacer.watch.music.MusicLibraryState
import com.mpacer.watch.music.MusicPlayer
import com.mpacer.watch.music.MusicPlayerState
import com.mpacer.watch.music.MusicSession
import kotlinx.coroutines.launch
import java.util.Locale
import kotlin.math.roundToInt

/**
 * Ecran Musique de la montre (docs/07 v2, section 6.4).
 *
 * Deux panneaux dans un ecran rond :
 *  1. Bibliotheque (USB) : playlists copiees par `mpacer-music`, espace libre et
 *     utilise, bouton « Importer (USB) », suppression d'une playlist importee ;
 *  2. Lecture : piste en cours, BPM de la piste et consigne du moteur, commandes.
 *
 * La montre ne joue que des fichiers presents sur son disque : aucun acces
 * reseau, aucun pilotage d'une application tierce. Le coeur Rust reste seul
 * decideur du tempo ; cet ecran ne fait qu'afficher son etat et pousser la
 * playlist choisie.
 */
@Composable
fun MusicScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val library by MusicLibrary.state.collectAsState()
    val player by MusicPlayer.state.collectAsState()
    val watch by TrackingService.state.collectAsState()
    var showPlayer by remember { mutableStateOf(false) }
    var aSupprimer by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) {
        MusicLibrary.reload(context)
        MusicPlayer.ensure(context)
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 10.dp, vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(if (showPlayer) "Lecture" else "Bibliotheque (USB)", fontWeight = FontWeight.Bold)

        if (showPlayer) {
            PlayerPane(
                player = player,
                targetBpm = watch.output?.music?.targetBpm,
                trackBpm = watch.output?.music?.current?.bpm ?: player.track?.bpm,
                cadence = watch.output?.music?.cadenceSpm,
                onToggle = { MusicPlayer.toggle() },
                onNext = { MusicPlayer.next() },
                onPrevious = { MusicPlayer.previous() },
                onLibrary = { showPlayer = false },
            )
        } else {
            LibraryPane(
                library = library,
                onImport = { scope.launch { MusicLibrary.scan(context) } },
                onPlay = { playlist ->
                    MusicSession.setPlaylist(playlist)
                    MusicPlayer.play(playlist)
                    showPlayer = true
                },
                aSupprimer = aSupprimer,
                onAskDelete = { id -> aSupprimer = id },
                onDelete = { playlist ->
                    scope.launch {
                        MusicLibrary.delete(context, playlist.id)
                    }
                    if (MusicSession.local?.id == playlist.id) MusicSession.setPlaylist(null)
                    aSupprimer = null
                },
            )
        }

        Button(onClick = onBack, colors = ButtonDefaults.secondaryButtonColors()) { Text("Retour") }
    }
}

// ------------------------------------------------------------- bibliotheque

@Composable
private fun LibraryPane(
    library: MusicLibraryState,
    onImport: () -> Unit,
    onPlay: (LocalPlaylist) -> Unit,
    aSupprimer: String?,
    onAskDelete: (String) -> Unit,
    onDelete: (LocalPlaylist) -> Unit,
) {
    Chip(
        label = { Text(if (library.busy) "Analyse du dossier..." else "Importer (USB)") },
        enabled = !library.busy,
        colors = ChipDefaults.primaryChipColors(),
        onClick = onImport,
    )
    Text(
        text = "libre " + formatBytes(library.freeBytes) + "   utilise " + formatBytes(library.usedBytes),
        color = Palette.muted,
        fontSize = 12.sp,
    )
    if (!library.usbAvailable) {
        Text(
            "Dossier Music/ absent : branchez la montre et lancez mpacer-music transfer.",
            color = Palette.attention,
            textAlign = TextAlign.Center,
            fontSize = 11.sp,
        )
    }

    if (library.playlists.isEmpty()) {
        Text(
            "Aucune playlist importee. Sur l'ordinateur : mpacer-music transfer, puis Importer (USB).",
            color = Palette.muted,
            textAlign = TextAlign.Center,
            fontSize = 12.sp,
        )
    }

    library.playlists.forEach { playlist ->
        PlaylistRow(
            playlist = playlist,
            confirmation = aSupprimer == playlist.id,
            onPlay = { onPlay(playlist) },
            onAskDelete = { onAskDelete(playlist.id) },
            onDelete = { onDelete(playlist) },
        )
    }

    library.message?.let { message ->
        Text(message, color = Palette.muted, textAlign = TextAlign.Center, fontSize = 12.sp)
    }
}

@Composable
private fun PlaylistRow(
    playlist: LocalPlaylist,
    confirmation: Boolean,
    onPlay: () -> Unit,
    onAskDelete: () -> Unit,
    onDelete: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Text(playlist.name, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center, fontSize = 14.sp)
        Text(
            text = playlist.trackCount.toString() + " p.  " + formatBytes(playlist.sizeBytes),
            color = Palette.muted,
            fontSize = 11.sp,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (playlist.playable.isNotEmpty()) {
                Chip(
                    label = { Text("Lire") },
                    colors = ChipDefaults.primaryChipColors(),
                    onClick = onPlay,
                )
            }
            if (confirmation) {
                Chip(
                    label = { Text("Confirmer") },
                    colors = ChipDefaults.primaryChipColors(),
                    onClick = onDelete,
                )
            } else {
                Chip(label = { Text("Supprimer") }, onClick = onAskDelete)
            }
        }
    }
}

// ------------------------------------------------------------------ lecture

@Composable
private fun PlayerPane(
    player: MusicPlayerState,
    targetBpm: Double?,
    trackBpm: Double?,
    cadence: Double?,
    onToggle: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onLibrary: () -> Unit,
) {
    Text(
        text = player.track?.title ?: "Aucune piste",
        fontWeight = FontWeight.Bold,
        textAlign = TextAlign.Center,
        fontSize = 15.sp,
    )
    player.track?.artist?.takeIf { it.isNotBlank() }?.let {
        Text(it, color = Palette.muted, textAlign = TextAlign.Center, fontSize = 12.sp)
    }
    Text(
        text = "piste " + bpmLabel(trackBpm) + "   cible " + bpmLabel(targetBpm),
        color = Palette.muted,
        fontSize = 12.sp,
    )
    cadence?.let { Text("cadence " + it.roundToInt() + " pas/min", color = Palette.muted2, fontSize = 11.sp) }
    player.playlistName?.let {
        Text(it, color = Palette.muted2, fontSize = 10.sp, textAlign = TextAlign.Center)
    }

    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Chip(label = { Text("<<") }, onClick = onPrevious)
        Chip(
            label = { Text(if (player.playing) "Pause" else "Lire") },
            colors = ChipDefaults.primaryChipColors(),
            onClick = onToggle,
        )
        Chip(label = { Text(">>") }, onClick = onNext)
    }

    player.message?.let {
        Text(it, color = Palette.attention, textAlign = TextAlign.Center, fontSize = 11.sp)
    }

    Chip(label = { Text("Bibliotheque") }, onClick = onLibrary)
}

// --------------------------------------------------------------- utilitaires

private fun bpmLabel(value: Double?): String =
    if (value == null || value <= 0.0) "--" else value.roundToInt().toString() + " BPM"

private fun formatBytes(bytes: Long): String = when {
    bytes >= 1024L * 1024L * 1024L -> String.format(Locale.ROOT, "%.1f Go", bytes / (1024.0 * 1024.0 * 1024.0))
    bytes >= 1024L * 1024L -> String.format(Locale.ROOT, "%.0f Mo", bytes / (1024.0 * 1024.0))
    bytes >= 1024L -> String.format(Locale.ROOT, "%.0f Ko", bytes / 1024.0)
    else -> bytes.toString() + " o"
}
