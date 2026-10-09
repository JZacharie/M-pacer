package com.mpacer.watch.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.Text
import com.mpacer.core.live.LiveConfig
import com.mpacer.core.live.LiveSettings
import com.mpacer.core.live.LiveState
import com.mpacer.core.live.ProbeState
import com.mpacer.core.ui.Palette

/**
 * Parametres du suivi en direct (MQTT), saisis **depuis la montre**.
 *
 * Historiquement l'adresse du broker ne se reglait qu'en ligne de commande
 * (adb shell am start --es mqtt_url ...) : ce n'est pas utilisable une fois la
 * montre au poignet. Cet ecran ouvre un clavier Wear pour l'adresse, les
 * identifiants, le sujet et la cadence, avec deux garde-fous :
 *
 *  * **Tester la connexion** verifie le brouillon (CONNACK + publication d'un
 *    point de test hors du sujet de suivi) sans rien enregistrer ;
 *  * **Enregistrer** (ou Retour) persiste les reglages dans
 *    EncryptedSharedPreferences : le mot de passe n'est jamais ecrit en clair.
 *
 * La cadence suit le principe de l'ecran Reglages : une ligne qui affiche la
 * valeur courante et la fait tourner, plutot que quatre puces par cadence.
 */
@Composable
fun LiveSettingsScreen(
    config: LiveConfig,
    liveState: LiveState,
    probeState: ProbeState,
    onSave: (LiveConfig) -> Unit,
    onTest: (LiveConfig) -> Unit,
    onBack: () -> Unit,
) {
    // Brouillon local : on n'ecrit dans les prefs chiffrees qu'a l'enregistrement,
    // pas a chaque frappe (le Keystore Android n'est pas gratuit sur une montre).
    var brouillon by remember { mutableStateOf(config) }
    var confirmation by remember { mutableStateOf<String?>(null) }
    val clavier = LocalSoftwareKeyboardController.current
    val focus = LocalFocusManager.current
    val pressePapiers = LocalClipboardManager.current
    val contexte = LocalContext.current
    // Sujet reellement publie : <prefixe>/live/<montre>, montre = ANDROID_ID a defaut.
    val montreId = remember { LiveSettings.deviceId(contexte) }

    SecondaryScreen(
        onBack = {
            clavier?.hide()
            focus.clearFocus()
            // Retour : on enregistre, pour ne pas perdre une saisie faite au clavier.
            onSave(brouillon)
            onBack()
        },
    ) {
        ScreenTitle("Suivi en direct")
        Text(
            text = liveState.resume,
            color = Palette.muted,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
        )

        SettingRow(
            label = if (brouillon.enabled) "MQTT actif" else "MQTT coupe",
            onClick = { brouillon = brouillon.copy(enabled = !brouillon.enabled) },
            selected = brouillon.enabled,
            icon = WatchIcons.Location,
        )

        Champ(
            label = "Adresse du broker",
            valeur = brouillon.url,
            indice = "mqtt://192.168.0.115:1883",
            typeClavier = KeyboardType.Uri,
            onValeur = { brouillon = brouillon.copy(url = it) },
        )
        SettingRow(
            label = "Coller l'adresse",
            onClick = {
                val colle = pressePapiers.getText()?.text?.trim().orEmpty()
                if (colle.isNotBlank()) brouillon = brouillon.copy(url = colle)
            },
            icon = WatchIcons.Sync,
        )

        SectionTitle("Identifiants (facultatifs)")
        Champ(
            label = "Utilisateur",
            valeur = brouillon.username,
            onValeur = { brouillon = brouillon.copy(username = it) },
        )
        Champ(
            label = "Mot de passe",
            valeur = brouillon.password,
            secret = true,
            onValeur = { brouillon = brouillon.copy(password = it) },
        )

        SectionTitle("Sujet publie")
        Champ(
            label = "Prefixe",
            valeur = brouillon.topicPrefix,
            indice = "mpacer",
            onValeur = { brouillon = brouillon.copy(topicPrefix = it) },
        )
        Champ(
            label = "Nom de la montre",
            valeur = brouillon.device,
            indice = "identifiant de la montre",
            onValeur = { brouillon = brouillon.copy(device = it) },
        )
        Text(
            text = "Publie sur " + brouillon.topic(montreId),
            color = Palette.muted2,
            fontSize = 10.sp,
            textAlign = TextAlign.Center,
        )

        SectionTitle("Cadence")
        SettingRow(
            label = "En course : " + brouillon.intervalS + " s",
            onClick = {
                brouillon = brouillon.copy(
                    intervalS = suivant(LiveConfig.INTERVALS, brouillon.intervalS)
                )
            },
        )
        SettingRow(
            label = "En pause : " + brouillon.pausedIntervalS + " s",
            onClick = {
                brouillon = brouillon.copy(
                    pausedIntervalS = suivant(PAUSED_INTERVALS, brouillon.pausedIntervalS)
                )
            },
        )
        SettingRow(
            label = if (brouillon.retain) "Dernier point conserve" else "Dernier point non conserve",
            onClick = { brouillon = brouillon.copy(retain = !brouillon.retain) },
            selected = brouillon.retain,
        )

        SectionTitle("Verification")
        SettingRow(
            label = if (probeState.running) "Test en cours..." else "Tester la connexion",
            onClick = {
                clavier?.hide()
                focus.clearFocus()
                confirmation = null
                onTest(brouillon)
            },
            enabled = !probeState.running,
            icon = WatchIcons.Sync,
        )
        Text(
            text = probeState.message,
            color = when {
                probeState.running -> Palette.muted
                probeState.ok -> Palette.ok
                else -> Palette.attention
            },
            fontSize = 10.sp,
            textAlign = TextAlign.Center,
        )
        SettingRow(
            label = "Enregistrer",
            onClick = {
                clavier?.hide()
                focus.clearFocus()
                onSave(brouillon)
                confirmation = if (brouillon.configured) {
                    "Enregistre : effectif a la prochaine seance"
                } else {
                    "Enregistre : suivi inactif sans adresse"
                }
            },
            selected = true,
            icon = WatchIcons.Check,
        )
        confirmation?.let {
            Text(
                text = it,
                color = Palette.ok,
                fontSize = 10.sp,
                textAlign = TextAlign.Center,
            )
        }
    }
}

