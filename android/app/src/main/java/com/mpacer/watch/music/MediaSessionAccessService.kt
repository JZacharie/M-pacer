package com.mpacer.watch.music

import android.service.notification.NotificationListenerService

/**
 * Declare l'application comme ecouteur de notifications.
 *
 * Android n'autorise une application a voir les sessions medias d'une autre
 * application (ici Spotify, installe sur la montre) que si elle detient cet
 * acces. Le service ne lit aucune notification : il sert uniquement de justificatif
 * a [SpotifyRemote] pour interroger MediaSessionManager.
 *
 * L'utilisateur autorise l'acces depuis Reglages > Applications > Acces aux
 * notifications (raccourci propose par l'ecran Musique).
 */
class MediaSessionAccessService : NotificationListenerService()
