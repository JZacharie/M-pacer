package com.mpacer.watch

import android.Manifest
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.mpacer.watch.music.MusicConfig
import com.mpacer.watch.music.MusicSession
import com.mpacer.watch.ui.MainScreen
import com.mpacer.watch.ui.MusicScreen
import com.mpacer.watch.ui.SettingsScreen
import com.mpacer.watch.ui.SyncScreen

class MainActivity : ComponentActivity() {

    private val requestPermissions = registerResultLauncher()

    private fun registerResultLauncher() = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { _ -> /* l ecran principal reflete l etat GPS renvoye par le moteur */ }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        VoiceCoach.initialise(this)
        applyApiUrl(intent)
        requestPermissions.launch(
            arrayOf(
                Manifest.permission.ACCESS_FINE_LOCATION,
                Manifest.permission.POST_NOTIFICATIONS,
            )
        )

        setContent {
            val state by TrackingService.state.collectAsState()
            var showSettings by remember { mutableStateOf(false) }
            var showSync by remember { mutableStateOf(false) }
            var showMusic by remember { mutableStateOf(false) }
            var settings by remember { mutableStateOf(WatchSettings()) }

            // Les reglages musique partent tout de suite au moteur (ou sont gardes
            // par MusicSession si la seance n'a pas encore demarre).
            val updateSettings: (WatchSettings) -> Unit = { nouveau ->
                settings = nouveau
                MusicSession.setConfig(nouveau.music)
            }

            when {
                showSettings -> SettingsScreen(
                    settings = settings,
                    onSettingsChange = updateSettings,
                    onBack = { showSettings = false },
                    onMusic = {
                        showSettings = false
                        showMusic = true
                    },
                )
                showSync -> SyncScreen(onBack = { showSync = false })
                showMusic -> MusicScreen(onBack = { showMusic = false })
                else -> MainScreen(
                    state = state,
                    onStart = { TrackingService.send(this, TrackingService.ACTION_START) },
                    onPause = { TrackingService.send(this, TrackingService.ACTION_PAUSE) },
                    onResume = { TrackingService.send(this, TrackingService.ACTION_RESUME) },
                    onStop = { TrackingService.send(this, TrackingService.ACTION_STOP) },
                    onSettings = { showSettings = true },
                    onSync = { showSync = true },
                    onMusic = { showMusic = true },
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        applyApiUrl(intent)
    }

    /**
     * URL du backend surchargeable au lancement, utile en developpement :
     *   adb shell am start -n com.mpacer.watch/.MainActivity --es api_url http://hote:8080
     */
    private fun applyApiUrl(intent: Intent?) {
        val url = intent?.getStringExtra(EXTRA_API_URL)?.takeIf { it.isNotBlank() } ?: return
        SyncClient.setBaseUrl(this, url)
    }

    override fun onDestroy() {
        VoiceCoach.shutdown()
        super.onDestroy()
    }

    companion object {
        const val EXTRA_API_URL = "api_url"
    }
}

/** Reglages locaux (miroir de l ecran Settings ; a persister avec DataStore). */
data class WatchSettings(
    val mode: AssistantMode = AssistantMode.TRACK_PACE,
    val raceDistanceM: Double? = null,
    val plannedTimeS: Double? = null,
    val negativeSplitRatio: Double = 0.0,
    val metric: Boolean = true,
    val voice: VoiceConfig = VoiceConfig(),
    val music: MusicConfig = MusicConfig(),
)
