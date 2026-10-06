package com.mpacer.phone.ui

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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.core.EngineOutput
import com.mpacer.core.MpacerFormat
import com.mpacer.core.SessionState
import com.mpacer.core.live.LiveState
import com.mpacer.core.music.MusicDirective
import com.mpacer.core.music.MusicState
import com.mpacer.core.ui.GpsLight
import com.mpacer.core.ui.Palette
import com.mpacer.phone.PhoneSettings
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
        MusicCard(output?.music)

        if (enSeance) {
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                TextButton(onClick = onAnnounce) { Text("Annonce vocale") }
                TextButton(onClick = onResetPaceWindow) { Text("Fenetre d'allure") }
            }
        }

        Controls(output, onStart, onArm, onPause, onResume, onStop)
        Spacer(Modifier.height(4.dp))
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

/** L'allure courante, en tres grand : la seule valeur lue en courant. */
@Composable
private fun PaceBlock(output: EngineOutput?, settings: PhoneSettings) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text = MpacerFormat.pace(output?.currentPace),
            fontSize = 84.sp,
            fontWeight = FontWeight.Bold,
            color = if (output?.isPaused == true) Palette.muted else Palette.texte,
        )
        Text(
            text = if (settings.metric) "min/km" else "min/mi",
            color = Palette.muted,
            fontSize = 14.sp,
        )
        output?.previousLapPace?.let { precedente ->
            Text(
                text = "tour precedent " + MpacerFormat.pace(precedente),
                color = Palette.muted2,
                fontSize = 13.sp,
            )
        }
    }
}

/** Distance, duree, frequence cardiaque, tour courant. */
@Composable
private fun StatsRow(output: EngineOutput?, settings: PhoneSettings) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Tuile(
            libelle = "Distance",
            valeur = MpacerFormat.distance(output?.distanceM ?: 0.0, imperial = !settings.metric),
            modifier = Modifier.weight(1f),
        )
        Tuile(
            libelle = "Duree",
            valeur = MpacerFormat.duration(output?.elapsedS ?: 0.0),
            modifier = Modifier.weight(1f),
        )
        Tuile(
            libelle = "Cardio",
            valeur = output?.heartRateBpm?.let { bpm ->
                output.heartRateZone?.let { "Z" + it + " " + bpm } ?: bpm.toString()
            } ?: "--",
            couleur = Palette.zoneColor(output?.heartRateZone),
            modifier = Modifier.weight(1f),
        )
    }
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Tuile(
            libelle = "Tour courant",
            valeur = MpacerFormat.distance(output?.currentLapDistanceM ?: 0.0, imperial = !settings.metric),
            modifier = Modifier.weight(1f),
        )
        Tuile(
            libelle = "Allure du tour",
            valeur = MpacerFormat.pace(output?.currentLapPace),
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
        colors = CardDefaults.cardColors(containerColor = Palette.surface),
    ) {
        Column(modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp)) {
            Text(libelle, color = Palette.muted, fontSize = 12.sp)
            Text(valeur, color = couleur, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
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
        "Idle", "Finished" -> Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = onStart,
                colors = principal,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(56.dp),
            ) {
                Text("Demarrer la course", fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
            }
            Button(
                onClick = onArm,
                colors = secondaire,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Preparer (depart au premier pas)")
            }
        }

        "Running" -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = onPause,
                colors = secondaire,
                modifier = Modifier
                    .weight(1f)
                    .height(56.dp),
            ) {
                Text("Pause", fontSize = 17.sp)
            }
            Button(
                onClick = onStop,
                colors = arret,
                modifier = Modifier
                    .weight(1f)
                    .height(56.dp),
            ) {
                Text("Arreter", fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
            }
        }

        else -> Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = onResume,
                colors = principal,
                modifier = Modifier
                    .weight(1f)
                    .height(56.dp),
            ) {
                Text("Reprendre", fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
            }
            Button(
                onClick = onStop,
                colors = arret,
                modifier = Modifier
                    .weight(1f)
                    .height(56.dp),
            ) {
                Text("Arreter", fontSize = 17.sp)
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
