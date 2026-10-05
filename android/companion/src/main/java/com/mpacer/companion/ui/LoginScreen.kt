package com.mpacer.companion.ui

import android.net.Uri
import androidx.browser.customtabs.CustomTabsIntent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.companion.PairingState
import com.mpacer.companion.UiState

/**
 * Ecran de connexion : URL du backend, puis flux d appairage par code.
 *
 * Le code utilisateur est valide sur la page /link ; elle s ouvre dans un onglet
 * personnalise pour garder l utilisateur dans l application.
 */
@Composable
fun LoginScreen(
    state: UiState,
    onBaseUrlChange: (String) -> Unit,
    onConnect: () -> Unit,
    onCancel: () -> Unit,
) {
    val context = LocalContext.current
    var url by remember { mutableStateOf(state.baseUrl) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("M-pacer", fontSize = 30.sp, fontWeight = FontWeight.Bold)
        Text(
            "Connexion au backend auto-heberge",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        OutlinedTextField(
            value = url,
            onValueChange = { value ->
                url = value
                onBaseUrlChange(value)
            },
            label = { Text("URL du backend") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )

        when (state.pairingState) {
            PairingState.Idle, PairingState.Failed -> {
                Button(onClick = onConnect, modifier = Modifier.fillMaxWidth()) {
                    Text("Se connecter")
                }
            }

            PairingState.Requesting -> {
                CircularProgressIndicator()
                Text("Demande du code...")
            }

            PairingState.Waiting -> {
                val code = state.pairing
                if (code != null) {
                    Card(modifier = Modifier.fillMaxWidth()) {
                        Column(
                            modifier = Modifier.padding(16.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            Text("Code d appairage")
                            Text(code.userCode, fontSize = 34.sp, fontWeight = FontWeight.Bold)
                            Text(
                                "Saisissez ce code sur la page /link du backend.",
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                    Button(
                        onClick = {
                            val tabs = CustomTabsIntent.Builder().build()
                            tabs.launchUrl(context, Uri.parse(code.verificationUriComplete))
                        },
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text("Ouvrir la page /link")
                    }
                    OutlinedButton(onClick = onCancel, modifier = Modifier.fillMaxWidth()) {
                        Text("Annuler")
                    }
                }
            }

            PairingState.Paired -> Text("Connexion reussie")
        }

        state.message?.let { message ->
            Text(message, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}
