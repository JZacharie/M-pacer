package com.mpacer.watch.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
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
import com.mpacer.core.music.MusicPlayerState
import com.mpacer.core.music.MusicState
import com.mpacer.core.ui.GpsLight
import com.mpacer.core.ui.Palette
import kotlinx.coroutines.delay
import java.time.LocalDateTime
import java.time.format.DateTimeFormatter
import java.util.Locale
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
    onMusic: () -> Unit,
    player: MusicPlayerState = MusicPlayerState(),
    onVolumeDown: () -> Unit = {},
    onVolumeUp: () -> Unit = {},
    onMusicPrevious: () -> Unit = {},
    onMusicNext: () -> Unit = {},
    onMusicToggle: () -> Unit = {},
) {
    val output = state.output
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Palette.encre),
        contentAlignment = Alignment.Center,
    ) {
        when (output?.state) {
            "Running" -> Seance(output, metric, enPause = false, onPrincipal = onPause, onStop = onStop, player = player, onVolumeDown = onVolumeDown, onVolumeUp = onVolumeUp, onMusicPrevious = onMusicPrevious, onMusicNext = onMusicNext, onMusicToggle = onMusicToggle)
            "Paused", "AutoPaused" -> Seance(output, metric, enPause = true, onPrincipal = onResume, onStop = onStop, player = player, onVolumeDown = onVolumeDown, onVolumeUp = onVolumeUp, onMusicPrevious = onMusicPrevious, onMusicNext = onMusicNext, onMusicToggle = onMusicToggle)
            else -> AuRepos(output, metric, onStart, onStop, onMusic, onSettings)
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
        // L'appairage et la synchronisation vivent dans les Reglages : un cadran
        // de 40 mm ne garde que ce qui sert au depart de la seance.
        Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            RoundButton(WatchIcons.Music, "Musique", onMusic, size = 40.dp, iconSize = 20.dp)
            RoundButton(WatchIcons.Settings, "Reglages", onSettings, size = 40.dp, iconSize = 20.dp)
        }
    }
}

// --------------------------------------------------------------------- seance

/**
 * Les vues d'une seance en cours, dans l'ordre ou elles defilent.
 *
 * Un cadran de 40 mm ne montre pas tout a la fois, et la bonne reponse n'est pas
 * de rapetisser les chiffres : c'est de separer ce qu'on ne regarde pas au meme
 * moment. L'allure reste la premiere vue -- la seule qu'on lit en courant -- et
 * un glissement de gauche a droite amene les autres.
 *
 * Les commandes (pause, arret) vivent sur la **premiere vue** seulement : on ne
 * les croise pas en changeant d'ecran, et un arret demande une **confirmation**
 * avant de fermer la seance.
 *
 * Deux vues supplementaires repondent aux besoins d'une sortie longue : la
 * **musique** (volume et changement de piste, pour ne pas sortir le telephone)
 * et l'**heure** (l'heure courante et le temps de course, pour savoir ou l'on
 * en est sans quitter la seance).
 */
private enum class VueCourse { ALLURE, TOUR, CARDIO, OBJECTIF, MUSIQUE, HEURE }

private val VUES_EN_COURSE = VueCourse.entries

