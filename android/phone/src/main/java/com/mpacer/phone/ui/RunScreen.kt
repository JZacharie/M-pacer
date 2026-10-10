package com.mpacer.phone.ui

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.core.EngineOutput
import com.mpacer.core.MpacerFormat
import com.mpacer.core.SessionState
import com.mpacer.core.live.LiveState
import com.mpacer.core.live.PlannedRouteStore
import com.mpacer.core.music.MusicDirective
import com.mpacer.core.music.MusicState
import com.mpacer.core.ui.GpsLight
import com.mpacer.core.ui.Palette
import com.mpacer.core.social.FriendsClient
import com.mpacer.phone.PhoneSettings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.abs
import kotlin.math.roundToInt

/**
 * Ecran de course du telephone.
 *
 * Meme information que la montre, mise en page pour un grand ecran : l'allure
 * reste l'element dominant (c'est la seule valeur vraiment lue en courant), les
 * autres mesures sont rangees en tuiles. Aucun calcul ici : tout vient du moteur
 * Rust, y compris les textes des annonces vocales.
 */
@Composable
fun RunScreen(
    state: SessionState,
    settings: PhoneSettings,
    live: LiveState,
    onStart: () -> Unit,
    onArm: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onStop: () -> Unit,
    onAnnounce: () -> Unit,
    onResetPaceWindow: () -> Unit,
) {
    val output = state.output
    val enSeance = output != null && output.state != "Idle" && output.state != "Finished"
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        StatusRow(state, settings, live)
        PaceBlock(output, settings)
        StatsRow(output, settings)
        AssistantCard(output, settings)
        ParcoursCard()
        MusicCard(output?.music)

        if (enSeance) {
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                TextButton(onClick = onAnnounce) {
                    Icon(PhoneIcons.VolumeUp, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Annonce vocale")
                }
                TextButton(onClick = onResetPaceWindow) {
                    Icon(PhoneIcons.Refresh, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Fenetre d'allure")
                }
            }
        }

        Controls(output, onStart, onArm, onPause, onResume, onStop)
        Spacer(Modifier.height(4.dp))
    }
}

/**
 * Parcours planifie : le trace que le coureur compte suivre.
 *
 * Le fichier GPX est choisi une fois ; il est garde par l'application et
 * publie au depart de chaque seance, pour que les suiveurs voient le trace
 * prevu **et** la position, avec le pourcentage deja couvert. Tant qu'aucun
 * parcours n'est choisi, seuls les points de la seance circulent.
 */
@Composable
private fun ParcoursCard() {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val cercle by FriendsClient.state.collectAsState()
    var version by remember { mutableStateOf(0) }
    var message by remember { mutableStateOf<String?>(null) }
    val route = remember(version) { PlannedRouteStore.load(context) }

    val choisir = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            val texte = withContext(Dispatchers.IO) {
                runCatching {
                    context.contentResolver.openInputStream(uri)?.bufferedReader()?.use { it.readText() }
                }.getOrNull()
            }
            val points = texte?.let(PlannedRouteStore::parseGpx).orEmpty()
            if (points.size >= 2) {
                val nom = uri.lastPathSegment?.substringAfterLast('/')?.takeIf { it.isNotBlank() }
                    ?: "parcours.gpx"
                val retenu = PlannedRouteStore.Route(nom, points)
                PlannedRouteStore.save(context, retenu)
                message = "Parcours retenu : " + retenu.resume
                version++
            } else {
                message = "GPX illisible : aucun point latitude/longitude trouve."
            }
        }
    }

    Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Icon(PhoneIcons.Location, contentDescription = null, tint = Palette.muted, modifier = Modifier.size(14.dp))
                Text("Parcours planifie", color = Palette.muted, fontSize = 12.sp)
            }
            Text(
                route?.let { it.name + "  -  " + it.resume }
                    ?: "Aucun parcours : vos suiveurs ne verront que votre position.",
                color = if (route == null) Palette.muted else Palette.texte,
                fontSize = 13.sp,
            )
            cercle?.circle?.me?.parcoursResume?.let { avancement ->
                Text("Avancement : " + avancement, color = Palette.ok, fontSize = 13.sp)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Button(
                    onClick = {
                        choisir.launch(
                            arrayOf("application/gpx+xml", "application/xml", "text/xml", "*/*"),
                        )
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = if (route == null) Palette.orange else Palette.surface2,
                        contentColor = if (route == null) Color.White else Palette.texte,
                    ),
                ) {
                    Icon(PhoneIcons.Upload, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(if (route == null) "Choisir un GPX" else "Changer de GPX")
                }
                if (route != null) {
                    TextButton(onClick = {
                        PlannedRouteStore.clear(context)
                        message = "Parcours retire."
                        version++
                    }) {
                        Icon(PhoneIcons.Delete, contentDescription = null, tint = Palette.danger, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text("Retirer", color = Palette.danger)
                    }
                }
            }
            message?.let { Text(it, color = Palette.muted, fontSize = 12.sp) }
        }
    }
}

