package com.mpacer.phone.hr

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.Build
import android.os.ParcelUuid
import android.util.Log
import com.mpacer.core.HeartRateSource
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import java.util.UUID

/**
 * Ceinture cardiaque Bluetooth LE : la source de frequence du telephone.
 *
 * Un telephone n'a presque jamais de capteur cardio. La montre, elle, en a un ;
 * c'est donc une ceinture (Polar H10, Garmin HRM, Decathlon Dual...) qui apporte
 * au telephone les zones, la derive cardiaque et l'export GPX.
 *
 * Protocole : profil Heart Rate (service 0x180D, mesure 0x2A37), le seul que
 * toutes les ceintures exposent. Aucun calcul ici : chaque battement part tel
 * quel dans le moteur Rust via [HeartRateSource], exactement comme sur la montre.
 *
 * Rien n'est obligatoire : sans ceinture, la seance reste complete.
 */
class BleHeartRate(
    private val context: Context,
    private val address: String,
) : HeartRateSource {

    override val label: String = "Ceinture BLE"

    private var gatt: BluetoothGatt? = null
    private val onSampleRef = java.util.concurrent.atomic.AtomicReference<((Long, Int) -> Unit)?>(null)

    /** Vrai si la ceinture est en mesure : l'ecran peut afficher "Cardio connecte". */
    private val _connected = MutableStateFlow(false)
    val connected: StateFlow<Boolean> = _connected.asStateFlow()

    override fun start(onSample: (tMs: Long, bpm: Int) -> Unit) {
        onSampleRef.set(onSample)
        val device = device(context, address) ?: run {
            Log.w(TAG, "ceinture introuvable : " + address)
            return
        }
        gatt = try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                device.connectGatt(context, true, callback, BluetoothDevice.TRANSPORT_LE)
            } else {
                device.connectGatt(context, true, callback)
            }
        } catch (erreur: SecurityException) {
            Log.w(TAG, "connexion refusee : permission Bluetooth manquante", erreur)
            null
        }
    }

    override fun stop() {
        onSampleRef.set(null)
        _connected.value = false
        runCatching { gatt?.disconnect() }
        runCatching { gatt?.close() }
        gatt = null
    }

    private val callback = object : BluetoothGattCallback() {

        override fun onConnectionStateChange(connexion: BluetoothGatt, status: Int, nouvelEtat: Int) {
            when (nouvelEtat) {
                BluetoothProfile.STATE_CONNECTED -> runCatching { connexion.discoverServices() }
                BluetoothProfile.STATE_DISCONNECTED -> {
                    _connected.value = false
                    runCatching { connexion.close() }
                }
            }
        }

        override fun onServicesDiscovered(connexion: BluetoothGatt, status: Int) {
            val service = connexion.getService(HEART_RATE_SERVICE) ?: return
            val mesure = service.getCharacteristic(MEASUREMENT) ?: return
            runCatching {
                connexion.setCharacteristicNotification(mesure, true)
                val configuration = mesure.getDescriptor(CLIENT_CONFIG) ?: return@runCatching
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    connexion.writeDescriptor(configuration, BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE)
                } else {
                    @Suppress("DEPRECATION")
                    configuration.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
                    @Suppress("DEPRECATION")
                    connexion.writeDescriptor(configuration)
                }
                _connected.value = true
            }.onFailure { Log.w(TAG, "abonnement impossible", it) }
        }

        override fun onDescriptorWrite(
            connexion: BluetoothGatt,
            descriptor: BluetoothGattDescriptor,
            status: Int,
        ) {
            if (descriptor.uuid == CLIENT_CONFIG) _connected.value = status == BluetoothGatt.GATT_SUCCESS
        }

        override fun onCharacteristicChanged(
            connexion: BluetoothGatt,
            caracteristique: BluetoothGattCharacteristic,
            value: ByteArray,
        ) {
            if (caracteristique.uuid == MEASUREMENT) publier(value)
        }

        @Deprecated("Utilise par Android 12 et avant : le systeme appelle l'autre surcharge.")
        override fun onCharacteristicChanged(
            connexion: BluetoothGatt,
            caracteristique: BluetoothGattCharacteristic,
        ) {
            if (caracteristique.uuid != MEASUREMENT) return
            @Suppress("DEPRECATION")
            publier(caracteristique.value ?: return)
        }
    }

    /**
     * Decode la mesure du profil Heart Rate (spec Bluetooth SIG) :
     * octet 0 = drapeaux, bit 0 = format 16 bits, puis la valeur en battements
     * par minute, petit-boutiste.
     */
    private fun publier(value: ByteArray) {
        if (value.size < 2) return
        val drapeaux = value[0].toInt() and 0xFF
        val bpm = if (drapeaux and 0x01 != 0) {
            if (value.size < 3) return
            (value[1].toInt() and 0xFF) or ((value[2].toInt() and 0xFF) shl 8)
        } else {
            value[1].toInt() and 0xFF
        }
        // La ceinture annonce 0 tant qu'elle n'a pas accroche le pouls.
        if (bpm <= 0) return
        onSampleRef.get()?.invoke(System.currentTimeMillis(), bpm)
    }

    companion object {
        private const val TAG = "BleHeartRate"
        private val HEART_RATE_SERVICE: UUID = UUID.fromString("0000180d-0000-1000-8000-00805f9b34fb")
        private val MEASUREMENT: UUID = UUID.fromString("00002a37-0000-1000-8000-00805f9b34fb")
        private val CLIENT_CONFIG: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")

        /** Retrouve un appareil deja apparie par son adresse. */
        fun device(context: Context, address: String): BluetoothDevice? = runCatching {
            val gestionnaire = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
            gestionnaire?.adapter?.getRemoteDevice(address)
        }.getOrNull()

        /** Vrai si l'adaptateur Bluetooth est present et actif. */
        fun available(context: Context): Boolean = runCatching {
            val gestionnaire = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
            gestionnaire?.adapter?.isEnabled == true
        }.getOrDefault(false)
    }
}

