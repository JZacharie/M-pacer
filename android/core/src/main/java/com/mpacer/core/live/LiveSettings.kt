package com.mpacer.core.live

import android.content.Context
import android.content.SharedPreferences
import android.provider.Settings
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

/**
 * Persistance des reglages du suivi en direct.
 *
 * Le mot de passe du broker, s'il y en a un, est range dans
 * EncryptedSharedPreferences (cle AES256-GCM du Keystore Android), comme le
 * jeton d'appairage de [com.mpacer.core.SyncClient] : aucun secret en clair.
 */
object LiveSettings {

    private const val FICHIER = "mpacer_live"
    private const val KEY_ENABLED = "enabled"
    private const val KEY_URL = "url"
    private const val KEY_TOPIC = "topic_prefix"
    private const val KEY_DEVICE = "device"
    private const val KEY_USER = "username"
    private const val KEY_PASSWORD = "password"
    private const val KEY_INTERVAL = "interval_s"
    private const val KEY_PAUSED_INTERVAL = "paused_interval_s"
    private const val KEY_ACCURACY = "min_accuracy_m"

    @Volatile private var cache: SharedPreferences? = null

    /**
     * Suffixe utilise quand le systeme ne fournit aucun identifiant exploitable.
     * L'application le remplace au demarrage ("montre", "telephone").
     */
    @Volatile var deviceFallback: String = "appareil"

    /** Reglages enregistres, completees par les valeurs par defaut. */
    fun load(context: Context): LiveConfig {
        val prefs = prefs(context)
        val defaut = LiveConfig.DEFAULT
        return LiveConfig.normalise(
            LiveConfig(
                enabled = prefs.getBoolean(KEY_ENABLED, defaut.enabled),
                url = prefs.getString(KEY_URL, defaut.url) ?: defaut.url,
                topicPrefix = prefs.getString(KEY_TOPIC, defaut.topicPrefix) ?: defaut.topicPrefix,
                device = prefs.getString(KEY_DEVICE, defaut.device) ?: defaut.device,
                username = prefs.getString(KEY_USER, defaut.username) ?: defaut.username,
                password = prefs.getString(KEY_PASSWORD, defaut.password) ?: defaut.password,
                intervalS = prefs.getInt(KEY_INTERVAL, defaut.intervalS),
                pausedIntervalS = prefs.getInt(KEY_PAUSED_INTERVAL, defaut.pausedIntervalS),
                minAccuracyM = prefs.getFloat(KEY_ACCURACY, defaut.minAccuracyM.toFloat()).toDouble(),
            )
        )
    }

    /** Enregistre les reglages ; renvoie la version normalisee. */
    fun save(context: Context, config: LiveConfig): LiveConfig {
        val propre = LiveConfig.normalise(config)
        prefs(context).edit()
            .putBoolean(KEY_ENABLED, propre.enabled)
            .putString(KEY_URL, propre.url)
            .putString(KEY_TOPIC, propre.topicPrefix)
            .putString(KEY_DEVICE, propre.device)
            .putString(KEY_USER, propre.username)
            .putString(KEY_PASSWORD, propre.password)
            .putInt(KEY_INTERVAL, propre.intervalS)
            .putInt(KEY_PAUSED_INTERVAL, propre.pausedIntervalS)
            .putFloat(KEY_ACCURACY, propre.minAccuracyM.toFloat())
            .apply()
        return propre
    }

    /**
     * Adresse du broker, modifiable au lancement pour le developpement :
     *   adb shell am start -n <paquet>/.MainActivity --es mqtt_url mqtt://hote:1883
     */
    fun setUrl(context: Context, url: String): LiveConfig =
        save(context, load(context).copy(url = url.trim()))

    /**
     * Identifiant stable de la montre (8 caracteres), utilise comme nom de sujet
     * quand l'utilisateur n'en a pas choisi un. Ce n'est pas un secret : il
     * n'apparait que dans le sujet MQTT.
     */
    fun deviceId(context: Context): String {
        val brut = runCatching {
            Settings.Secure.getString(context.contentResolver, Settings.Secure.ANDROID_ID)
        }.getOrNull()
        val propre = brut.orEmpty().filter { it.isLetterOrDigit() }
        return if (propre.isBlank()) deviceFallback else propre.take(8).lowercase()
    }

    private fun prefs(context: Context): SharedPreferences {
        cache?.let { return it }
        synchronized(this) {
            cache?.let { return it }
            val cle = MasterKey.Builder(context.applicationContext)
                .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
                .build()
            val prefs = EncryptedSharedPreferences.create(
                context.applicationContext,
                FICHIER,
                cle,
                EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
            )
            cache = prefs
            return prefs
        }
    }
}