/** Voyant GPS, precision et etat du suivi en direct. */
@Composable
private fun StatusRow(state: SessionState, settings: PhoneSettings, live: LiveState) {
    val output = state.output
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        GpsLight(output?.light)
        Text(
            text = etatLibelle(output?.state),
            color = Palette.texte,
            fontWeight = FontWeight.Medium,
        )
        Text(
            text = state.accuracyM?.let { "GPS +/- " + it.roundToInt() + " m" } ?: "GPS en attente",
            color = Palette.muted,
            fontSize = 13.sp,
        )
        Spacer(Modifier.weight(1f))
        if (settings.live.configured) {
            Puce(
                texte = if (live.connected) "Direct " + live.sent else "Direct...",
                couleur = if (live.connected) Palette.ok else Palette.attention,
            )
        }
    }
}

@Composable
private fun Puce(texte: String, couleur: Color) {
    Box(
        modifier = Modifier
            .background(couleur.copy(alpha = 0.16f), RoundedCornerShape(50))
            .padding(horizontal = 10.dp, vertical = 3.dp),
    ) {
        Text(texte, color = couleur, fontSize = 12.sp)
    }
}

/** L'allure courante avec badge d'unité et carte hero façon Strava. */
@Composable
private fun PaceBlock(output: EngineOutput?, settings: PhoneSettings) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(20.dp),
        colors = CardDefaults.cardColors(containerColor = Palette.surface),
        border = androidx.compose.foundation.BorderStroke(1.dp, Color(0x1FFFFFFF)),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 20.dp, horizontal = 16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                text = "ALLURE INSTANTANÉE",
                color = Palette.muted,
                fontSize = 11.sp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 1.sp,
            )
            Spacer(Modifier.height(4.dp))
            Row(
                verticalAlignment = Alignment.Bottom,
                horizontalArrangement = Arrangement.Center,
            ) {
                Text(
                    text = MpacerFormat.pace(output?.currentPace),
                    fontSize = 76.sp,
                    fontWeight = FontWeight.ExtraBold,
                    color = if (output?.isPaused == true) Palette.muted else Palette.texte,
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    text = if (settings.metric) "/km" else "/mi",
                    color = Palette.orange,
                    fontSize = 20.sp,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(bottom = 14.dp),
                )
            }
            output?.previousLapPace?.let { precedente ->
                Box(
                    modifier = Modifier
                        .background(Palette.surface2, RoundedCornerShape(50))
                        .padding(horizontal = 12.dp, vertical = 4.dp),
                ) {
                    Text(
                        text = "Tour précédent : " + MpacerFormat.pace(precedente) + " /km",
                        color = Palette.muted,
                        fontSize = 12.sp,
                        fontWeight = FontWeight.Medium,
                    )
                }
            }
        }
    }
}