@Composable
private fun Seance(
    output: EngineOutput?,
    metric: Boolean,
    enPause: Boolean,
    onPrincipal: () -> Unit,
    onStop: () -> Unit,
    player: MusicPlayerState,
    onVolumeDown: () -> Unit,
    onVolumeUp: () -> Unit,
    onMusicPrevious: () -> Unit,
    onMusicNext: () -> Unit,
    onMusicToggle: () -> Unit,
) {
    val pages = rememberPagerState(pageCount = { VUES_EN_COURSE.size })
    // Arret en deux temps : le premier appui arme la confirmation, le second
    // arrete la seance. Quitter la premiere vue desarme.
    var confirmationStop by remember { mutableStateOf(false) }
    LaunchedEffect(pages.currentPage) {
        if (pages.currentPage != 0) confirmationStop = false
    }
    Column(
        modifier = Modifier.fillMaxSize(),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        LigneEtat(output)
        // Le pager occupe tout l'espace entre l'etat et les points de page : les
        // commandes font partie de la premiere vue, elles ne sont donc plus
        // reservees ici.
        Box(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth(),
            contentAlignment = Alignment.Center,
        ) {
            HorizontalPager(state = pages, modifier = Modifier.fillMaxSize()) { index ->
                // Les marges laterales tiennent le texte dans le cercle : sur un
                // cadran rond, les coins sont coupes, et une phrase qui touche le
                // bord perd ses premiers et derniers caracteres.
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(horizontal = 22.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    when (VUES_EN_COURSE[index]) {
                        VueCourse.ALLURE -> VueAllure(
                            output = output,
                            metric = metric,
                            enPause = enPause,
                            confirmationStop = confirmationStop,
                            onPrincipal = onPrincipal,
                            onDemanderStop = { confirmationStop = true },
                            onConfirmerStop = {
                                confirmationStop = false
                                onStop()
                            },
                            onAnnulerStop = { confirmationStop = false },
                        )
                        VueCourse.TOUR -> VueTour(output, metric)
                        VueCourse.CARDIO -> VueCardio(output)
                        VueCourse.OBJECTIF -> VueObjectif(output, metric)
                        VueCourse.MUSIQUE -> VueMusique(
                            player = player,
                            onVolumeDown = onVolumeDown,
                            onVolumeUp = onVolumeUp,
                            onPrevious = onMusicPrevious,
                            onToggle = onMusicToggle,
                            onNext = onMusicNext,
                        )
                        VueCourse.HEURE -> VueHeure(output)
                    }
                }
            }
        }
        Spacer(Modifier.height(6.dp))
        PageIndicator(count = VUES_EN_COURSE.size, current = pages.currentPage)
        Spacer(Modifier.height(8.dp))
    }
}

/**
 * Premiere vue : l'allure, la distance, le temps, et **les commandes**.
 *
 * C'est la vue qu'on a sous les yeux pendant l'effort : c'est donc la seule qui
 * porte Pause et Arreter. Les autres vues restent des lectures, sans risque de
 * toucher un bouton en changeant d'ecran.
 */
@Composable
private fun VueAllure(
    output: EngineOutput?,
    metric: Boolean,
    enPause: Boolean,
    confirmationStop: Boolean,
    onPrincipal: () -> Unit,
    onDemanderStop: () -> Unit,
    onConfirmerStop: () -> Unit,
    onAnnulerStop: () -> Unit,
) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Allure(
            pace = output?.currentPace,
            metric = metric,
            couleur = if (enPause) Palette.muted else Palette.texte,
        )
        Spacer(Modifier.height(4.dp))
        LigneMetriques(output, metric)
        // La pastille garde sa hauteur meme vide : la vue ne saute pas d'un mode
        // d'assistant a l'autre.
        Box(modifier = Modifier.height(24.dp), contentAlignment = Alignment.Center) {
            PastilleAssistant(output, enPause)
        }
        Spacer(Modifier.height(8.dp))
        Commandes(
            enPause = enPause,
            confirmationStop = confirmationStop,
            onPrincipal = onPrincipal,
            onDemanderStop = onDemanderStop,
            onConfirmerStop = onConfirmerStop,
            onAnnulerStop = onAnnulerStop,
        )
    }
}

/** Deuxieme vue : le tour en cours, compare au precedent. */
@Composable
private fun VueTour(output: EngineOutput?, metric: Boolean) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        SectionTitle("Tour " + (output?.lapIndex ?: 1))
        Spacer(Modifier.height(2.dp))
        Allure(output?.currentLapPace, metric, Palette.texte)
        Spacer(Modifier.height(6.dp))
        Text(
            text = "precedent " + allureCourte(output?.previousLapPace),
            color = Palette.muted,
            fontSize = 13.sp,
            maxLines = 1,
        )
        Text(
            text = MpacerFormat.distance(output?.currentLapDistanceM ?: 0.0, imperial = !metric) +
                " / " + MpacerFormat.distance(longueurDeTour(metric), imperial = !metric),
            color = Palette.muted2,
            fontSize = 12.sp,
            maxLines = 1,
        )
    }
}

