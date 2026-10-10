package com.mpacer.phone

import android.Manifest
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.core.ui.Palette
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
        // Broker MQTT surchargeable au lancement (developpement, deploiement
        // assiste) : les valeurs sont enregistrees, donc elles survivent au
        // redemarrage de l'application.
        appliquerBroker(intent)
        appliquerBackend(intent)
        // Trace la configuration effective (jamais le mot de passe) : c'est ce
        // qui permet de verifier ce que l'application utilise vraiment.
        journaliserConfiguration()
        demanderPermissions.launch(permissions())
        setContent {
            MpacerTheme {
                MpacerApp()
            }
        }
    }

    /** Nouveau lancement sur une activite deja ouverte (mises a jour des reglages). */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        appliquerBroker(intent)
        appliquerBackend(intent)
    }

    /**
     * Configuration effective, journalisee au demarrage.
     *
     * Le mot de passe du broker n'apparait jamais : seuls l'adresse, le nom
     * d'utilisateur et le nom d'appareil sont utiles au diagnostic.
     */
    private fun journaliserConfiguration() {
        val live = LiveSettings.load(this)
        Log.i(
            TAG,
            "configuration : backend=" + SyncClient.baseUrl(this) +
                " broker=" + live.url +
                " utilisateur=" + live.username.ifBlank { "(aucun)" } +
                " appareil=" + live.deviceName(LiveSettings.deviceId(this)),
        )
    }

    /**
     * Adresse du backend transmise au lancement, pour le deploiement assiste :
     *
     *   adb shell am start -n com.mpacer.phone/.MainActivity \
     *     --es api_url https://mpacer.p.zacharie.org
     *
     * Elle sert a la synchronisation des seances, a l'appairage et au partage
     * entre amis. Enregistree, elle survit au redemarrage de l'application.
     */
    private fun appliquerBackend(intent: Intent?) {
        val url = intent?.getStringExtra(EXTRA_API_URL)?.takeIf { it.isNotBlank() } ?: return
        SyncClient.setBaseUrl(this, url.trim())
        Log.i(TAG, "backend applique : " + url.trim())
    }

    /**
     * Reglages du broker MQTT transmis au lancement, pour le deploiement assiste :
     *
     *   adb shell am start -n com.mpacer.phone/.MainActivity \
     *     --es mqtt_url mqtt://hote:1883 --es mqtt_user mpacer --es mqtt_password secret
     *
     * Seuls les champs fournis sont ecrits : le mot de passe seul peut etre
     * corrige sans retaper l'adresse. Rien n'est journalise du mot de passe.
     */
    private fun appliquerBroker(intent: Intent?) {
        val url = intent?.getStringExtra(EXTRA_MQTT_URL)?.takeIf { it.isNotBlank() } ?: return
        val actuel = LiveSettings.load(this)
        val utilisateur = intent.getStringExtra(EXTRA_MQTT_USER)
        val motDePasse = intent.getStringExtra(EXTRA_MQTT_PASSWORD)
        val nouveau = LiveSettings.save(
            this,
            actuel.copy(
                url = url.trim(),
                username = utilisateur ?: actuel.username,
                password = motDePasse ?: actuel.password,
            ),
        )
        LiveTracker.refresh(this)
        Log.i(TAG, "broker MQTT applique : " + nouveau.url + " (utilisateur " + nouveau.username + ")")
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

    companion object {
        private const val TAG = "MpacerConfig"
        const val EXTRA_API_URL = "api_url"
        const val EXTRA_MQTT_URL = "mqtt_url"
        const val EXTRA_MQTT_USER = "mqtt_user"
        const val EXTRA_MQTT_PASSWORD = "mqtt_password"
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
            NavigationBar(
                containerColor = Palette.surface,
                contentColor = Palette.texte,
                tonalElevation = 8.dp,
            ) {
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
                        label = {
                            Text(
                                text = destination.libelle,
                                fontSize = 11.sp,
                                fontWeight = if (onglet == destination) FontWeight.Bold else FontWeight.Normal,
                            )
                        },
                        colors = NavigationBarItemDefaults.colors(
                            selectedIconColor = Palette.orange,
                            selectedTextColor = Palette.orange,
                            unselectedIconColor = Palette.muted,
                            unselectedTextColor = Palette.muted,
                            indicatorColor = Color(0x24FC4C02),
                        ),
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