/** Distance, durée, fréquence cardiaque, tour courant en grille 2x2 Strava. */
@Composable
private fun StatsRow(output: EngineOutput?, settings: PhoneSettings) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Tuile(
            libelle = "DISTANCE",
            valeur = MpacerFormat.distance(output?.distanceM ?: 0.0, imperial = !settings.metric),
            modifier = Modifier.weight(1f),
        )
        Tuile(
            libelle = "TEMPS ÉCOULÉ",
            valeur = MpacerFormat.duration(output?.elapsedS ?: 0.0),
            modifier = Modifier.weight(1f),
        )
    }
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        val zone = output?.heartRateZone
        Tuile(
            libelle = "FRÉQUENCE CARDIAQUE",
            valeur = output?.heartRateBpm?.let { bpm ->
                zone?.let { "Z" + it + "  " + bpm + " bpm" } ?: (bpm.toString() + " bpm")
            } ?: "-- bpm",
            couleur = Palette.zoneColor(zone),
            modifier = Modifier.weight(1f),
        )
        Tuile(
            libelle = "ALLURE MOYENNE",
            valeur = MpacerFormat.pace(output?.currentLapPace) + " /km",
            modifier = Modifier.weight(1f),
        )
    }
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        val cadence = output?.cadenceSpm?.let { Math.round(it).toString() + " spm" } ?: "-- spm"
        Tuile(
            libelle = "CADENCE",
            valeur = cadence,
            modifier = Modifier.weight(1f),
        )
        val foulee = output?.strideM?.let { String.format(java.util.Locale.US, "%.2f m", it) } ?: "-- m"
        Tuile(
            libelle = "LONGUEUR DE FOULÉE",
            valeur = foulee,
            couleur = Palette.orange,
            modifier = Modifier.weight(1f),
        )
    }
}

@Composable
private fun Tuile(
    libelle: String,
    valeur: String,
    modifier: Modifier = Modifier,
    couleur: Color = Palette.texte,
) {
    Card(
        modifier = modifier,
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = Palette.surface),
        border = androidx.compose.foundation.BorderStroke(1.dp, Color(0x1FFFFFFF)),
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(
                text = libelle,
                color = Palette.muted2,
                fontSize = 10.sp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.5.sp,
            )
            Text(
                text = valeur,
                color = couleur,
                fontSize = 20.sp,
                fontWeight = FontWeight.Bold,
            )
        }
    }
}

/**
 * Panneau d'assistant : temps de finish estime (modes predire / planifie) ou
 * ecart au shadow runner (mode "atteindre le temps prevu"), et distance restante.
 */
@Composable
private fun AssistantCard(output: EngineOutput?, settings: PhoneSettings) {
    val shadow = output?.shadow
    val finish = output?.estimatedFinishS
    val restante = output?.remainingM
    if (shadow == null && finish == null && restante == null) return

    Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text("Assistant", color = Palette.muted, fontSize = 12.sp)
            if (shadow != null) {
                val texte: String
                val couleur: Color
                if (shadow.onPlan) {
                    texte = "Sur le plan"
                    couleur = Palette.ok
                } else if (shadow.ahead) {
                    texte = "+" + abs(shadow.distanceDeltaM).roundToInt() + " m d'avance"
                    couleur = Palette.ok
                } else {
                    texte = "-" + abs(shadow.distanceDeltaM).roundToInt() + " m de retard"
                    couleur = Palette.orange
                }
                Text(texte, color = couleur, fontSize = 20.sp, fontWeight = FontWeight.SemiBold)
                Text(
                    "ecart temps " + (if (shadow.timeDeltaS >= 0) "+" else "-") +
                        MpacerFormat.duration(abs(shadow.timeDeltaS)),
                    color = Palette.muted,
                    fontSize = 13.sp,
                )
            } else if (finish != null) {
                Text(
                    "Finish estime " + MpacerFormat.duration(finish),
                    color = Palette.texte,
                    fontSize = 20.sp,
                    fontWeight = FontWeight.SemiBold,
                )
            }
            if (restante != null) {
                Text(
                    "Reste " + MpacerFormat.distance(restante, imperial = !settings.metric),
                    color = Palette.muted,
                    fontSize = 13.sp,
                )
            }
        }
    }
}

/** Pastille musique : BPM consigne par le moteur, directive et piste en cours. */
@Composable
private fun MusicCard(music: MusicState?) {
    if (music == null || !music.enabled) return
    val cible = music.targetBpm ?: return
    val fleche = when (music.directive) {
        MusicDirective.BOOST -> "  ^ accelerer"
        MusicDirective.RELAX -> "  v calmer"
        MusicDirective.SKIP_TO -> "  >> piste suivante"
        else -> ""
    }
    val couleur = when (music.directive) {
        MusicDirective.BOOST -> Palette.orange
        MusicDirective.RELAX -> Palette.ok
        else -> Palette.muted
    }
    Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 10.dp),
        ) {
            Text("Musique", color = Palette.muted, fontSize = 12.sp)
            Text(
                cible.roundToInt().toString() + " BPM" + fleche,
                color = couleur,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
            )
            music.current?.let { piste ->
                Text(
                    piste.title + (piste.artist?.let { " - " + it } ?: ""),
                    color = Palette.muted,
                    fontSize = 13.sp,
                )
            }
        }
    }
}

