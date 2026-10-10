package com.mpacer.phone.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
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
import com.mpacer.core.music.MusicDownloadService
import com.mpacer.core.music.MusicDownloader
import com.mpacer.core.music.MusicLibrary
import com.mpacer.core.music.MusicPlayer
import com.mpacer.core.music.MusicPlayerState
import com.mpacer.core.music.MusicSession
import com.mpacer.core.ui.Palette
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

/**
 * Musique locale du telephone (docs/07 v2).
 *
 * Meme contrat que la montre : le moteur Rust decide quelle piste jouer et a quel
 * tempo ; l'application ne joue que des fichiers presents sur son disque, copies
 * par USB dans son dossier Music/ avec leur manifest.json (outil PC mpacer-music).
 * Aucun flux, aucun DRM, aucun appel reseau pendant la course.
 *
 * L'ecran est celui d'un lecteur : les commandes sont des icones (piste
 * precedente, lecture/pause, piste suivante), le volume est un curseur, et
 * chaque ligne de playlist porte ses deux actions (jouer, supprimer). Le texte
 * reste la ou il informe (titre, artiste, compteur), jamais la ou il commande.
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
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(
                imageVector = PhoneIcons.Music,
                contentDescription = null,
                tint = Palette.orange,
                modifier = Modifier.size(26.dp),
            )
            Text("Musique", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, color = Palette.texte)
        }
        Text(
            "Fichiers locaux : " + (MusicLibrary.usbDirectory(context)?.absolutePath ?: "dossier indisponible"),
            color = Palette.muted,
            fontSize = 11.sp,
        )

        PlayerCard(
            player = player,
            onPrevious = { MusicPlayer.previous() },
            onToggle = { MusicPlayer.toggle() },
            onNext = { MusicPlayer.next() },
            onVolume = { MusicPlayer.setVolumePercent(it) },
        )

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Button(
                onClick = { scope.launch { MusicLibrary.scan(context) } },
                enabled = !library.busy,
                colors = ButtonDefaults.buttonColors(
                    containerColor = Palette.orange,
                    contentColor = Color.White,
                ),
            ) {
                Icon(PhoneIcons.Upload, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(if (library.busy) "Import..." else "Importer (USB)")
            }
            Text(
                "utilise " + (library.usedBytes / (1024 * 1024)) + " Mo / libre " +
                    (library.freeBytes / (1024 * 1024)) + " Mo",
                color = Palette.muted,
                fontSize = 11.sp,
            )
        }

        // Depot Wi-Fi (docs/16) : recupere les MP3 deposes sur la page /music,
        // puis acquitte chaque piste pour liberer le serveur.
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Button(
                onClick = { MusicDownloadService.start(context) },
                enabled = !telechargement.busy,
                colors = ButtonDefaults.buttonColors(
                    containerColor = Palette.surface2,
                    contentColor = Palette.texte,
                ),
            ) {
                Icon(PhoneIcons.Download, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
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

/**
 * Carte du lecteur : piste en cours, transport et volume.
 *
 * Le transport reprend le vocabulaire de la montre (precedent / lecture-pause /
 * suivant) mais en icones Material, et le volume est un curseur : sur un
 * telephone, on regle d'un geste ce qu'on ne peut que pousser par crans sur un
 * cadran rond. Le volume affiche est celui du flux media du systeme, donc celui
 * que changent les boutons physiques.
 */
