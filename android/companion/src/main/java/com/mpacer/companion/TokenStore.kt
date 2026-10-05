package com.mpacer.companion

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

/**
 * Stockage local du jeton d appareil et de l URL du backend.
 *
 * EncryptedSharedPreferences chiffre les cles et les valeurs (AES256-SIV / AES256-GCM)
 * avec une cle maitresse du Keystore Android. Aucun secret n est ecrit en dur.
 */
class TokenStore(context: Context) {

    private val appContext = context.applicationContext

    private val prefs: SharedPreferences = EncryptedSharedPreferences.create(
        appContext,
        FILE_NAME,
        MasterKey.Builder(appContext)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build(),
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    var baseUrl: String
        get() = prefs.getString(KEY_BASE_URL, null)?.takeIf { it.isNotBlank() }
            ?: BuildConfig.DEFAULT_API_URL
        set(value) {
            prefs.edit().putString(KEY_BASE_URL, value.trim().trimEnd('/')).apply()
        }

    var token: String?
        get() = prefs.getString(KEY_TOKEN, null)?.takeIf { it.isNotBlank() }
        set(value) {
            prefs.edit().putString(KEY_TOKEN, value).apply()
        }

    fun clearToken() {
        prefs.edit().remove(KEY_TOKEN).apply()
    }

    companion object {
        private const val FILE_NAME = "mpacer_companion"
        private const val KEY_TOKEN = "device_token"
        private const val KEY_BASE_URL = "base_url"
    }
}
