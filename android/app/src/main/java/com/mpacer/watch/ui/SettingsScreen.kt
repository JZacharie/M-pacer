package com.mpacer.watch.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.wear.compose.material.Button
import androidx.wear.compose.material.Chip
import androidx.wear.compose.material.ChipDefaults
import androidx.wear.compose.material.Text
import com.mpacer.watch.AssistantMode
import com.mpacer.watch.WatchSettings

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
    onSettingsChange: (WatchSettings) -> Unit,
    onBack: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
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
        Button(onClick = onBack) { Text("Retour") }
    }
}

private fun AssistantMode.label(): String = when (this) {
    AssistantMode.TRACK_PACE -> "Allure seule"
    AssistantMode.PREDICT_FINISH_TIME -> "Temps de finish estime"
    AssistantMode.ACHIEVE_PLANNED_TIME -> "Shadow runner"
    AssistantMode.REMOTE_RACE -> "Course a distance"
}
