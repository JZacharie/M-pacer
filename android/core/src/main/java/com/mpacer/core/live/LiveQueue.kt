package com.mpacer.core.live

/**
 * File d'attente des positions en attente de publication sur le broker.
 *
 * Le suivi en direct ne doit pas perdre la trace quand le reseau tombe : les
 * points s'accumulent ici pendant la coupure, puis repartent en rafale a la
 * reconnexion. Trois garde-fous, dans cet ordre :
 *
 *  1. **fenetre temporelle** : rien de plus vieux que [windowMs] apres le point
 *     le plus recent. A dix secondes d'intervalle, vingt minutes : au-dela, un
 *     suiveur ne peut de toute facon plus rattraper la course en direct ;
 *  2. **plafond de points** : au-dela de [maxPoints], l'historique est compresse
 *     de moitie (un point sur deux). Le trace garde sa forme, la memoire et la
 *     rafale restent bornees ;
 *  3. **arret** : [keepNewest] ne garde que la position courante, pour que le
 *     dernier message (l'etat `stop`) parte tout de suite.
 *
 * La classe ne prend aucun verrou : [LiveTracker] la protege par son moniteur,
 * qui couvre la file et ses compteurs.
 */
internal class LiveQueue(
    private val maxPoints: Int = MAX_POINTS,
    private val windowMs: Long = WINDOW_MS,
) {

    /** Un point pret a partir, avec l'horodatage qui l'ordonne. */
    class Entry(val tMs: Long, val packet: ByteArray)

    private val entrees = ArrayDeque<Entry>()

    /** Nombre de points en attente. */
    val size: Int get() = entrees.size

    val isEmpty: Boolean get() = entrees.isEmpty()

    /** Points abandonnes : hors fenetre, compression ou arret de la seance. */
    var dropped: Int = 0
        private set

    fun clear() {
        entrees.clear()
        dropped = 0
    }

    /**
     * Ajoute un point. Le point le plus recent sert de reference a la fenetre :
     * une position plus ancienne que [windowMs] est abandonnee.
     */
    fun add(entry: Entry) {
        entrees.addLast(entry)
        evincerHorsFenetre(entry.tMs)
        compresser()
    }

    fun removeFirst(): Entry? = entrees.removeFirstOrNull()

    /** A l'arret : ne garder que la position courante, tout de suite. */
    fun keepNewest() {
        while (entrees.size > 1) {
            entrees.removeFirst()
            dropped++
        }
    }

    private fun evincerHorsFenetre(referenceMs: Long) {
        val limite = referenceMs - windowMs
        while (entrees.isNotEmpty() && entrees.first().tMs < limite) {
            entrees.removeFirst()
            dropped++
        }
    }

    /**
     * Compresse la file tant qu'elle depasse [maxPoints] : un point sur deux,
     * en forcant la position courante (le dernier point), sinon le trace
     * s'arreterait avant l'arrivee.
     */
    private fun compresser() {
        while (entrees.size > maxPoints) {
            val taille = entrees.size
            val gardees = ArrayDeque<Entry>(taille / 2 + 1)
            var index = 0
            for (entree in entrees) {
                if (index % 2 == 0) gardees.addLast(entree)
                index++
            }
            // Dernier point de la file : il porte la position courante.
            if (taille % 2 == 0) {
                gardees.removeLast()
                gardees.addLast(entrees.last())
            }
            if (gardees.size >= taille) break
            dropped += taille - gardees.size
            entrees.clear()
            entrees.addAll(gardees)
        }
    }

    companion object {
        /** Vingt minutes a une position toutes les dix secondes. */
        const val MAX_POINTS = LiveConfig.MAX_QUEUE_POINTS
        const val WINDOW_MS = 20 * 60 * 1000L
    }
}
