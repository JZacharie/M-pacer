package com.mpacer.watch.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.Text
import com.mpacer.core.TrackingService
import com.mpacer.core.music.LocalPlaylist
import com.mpacer.core.music.MusicDownloadState
import com.mpacer.core.music.MusicDownloadService
import com.mpacer.core.music.MusicDownloader
import com.mpacer.core.music.MusicLibrary
import com.mpacer.core.music.MusicLibraryState
import com.mpacer.core.music.MusicPlayer
import com.mpacer.core.music.MusicPlayerState
import com.mpacer.core.music.MusicSession
import com.mpacer.core.ui.Palette
import kotlinx.coroutines.launch
import java.util.Locale
import kotlin.math.roundToInt

/**
 * Ecran Musique de la montre (docs/07 v2, section 6.4).
 *
 * Deux panneaux dans un cadran rond :
 *  1. Bibliotheque (USB) : playlists copiees par mpacer-music, espace libre et
 *     utilise, import, suppression ;
 *  2. Lecture : piste en cours, BPM de la piste et consigne du moteur, transport.
 *
 * Les commandes de lecture suivent le meme vocabulaire que l'ecran principal :
 * des ronds a icone (precedent, lecture, suivant), et non des puces << >> et
 * « Lire » de largeurs inegales. La montre ne joue que des fichiers presents
 * sur son disque : aucun acces reseau, le coeur Rust reste seul decideur du tempo.
 */
@Composable
fun MusicScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val library by MusicLibrary.state.collectAsState()
    val player by MusicPlayer.state.collectAsState()
    val telechargement by MusicDownloader.state.collectAsState()
    val watch by TrackingService.state.collectAsState()
    var showPlayer by remember { mutableStateOf(false) }
    var aSupprimer by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) {
        MusicLibrary.reload(context)
        MusicPlayer.prepare(context)
    }

    SecondaryScreen(onBack = onBack) {
        ScreenTitle(if (showPlayer) "Lecture" else "Bibliotheque")

        if (showPlayer) {
            PlayerPane(
                player = player,
                shuffle = player.shuffle,
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
                telechargement = telechargement,
                shuffle = player.shuffle,
                onToggleShuffle = { MusicPlayer.setShuffle(!player.shuffle) },
                onDownload = { MusicDownloadService.start(context) },
                onImport = { scope.launch { MusicLibrary.scan(context) } },
                onPlay = { playlist ->
                    MusicSession.setPlaylist(playlist)
                    MusicPlayer.play(playlist)
                    showPlayer = true
                },
                aSupprimer = aSupprimer,
                onAskDelete = { id -> aSupprimer = id },
                onDelete = { playlist ->
                    scope.launch { MusicLibrary.delete(context, playlist.id) }
                    if (MusicSession.local?.id == playlist.id) MusicSession.setPlaylist(null)
                    aSupprimer = null
                },
            )
        }
    }
}

// ------------------------------------------------------------- bibliotheque

