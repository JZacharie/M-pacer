package com.mpacer.companion

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.viewmodel.compose.viewModel
import com.mpacer.companion.ui.LoginScreen
import com.mpacer.companion.ui.MpacerTheme
import com.mpacer.companion.ui.MusicScreen
import com.mpacer.companion.ui.SendScreen
import com.mpacer.companion.ui.WorkoutDetailScreen
import com.mpacer.companion.ui.WorkoutListScreen

/**
 * Application telephone : navigation minimale entre quatre ecrans.
 *
 *  Connexion -> Liste -> Detail
 *                    -> Envoi d un fichier / installation de la montre
 */
class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MpacerTheme {
                val viewModel: AppViewModel = viewModel()
                MpacerApp(viewModel)
            }
        }
    }
}

private enum class Dest { Login, List, Detail, Send, Music }

@Composable
fun MpacerApp(viewModel: AppViewModel) {
    val state by viewModel.state.collectAsState()
    val context = LocalContext.current
    var dest by remember { mutableStateOf(Dest.Login) }

    // La connexion (ou son absence) pilote l ecran par defaut.
    LaunchedEffect(state.paired) {
        dest = if (state.paired) Dest.List else Dest.Login
    }

    Surface(modifier = Modifier.fillMaxSize().safeDrawingPadding()) {
        when (dest) {
            Dest.Login -> LoginScreen(
                state = state,
                onBaseUrlChange = viewModel::setBaseUrl,
                onConnect = viewModel::startPairing,
                onCancel = viewModel::cancelPairing,
            )

            Dest.List -> WorkoutListScreen(
                state = state,
                onRefresh = viewModel::refresh,
                onOpen = { id ->
                    viewModel.openDetail(id)
                    dest = Dest.Detail
                },
                onImport = { dest = Dest.Send },
                onInstallWatch = { WearSync.openWatchStore(context) },
                onDisconnect = viewModel::disconnect,
                onOpenMusic = {
                    viewModel.refreshMusic()
                    dest = Dest.Music
                },
            )

            Dest.Music -> MusicScreen(
                state = state,
                onRefresh = viewModel::refreshMusic,
                onOpenPage = { WearSync.openUrl(context, state.baseUrl + "/music") },
                onOpenWorkouts = { dest = Dest.List },
            )

            Dest.Detail -> WorkoutDetailScreen(
                state = state,
                onBack = {
                    viewModel.clearDetail()
                    dest = Dest.List
                },
                onSendToWatch = viewModel::sendToWatch,
                onOpenWeb = { id -> WearSync.openUrl(context, state.baseUrl + "/workouts/" + id) },
            )

            Dest.Send -> SendScreen(
                state = state,
                onPickFile = viewModel::importFile,
                onPickApk = { uri -> WearSync.shareApk(context, uri) },
                onInstallWatch = { WearSync.openWatchStore(context) },
                onBack = { dest = Dest.List },
            )
        }
    }
}
