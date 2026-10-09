package com.mpacer.watch.ui

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.Text
import com.mpacer.core.SyncClient
import com.mpacer.core.SyncPhase
import com.mpacer.core.ui.Palette
import kotlinx.coroutines.launch

/**
 * Ecran d'appairage et de synchronisation.
 *
 * L'etat de la connexion est une pastille, pas une phrase : vert connecte,
 * orange non connecte. Le code d'appairage reste la seule grande valeur de
 * l'ecran, et les deux commandes sont des lignes a icone -- l'ancien ecran
 * empilait quatre boutons texte de largeurs differentes.
 */
@Composable
fun SyncScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val state by SyncClient.state.collectAsState()

    LaunchedEffect(Unit) { SyncClient.refresh(context) }

    SecondaryScreen(onBack = onBack) {
        ScreenTitle("Synchronisation")
        StatusPill(
            text = if (state.paired) "Connecte" else "Non connecte",
            color = if (state.paired) Palette.ok else Palette.orange,
            icon = if (state.paired) WatchIcons.Check else WatchIcons.Close,
        )
        Text(
            text = "En attente : " + state.pending + " seance(s)",
            color = Palette.muted,
            fontSize = 12.sp,
            textAlign = TextAlign.Center,
        )

        val pairing = state.pairing
        if (pairing != null) {
            SectionTitle("Code d'appairage")
            Text(
                text = pairing.userCode,
                color = Palette.texte,
                fontSize = 28.sp,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
            )
            Text(
                text = "A saisir sur " + pairing.verificationUri,
                color = Palette.muted,
                fontSize = 11.sp,
                textAlign = TextAlign.Center,
            )
        }

        state.message?.let { message ->
            Text(
                text = message,
                color = Palette.muted,
                fontSize = 11.sp,
                textAlign = TextAlign.Center,
            )
        }
        Spacer(Modifier.height(4.dp))

        if (!state.paired) {
            val occupe = state.phase == SyncPhase.RequestingCode || state.phase == SyncPhase.WaitingApproval
            SettingRow(
                label = when (state.phase) {
                    SyncPhase.RequestingCode -> "Demande du code..."
                    SyncPhase.WaitingApproval -> "Attente d'approbation..."
                    else -> "S'appairer"
                },
                onClick = {
                    scope.launch {
                        val code = SyncClient.startPairing(context)
                        if (code != null) SyncClient.awaitApproval(context, code)
                    }
                },
                selected = true,
                enabled = !occupe,
                icon = WatchIcons.Sync,
            )
        } else {
            SettingRow(
                label = if (state.phase == SyncPhase.Syncing) "Envoi en cours..." else "Synchroniser",
                onClick = { scope.launch { SyncClient.syncPending(context) } },
                selected = true,
                enabled = state.phase != SyncPhase.Syncing,
                icon = WatchIcons.Sync,
            )
            SettingRow(
                label = "Se deconnecter",
                onClick = { SyncClient.disconnect(context) },
                icon = WatchIcons.Close,
            )
        }
    }
}
