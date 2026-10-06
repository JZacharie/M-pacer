package com.mpacer.watch

import android.app.NotificationChannel
import android.os.ParcelFileDescriptor
import android.app.NotificationManager
import android.content.Context
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import com.mpacer.watch.music.MusicLibrary
import com.mpacer.watch.music.PreparePlan
import com.google.android.gms.wearable.Asset
import com.google.android.gms.wearable.DataEvent
import com.google.android.gms.wearable.DataEventBuffer
import com.google.android.gms.wearable.DataMapItem
import com.google.android.gms.wearable.MessageEvent
import com.google.android.gms.wearable.Wearable
import com.google.android.gms.wearable.WearableListenerService
import org.json.JSONObject

/**
 * Reception des seances envoyees par le telephone (Data Layer Wear OS).
 *
 * Le telephone envoie soit un WorkoutSummary unique, soit un fichier .pac complet
 * (champ "workouts"). Dans les deux cas la montre se contente d archiver le JSON
 * produit par mpacer-core via [WorkoutArchive] : aucun recalcul local.
 *
 * Deux canaux coexistent cote telephone :
 *  - MessageClient pour les charges utiles de moins de 100 Ko ;
 *  - DataClient + Asset pour les fichiers plus gros (trace GPS complete).
 *
 * Le chemin /mpacer/music porte les plans de preparation musicale (docs/07
 * section 7.3) : la montre les met en file et telecharge au prochain reveil.
 */
class WearSyncListener : WearableListenerService() {

    override fun onMessageReceived(event: MessageEvent) {
        when (event.path) {
            PATH_WORKOUT -> importPayload(String(event.data, Charsets.UTF_8))
            PATH_MUSIC -> importMusicPlan(String(event.data, Charsets.UTF_8))
        }
    }

    override fun onDataChanged(events: DataEventBuffer) {
        for (event in events) {
            if (event.type != DataEvent.TYPE_CHANGED) continue
            val item = event.dataItem
            if (item.uri.path != PATH_WORKOUT) continue
            val dataMap = DataMapItem.fromDataItem(item).dataMap
            val asset = dataMap.getAsset(KEY_ASSET) ?: continue
            val bytes = readAsset(asset) ?: continue
            importPayload(String(bytes, Charsets.UTF_8))
            runCatching { Wearable.getDataClient(this).deleteDataItems(item.uri) }
        }
    }

    /**
     * Lit le contenu d'un Asset recu par le Data Layer.
     *
     * Play Services 18.x n'expose plus de champ "data" sur Asset : cote recepteur le
     * contenu est materialise dans un fichier, accessible par descripteur ou par URI.
     */
    private fun readAsset(asset: Asset): ByteArray? = runCatching {
        asset.fd?.let { fd -> ParcelFileDescriptor.AutoCloseInputStream(fd).use { it.readBytes() } }
            ?: asset.uri?.let { uri -> contentResolver.openInputStream(uri)?.use { it.readBytes() } }
    }.getOrNull()

    private fun importPayload(text: String) {
        val root = runCatching { JSONObject(text) }.getOrNull() ?: return
        var imported = 0
        val workouts = root.optJSONArray("workouts")
        if (workouts != null) {
            for (index in 0 until workouts.length()) {
                val summary = workouts.optJSONObject(index) ?: continue
                WorkoutArchive.save(this, summary)
                imported += 1
            }
        } else if (root.has("id") || root.has("started_at_ms")) {
            WorkoutArchive.save(this, root)
            imported = 1
        }
        if (imported > 0) notifyImport(imported)
    }

    /** Plan de preparation musicale envoye par le telephone : mis en file localement. */
    private fun importMusicPlan(text: String) {
        val plan = MusicLibrary.onPlanMessage(this, text) ?: return
        notifyMusicPlan(plan)
    }

    private fun notifyMusicPlan(plan: PreparePlan) {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL_MUSIC, "Musique", NotificationManager.IMPORTANCE_DEFAULT)
            )
        }
        val notification = NotificationCompat.Builder(this, CHANNEL_MUSIC)
            .setContentTitle("Musique a preparer")
            .setContentText(plan.name.ifBlank { plan.playlistId } + " : ouvrez Musique sur la montre")
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setAutoCancel(true)
            .build()
        runCatching { manager.notify(NOTIFICATION_ID_MUSIC, notification) }
            .onFailure { Log.w(TAG, "notification musique impossible", it) }
    }

    private fun notifyImport(count: Int) {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(CHANNEL_ID, "Reception", NotificationManager.IMPORTANCE_DEFAULT)
            )
        }
        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Seance recue")
            .setContentText(count.toString() + " seance(s) ajoutee(s) a l historique")
            .setSmallIcon(android.R.drawable.stat_sys_download_done)
            .setAutoCancel(true)
            .build()
        runCatching { manager.notify(NOTIFICATION_ID, notification) }
            .onFailure { Log.w(TAG, "notification impossible", it) }
    }

    companion object {
        /** Chemin Data Layer partage avec le module :companion. */
        const val PATH_WORKOUT = "/mpacer/workout"
        const val KEY_ASSET = "workout"
        /** Plans de preparation musicale (docs/07 section 7.3). */
        const val PATH_MUSIC = "/mpacer/music"
        private const val CHANNEL_ID = "mpacer.sync"
        private const val CHANNEL_MUSIC = "mpacer.music"
        private const val NOTIFICATION_ID = 43
        private const val NOTIFICATION_ID_MUSIC = 44
        private const val TAG = "WearSyncListener"
    }
}