@Composable
private fun PlayerCard(
    player: MusicPlayerState,
    onPrevious: () -> Unit,
    onToggle: () -> Unit,
    onNext: () -> Unit,
    onVolume: (Int) -> Unit,
) {
    Card(
        shape = androidx.compose.foundation.shape.RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = Palette.surface),
        border = androidx.compose.foundation.BorderStroke(1.dp, Color(0x1FFFFFFF)),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Icon(PhoneIcons.Music, contentDescription = null, tint = Palette.muted, modifier = Modifier.size(14.dp))
                Text("Lecture", color = Palette.muted, fontSize = 12.sp)
            }
            Text(
                player.track?.let { (it.title + (it.artist?.let { artiste -> " - " + artiste } ?: "")) }
                    ?: "Aucune piste",
                color = Palette.texte,
                fontWeight = FontWeight.Medium,
            )
            player.playlistName?.let {
                Text(it + "  " + (player.index + 1) + "/" + player.count, color = Palette.muted, fontSize = 12.sp)
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(20.dp, Alignment.CenterHorizontally),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onPrevious, modifier = Modifier.size(52.dp)) {
                    Icon(
                        imageVector = PhoneIcons.SkipPrevious,
                        contentDescription = "Piste precedente",
                        tint = Palette.texte,
                        modifier = Modifier.size(32.dp),
                    )
                }
                FilledIconButton(
                    onClick = onToggle,
                    modifier = Modifier.size(64.dp),
                    colors = IconButtonDefaults.filledIconButtonColors(
                        containerColor = Palette.orange,
                        contentColor = Color.White,
                    ),
                ) {
                    Icon(
                        imageVector = if (player.playing) PhoneIcons.Pause else PhoneIcons.Play,
                        contentDescription = if (player.playing) "Pause" else "Lire",
                        modifier = Modifier.size(32.dp),
                    )
                }
                IconButton(onClick = onNext, modifier = Modifier.size(52.dp)) {
                    Icon(
                        imageVector = PhoneIcons.SkipNext,
                        contentDescription = "Piste suivante",
                        tint = Palette.texte,
                        modifier = Modifier.size(32.dp),
                    )
                }
            }

            val volumeReglable = player.volumeMax > 0
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Icon(
                    imageVector = PhoneIcons.VolumeDown,
                    contentDescription = "Volume",
                    tint = Palette.muted,
                    modifier = Modifier.size(20.dp),
                )
                Slider(
                    value = player.volumePercent.toFloat(),
                    onValueChange = { onVolume(it.roundToInt()) },
                    valueRange = 0f..100f,
                    enabled = volumeReglable,
                    modifier = Modifier.weight(1f),
                )
                Icon(
                    imageVector = PhoneIcons.VolumeUp,
                    contentDescription = null,
                    tint = Palette.muted,
                    modifier = Modifier.size(20.dp),
                )
                Text(
                    if (volumeReglable) player.volumePercent.toString() + " %" else "--",
                    color = Palette.muted,
                    fontSize = 12.sp,
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
                .padding(horizontal = 10.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = PhoneIcons.Music,
                contentDescription = null,
                tint = if (active) Palette.orange else Palette.muted2,
                modifier = Modifier.size(20.dp),
            )
            Spacer(Modifier.width(10.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(playlist.name, color = Palette.texte, fontSize = 15.sp)
                Text(
                    playlist.trackCount.toString() + " titre(s), " + playlist.playable.size +
                        " jouable(s)" + (playlist.targetBpm?.let { "  cible " + it.toInt() + " BPM" } ?: ""),
                    color = Palette.muted,
                    fontSize = 12.sp,
                )
            }
            IconButton(
                onClick = onPlay,
                enabled = playlist.playable.isNotEmpty(),
                modifier = Modifier.size(44.dp),
            ) {
                Icon(
                    imageVector = PhoneIcons.Play,
                    contentDescription = if (active) "Relire " + playlist.name else "Jouer " + playlist.name,
                    tint = if (playlist.playable.isNotEmpty()) Palette.orange else Palette.muted2,
                    modifier = Modifier.size(26.dp),
                )
            }
            IconButton(onClick = onDelete, modifier = Modifier.size(44.dp)) {
                Icon(
                    imageVector = PhoneIcons.Delete,
                    contentDescription = "Supprimer " + playlist.name,
                    tint = Palette.danger,
                    modifier = Modifier.size(22.dp),
                )
            }
        }
    }
}
