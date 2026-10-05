package com.mpacer.watch

import android.Manifest
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.mpacer.watch.ui.MainScreen
import com.mpacer.watch.ui.SettingsScreen

class MainActivity : ComponentActivity() {

    private val requestPermissions = registerResultLauncher()

    private fun registerResultLauncher() = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { _ -> /* l'ecran principal reflète l'etat GPS renvoye par le moteur */ }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        VoiceCoach.initialise(this)
        requestPermissions.launch(
            arrayOf(
                Manifest.permission.ACCESS_FINE_LOCATION,
                Manifest.permission.POST_NOTIFICATIONS,
            )
        )

        setContent {
            val state by TrackingService.state.collectAsState()
            var showSettings by remember { mutableStateOf(false) }
            var settings by remember { mutableStateOf(WatchSettings()) }

            if (showSettings) {
                SettingsScreen(
                    settings = settings,
                    onSettingsChange = { settings = it },
                    onBack = { showSettings = false },
                )
            } else {
                MainScreen(
                    state = state,
                    onStart = { TrackingService.send(this, TrackingService.ACTION_START) },
                    onPause = { TrackingService.send(this, TrackingService.ACTION_PAUSE) },
                    onResume = { TrackingService.send(this, TrackingService.ACTION_RESUME) },
                    onStop = { TrackingService.send(this, TrackingService.ACTION_STOP) },
                    onSettings = { showSettings = true },
                )
            }
        }
    }

    override fun onDestroy() {
        VoiceCoach.shutdown()
        super.onDestroy()
    }
}

/** Reglages locaux (miroir de l'ecran Settings ; a persister avec DataStore). */
data class WatchSettings(
    val mode: AssistantMode = AssistantMode.TRACK_PACE,
    val raceDistanceM: Double? = null,
    val plannedTimeS: Double? = null,
    val negativeSplitRatio: Double = 0.0,
    val metric: Boolean = true,
    val voice: VoiceConfig = VoiceConfig(),
)