/** Valeurs proposees pour la cadence en pause (la montre ne bouge plus). */
private val PAUSED_INTERVALS = listOf(30, 60, 120, 300)

/** Valeur suivante dans une liste finie : le dernier retourne au premier. */
private fun <T> suivant(valeurs: List<T>, courant: T): T =
    valeurs[(valeurs.indexOf(courant) + 1).mod(valeurs.size)]

/**
 * Champ de saisie adapte a un ecran rond : Wear Compose Material 1.4 ne fournit
 * pas de TextField, on habille donc BasicTextField (fond, bordure, indice).
 */
@Composable
private fun Champ(
    label: String,
    valeur: String,
    onValeur: (String) -> Unit,
    indice: String = "",
    secret: Boolean = false,
    typeClavier: KeyboardType = KeyboardType.Text,
) {
    val focus = LocalFocusManager.current
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.Start,
    ) {
        Text(label, color = Palette.muted, fontSize = 11.sp)
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(9.dp))
                .background(Palette.surface2)
                .border(1.dp, Palette.filet, RoundedCornerShape(9.dp))
                .padding(horizontal = 9.dp, vertical = 7.dp),
        ) {
            BasicTextField(
                value = valeur,
                onValueChange = onValeur,
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
                textStyle = TextStyle(color = Palette.texte, fontSize = 13.sp),
                cursorBrush = SolidColor(Palette.orange),
                visualTransformation = if (secret) {
                    PasswordVisualTransformation()
                } else {
                    VisualTransformation.None
                },
                keyboardOptions = KeyboardOptions(
                    keyboardType = typeClavier,
                    imeAction = ImeAction.Done,
                ),
                keyboardActions = KeyboardActions(onDone = { focus.clearFocus() }),
                decorationBox = { interieur ->
                    if (valeur.isEmpty() && indice.isNotEmpty()) {
                        Text(indice, color = Palette.muted2, fontSize = 13.sp, maxLines = 1)
                    }
                    interieur()
                },
            )
        }
    }
}
