package com.mpacer.core.social

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log

/**
 * Notification systeme a l'arrivee d'une demande d'ami.
 *
 * Le service n'a pas de poussee : c'est le chargement des demandes (ouverture de
 * l'application, onglet Amis, rafraichissement) qui declenche la notification.
 * Aucune notification n'est postee deux fois : l'appelant ne transmet que les
 * demandes qui viennent d'apparaitre.
 */
object FriendsNotifier {

    private const val TAG = "FriendsNotifier"
    private const val CANAL = "mpacer-amis"
    private const val ID = 4201

    /**
     * Previent qu'une ou plusieurs demandes viennent d'arriver.
     *
     * Sans autorisation (Android 13+ refuse), la notification est simplement
     * ignoree : l'onglet Amis reste la source de verite.
     */
    fun notifyNewRequests(context: Context, demandes: List<FriendRequest>) {
        if (demandes.isEmpty()) return
        val gestionnaire = context.getSystemService(Context.NOTIFICATION_SERVICE) as? NotificationManager
            ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            Log.i(TAG, "notification non autorisee : demande visible dans l'onglet Amis")
            return
        }
        val canal = NotificationChannel(
            CANAL,
            "Demandes d'amis",
            NotificationManager.IMPORTANCE_DEFAULT,
        ).apply {
            description = "Une personne souhaite vous ajouter dans M-pacer"
        }
        gestionnaire.createNotificationChannel(canal)

        val texte = if (demandes.size > 1) {
            demandes.size.toString() + " personnes souhaitent vous ajouter en ami"
        } else {
            demandes.first().autre.displayName + " souhaite vous ajouter en ami"
        }
        val ouverture = context.packageManager.getLaunchIntentForPackage(context.packageName)
        val intention = ouverture?.let {
            PendingIntent.getActivity(
                context,
                0,
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
        val notification = Notification.Builder(context, CANAL)
            .setSmallIcon(android.R.drawable.stat_notify_more)
            .setContentTitle("Demande d'ami M-pacer")
            .setContentText(texte)
            .setAutoCancel(true)
            .apply { if (intention != null) setContentIntent(intention) }
            .build()
        gestionnaire.notify(ID, notification)
    }
}
