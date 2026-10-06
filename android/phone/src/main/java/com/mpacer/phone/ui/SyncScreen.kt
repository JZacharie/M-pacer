package com.mpacer.phone.ui

import android.content.Context
import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.core.SyncClient
import com.mpacer.core.SyncPhase
import com.mpacer.core.ui.Palette
import kotlinx.coroutines.launch

/**
 * Appairage et envoi des seances vers le backend auto-heberge.
 *
 * Meme flux que la montre (RFC 8628 simplifie) : l'application demande un code,
 * ouvre la page /link dans un onglet personnalise, puis sonde le jeton jusqu a
 * approbation. Le jeton reste chiffre dans le socle, jamais en clair.
 */
@Composable
fun SyncScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val state by SyncClient.state.collectAsState()
    var url by remember { mutableStateOf(SyncClient.baseUrl(context)) }

    LaunchedEffect(Unit) { SyncClient.refresh(context) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text("Synchronisation", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, color = Palette.texte)
        TextButton(onClick = onBack) { Text("Retour aux reglages") }

        Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text("Backend", color = Palette.muted, fontSize = 12.sp)
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text("Adresse du backend") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(
                        onClick = {
                            SyncClient.setBaseUrl(context, url)
                            SyncClient.refresh(context)
                        },
                    ) {
                        Text("Enregistrer")
                    }
                    TextButton(onClick = { SyncClient.refresh(context) }) { Text("Rafraichir l'etat") }
                }
                Text(
                    text = if (state.paired) "Appareil appaire" else "Appareil non appaire",
                    color = if (state.paired) Palette.ok else Palette.orange,
                    fontWeight = FontWeight.Medium,
                )
                Text(
                    "Seances en attente : " + state.pending,
                    color = Palette.muted,
                    fontSize = 13.sp,
                )
                state.message?.let { Text(it, color = Palette.muted, fontSize = 13.sp) }
            }
        }

        val code = state.pairing
        if (code != null) {
            Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(14.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Text("Code d'appairage", color = Palette.muted, fontSize = 12.sp)
                    Text(code.userCode, fontSize = 30.sp, fontWeight = FontWeight.Bold, color = Palette.texte)
                    Text("A saisir sur " + code.verificationUri, color = Palette.muted, fontSize = 13.sp)
                    TextButton(onClick = {
                        ouvrirOnglet(context, code.verificationUriComplete.ifBlank { code.verificationUri })
                    }) {
                        Text("Ouvrir la page d'appairage")
                    }
                }
            }
        }

        if (!state.paired) {
            Button(
                onClick = {
                    scope.launch {
                        val nouveau = SyncClient.startPairing(context)
                        if (nouveau != null) {
                            // La page s'ouvre tout de suite : l'utilisateur n'a plus
                            // qu'a saisir le code, l'application sonde en parallele.
                            ouvrirOnglet(
                                context,
                                nouveau.verificationUriComplete.ifBlank { nouveau.verificationUri },
                            )
                            SyncClient.awaitApproval(context, nouveau)
                        }
                    }
                },
                enabled = state.phase != SyncPhase.RequestingCode && state.phase != SyncPhase.WaitingApproval,
                colors = ButtonDefaults.buttonColors(
                    containerColor = Palette.orange,
                    contentColor = Color.White,
                ),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(
                    when (state.phase) {
                        SyncPhase.RequestingCode -> "Demande du code..."
                        SyncPhase.WaitingApproval -> "En attente d'approbation..."
                        else -> "S'appairer"
                    },
                    fontSize = 16.sp,
                )
            }
        } else {
            Button(
                onClick = { scope.launch { SyncClient.syncPending(context) } },
                enabled = state.phase != SyncPhase.Syncing,
                colors = ButtonDefaults.buttonColors(
                    containerColor = Palette.orange,
                    contentColor = Color.White,
                ),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(if (state.phase == SyncPhase.Syncing) "Envoi en cours..." else "Envoyer les seances", fontSize = 16.sp)
            }
            TextButton(onClick = { SyncClient.disconnect(context) }) {
                Text("Se deconnecter", color = Palette.danger)
            }
        }
    }
}

/** Ouvre la page /link dans un onglet personnalise, sans quitter l'application. */
private fun ouvrirOnglet(context: Context, url: String) {
    if (url.isBlank()) return
    runCatching {
        CustomTabsIntent.Builder().build().launchUrl(context, Uri.parse(url))
    }
}
