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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.core.AssistantConfig
import com.mpacer.core.AssistantMode
import com.mpacer.core.BuildInfo
import com.mpacer.core.MusicPolicy
import com.mpacer.core.VoiceFrequency
import com.mpacer.core.VoiceLanguage
import com.mpacer.core.live.LiveConfig
import com.mpacer.core.live.LiveState
import com.mpacer.core.live.ProbeState
import com.mpacer.core.ui.Palette
import com.mpacer.phone.PhoneSettings
import com.mpacer.phone.hr.BleHeartRateScanner
import java.util.Locale

/**
 * Reglages du telephone, en sections depliables a la verticale.
 *
 * Tout ce qui touche au moteur part par SessionConfig (assistant, voix) ou
 * MusicSession (musique) : le socle conserve ces choix et les transmet au depart
 * de la seance. Le suivi en direct, lui, s'enregistre dans le socle, mot de passe
 * chiffre.
 */
@Composable
fun SettingsScreen(
    settings: PhoneSettings,
    liveState: LiveState,
    probeState: ProbeState,
    onSettingsChange: (PhoneSettings) -> Unit,
    onTestLive: (LiveConfig) -> Unit,
    onOuvrirSync: () -> Unit,
) {
    val context = LocalContext.current
    val scan by BleHeartRateScanner.state.collectAsState()
    // Brouillon du broker : les identifiants ne s'enregistrent qu'a la demande,
    // pour ne pas ecrire dans le Keystore a chaque caractere saisi.
    var brouillon by remember(settings.live) { mutableStateOf(settings.live) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text("Reglages", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, color = Palette.texte)

        // ------------------------------------------------- compte et backend
        Section("Compte et synchronisation") {
            Text(
                "Appairez l'appareil pour envoyer vos seances et partager votre position " +
                    "avec vos amis (onglet Amis).",
                color = Palette.muted,
                fontSize = 12.sp,
            )
            Button(
                onClick = onOuvrirSync,
                colors = ButtonDefaults.buttonColors(
                    containerColor = Palette.orange,
                    contentColor = Color.White,
                ),
            ) {
                Icon(PhoneIcons.Refresh, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text("Synchronisation et backend")
            }
        }

        // ------------------------------------------------------------ assistant
        Section("Assistant de course") {
            LignesDeChoix(
                options = AssistantMode.entries.map { libelleMode(it) to (settings.assistant.mode == it) },
            ) { index ->
                onSettingsChange(
                    settings.copy(assistant = settings.assistant.copy(mode = AssistantMode.entries[index]))
                )
            }
            Text(
                text = explicationMode(settings.assistant.mode),
                color = Palette.muted,
                fontSize = 12.sp,
            )
            ChampNombre(
                libelle = "Distance de course (km)",
                valeur = settings.assistant.raceDistanceM?.let { formatNombre(it / 1000.0) } ?: "",
                onValeur = { texte ->
                    onSettingsChange(
                        settings.copy(
                            assistant = settings.assistant.copy(raceDistanceM = texte.toDoubleOrNull()?.times(1000.0))
                        )
                    )
                },
            )
            ChampTexte(
                libelle = "Temps vise (h:mm:ss ou mm:ss)",
                valeur = settings.assistant.plannedTimeS?.let { formatTemps(it) } ?: "",
                onValeur = { texte ->
                    onSettingsChange(
                        settings.copy(assistant = settings.assistant.copy(plannedTimeS = parseTemps(texte)))
                    )
                },
            )
            Text(
                "Part negative : " + (settings.assistant.negativeSplitRatio * 100).toInt() + " %",
                color = Palette.muted,
                fontSize = 12.sp,
            )
            LignesDeChoix(
                options = listOf(0.0, 0.02, 0.05, 0.10).map {
                    (it * 100).toInt().toString() + " %" to (settings.assistant.negativeSplitRatio == it)
                },
            ) { index ->
                val ratio = listOf(0.0, 0.02, 0.05, 0.10)[index]
                onSettingsChange(settings.copy(assistant = settings.assistant.copy(negativeSplitRatio = ratio)))
            }
        }

        // ------------------------------------------------------------ affichage
        Section("Affichage") {
            LigneBascule(
                libelle = "Unites metriques (km) sinon miles",
                actif = settings.metric,
                onBascule = { onSettingsChange(settings.copy(metric = it)) },
            )
            LigneBascule(
                libelle = "Garder l'ecran allume pendant la seance",
                actif = settings.keepScreenOn,
                onBascule = { onSettingsChange(settings.copy(keepScreenOn = it)) },
            )
        }

        // --------------------------------------------------------------- cardio
        Section("Frequence cardiaque") {
            Text(
                "Maximum de l'utilisateur. C'est la reference des zones du compte rendu : " +
                    "une zone n'a de sens que rapporte a sa propre frequence maximale.",
                color = Palette.muted,
                fontSize = 12.sp,
            )
            LignesDeChoix(
                options = FREQUENCES_MAX.map { it.toString() + " bpm" to (settings.heartRateMax == it) },
            ) { index ->
                onSettingsChange(settings.copy(heartRateMax = FREQUENCES_MAX[index]))
            }
        }

        // ----------------------------------------------------------------- voix
        Section("Retour vocal") {
            LigneBascule(
                libelle = "Voix activee",
                actif = settings.voice.enabled,
                onBascule = { onSettingsChange(settings.copy(voice = settings.voice.copy(enabled = it))) },
            )
            Text("Frequence des annonces", color = Palette.muted, fontSize = 12.sp)
            LignesDeChoix(
                options = VoiceFrequency.entries.map { libelleVoix(it) to (settings.voice.frequency == it) },
            ) { index ->
                onSettingsChange(
                    settings.copy(voice = settings.voice.copy(frequency = VoiceFrequency.entries[index]))
                )
            }
            Text("Langue", color = Palette.muted, fontSize = 12.sp)
            LignesDeChoix(
                options = listOf("Francais" to (settings.voice.language == VoiceLanguage.FR), "English" to (settings.voice.language == VoiceLanguage.EN)),
            ) { index ->
                val langue = if (index == 0) VoiceLanguage.FR else VoiceLanguage.EN
                onSettingsChange(settings.copy(voice = settings.voice.copy(language = langue)))
            }
            LigneBascule(
                libelle = "Detail du tour (allure precedente)",
                actif = settings.voice.extendedLapInfo,
                onBascule = { onSettingsChange(settings.copy(voice = settings.voice.copy(extendedLapInfo = it))) },
            )
            LigneBascule(
                libelle = "Formes courtes",
                actif = settings.voice.shortForms,
                onBascule = { onSettingsChange(settings.copy(voice = settings.voice.copy(shortForms = it))) },
            )
            Text("Quand la voix parle", color = Palette.muted, fontSize = 12.sp)
            LignesDeChoix(
                options = MusicPolicy.entries.map { libellePolitique(it) to (settings.voice.musicPolicy == it) },
            ) { index ->
                onSettingsChange(
                    settings.copy(voice = settings.voice.copy(musicPolicy = MusicPolicy.entries[index]))
                )
            }
        }

        // -------------------------------------------------------------- musique
        Section("Musique") {
            LigneBascule(
                libelle = "Musique activee (tempo pilote par le moteur)",
                actif = settings.music.enabled,
                onBascule = { onSettingsChange(settings.copy(music = settings.music.copy(enabled = it))) },
            )
            LigneBascule(
                libelle = "Annoncer les changements de tempo",
                actif = settings.music.announce,
                onBascule = { onSettingsChange(settings.copy(music = settings.music.copy(announce = it))) },
            )
            Text("BPM de reference (allure 5:00/km)", color = Palette.muted, fontSize = 12.sp)
            LignesDeChoix(
                options = listOf(160.0, 170.0, 180.0).map {
                    it.toInt().toString() to (settings.music.referenceBpm == it)
                },
            ) { index ->
                val bpm = listOf(160.0, 170.0, 180.0)[index]
                onSettingsChange(settings.copy(music = settings.music.copy(referenceBpm = bpm)))
            }
        }

        // ------------------------------------------------------------ cardio
        Section("Cardiaque") {
            Text(
                settings.strapLabel?.let { "Ceinture retenue : " + it } ?: "Aucune ceinture : capteur integre seulement",
                color = Palette.muted,
                fontSize = 12.sp,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = { BleHeartRateScanner.start(context) },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = Palette.orange,
                        contentColor = Color.White,
                    ),
                ) {
                    Icon(PhoneIcons.Search, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text(if (scan.scanning) "Recherche..." else "Chercher une ceinture")
                }
                if (settings.strapLabel != null) {
                    TextButton(onClick = {
                        onSettingsChange(settings.copy(strapAddress = "", strapName = ""))
                    }) {
                        Icon(PhoneIcons.Delete, contentDescription = null, tint = Palette.danger, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text("Retirer", color = Palette.danger)
                    }
                }
            }
            scan.message?.let { Text(it, color = Palette.muted, fontSize = 12.sp) }
            scan.devices.forEach { appareil ->
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(appareil.name, color = Palette.texte, fontSize = 14.sp)
                        Text(appareil.address + "  " + appareil.rssi + " dBm", color = Palette.muted, fontSize = 11.sp)
                    }
                    TextButton(onClick = {
                        onSettingsChange(settings.copy(strapAddress = appareil.address, strapName = appareil.name))
                        BleHeartRateScanner.stop()
                    }) {
                        Icon(PhoneIcons.Check, contentDescription = null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text("Choisir")
                    }
                }
            }
            Text(
                "La ceinture est relue au depart de chaque seance ; le capteur integre reste utilise s'il existe.",
                color = Palette.muted,
                fontSize = 11.sp,
            )
        }

        // --------------------------------------------------------- suivi MQTT
        Section("Suivi en direct (MQTT)") {
            Text(liveState.resume, color = Palette.muted, fontSize = 12.sp)
            LigneBascule(
                libelle = "Publier la position pendant la course",
                actif = brouillon.enabled,
                onBascule = { brouillon = brouillon.copy(enabled = it) },
            )
            ChampTexte(
                libelle = "Adresse du broker (mqtt://hote:1883)",
                valeur = brouillon.url,
                onValeur = { brouillon = brouillon.copy(url = it) },
            )
            ChampTexte(
                libelle = "Prefixe des sujets",
                valeur = brouillon.topicPrefix,
                onValeur = { brouillon = brouillon.copy(topicPrefix = it) },
            )
            ChampTexte(
                libelle = "Nom de l'appareil dans le sujet",
                valeur = brouillon.device,
                onValeur = { brouillon = brouillon.copy(device = it) },
            )
            ChampTexte(
                libelle = "Utilisateur",
                valeur = brouillon.username,
                onValeur = { brouillon = brouillon.copy(username = it) },
            )
            ChampTexte(
                libelle = "Mot de passe (chiffre sur l'appareil)",
                valeur = brouillon.password,
                onValeur = { brouillon = brouillon.copy(password = it) },
            )
            Text("Position toutes les...", color = Palette.muted, fontSize = 12.sp)
            LignesDeChoix(
                options = LiveConfig.INTERVALS.map { it.toString() + " s" to (brouillon.intervalS == it) },
            ) { index ->
                brouillon = brouillon.copy(intervalS = LiveConfig.INTERVALS[index])
            }
            Text("... et en pause", color = Palette.muted, fontSize = 12.sp)
            LignesDeChoix(
                options = LiveConfig.INTERVALS.map { it.toString() + " s" to (brouillon.pausedIntervalS == it) },
            ) { index ->
                brouillon = brouillon.copy(pausedIntervalS = LiveConfig.INTERVALS[index])
            }
            LigneBascule(
                libelle = "Conserver le dernier point sur le broker",
                actif = brouillon.retain,
                onBascule = { brouillon = brouillon.copy(retain = it) },
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = { onSettingsChange(settings.copy(live = LiveConfig.normalise(brouillon))) },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = Palette.orange,
                        contentColor = Color.White,
                    ),
                ) {
                    Icon(PhoneIcons.Check, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Enregistrer")
                }
                TextButton(onClick = { onTestLive(LiveConfig.normalise(brouillon)) }) {
                    Icon(PhoneIcons.Refresh, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text(if (probeState.running) "Test en cours..." else "Tester la connexion")
                }
            }
            Text(
                text = probeState.message,
                color = if (probeState.ok) Palette.ok else Palette.muted,
                fontSize = 12.sp,
            )
        }

        // ------------------------------------------------------------- version
        Section("Version") {
            Text(
                BuildInfo.resume(context),
                color = Palette.muted,
                fontSize = 12.sp,
            )
            Text(
                "Le jour indique est celui de la compilation de l'APK (celui de la " +
                    "publication pour une version publiee).",
                color = Palette.muted2,
                fontSize = 11.sp,
            )
        }
    }
}

