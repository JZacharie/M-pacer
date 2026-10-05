package com.mpacer.companion.ui

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.companion.UiState

/**
 * Envoi d un fichier local (.pac exporte par la montre, ou JSON de seance) et
 * rappel des deux voies d installation de l application montre.
 */
@Composable
fun SendScreen(
    state: UiState,
    onPickFile: (Uri) -> Unit,
    onPickApk: (Uri) -> Unit,
    onInstallWatch: () -> Unit,
    onBack: () -> Unit,
) {
    val filePicker = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri -> if (uri != null) onPickFile(uri) }

    val apkPicker = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri -> if (uri != null) onPickApk(uri) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text("Envoyer une seance", fontSize = 22.sp, fontWeight = FontWeight.Bold)
        Text(
            "Le fichier .pac produit par la montre (JSON versionne) est relu puis",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            "envoye au backend. Un WorkoutSummary JSON seul est aussi accepte.",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Button(
            onClick = { filePicker.launch(arrayOf("*/*")) },
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Choisir un fichier .pac ou JSON")
        }

        Card(modifier = Modifier.fillMaxWidth()) {
            Column(
                modifier = Modifier.padding(12.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Text("Application montre", fontWeight = FontWeight.Bold)
                Text(state.watchNodes.toString() + " montre(s) joignable(s)")
                Button(onClick = onInstallWatch, modifier = Modifier.fillMaxWidth()) {
                    Text("Installer depuis le Play Store")
                }
                OutlinedButton(
                    onClick = { apkPicker.launch(arrayOf("application/vnd.android.package-archive")) },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Partager un APK compile")
                }
            }
        }

        if (state.busy) {
            CircularProgressIndicator()
        }

        state.message?.let { message ->
            Text(message, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }

        state.syncLog.forEach { entry ->
            Text(entry, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }

        OutlinedButton(onClick = onBack, modifier = Modifier.fillMaxWidth()) {
            Text("Retour")
        }
    }
}