/** Une ceinture vue pendant la recherche. */
data class StrapDevice(val address: String, val name: String, val rssi: Int)

/** Etat de la recherche de ceintures, observe par l'ecran Reglages. */
data class StrapScanState(
    val scanning: Boolean = false,
    val devices: List<StrapDevice> = emptyList(),
    val message: String? = null,
)

/**
 * Recherche des ceintures cardiaques a portee.
 *
 * Le filtre ne retient que les appareils qui annoncent le service Heart Rate :
 * la liste reste courte et ne montre pas les enceintes ou les montres du voisin.
 * La recherche s'arrete d'elle-meme apres [DUREE_SCAN_MS] ; aucun reveil n'est
 * programme, aucun service de fond n'est demarre.
 */
object BleHeartRateScanner {

    private const val DUREE_SCAN_MS = 15_000L
    private val HEART_RATE_SERVICE: UUID = UUID.fromString("0000180d-0000-1000-8000-00805f9b34fb")

    private val _state = MutableStateFlow(StrapScanState())
    val state: StateFlow<StrapScanState> = _state.asStateFlow()

    private var scanner: android.bluetooth.le.BluetoothLeScanner? = null

    private val callback = object : ScanCallback() {
        override fun onScanResult(callbackType: Int, result: ScanResult) {
            val appareil = result.device ?: return
            val nom = runCatching { appareil.name }.getOrNull().orEmpty()
            val vue = StrapDevice(appareil.address, nom.ifBlank { "Ceinture sans nom" }, result.rssi)
            _state.update { etat ->
                if (etat.devices.any { it.address == vue.address }) etat
                else etat.copy(devices = (etat.devices + vue).sortedByDescending { it.rssi })
            }
        }

        override fun onScanFailed(errorCode: Int) {
            _state.update { it.copy(scanning = false, message = "Recherche refusee (code " + errorCode + ")") }
        }
    }

    @SuppressLint("MissingPermission")
    fun start(context: Context) {
        if (_state.value.scanning) return
        val gestionnaire = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
        val adaptateur = gestionnaire?.adapter
        if (adaptateur == null || !adaptateur.isEnabled) {
            _state.value = StrapScanState(message = "Bluetooth desactive : activez-le pour chercher une ceinture")
            return
        }
        scanner = adaptateur.bluetoothLeScanner
        if (scanner == null) {
            _state.value = StrapScanState(message = "Recherche Bluetooth indisponible sur cet appareil")
            return
        }
        val filtre = ScanFilter.Builder()
            .setServiceUuid(ParcelUuid(HEART_RATE_SERVICE))
            .build()
        val reglages = ScanSettings.Builder()
            .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
            .build()
        _state.value = StrapScanState(scanning = true, message = "Recherche des ceintures a portee...")
        try {
            scanner?.startScan(listOf(filtre), reglages, callback)
        } catch (erreur: SecurityException) {
            _state.value = StrapScanState(message = "Permission Bluetooth refusee")
            return
        }
        // Arret automatique : la recherche Bluetooth consomme, elle ne dure pas.
        Thread({
            try {
                Thread.sleep(DUREE_SCAN_MS)
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
            }
            stop()
        }, "mpacer-strap-scan").apply { isDaemon = true }.start()
    }

    @SuppressLint("MissingPermission")
    fun stop() {
        runCatching { scanner?.stopScan(callback) }
        scanner = null
        _state.update {
            it.copy(
                scanning = false,
                message = if (it.devices.isEmpty()) "Aucune ceinture trouvee" else null,
            )
        }
    }
}
