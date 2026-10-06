package com.mpacer.watch.ui

import com.mpacer.core.ui.GpsLight
import com.mpacer.core.ui.Palette

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.Button
import androidx.wear.compose.material.Chip
import androidx.wear.compose.material.ChipDefaults
import androidx.wear.compose.material.Text
import com.mpacer.core.AssistantMode
import com.mpacer.watch.WatchSettings
import com.mpacer.core.live.LiveConfig
import com.mpacer.core.live.LiveState

/**
 * Reglages essentiels, en listes de puces (pattern Wear OS) plutot qu'en formulaires.
 *
 * Chaque changement doit etre pousse au moteur Rust (`MpacerCore.setAssistant`,
 * `setVoice`) : c'est le coeur qui valide la configuration et signale les manques
 * (distance ou temps cible absents).
 */
@Composable
fun SettingsScreen(
    settings: WatchSettings,
    liveState: LiveState,
    onSettingsChange: (WatchSettings) -> Unit,
    onBack: () -> Unit,
    onMusic: () -> Unit,
    onLive: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(10.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text("Assistant")
        AssistantMode.entries.forEach { mode ->
            Chip(
                label = { Text(mode.label()) },
                onClick = { onSettingsChange(settings.copy(mode = mode)) },
                colors = if (settings.mode == mode) {
                    ChipDefaults.primaryChipColors()
                } else {
                    ChipDefaults.secondaryChipColors()
                },
            )
        }
        Chip(
            label = { Text(if (settings.metric) "Unites : km" else "Unites : miles") },
            onClick = { onSettingsChange(settings.copy(metric = !settings.metric)) },
        )
        Chip(
            label = { Text(if (settings.voice.enabled) "Voix : activee" else "Voix : coupee") },
            onClick = {
                onSettingsChange(settings.copy(voice = settings.voice.copy(enabled = !settings.voice.enabled)))
            },
        )
        Text("Musique")
        Chip(
            label = { Text(if (settings.music.enabled) "Musique : activee" else "Musique : coupee") },
            onClick = { onSettingsChange(settings.copy(music = settings.music.copy(enabled = !settings.music.enabled))) },
            colors = if (settings.music.enabled) {
                ChipDefaults.primaryChipColors()
            } else {
                ChipDefaults.secondaryChipColors()
            },
        )
        Chip(
            label = { Text(if (settings.music.announce) "Annonces : oui" else "Annonces : non") },
            onClick = { onSettingsChange(settings.copy(music = settings.music.copy(announce = !settings.music.announce))) },
        )
        BPM_REFERENCE.forEach { bpm ->
            Chip(
                label = { Text("BPM de reference : " + bpm.toInt()) },
                onClick = { onSettingsChange(settings.copy(music = settings.music.copy(referenceBpm = bpm))) },
                colors = if (settings.music.referenceBpm == bpm) {
                    ChipDefaults.primaryChipColors()
                } else {
                    ChipDefaults.secondaryChipColors()
                },
            )
        }
        Chip(label = { Text("Ouvrir la bibliotheque") }, onClick = onMusic)

        // Suivi en direct : la position part sur le broker MQTT choisi. Tout se
        // regle desormais sur la montre (adresse, identifiants, sujet) : l'ecran
        // dedie ouvre un clavier et teste la connexion avant d'enregistrer.
        Text("Suivi en direct")
        Text(liveState.resume)
        Chip(
            label = { Text(if (settings.live.enabled) "MQTT : active" else "MQTT : coupe") },
            onClick = {
                onSettingsChange(settings.copy(live = settings.live.copy(enabled = !settings.live.enabled)))
            },
            colors = if (settings.live.enabled) {
                ChipDefaults.primaryChipColors()
            } else {
                ChipDefaults.secondaryChipColors()
            },
        )
        LiveConfig.INTERVALS.forEach { secondes ->
            Chip(
                label = { Text("Position toutes les " + secondes + " s") },
                onClick = { onSettingsChange(settings.copy(live = settings.live.copy(intervalS = secondes))) },
                colors = if (settings.live.intervalS == secondes) {
                    ChipDefaults.primaryChipColors()
                } else {
                    ChipDefaults.secondaryChipColors()
                },
            )
        }
        Chip(
            label = { Text("Broker MQTT") },
            onClick = onLive,
            colors = if (settings.live.url.isBlank()) {
                ChipDefaults.secondaryChipColors()
            } else {
                ChipDefaults.primaryChipColors()
            },
        )
        Text(
            if (settings.live.url.isBlank()) "Broker non configure" else settings.live.url,
            fontSize = 11.sp,
        )

        Button(onClick = onBack) { Text("Retour") }
    }
}

/** Valeurs proposees : le BPM de reference colle a l'allure de reference (5:00/km). */
private val BPM_REFERENCE = listOf(160.0, 170.0, 180.0)

private fun AssistantMode.label(): String = when (this) {
    AssistantMode.TRACK_PACE -> "Allure seule"
    AssistantMode.PREDICT_FINISH_TIME -> "Temps de finish estime"
    AssistantMode.ACHIEVE_PLANNED_TIME -> "Shadow runner"
    AssistantMode.REMOTE_RACE -> "Course a distance"
}
