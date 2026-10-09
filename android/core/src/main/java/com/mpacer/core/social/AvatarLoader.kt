package com.mpacer.core.social

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import com.mpacer.core.SyncClient
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.util.concurrent.TimeUnit

/**
 * Photo de profil d'un compte, servie par le backend.
 *
 * Le telephone ne contacte jamais Google : la route `/avatar/{id}` telecharge la
 * photo (ou dessine la pastille aux initiales) et la sert avec le meme jeton que
 * la synchronisation. Le chargement est fait au mieux : sans reseau, sans photo
 * ou sans jeton, l'ecran affiche les initiales du compte.
 */
object AvatarLoader {

    private const val TAG = "AvatarLoader"

    /** Quelques dizaines de photos suffisent : un ecran, pas un annuaire. */
    private val cache = object : LruCache<String, Bitmap>(64) {
        override fun sizeOf(key: String, value: Bitmap): Int = 1
    }

    private val http: OkHttpClient by lazy {
        OkHttpClient.Builder()
            .connectTimeout(10, TimeUnit.SECONDS)
            .readTimeout(20, TimeUnit.SECONDS)
            .build()
    }

    /** Photo du compte, ou null si elle n'est pas disponible. */
    suspend fun load(context: Context, userId: String): Bitmap? = withContext(Dispatchers.IO) {
        if (userId.isBlank()) return@withContext null
        cache.get(userId)?.let { return@withContext it }
        val token = SyncClient.token(context) ?: return@withContext null
        val url = SyncClient.baseUrl(context) + "/avatar/" + userId
        val requete = Request.Builder()
            .url(url)
            .header("Authorization", "Bearer " + token)
            .build()
        try {
            http.newCall(requete).execute().use { reponse ->
                if (!reponse.isSuccessful) return@withContext null
                val bitmap = reponse.body?.byteStream()?.use { flux ->
                    BitmapFactory.decodeStream(flux)
                } ?: return@withContext null
                cache.put(userId, bitmap)
                bitmap
            }
        } catch (erreur: Exception) {
            android.util.Log.w(TAG, "avatar injoignable pour " + userId, erreur)
            null
        }
    }
}