/** Troisieme vue : le coeur, quand la montre a un capteur. */
@Composable
private fun VueCardio(output: EngineOutput?) {
    val bpm = output?.heartRateBpm
    val cadence = output?.cadenceSpm?.let { Math.round(it).toString() + " spm" } ?: "-- spm"
    val foulee = output?.strideM?.let { String.format(java.util.Locale.US, "%.2f m", it) } ?: "-- m"

    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        SectionTitle("Cardio & Foulée")
        Spacer(Modifier.height(2.dp))
        if (bpm == null) {
            Text(
                text = "-- bpm",
                color = Palette.muted2,
                fontSize = 32.sp,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
            )
        } else {
            val zone = output.heartRateZone
            Row(
                verticalAlignment = Alignment.Bottom,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Text(
                    text = bpm.toString(),
                    color = Palette.zoneColor(zone),
                    fontSize = 38.sp,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                )
                Text(
                    text = "bpm",
                    color = Palette.muted2,
                    fontSize = 12.sp,
                    modifier = Modifier.padding(bottom = 6.dp),
                    maxLines = 1,
                )
                if (zone != null) {
                    Spacer(Modifier.width(4.dp))
                    StatusPill(text = "Z" + zone, color = Palette.zoneColor(zone))
                }
            }
        }
        Spacer(Modifier.height(4.dp))
        Row(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = cadence,
                color = Palette.texte,
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
            )
            Text(
                text = "•",
                color = Palette.muted2,
                fontSize = 11.sp,
            )
            Text(
                text = foulee,
                color = Palette.orange,
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
            )
        }
    }
}

/** Quatrieme vue : l'objectif, quand un plan de course est regle. */
@Composable
private fun VueObjectif(output: EngineOutput?, metric: Boolean) {
    val finish = output?.estimatedFinishS
    if (finish == null && output?.shadow == null) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            SectionTitle("Objectif")
            Spacer(Modifier.height(4.dp))
            Text(
                text = "Aucun plan de course",
                color = Palette.texte,
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
            )
            Text(
                text = "Reglez un temps vise ou une distance dans Reglages.",
                color = Palette.muted2,
                fontSize = 11.sp,
                maxLines = 3,
            )
        }
        return
    }
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        SectionTitle("Objectif")
        finish?.let {
            Box(modifier = Modifier.height(52.dp), contentAlignment = Alignment.Center) {
                Text(
                    text = MpacerFormat.duration(it),
                    color = Palette.texte,
                    fontSize = 40.sp,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                )
            }
        }
        output?.remainingM?.let {
            Text(
                text = "reste " + MpacerFormat.distance(it, imperial = !metric),
                color = Palette.muted,
                fontSize = 13.sp,
                maxLines = 1,
            )
        }
        Spacer(Modifier.height(4.dp))
        PastilleAssistant(output, enPause = false)
    }
}

/**
 * Cinquieme vue : la musique, sans quitter la seance.
 *
 * Trois commandes suffisent pendant l'effort : le volume (deux ronds, un
 * pourcentage lisible au centre) et le changement de piste (precedente,
 * lecture/pause, suivante). Le titre reste sur une ligne : c'est un repere, pas
 * une fiche. Le volume est celui du flux media de la montre, donc celui que
 * baissent et montent les boutons physiques.
 */
@Composable
private fun VueMusique(
    player: MusicPlayerState,
    onVolumeDown: () -> Unit,
    onVolumeUp: () -> Unit,
    onPrevious: () -> Unit,
    onToggle: () -> Unit,
    onNext: () -> Unit,
) {
    val volumeReglable = player.volumeMax > 0
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        SectionTitle("Musique")
        Spacer(Modifier.height(2.dp))
        Text(
            text = player.track?.title ?: "Aucune piste",
            color = Palette.texte,
            fontSize = 14.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
        )
        player.track?.artist?.takeIf { it.isNotBlank() }?.let {
            Text(text = it, color = Palette.muted2, fontSize = 11.sp, maxLines = 1)
        }
        Spacer(Modifier.height(6.dp))
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(9.dp),
        ) {
            RoundButton(
                icon = WatchIcons.Minus,
                label = "Baisser le volume",
                onClick = onVolumeDown,
                size = 38.dp,
                iconSize = 19.dp,
                enabled = volumeReglable,
            )
            Text(
                text = if (volumeReglable) player.volumePercent.toString() + " %" else "--",
                color = Palette.texte,
                fontSize = 26.sp,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
            )
            RoundButton(
                icon = WatchIcons.Plus,
                label = "Monter le volume",
                onClick = onVolumeUp,
                size = 38.dp,
                iconSize = 19.dp,
                enabled = volumeReglable,
            )
        }
        Spacer(Modifier.height(8.dp))
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            RoundButton(
                icon = WatchIcons.Previous,
                label = "Piste precedente",
                onClick = onPrevious,
                size = 38.dp,
                iconSize = 19.dp,
            )
            RoundButton(
                icon = if (player.playing) WatchIcons.Pause else WatchIcons.Play,
                label = if (player.playing) "Pause" else "Lire",
                onClick = onToggle,
                size = 46.dp,
                iconSize = 23.dp,
                background = Palette.orange,
                contentColor = Color.White,
            )
            RoundButton(
                icon = WatchIcons.Next,
                label = "Piste suivante",
                onClick = onNext,
                size = 38.dp,
                iconSize = 19.dp,
            )
        }
    }
}

