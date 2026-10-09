package com.mpacer.phone

import android.Manifest
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.sp
import com.mpacer.core.HeartRateSources
import com.mpacer.core.SessionConfig
import com.mpacer.core.SyncClient
import com.mpacer.core.TrackingService
import com.mpacer.core.VoiceCoach
import com.mpacer.core.live.LiveProbe
import com.mpacer.core.live.LiveSettings
import com.mpacer.core.live.LiveTracker
import com.mpacer.core.music.MusicSession
import com.mpacer.phone.hr.BleHeartRate
import com.mpacer.phone.ui.FriendsScreen
import com.mpacer.phone.ui.HistoryScreen
import com.mpacer.phone.ui.MpacerTheme
import com.mpacer.phone.ui.MusicScreen
import com.mpacer.phone.ui.PhoneIcons
import com.mpacer.phone.ui.RunScreen
import com.mpacer.phone.ui.SettingsScreen
import com.mpacer.phone.ui.SyncScreen

/**
 * Application telephone : courir avec le telephone, sans montre.
 *
 * Meme socle que la montre (:core) : le moteur Rust calcule tout, le service de
 * seance tient le GPS en arriere-plan, la voix parle, le suivi MQTT publie, la
 * synchronisation envoie les seances. Cette activite n'ajoute que l'interface et
 * les permissions du telephone.
 */
class MainActivity : ComponentActivity() {

    private val demanderPermissions =
        registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Identite de l'appareil : le socle ne connait aucun nom d'ecran.
        SyncClient.pairingLabel = BuildConfig.PAIRING_LABEL
        LiveSettings.deviceFallback = "telephone"
        VoiceCoach.initialise(this)
        // Les reglages enregistres partent au moteur avant le premier affichage.
        appliquerReglages(this, PhoneSettingsStore.load(this))
        demanderPermissions.launch(permissions())
        setContent {
            MpacerTheme {
                MpacerApp()
            }
        }
    }

    /**
     * Reprise automatique des seances du compte.
     *
     * Une course faite avec la montre part au backend des la fin de la seance ;
     * ici, le telephone la reprend. Le declencheur est le retour au premier plan
     * (lancement, retour d'arriere-plan), pas une minuterie : rien ne tourne en
     * veille, et un appareil non appaire ne fait aucune requete.
     */
    override fun onStart() {
        super.onStart()
        SyncClient.pullInBackground(this)
    }

    override fun onDestroy() {
        VoiceCoach.shutdown()
        super.onDestroy()
    }

    /** Permissions demandees au premier lancement ; chacune est refusable. */
    private fun permissions(): Array<String> {
        val liste = mutableListOf(
            Manifest.permission.ACCESS_FINE_LOCATION,
            Manifest.permission.ACTIVITY_RECOGNITION,
            Manifest.permission.BODY_SENSORS,
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            liste += Manifest.permission.POST_NOTIFICATIONS
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            // Ceinture cardiaque : recherche et connexion Bluetooth.
            liste += Manifest.permission.BLUETOOTH_SCAN
            liste += Manifest.permission.BLUETOOTH_CONNECT
        }
        return liste.toTypedArray()
    }
}

/** Transmet les reglages au socle (moteur, voix, musique, suivi en direct). */
private fun appliquerReglages(context: android.content.Context, settings: PhoneSettings) {
    SessionConfig.setAssistant(settings.assistant)
    SessionConfig.setVoice(settings.voice)
    MusicSession.setConfig(settings.music)
    LiveTracker.refresh(context)
}

/**
 * Cinq destinations au maximum (conseil Material) : l'appairage du backend vit
 * dans Reglages, comme dans l'interface web ou il a rejoint /settings.
 */
private enum class Onglet(val libelle: String, val icone: ImageVector) {
    Course("Course", PhoneIcons.Run),
    Amis("Amis", PhoneIcons.Group),
    Historique("Historique", PhoneIcons.Chart),
    Musique("Musique", PhoneIcons.Music),
    Reglages("Reglages", PhoneIcons.Settings),
}

@Composable
private fun MpacerApp() {
    val context = LocalContext.current
    var onglet by remember { mutableStateOf(Onglet.Course) }
    // Sous-page d'appairage, ouverte depuis Reglages.
    var syncOuvert by remember { mutableStateOf(false) }
    var settings by remember { mutableStateOf(PhoneSettingsStore.load(context)) }
    val session by TrackingService.state.collectAsState()
    val live by LiveTracker.state.collectAsState()
    val probe by LiveProbe.state.collectAsState()

    // Toute modification de l'ecran Reglages est enregistree et transmise au socle :
    // le moteur n'existe que pendant une seance, le socle garde le reste.
    val majReglages: (PhoneSettings) -> Unit = { nouveau ->
        settings = nouveau
        PhoneSettingsStore.save(context, nouveau)
        appliquerReglages(context, nouveau)
    }

    // Ceinture cardiaque : la seance ne connait qu'une HeartRateSource du socle.
    // Changer de ceinture ferme proprement la precedente (liaison GATT comprise).
    LaunchedEffect(settings.strapAddress) {
        HeartRateSources.external?.stop()
        HeartRateSources.external = settings.strapAddress
            .takeIf { it.isNotBlank() }
            ?.let { BleHeartRate(context, it) }
    }

    // Ecran allume pendant la seance : telephone pose ou a la ceinture.
    val activity = context as? ComponentActivity
    val enSeance = session.output?.let { it.state != "Idle" && it.state != "Finished" } == true
    DisposableEffect(enSeance, settings.keepScreenOn) {
        if (activity != null) {
            if (enSeance && settings.keepScreenOn) {
                activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            } else {
                activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            }
        }
        onDispose { activity?.window?.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON) }
    }

    Scaffold(
        bottomBar = {
            NavigationBar {
                Onglet.entries.forEach { destination ->
                    NavigationBarItem(
                        selected = onglet == destination,
                        onClick = { onglet = destination },
                        icon = {
                            Icon(
                                imageVector = destination.icone,
                                contentDescription = destination.libelle,
                            )
                        },
                        label = { Text(destination.libelle, fontSize = 11.sp) },
                    )
                }
            }
        },
    ) { marges ->
        Surface(modifier = Modifier.fillMaxSize().padding(marges)) {
            when (onglet) {
                Onglet.Course -> RunScreen(
                    state = session,
                    settings = settings,
                    live = live,
                    onStart = { TrackingService.send(context, TrackingService.ACTION_START) },
                    onArm = { TrackingService.send(context, TrackingService.ACTION_ARM) },
                    onPause = { TrackingService.send(context, TrackingService.ACTION_PAUSE) },
                    onResume = { TrackingService.send(context, TrackingService.ACTION_RESUME) },
                    onStop = { TrackingService.send(context, TrackingService.ACTION_STOP) },
                    onAnnounce = { TrackingService.send(context, TrackingService.ACTION_ANNOUNCE) },
                    onResetPaceWindow = { TrackingService.send(context, TrackingService.ACTION_RESET_PACE) },
                )

                Onglet.Amis -> FriendsScreen()
                Onglet.Historique -> HistoryScreen()
                Onglet.Musique -> MusicScreen()
                Onglet.Reglages -> if (syncOuvert) {
                    SyncScreen(onBack = { syncOuvert = false })
                } else {
                    SettingsScreen(
                        settings = settings,
                        liveState = live,
                        probeState = probe,
                        onSettingsChange = majReglages,
                        onTestLive = { LiveProbe.test(it) },
                        onOuvrirSync = { syncOuvert = true },
                    )
                }
            }
        }
    }
}