// ------------------------------------------------------------------ helpers

/** Frequences maximales proposees : la plage usuelle d'un coureur adulte. */
private val FREQUENCES_MAX = listOf(170, 180, 190, 200, 210)

@Composable
private fun Section(titre: String, contenu: @Composable () -> Unit) {
    Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(titre, color = Palette.texte, fontWeight = FontWeight.SemiBold)
            contenu()
        }
    }
}

@Composable
private fun LigneBascule(libelle: String, actif: Boolean, onBascule: (Boolean) -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(libelle, color = Palette.texte, fontSize = 14.sp, modifier = Modifier.weight(1f))
        Switch(checked = actif, onCheckedChange = onBascule)
    }
}

/** Suite de puces a choix unique ; l'index choisi est renvoye a l'appelant. */
@Composable
private fun LignesDeChoix(options: List<Pair<String, Boolean>>, onChoix: (Int) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        options.chunked(2).forEachIndexed { ligne, paire ->
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                paire.forEachIndexed { colonne, (libelle, actif) ->
                    val index = ligne * 2 + colonne
                    FilterChip(
                        selected = actif,
                        onClick = { onChoix(index) },
                        label = { Text(libelle, fontSize = 12.sp) },
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = Palette.orange,
                            selectedLabelColor = Color.White,
                        ),
                    )
                }
            }
        }
    }
}

