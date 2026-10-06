package com.mpacer.watch.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.Button
import androidx.wear.compose.material.ButtonDefaults
import androidx.wear.compose.material.Chip
import androidx.wear.compose.material.Text
import com.mpacer.watch.SyncClient
import com.mpacer.watch.SyncPhase
import kotlinx.coroutines.launch

/**
 * Ecran rond d appairage et de synchronisation.
 *
 * Affiche le code utilisateur a saisir sur la page /link du backend, l etat de la
 * connexion (jeton d appareil present ou non) et le nombre de seances encore locales.
 * Aucune seance n est envoyee automatiquement : l utilisateur declenche l envoi.
 */
@Composable
fun SyncScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val state by SyncClient.state.collectAsState()

    LaunchedEffect(Unit) { SyncClient.refresh(context) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text("Synchronisation", fontWeight = FontWeight.Bold)
        Text(
            text = if (state.paired) "Connecte" else "Non connecte",
            color = if (state.paired) Color(0xFF2ECC71) else Color(0xFFE67E22),
        )
        Text("En attente : " + state.pending + " seance(s)", textAlign = TextAlign.Center)

        val pairing = state.pairing
        if (pairing != null) {
            Text("Code d appairage", textAlign = TextAlign.Center)
            Text(pairing.userCode, fontSize = 28.sp, fontWeight = FontWeight.Bold)
            Text(
                text = "A saisir sur " + pairing.verificationUri,
                textAlign = TextAlign.Center,
            )
        }

        state.message?.let { message ->
            Text(message, textAlign = TextAlign.Center, color = Color.LightGray)
        }

        if (!state.paired) {
            Chip(
                label = {
                    val libelle = when (state.phase) {
                        SyncPhase.RequestingCode -> "Demande du code..."
                        SyncPhase.WaitingApproval -> "En attente d approbation..."
                        else -> "S appairer"
                    }
                    Text(libelle)
                },
                enabled = state.phase != SyncPhase.RequestingCode && state.phase != SyncPhase.WaitingApproval,
                onClick = {
                    scope.launch {
                        val code = SyncClient.startPairing(context)
                        if (code != null) SyncClient.awaitApproval(context, code)
                    }
                },
            )
        } else {
            Chip(
                label = { Text(if (state.phase == SyncPhase.Syncing) "Envoi en cours..." else "Synchroniser") },
                enabled = state.phase != SyncPhase.Syncing,
                onClick = { scope.launch { SyncClient.syncPending(context) } },
            )
            Chip(
                label = { Text("Se deconnecter") },
                onClick = { SyncClient.disconnect(context) },
            )
        }

        Button(
            onClick = onBack,
            colors = ButtonDefaults.secondaryButtonColors(),
        ) {
            Text("Retour")
        }
    }
}
