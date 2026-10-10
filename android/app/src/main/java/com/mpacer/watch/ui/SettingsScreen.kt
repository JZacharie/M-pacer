package com.mpacer.watch.ui

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.Text
import com.mpacer.core.AssistantMode
import com.mpacer.core.BuildInfo
import com.mpacer.core.SyncState
import com.mpacer.core.live.LiveConfig
import com.mpacer.core.live.LiveState
import com.mpacer.core.ui.Palette
import com.mpacer.watch.WatchSettings

/**
 * Reglages essentiels : une ligne par reglage, un intertitre par famille.
 *
 * Les listes de puces de l'ecran precedent tenaient sur trois ecrans de
 * defilement, avec trois puces « BPM de reference » et quatre puces de cadence
 * MQTT. Une valeur qui n'a que trois etats n'a pas besoin de trois puces : la
 * ligne affiche la valeur courante et la fait tourner a l'appui. L'ecran tient
 * desormais sans qu'on ait a chercher.
 *
 * Chaque changement est pousse au moteur Rust ([com.mpacer.core.SessionConfig]) :
 * c'est lui qui valide la configuration et signale les manques.
 */
@Composable
fun SettingsScreen(
    settings: WatchSettings,
    liveState: LiveState,
    syncState: SyncState,
    onSettingsChange: (WatchSettings) -> Unit,
    onBack: () -> Unit,
    onMusic: () -> Unit,
    onSync: () -> Unit,
    onLive: () -> Unit,
) {
    SecondaryScreen(onBack = onBack) {
        ScreenTitle("Reglages")

        SectionTitle("Assistant")
        AssistantMode.entries.forEach { mode ->
            SettingRow(
                label = mode.libelle(),
                onClick = { onSettingsChange(settings.copy(mode = mode)) },
                selected = settings.mode == mode,
                icon = mode.icone(),
            )
        }

        SectionTitle("Affichage")
        SettingRow(
            label = if (settings.metric) "Allure au kilometre" else "Allure au mile",
            onClick = { onSettingsChange(settings.copy(metric = !settings.metric)) },
            selected = settings.metric,
        )
        SettingRow(
            label = if (settings.voice.enabled) "Voix activee" else "Voix coupee",
            onClick = {
                onSettingsChange(
                    settings.copy(voice = settings.voice.copy(enabled = !settings.voice.enabled))
                )
            },
            selected = settings.voice.enabled,
            icon = WatchIcons.Sound,
        )

        SectionTitle("Musique")
        SettingRow(
            label = if (settings.music.enabled) "Musique activee" else "Musique coupee",
            onClick = {
                onSettingsChange(
                    settings.copy(music = settings.music.copy(enabled = !settings.music.enabled))
                )
            },
            selected = settings.music.enabled,
            icon = WatchIcons.Music,
        )
        SettingRow(
            label = if (settings.music.announce) "Annonces oui" else "Annonces non",
            onClick = {
                onSettingsChange(
                    settings.copy(music = settings.music.copy(announce = !settings.music.announce))
                )
            },
            selected = settings.music.announce,
        )
        SettingRow(
            label = "BPM de reference : " + settings.music.referenceBpm.toInt(),
            onClick = {
                onSettingsChange(
                    settings.copy(
                        music = settings.music.copy(
                            referenceBpm = suivant(BPM_REFERENCE, settings.music.referenceBpm)
                        )
                    )
                )
            },
        )
        SettingRow(label = "Bibliotheque", onClick = onMusic, icon = WatchIcons.Music)

        SectionTitle("Suivi en direct")
        Text(
            text = liveState.resume,
            color = Palette.muted,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
        )
        SettingRow(
            label = if (settings.live.enabled) "MQTT actif" else "MQTT coupe",
            onClick = {
                onSettingsChange(
                    settings.copy(live = settings.live.copy(enabled = !settings.live.enabled))
                )
            },
            selected = settings.live.enabled,
            icon = WatchIcons.Location,
        )
        SettingRow(
            label = "Position toutes les " + settings.live.intervalS + " s",
            onClick = {
                onSettingsChange(
                    settings.copy(
                        live = settings.live.copy(
                            intervalS = suivant(LiveConfig.INTERVALS, settings.live.intervalS)
                        )
                    )
                )
            },
        )
        SettingRow(
            label = if (settings.live.url.isBlank()) "Broker MQTT" else "Broker configure",
            onClick = onLive,
            selected = settings.live.url.isNotBlank(),
            icon = WatchIcons.Sync,
        )

        // L'appairage et la synchronisation ont quitte le cadran d'accueil :
        // c'est un reglage comme un autre, on le retrouve ici.
        SectionTitle("Synchronisation")
        Text(
            text = if (syncState.paired) {
                "Connecte - " + syncState.pending + " seance(s) en attente"
            } else {
                "Non connecte"
            },
            color = if (syncState.paired) Palette.ok else Palette.orange,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
        )
        SettingRow(
            label = if (syncState.paired) "Synchroniser" else "S'appairer",
            onClick = onSync,
            selected = syncState.paired,
            icon = WatchIcons.Sync,
        )

        // Version et jour de compilation : la montre affiche l'APK qu'elle
        // execute, comme le telephone et le compagnon.
        SectionTitle("Version")
        Text(
            text = BuildInfo.resume(LocalContext.current),
            color = Palette.muted,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(4.dp))
    }
}

/** Valeurs proposees : le BPM de reference colle a l'allure de reference (5:00/km). */
private val BPM_REFERENCE = listOf(160.0, 170.0, 180.0)

/** Valeur suivante dans une liste finie : le dernier retourne au premier. */
private fun <T> suivant(valeurs: List<T>, courant: T): T =
    valeurs[(valeurs.indexOf(courant) + 1).mod(valeurs.size)]

private fun AssistantMode.libelle(): String = when (this) {
    AssistantMode.TRACK_PACE -> "Allure seule"
    AssistantMode.PREDICT_FINISH_TIME -> "Temps de finish estime"
    AssistantMode.ACHIEVE_PLANNED_TIME -> "Shadow runner"
    AssistantMode.REMOTE_RACE -> "Course a distance"
}

private fun AssistantMode.icone(): ImageVector? = when (this) {
    AssistantMode.TRACK_PACE -> null
    AssistantMode.PREDICT_FINISH_TIME -> WatchIcons.Flag
    AssistantMode.ACHIEVE_PLANNED_TIME -> WatchIcons.Person
    AssistantMode.REMOTE_RACE -> WatchIcons.Location
}