/** Commandes de seance : une seule action principale a la fois. */
@Composable
private fun Controls(
    output: EngineOutput?,
    onStart: () -> Unit,
    onArm: () -> Unit,
    onPause: () -> Unit,
    onResume: () -> Unit,
    onStop: () -> Unit,
) {
    val etat = output?.state ?: "Idle"
    val principal = ButtonDefaults.buttonColors(
        containerColor = Palette.orange,
        contentColor = Color.White,
    )
    val secondaire = ButtonDefaults.buttonColors(
        containerColor = Palette.surface2,
        contentColor = Palette.texte,
    )
    val arret = ButtonDefaults.buttonColors(
        containerColor = Palette.surface2,
        contentColor = Palette.danger,
    )

    when (etat) {
        "Idle", "Finished" -> Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Button(
                onClick = onStart,
                colors = principal,
                shape = RoundedCornerShape(28.dp),
                elevation = ButtonDefaults.buttonElevation(defaultElevation = 6.dp, pressedElevation = 2.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(60.dp),
            ) {
                Icon(PhoneIcons.Play, contentDescription = null, modifier = Modifier.size(24.dp))
                Spacer(Modifier.width(10.dp))
                Text("DÉMARRER LA COURSE", fontSize = 16.sp, fontWeight = FontWeight.Bold, letterSpacing = 0.5.sp)
            }
            Button(
                onClick = onArm,
                colors = secondaire,
                shape = RoundedCornerShape(20.dp),
                modifier = Modifier.fillMaxWidth().height(48.dp),
            ) {
                Icon(PhoneIcons.Flag, contentDescription = null, tint = Palette.muted, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text("Départ au premier pas", color = Palette.texte, fontSize = 14.sp, fontWeight = FontWeight.Medium)
            }
        }

        "Running" -> Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Button(
                onClick = onPause,
                colors = secondaire,
                shape = RoundedCornerShape(28.dp),
                modifier = Modifier
                    .weight(1f)
                    .height(58.dp),
            ) {
                Icon(PhoneIcons.Pause, contentDescription = null, modifier = Modifier.size(22.dp))
                Spacer(Modifier.width(8.dp))
                Text("PAUSE", fontSize = 16.sp, fontWeight = FontWeight.Bold)
            }
            Button(
                onClick = onStop,
                colors = arret,
                shape = RoundedCornerShape(28.dp),
                modifier = Modifier
                    .weight(1f)
                    .height(58.dp),
            ) {
                Icon(PhoneIcons.Stop, contentDescription = null, tint = Palette.danger, modifier = Modifier.size(20.dp))
                Spacer(Modifier.width(8.dp))
                Text("ARRÊTER", fontSize = 16.sp, fontWeight = FontWeight.Bold, color = Palette.danger)
            }
        }

        else -> Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Button(
                onClick = onResume,
                colors = principal,
                shape = RoundedCornerShape(28.dp),
                elevation = ButtonDefaults.buttonElevation(defaultElevation = 4.dp),
                modifier = Modifier
                    .weight(1f)
                    .height(58.dp),
            ) {
                Icon(PhoneIcons.Play, contentDescription = null, modifier = Modifier.size(22.dp))
                Spacer(Modifier.width(8.dp))
                Text("REPRENDRE", fontSize = 16.sp, fontWeight = FontWeight.Bold)
            }
            Button(
                onClick = onStop,
                colors = arret,
                shape = RoundedCornerShape(28.dp),
                modifier = Modifier
                    .weight(1f)
                    .height(58.dp),
            ) {
                Icon(PhoneIcons.Stop, contentDescription = null, tint = Palette.danger, modifier = Modifier.size(20.dp))
                Spacer(Modifier.width(8.dp))
                Text("ARRÊTER", fontSize = 16.sp, fontWeight = FontWeight.Bold, color = Palette.danger)
            }
        }
    }
}

private fun etatLibelle(etat: String?): String = when (etat) {
    "Running" -> "En course"
    "Paused" -> "En pause"
    "AutoPaused" -> "Pause automatique"
    "Armed" -> "Prepare"
    "Finished" -> "Termine"
    else -> "Pret"
}