/**
 * Sixieme vue : l'heure.
 *
 * En course, le telephone reste dans la ceinture : cette vue donne l'heure
 * courante (et les secondes), la date, puis le temps de course deja ecoule,
 * pour situer la sortie dans la journee sans interrompre l'enregistrement.
 */
@Composable
private fun VueHeure(output: EngineOutput?) {
    val maintenant = heureCourante()
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        SectionTitle("Heure")
        Row(
            verticalAlignment = Alignment.Bottom,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(
                text = maintenant.format(HEURE_MINUTES),
                color = Palette.texte,
                fontSize = 46.sp,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
            )
            Text(
                text = maintenant.format(HEURE_SECONDES),
                color = Palette.muted2,
                fontSize = 16.sp,
                modifier = Modifier.padding(bottom = 8.dp),
                maxLines = 1,
            )
        }
        Text(
            text = maintenant.format(DATE_FRANCAISE),
            color = Palette.muted,
            fontSize = 12.sp,
            maxLines = 1,
        )
        Spacer(Modifier.height(6.dp))
        Text(
            text = "en course " + MpacerFormat.duration(output?.elapsedS ?: 0.0),
            color = Palette.muted2,
            fontSize = 12.sp,
            maxLines = 1,
        )
    }
}

private val HEURE_MINUTES: DateTimeFormatter = DateTimeFormatter.ofPattern("HH:mm")
private val HEURE_SECONDES: DateTimeFormatter = DateTimeFormatter.ofPattern("ss")
private val DATE_FRANCAISE: DateTimeFormatter =
    DateTimeFormatter.ofPattern("EEEE d MMMM", Locale.FRENCH)

/**
 * Horloge de la vue Heure : l'ecran se rafraichit a la seconde.
 *
 * Le reveil est porte par la page, pas par un service : quand la vue n'est plus
 * composee, plus rien ne tourne. Une seconde de latence au pire a l'ouverture,
 * invisible pour une montre.
 */
@Composable
private fun heureCourante(): LocalDateTime {
    var heure by remember { mutableStateOf(LocalDateTime.now()) }
    LaunchedEffect(Unit) {
        while (true) {
            heure = LocalDateTime.now()
            delay(1_000L)
        }
    }
    return heure
}

/**
 * Les deux commandes de seance, sur la premiere vue seulement.
 *
 * Arreter ferme la seance et fige le resume : un appui malencontreux en pleine
 * course coute cher. Le bouton demande donc une confirmation -- un second rond,
 * en rouge, avec une croix pour renoncer -- plutot que d'arreter tout de suite.
 * La pause, elle, se rattrape d'un appui : elle reste immediate.
 */
@Composable
private fun Commandes(
    enPause: Boolean,
    confirmationStop: Boolean,
    onPrincipal: () -> Unit,
    onDemanderStop: () -> Unit,
    onConfirmerStop: () -> Unit,
    onAnnulerStop: () -> Unit,
) {
    if (confirmationStop) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = "Arreter la seance ?",
                color = Palette.attention,
                fontSize = 12.sp,
                fontWeight = FontWeight.Medium,
                maxLines = 1,
            )
            Spacer(Modifier.height(6.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                RoundButton(
                    icon = WatchIcons.Check,
                    label = "Confirmer l'arret",
                    onClick = onConfirmerStop,
                    size = 50.dp,
                    iconSize = 25.dp,
                    background = Palette.danger,
                    contentColor = Color.White,
                )
                RoundButton(
                    icon = WatchIcons.Close,
                    label = "Annuler",
                    onClick = onAnnulerStop,
                    size = 42.dp,
                    iconSize = 20.dp,
                    background = Palette.surface3,
                    contentColor = Palette.texte,
                )
            }
        }
        return
    }
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
            label = "Arreter la seance",
            onClick = onDemanderStop,
            size = 42.dp,
            iconSize = 19.dp,
            background = Palette.surface3,
            contentColor = Palette.danger,
        )
    }
}

/** Allure en clair, ou un tiret quand il n'y a rien a lire. */
private fun allureCourte(pace: Double?): String = MpacerFormat.pace(pace)

/** Longueur d'un tour : le kilometre, ou le mile selon les unites. */
private fun longueurDeTour(metric: Boolean): Double = if (metric) 1000.0 else 1609.344

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