@Composable
private fun LibraryPane(
    library: MusicLibraryState,
    telechargement: MusicDownloadState,
    shuffle: Boolean,
    onToggleShuffle: () -> Unit,
    onDownload: () -> Unit,
    onImport: () -> Unit,
    onPlay: (LocalPlaylist) -> Unit,
    aSupprimer: String?,
    onAskDelete: (String) -> Unit,
    onDelete: (LocalPlaylist) -> Unit,
) {
    // L'ordre de lecture se choisit ici, avant la playlist : c'est lui qui
    // decide de l'ordre des MP3 quand on appuie sur Lire.
    SectionTitle("Lecture")
    SettingRow(
        label = if (shuffle) "Aleatoire" else "Dans l'ordre",
        onClick = onToggleShuffle,
        selected = shuffle,
        icon = WatchIcons.Shuffle,
    )
    SectionTitle("Fichiers")
    SettingRow(
        label = if (library.busy) "Analyse du dossier..." else "Importer (USB)",
        onClick = onImport,
        enabled = !library.busy,
        selected = true,
        icon = WatchIcons.Sync,
    )
    // Depot Wi-Fi (docs/16) : recupere les MP3 deposes sur la page /music, puis
    // acquitte chaque piste. Sans fichier en attente, le serveur ne renvoie rien.
    SettingRow(
        label = if (telechargement.busy) "Telechargement..." else "Telecharger (serveur)",
        onClick = onDownload,
        enabled = !telechargement.busy && !library.busy,
        selected = false,
        icon = WatchIcons.Sync,
    )
    telechargement.message?.takeIf { telechargement.busy || it != "Aucun fichier a telecharger" }?.let { message ->
        Text(
            text = if (telechargement.busy && telechargement.total > 0) {
                message + "  " + telechargement.current + "/" + telechargement.total +
                    "  " + formatBytes(telechargement.bytes)
            } else {
                message
            },
            color = Palette.muted,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
        )
    }
    Text(
        text = "libre " + formatBytes(library.freeBytes) + "   utilise " + formatBytes(library.usedBytes),
        color = Palette.muted2,
        fontSize = 11.sp,
        textAlign = TextAlign.Center,
    )
    if (!library.usbAvailable) {
        Text(
            text = "Dossier Music/ absent : branchez la montre et lancez mpacer-music transfer.",
            color = Palette.attention,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
        )
    }
    if (library.playlists.isEmpty()) {
        Text(
            text = "Aucune playlist importee. Sur l'ordinateur : mpacer-music transfer, puis Importer (USB).",
            color = Palette.muted,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
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
        Text(
            text = message,
            color = Palette.muted,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
        )
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
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            text = playlist.name,
            color = Palette.texte,
            fontWeight = FontWeight.SemiBold,
            fontSize = 14.sp,
            textAlign = TextAlign.Center,
            maxLines = 1,
        )
        Text(
            text = playlist.trackCount.toString() + " pistes   " + formatBytes(playlist.sizeBytes),
            color = Palette.muted2,
            fontSize = 11.sp,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            if (playlist.playable.isNotEmpty()) {
                RoundButton(
                    icon = WatchIcons.Play,
                    label = "Lire",
                    onClick = onPlay,
                    size = 36.dp,
                    iconSize = 18.dp,
                    background = Palette.orange,
                    contentColor = Color.White,
                )
            }
            if (confirmation) {
                RoundButton(
                    icon = WatchIcons.Check,
                    label = "Confirmer la suppression",
                    onClick = onDelete,
                    size = 36.dp,
                    iconSize = 18.dp,
                    background = Palette.danger,
                    contentColor = Color.White,
                )
            } else {
                RoundButton(
                    icon = WatchIcons.Delete,
                    label = "Supprimer",
                    onClick = onAskDelete,
                    size = 36.dp,
                    iconSize = 18.dp,
                    background = Palette.surface3,
                    contentColor = Palette.danger,
                )
            }
        }
    }
}

// ------------------------------------------------------------------ lecture

@Composable
private fun PlayerPane(
    player: MusicPlayerState,
    shuffle: Boolean,
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
        color = Palette.texte,
        fontWeight = FontWeight.Bold,
        fontSize = 15.sp,
        textAlign = TextAlign.Center,
        maxLines = 1,
    )
    player.track?.artist?.takeIf { it.isNotBlank() }?.let {
        Text(
            text = it,
            color = Palette.muted,
            fontSize = 12.sp,
            textAlign = TextAlign.Center,
            maxLines = 1,
        )
    }
    Text(
        text = "piste " + bpmLabel(trackBpm) + "   cible " + bpmLabel(targetBpm),
        color = Palette.muted,
        fontSize = 12.sp,
    )
    cadence?.let {
        Text(
            text = "cadence " + it.roundToInt() + " pas/min",
            color = Palette.muted2,
            fontSize = 11.sp,
        )
    }
    player.playlistName?.let {
        Text(text = it, color = Palette.muted2, fontSize = 10.sp, textAlign = TextAlign.Center)
    }
    Text(
        text = if (shuffle) "aleatoire" else "dans l'ordre",
        color = Palette.muted2,
        fontSize = 10.sp,
        textAlign = TextAlign.Center,
    )

    Row(
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RoundButton(
            icon = WatchIcons.Previous,
            label = "Precedent",
            onClick = onPrevious,
            size = 40.dp,
            iconSize = 20.dp,
        )
        RoundButton(
            icon = if (player.playing) WatchIcons.Pause else WatchIcons.Play,
            label = if (player.playing) "Pause" else "Lire",
            onClick = onToggle,
            size = 52.dp,
            iconSize = 26.dp,
            background = Palette.orange,
            contentColor = Color.White,
        )
        RoundButton(
            icon = WatchIcons.Next,
            label = "Suivant",
            onClick = onNext,
            size = 40.dp,
            iconSize = 20.dp,
        )
    }

    player.message?.let {
        Text(
            text = it,
            color = Palette.attention,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
        )
    }

    SettingRow(label = "Bibliotheque", onClick = onLibrary, icon = WatchIcons.Music)
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
