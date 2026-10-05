package com.mpacer.companion

import android.content.Context
import android.content.Intent
import android.net.Uri
import com.google.android.gms.tasks.Tasks
import com.google.android.gms.wearable.Asset
import com.google.android.gms.wearable.CapabilityClient
import com.google.android.gms.wearable.DataMap
import com.google.android.gms.wearable.PutDataRequest
import com.google.android.gms.wearable.Wearable

/**
 * Pont Wear OS entre le telephone et la montre.
 *
 * Envoi d une seance :
 *  - MessageClient si la charge utile tient dans la limite de 100 Ko du Data Layer ;
 *  - DataClient + Asset au-dela (trace GPS complete), la montre lisant l Asset
 *    dans WearSyncListener.onDataChanged.
 *
 * Les methodes bloquent (Tasks.await) : les appeler depuis Dispatchers.IO.
 */
object WearSync {

    /** Capacite declaree par la montre dans res/values/wear.xml. */
    const val CAPABILITY = "mpacer_sync"
    /** Chemin Data Layer partage avec com.mpacer.watch.WearSyncListener. */
    const val PATH_WORKOUT = "/mpacer/workout"
    const val KEY_ASSET = "workout"

    private const val MESSAGE_LIMIT_BYTES = 90 * 1024
    private const val WEAR_COMPANION = "com.google.android.apps.wear.companion"
    private const val WEAR_LEGACY = "com.google.android.wearable.app"

    sealed interface Outcome {
        data class Sent(val nodes: Int) : Outcome
        object NoDevice : Outcome
        data class Failed(val reason: String) : Outcome
    }

    /** Noeuds joignables portant la capacite mpacer_sync. */
    fun reachableNodeIds(context: Context): List<String> {
        val info = Tasks.await(
            Wearable.getCapabilityClient(context)
                .getCapability(CAPABILITY, CapabilityClient.FILTER_REACHABLE)
        )
        return info.nodes.map { it.id }
    }

    /** Envoie un JSON de seance a toutes les montres joignables. */
    fun sendPayload(context: Context, payload: ByteArray): Outcome {
        val nodes = try {
            reachableNodeIds(context)
        } catch (error: Exception) {
            return Outcome.Failed(error.message ?: "Data Layer indisponible")
        }
        if (nodes.isEmpty()) return Outcome.NoDevice
        return try {
            if (payload.size <= MESSAGE_LIMIT_BYTES) {
                for (node in nodes) {
                    Tasks.await(Wearable.getMessageClient(context).sendMessage(node, PATH_WORKOUT, payload))
                }
            } else {
                val request = PutDataRequest.create(PATH_WORKOUT)
                val dataMap = DataMap().apply {
                    putAsset(KEY_ASSET, Asset.createFromBytes(payload))
                }
                request.setData(dataMap.toByteArray())
                request.setUrgent()
                for (node in nodes) {
                    Tasks.await(Wearable.getDataClient(context).putDataItem(request))
                }
            }
            Outcome.Sent(nodes.size)
        } catch (error: Exception) {
            Outcome.Failed(error.message ?: error.javaClass.simpleName)
        }
    }

    /**
     * Ouvre la fiche Play Store de l application montre. L application compagnon
     * Wear OS relaie l intent vers la montre quand elle est installee (Android 11+
     * exige les declarations <queries> du manifeste).
     */
    fun openWatchStore(context: Context) {
        val packageName = BuildConfig.WATCH_PACKAGE
        val market = Uri.parse("market://details?id=" + packageName)
        val candidates = listOf(
            Intent(Intent.ACTION_VIEW, market).setPackage(WEAR_COMPANION),
            Intent(Intent.ACTION_VIEW, market).setPackage(WEAR_LEGACY),
        )
        for (intent in candidates) {
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            if (intent.resolveActivity(context.packageManager) != null) {
                context.startActivity(intent)
                return
            }
        }
        openUrl(context, "https://play.google.com/store/apps/details?id=" + packageName)
    }

    /**
     * Repli documente : partage d un APK deja compile (adb, transfert manuel).
     * La voie normale reste l installation Play Store ci-dessus.
     */
    fun shareApk(context: Context, uri: Uri) {
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "application/vnd.android.package-archive"
            putExtra(Intent.EXTRA_STREAM, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        val chooser = Intent.createChooser(send, "Partager l APK de la montre")
        chooser.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { context.startActivity(chooser) }
    }

    fun openUrl(context: Context, url: String) {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        runCatching { context.startActivity(intent) }
    }
}
