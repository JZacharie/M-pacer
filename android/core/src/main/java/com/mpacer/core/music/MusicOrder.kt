package com.mpacer.core.music

import kotlin.random.Random

/**
 * Ordre de lecture d'une playlist locale (docs/07, section 6.4).
 *
 * L'utilisateur choisit, sur l'ecran Musique de la montre, entre **l'ordre de la
 * playlist** et le **melange**. Ce choix porte sur la lecture des MP3 de la
 * montre : le moteur Rust continue de decider *quelle* piste jouer selon le
 * tempo cible, cette fonction ne fixe que l'ordre de la file locale.
 *
 * Le tirage est explicite ([hasard]) pour que les tests soient reproductibles ;
 * en production, [Random.Default] melange vraiment a chaque lecture.
 */
internal fun ordreLecture(
    tracks: List<LocalTrack>,
    shuffle: Boolean,
    hasard: Random = Random.Default,
): List<LocalTrack> =
    if (!shuffle || tracks.size < 2) tracks else tracks.shuffled(hasard)
