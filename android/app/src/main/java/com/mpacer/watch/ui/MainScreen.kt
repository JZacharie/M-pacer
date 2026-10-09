package com.mpacer.watch.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.Text
import com.mpacer.core.EngineOutput
import com.mpacer.core.MpacerFormat
import com.mpacer.core.SessionState
import com.mpacer.core.music.MusicDirective
import com.mpacer.core.music.MusicState
import com.mpacer.core.ui.GpsLight
import com.mpacer.core.ui.Palette
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * Ecran principal, pense pour un cadran rond.
 *
 * Trois etats, trois compositions -- et jamais les trois a la fois :
 *
 *  1. **Au repos** (arrivee, seance terminee ou armee) : une seule commande,
 *     le rond orange du depart. Le reste de l'ecran est vide, volontairement :
 *     rien a lire avant de courir.
 *  2. **En course** : l'allure domine en 46 sp, la distance et le temps
 *     s'alignent dessous, la pastille d'assistant dit ou l'on en est du plan,
 *     puis les deux seules commandes utiles, Pause et Arreter.
 *  3. **En pause** : la meme colonne, l'allure eteinte et le rond du depart
 *     qui redevient Reprendre. On ne change pas de disposition en s'arretant.
 *
 * Toutes les commandes sont des [RoundButton] : une icone, jamais un mot. Les
 * quatre libelles de l'ecran precedent se tronquaient sur un cadran de 40 mm
 * des que la taille de police systeme augmentait.
 */
@Composable
fun MainScreen(
    state: SessionState,
    metric: Boolean = true,
    onStart: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onStop: () -> Unit,
    onSettings: () -> Unit,
    onSync: () -> Unit,
    onMusic: () -> Unit,
) {
    val output = state.output
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Palette.encre),
        contentAlignment = Alignment.Center,
    ) {
        when (output?.state) {
            "Running" -> Seance(output, metric, enPause = false, onPrincipal = onPause, onStop = onStop)
            "Paused", "AutoPaused" -> Seance(output, metric, enPause = true, onPrincipal = onResume, onStop = onStop)
            else -> AuRepos(output, metric, onStart, onStop, onMusic, onSync, onSettings)
        }
    }
}

// -------------------------------------------------------------------- au repos

@Composable
private fun AuRepos(
    output: EngineOutput?,
    metric: Boolean,
    onStart: () -> Unit,
    onStop: () -> Unit,
    onMusic: () -> Unit,
    onSync: () -> Unit,
    onSettings: () -> Unit,
) {
    val arme = output?.state == "Armed"
    val acheve = output?.state == "Finished" && output.distanceM > 0.0
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        LigneEtat(output)
        Spacer(Modifier.height(12.dp))
        if (arme) {
            // Le chronometre partira au premier mouvement : on ne propose donc
            // que l'arret, pas un second depart.
            Text(
                text = "Depart au mouvement",
                color = Palette.attention,
                fontSize = 13.sp,
                fontWeight = FontWeight.Medium,
                maxLines = 1,
            )
            Spacer(Modifier.height(12.dp))
            RoundButton(
                icon = WatchIcons.Stop,
                label = "Arreter",
                onClick = onStop,
                size = 56.dp,
                iconSize = 26.dp,
                background = Palette.surface3,
                contentColor = Palette.danger,
            )
        } else {
            if (acheve) {
                Text(
                    text = ligneMetriques(output, metric),
                    color = Palette.muted,
                    fontSize = 15.sp,
                    maxLines = 1,
                )
                Spacer(Modifier.height(12.dp))
            }
            RoundButton(
                icon = WatchIcons.Play,
                label = "Demarrer",
                onClick = onStart,
                size = 60.dp,
                iconSize = 30.dp,
                background = Palette.orange,
                contentColor = Color.White,
            )
            Spacer(Modifier.height(8.dp))
            Text(
                text = if (acheve) "Nouvelle seance" else "Demarrer",
                color = Palette.texte,
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
            )
        }
        Spacer(Modifier.height(18.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(11.dp)) {
            RoundButton(WatchIcons.Music, "Musique", onMusic, size = 40.dp, iconSize = 20.dp)
            RoundButton(WatchIcons.Sync, "Synchronisation", onSync, size = 40.dp, iconSize = 20.dp)
            RoundButton(WatchIcons.Settings, "Reglages", onSettings, size = 40.dp, iconSize = 20.dp)
        }
    }
}

// --------------------------------------------------------------------- seance

