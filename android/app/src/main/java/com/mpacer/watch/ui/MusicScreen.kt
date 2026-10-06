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
import com.mpacer.watch.music.MusicEntry
import com.mpacer.watch.music.MusicLibrary
import com.mpacer.watch.music.MusicLibraryState
import com.mpacer.watch.music.MusicPlayer
import com.mpacer.watch.music.MusicPlayerState
import com.mpacer.watch.music.MusicSession
import com.mpacer.watch.music.PreparePlan
import com.mpacer.watch.music.SpotifyRemote
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

/**
 * Ecran Musique de la montre (docs/07 section 7.2).
 *
 * Deux panneaux dans un ecran rond :
 *  1. Bibliotheque : playlists du backend, preparation hors ligne (fiche Spotify
 *     ou fichiers personnels) et espace utilise ;
 *  2. Lecture : piste en cours, BPM de la piste et consigne du moteur, commandes.
 *
 * Le coeur Rust reste seul decideur du tempo : cet ecran ne fait qu'afficher son
 * etat et pousser la playlist choisie.
 */
@Composable
fun MusicScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val library by MusicLibrary.state.collectAsState()
    val player by MusicPlayer.state.collectAsState()
    val spotify by SpotifyRemote.state.collectAsState()
    val watch by TrackingService.state.collectAsState()
    var showPlayer by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        MusicLibrary.reloadLocal(context)
        MusicLibrary.reloadPlans(context)
        MusicLibrary.refresh(context)
        MusicLibrary.refreshPlan(context)
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
        Text(if (showPlayer) "Lecture" else "Bibliotheque", fontWeight = FontWeight.Bold)

        if (showPlayer) {
            PlayerPane(
                player = player,
                targetBpm = watch.output?.music?.targetBpm,
                trackBpm = watch.output?.music?.current?.bpm ?: player.track?.bpm,
                cadence = watch.output?.music?.cadenceSpm,
                spotifyTitle = spotify.title,
                spotifyArtist = spotify.artist,
                onToggle = {
                    if (player.source == "spotify") SpotifyRemote.toggle(context) else MusicPlayer.toggle()
                },
                onNext = {
                    if (player.source == "spotify") SpotifyRemote.next(context) else MusicPlayer.next()
                },
                onPrevious = {
                    if (player.source == "spotify") SpotifyRemote.previous(context) else MusicPlayer.previous()
                },
                onLibrary = { showPlayer = false },
            )
        } else {
            LibraryPane(
                library = library,
                onPreparePlan = { plan ->
                    scope.launch {
                        MusicLibrary.download(context, plan.playlistId)
                        MusicLibrary.ackPlan(context, plan.id)
                    }
                },
                onPrepare = { id -> scope.launch { MusicLibrary.download(context, id) } },
                onPlay = { entry ->
                    entry.local?.let { local ->
                        MusicSession.setPlaylist(local)
                        MusicPlayer.play(local)
                    }
                    showPlayer = true
                },
                onSpotify = {
                    SpotifyRemote.connect(context)
                    showPlayer = true
                },
                onAccessSettings = { SpotifyRemote.openAccessSettings(context) },
            )
        }

        Button(onClick = onBack, colors = ButtonDefaults.secondaryButtonColors()) { Text("Retour") }
    }
}

// ------------------------------------------------------------- bibliotheque

@Composable
private fun LibraryPane(
    library: MusicLibraryState,
    onPreparePlan: (PreparePlan) -> Unit,
    onPrepare: (String) -> Unit,
    onPlay: (MusicEntry) -> Unit,
    onSpotify: () -> Unit,
    onAccessSettings: () -> Unit,
) {
    library.pendingPlan?.let { plan ->
        Text("A preparer : " + plan.name.ifBlank { plan.playlistId }, textAlign = TextAlign.Center)
        Chip(
            label = { Text("Telecharger la course") },
            enabled = !library.busy,
            onClick = { onPreparePlan(plan) },
        )
    }

    if (library.busy) {
        Text("Preparation " + (library.progress * 100).roundToInt() + " %")
    }

    if (library.entries.isEmpty()) {
        Text(
            "Aucune playlist. Preparez-en une depuis le site ou le telephone.",
            color = Palette.muted,
            textAlign = TextAlign.Center,
            fontSize = 12.sp,
        )
    }

    library.entries.forEach { entry ->
        EntryRow(
            entry = entry,
            busy = library.busy,
            onPrepare = { onPrepare(entry.id) },
            onPlay = { onPlay(entry) },
            onSpotify = onSpotify,
        )
    }

    library.message?.let { message ->
        Text(message, color = Palette.muted, textAlign = TextAlign.Center, fontSize = 12.sp)
    }

    val bytes = library.entries.sumOf { it.local?.tracks?.sumOf { track -> track.sizeBytes } ?: 0L }
    Text("Espace utilise : " + formatBytes(bytes), color = Palette.muted2, fontSize = 11.sp)

    Chip(
        label = { Text("Acces notifications (Spotify)") },
        onClick = onAccessSettings,
    )
}

@Composable
private fun EntryRow(
    entry: MusicEntry,
    busy: Boolean,
    onPrepare: () -> Unit,
    onPlay: () -> Unit,
    onSpotify: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Text(entry.name, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center, fontSize = 14.sp)
        Text(
            text = entry.source + "  " + entry.trackCount + " p." +
                if (entry.downloaded) "  (sur la montre)" else "  " + formatBytes(entry.totalBytes),
            color = Palette.muted,
            fontSize = 11.sp,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (!entry.downloaded) {
                Chip(
                    label = { Text(if (busy) "..." else "Preparer") },
                    enabled = !busy,
                    onClick = onPrepare,
                )
            } else if (entry.source == "spotify") {
                Chip(
                    label = { Text("Spotify") },
                    colors = ChipDefaults.primaryChipColors(),
                    onClick = onSpotify,
                )
            } else {
                Chip(
                    label = { Text("Lire") },
                    colors = ChipDefaults.primaryChipColors(),
                    onClick = onPlay,
                )
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
    spotifyTitle: String?,
    spotifyArtist: String?,
    onToggle: () -> Unit,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onLibrary: () -> Unit,
) {
    val remote = player.source == "spotify"
    val title = if (remote) (spotifyTitle ?: "Spotify") else (player.track?.title ?: "Aucune piste")
    val artist = if (remote) spotifyArtist else player.track?.artist

    Text(title, fontWeight = FontWeight.Bold, textAlign = TextAlign.Center, fontSize = 15.sp)
    artist?.takeIf { it.isNotBlank() }?.let {
        Text(it, color = Palette.muted, textAlign = TextAlign.Center, fontSize = 12.sp)
    }
    Text(
        text = "piste " + bpmLabel(trackBpm) + "   cible " + bpmLabel(targetBpm),
        color = Palette.muted,
        fontSize = 12.sp,
    )
    cadence?.let { Text("cadence " + it.roundToInt() + " pas/min", color = Palette.muted2, fontSize = 11.sp) }

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
    bytes >= 1024L * 1024L * 1024L -> String.format(java.util.Locale.ROOT, "%.1f Go", bytes / (1024.0 * 1024.0 * 1024.0))
    bytes >= 1024L * 1024L -> String.format(java.util.Locale.ROOT, "%.0f Mo", bytes / (1024.0 * 1024.0))
    bytes >= 1024L -> String.format(java.util.Locale.ROOT, "%.0f Ko", bytes / 1024.0)
    else -> bytes.toString() + " o"
}