@Composable
private fun ChampTexte(libelle: String, valeur: String, onValeur: (String) -> Unit) {
    OutlinedTextField(
        value = valeur,
        onValueChange = onValeur,
        label = { Text(libelle, fontSize = 12.sp) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
    )
}

@Composable
private fun ChampNombre(libelle: String, valeur: String, onValeur: (String) -> Unit) {
    OutlinedTextField(
        value = valeur,
        onValueChange = onValeur,
        label = { Text(libelle, fontSize = 12.sp) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
    )
}

/** "1:30:00" ou "45:30" ou "45" (minutes) -> secondes. */
private fun parseTemps(texte: String): Double? {
    val morceaux = texte.trim().split(":").map { it.trim() }
    if (morceaux.isEmpty() || morceaux.size > 3) return null
    val nombres = morceaux.map { it.toIntOrNull() ?: return null }
    return when (nombres.size) {
        1 -> nombres[0] * 60.0
        2 -> nombres[0] * 60.0 + nombres[1]
        else -> nombres[0] * 3600.0 + nombres[1] * 60.0 + nombres[2]
    }
}

private fun formatTemps(secondes: Double): String {
    val total = secondes.toInt()
    val heures = total / 3600
    val minutes = (total % 3600) / 60
    val secs = total % 60
    return if (heures > 0) {
        String.format(Locale.ROOT, "%d:%02d:%02d", heures, minutes, secs)
    } else {
        String.format(Locale.ROOT, "%d:%02d", minutes, secs)
    }
}

private fun formatNombre(valeur: Double): String =
    if (valeur == valeur.toInt().toDouble()) valeur.toInt().toString()
    else String.format(Locale.ROOT, "%.2f", valeur)

private fun libelleMode(mode: AssistantMode): String = when (mode) {
    AssistantMode.TRACK_PACE -> "Allure"
    AssistantMode.PREDICT_FINISH_TIME -> "Finish estime"
    AssistantMode.ACHIEVE_PLANNED_TIME -> "Temps vise"
    AssistantMode.REMOTE_RACE -> "Course a distance"
}

private fun explicationMode(mode: AssistantMode): String = when (mode) {
    AssistantMode.TRACK_PACE -> "Aucune cible : l'allure courante et les tours suffisent."
    AssistantMode.PREDICT_FINISH_TIME -> "Le moteur projette le temps de finish a partir de l'allure."
    AssistantMode.ACHIEVE_PLANNED_TIME -> "Un shadow runner avance au temps vise : l'ecart est affiche."
    AssistantMode.REMOTE_RACE -> "Comparaison a un adversaire distant (mode reserve)."
}

private fun libelleVoix(frequence: VoiceFrequency): String = when (frequence) {
    VoiceFrequency.OFF -> "Jamais"
    VoiceFrequency.EVERY_MINUTE -> "1 min"
    VoiceFrequency.EVERY_2_MINUTES -> "2 min"
    VoiceFrequency.EVERY_5_MINUTES -> "5 min"
    VoiceFrequency.EVERY_LAP -> "Chaque tour"
    VoiceFrequency.MANUAL_ONLY -> "Manuel"
}

private fun libellePolitique(politique: MusicPolicy): String = when (politique) {
    MusicPolicy.DUCK -> "Baisser la musique"
    MusicPolicy.PAUSE -> "Mettre en pause"
    MusicPolicy.IGNORE_AND_SPEAK -> "Parler par-dessus"
}