@Composable
private fun Seance(
    output: EngineOutput?,
    metric: Boolean,
    enPause: Boolean,
    onPrincipal: () -> Unit,
    onStop: () -> Unit,
) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        LigneEtat(output)
        Spacer(Modifier.height(8.dp))
        Allure(
            pace = output?.currentPace,
            metric = metric,
            couleur = if (enPause) Palette.muted else Palette.texte,
        )
        Spacer(Modifier.height(4.dp))
        LigneMetriques(output, metric)
        // La pastille garde sa hauteur meme vide : les commandes ne sautent pas
        // d'un mode d'assistant a l'autre.
        Box(modifier = Modifier.height(24.dp), contentAlignment = Alignment.Center) {
            PastilleAssistant(output, enPause)
        }
        Spacer(Modifier.height(16.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(13.dp)) {
            RoundButton(
                icon = if (enPause) WatchIcons.Play else WatchIcons.Pause,
                label = if (enPause) "Reprendre" else "Pause",
                onClick = onPrincipal,
                size = 52.dp,
                iconSize = 26.dp,
                background = Palette.orange,
                contentColor = Color.White,
            )
            RoundButton(
                icon = WatchIcons.Stop,
                label = "Arreter",
                onClick = onStop,
                size = 42.dp,
                iconSize = 19.dp,
                background = Palette.surface3,
                contentColor = Palette.danger,
            )
        }
    }
}

// -------------------------------------------------------------------- morceaux

/** Voyant GPS, et consigne musicale quand une playlist joue. */
@Composable
private fun LigneEtat(output: EngineOutput?) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        GpsLight(output?.light)
        PastilleMusique(output?.music)
    }
}

/**
 * L'allure, et son unite : la seule valeur vraiment lue en courant.
 *
 * Tant qu'aucune position exploitable n'est arrivee, la valeur est un tiret.
 * On la dessine alors plus petite et en gris : a 46 sp, les deux tirets se
 * lisaient comme une allure. La hauteur de la ligne est reservee dans tous les
 * cas, pour que rien ne saute au premier point GPS.
 */
@Composable
private fun Allure(pace: Double?, metric: Boolean, couleur: Color) {
    val connue = pace != null && pace > 0.0 && !pace.isNaN()
    Box(modifier = Modifier.height(58.dp), contentAlignment = Alignment.Center) {
        Row(
            verticalAlignment = Alignment.Bottom,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                text = MpacerFormat.pace(pace),
                color = if (connue) couleur else Palette.muted2,
                fontSize = if (connue) 46.sp else 30.sp,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
            )
            Text(
                text = if (metric) "/km" else "/mi",
                color = Palette.muted2,
                fontSize = 13.sp,
                modifier = Modifier.padding(bottom = 7.dp),
                maxLines = 1,
            )
        }
    }
}

/**
 * Distance et temps en mouvement, puis la frequence cardiaque si la montre en
 * fournit une. Le pouls prend la couleur de sa zone : c'est la seule valeur
 * dont la couleur dit quelque chose par elle-meme.
 */
@Composable
private fun LigneMetriques(output: EngineOutput?, metric: Boolean) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(7.dp),
    ) {
        Text(
            text = ligneMetriques(output, metric),
            color = Palette.muted,
            fontSize = 15.sp,
            maxLines = 1,
        )
        val bpm = output?.heartRateBpm
        if (bpm != null) {
            Text(
                text = bpm.toString(),
                color = Palette.zoneColor(output.heartRateZone),
                fontSize = 15.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
            )
        }
    }
}

/**
 * Pastille d'assistance : « sur le plan », l'ecart au shadow runner en metres,
 * ou l'heure de finish projetee. Elle est vide en mode allure seule.
 */
@Composable
private fun PastilleAssistant(output: EngineOutput?, enPause: Boolean) {
    if (enPause) {
        StatusPill(text = "pause", color = Palette.attention)
        return
    }
    val shadow = output?.shadow
    if (shadow != null) {
        if (shadow.onPlan) {
            StatusPill(text = "sur le plan", color = Palette.ok)
        } else {
            val metres = abs(shadow.distanceDeltaM).roundToInt()
            // Ecart en distance, plus parlant que l'ecart en temps pendant l'effort.
            StatusPill(
                text = (if (shadow.ahead) "+" else "-") + metres + " m",
                color = if (shadow.ahead) Palette.ok else Palette.orange,
            )
        }
        return
    }
    output?.estimatedFinishS?.let {
        StatusPill(text = "finish " + MpacerFormat.duration(it), color = Palette.muted)
    }
}

/** Consigne musicale du moteur (docs/07) : le BPM cible et le sens de la fleche. */
@Composable
private fun PastilleMusique(music: MusicState?) {
    val cible = music?.targetBpm ?: return
    if (!music.enabled) return
    val sens = when (music.directive) {
        MusicDirective.BOOST -> " ^"
        MusicDirective.RELAX -> " v"
        MusicDirective.SKIP_TO -> " >>"
        else -> ""
    }
    StatusPill(
        text = cible.roundToInt().toString() + " BPM" + sens,
        color = when (music.directive) {
            MusicDirective.BOOST -> Palette.orange
            MusicDirective.RELAX -> Palette.ok
            else -> Palette.muted
        },
        icon = WatchIcons.Music,
        fontSize = 11.sp,
    )
}

/** Ligne « distance · temps », partagee par l'ecran de seance et la fin de seance. */
private fun ligneMetriques(output: EngineOutput?, metric: Boolean): String =
    MpacerFormat.distance(output?.distanceM ?: 0.0, imperial = !metric) +
        "   ·   " +
        MpacerFormat.duration(output?.elapsedS ?: 0.0)
