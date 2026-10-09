package com.mpacer.core.live

import android.content.Context
import java.util.Locale
import kotlin.math.asin
import kotlin.math.cos
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * Parcours planifie : la trace que le coureur compte suivre.
 *
 * Le coureur la choisit une fois (fichier GPX), l'application la garde, puis la
 * publie au depart de chaque seance (PUT /api/v1/live/route, voir
 * FriendsClient.publishRoute). Les suiveurs voient alors, sur la carte, le
 * parcours prevu **et** la position courante, avec le pourcentage de parcours
 * deja couvert.
 *
 * Le parsing reste volontairement modeste : un GPX de course est une suite de
 * balises trkpt avec lat et lon ; les espaces de noms, les extensions et les
 * metadonnees ne nous interessent pas. Aucun ordre d'attribut n'est suppose.
 */
object PlannedRouteStore {

    /** Nombre de points conserves : le serveur en accepte autant. */
    const val MAX_POINTS = 2000

    /** Rayon terrestre moyen (m), comme le coeur Rust. */
    private const val EARTH_RADIUS_M = 6_371_008.8

    private const val PREFERENCES = "mpacer-planned-route"
    private const val CLE_NOM = "nom"
    private const val CLE_POINTS = "points"

    private val BALISE = Regex("<(trkpt|rtept|wpt)\\b([^>]*)>", RegexOption.IGNORE_CASE)
    private val ATTRIBUT_LAT = Regex("lat\\s*=\\s*[\"']([^\"']+)[\"']", RegexOption.IGNORE_CASE)
    private val ATTRIBUT_LON = Regex("lon\\s*=\\s*[\"']([^\"']+)[\"']", RegexOption.IGNORE_CASE)

    /** Parcours retenu : nom du fichier et points dans l'ordre. */
    data class Route(
        val name: String,
        val points: List<Pair<Double, Double>>,
    ) {
        val lengthM: Double get() = distanceM(points)
        val isEmpty: Boolean get() = points.size < 2

        /** Resume d'une ligne pour l'interface. */
        val resume: String
            get() = if (isEmpty) {
                "Aucun parcours"
            } else {
                String.format(Locale.ROOT, "%.2f km  %d points", lengthM / 1000.0, points.size)
            }
    }

    // ------------------------------------------------------------------ parsing

    /**
     * Points d'un fichier GPX, dans l'ordre du document.
     *
     * Les balises trkpt (trace) sont preferees ; a defaut rtept (route) puis
     * wpt (points isoles). Un fichier illisible renvoie une liste vide : le
     * coureur voit « aucun point » plutot qu'une erreur opaque.
     */
    fun parseGpx(xml: String): List<Pair<Double, Double>> {
        for (type in listOf("trkpt", "rtept", "wpt")) {
            val points = buildList {
                BALISE.findAll(xml).forEach { balise ->
                    if (!balise.groupValues[1].equals(type, ignoreCase = true)) return@forEach
                    val attributs = balise.groupValues[2]
                    val lat = ATTRIBUT_LAT.find(attributs)?.groupValues?.get(1)?.toDoubleOrNull()
                    val lon = ATTRIBUT_LON.find(attributs)?.groupValues?.get(1)?.toDoubleOrNull()
                    if (lat != null && lon != null &&
                        lat.isFinite() && lon.isFinite() &&
                        lat in -90.0..90.0 && lon in -180.0..180.0
                    ) {
                        add(lat to lon)
                    }
                }
            }
            if (points.isNotEmpty()) return decimer(points)
        }
        return emptyList()
    }

    /** Sous-echantillonne un parcours trop long, en gardant toujours l'arrivee. */
    fun decimer(points: List<Pair<Double, Double>>, maximum: Int = MAX_POINTS): List<Pair<Double, Double>> {
        if (points.size <= maximum || maximum < 2) return points
        val pas = (points.size + maximum - 1) / maximum
        val sortie = points.filterIndexed { index, _ -> index % pas == 0 }.toMutableList()
        if (sortie.last() != points.last()) sortie.add(points.last())
        return sortie
    }

    /** Longueur du parcours (m), distance orthodromique cumulee. */
    fun distanceM(points: List<Pair<Double, Double>>): Double {
        if (points.size < 2) return 0.0
        var total = 0.0
        for (index in 1 until points.size) {
            total += haversineM(points[index - 1], points[index])
        }
        return total
    }

    private fun haversineM(a: Pair<Double, Double>, b: Pair<Double, Double>): Double {
        val lat1 = Math.toRadians(a.first)
        val lat2 = Math.toRadians(b.first)
        val dLat = lat2 - lat1
        val dLon = Math.toRadians(b.second - a.second)
        val h = sin(dLat / 2).pow(2) + cos(lat1) * cos(lat2) * sin(dLon / 2).pow(2)
        return 2.0 * EARTH_RADIUS_M * asin(sqrt(h).coerceIn(0.0, 1.0))
    }

    // -------------------------------------------------------------- persistance

    /** Enregistre le parcours retenu (fichier GPX choisi par le coureur). */
    fun save(context: Context, route: Route) {
        val prefs = context.applicationContext.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
        val texte = route.points.joinToString(";") {
            String.format(Locale.ROOT, "%.6f,%.6f", it.first, it.second)
        }
        prefs.edit().putString(CLE_NOM, route.name).putString(CLE_POINTS, texte).apply()
    }

    /** Parcours retenu, ou null s'il n'y en a pas. */
    fun load(context: Context): Route? {
        val prefs = context.applicationContext.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
        val texte = prefs.getString(CLE_POINTS, null) ?: return null
        val points = texte.split(';').mapNotNull { couple ->
            val morceaux = couple.split(',')
            val lat = morceaux.getOrNull(0)?.toDoubleOrNull()
            val lon = morceaux.getOrNull(1)?.toDoubleOrNull()
            if (lat != null && lon != null) lat to lon else null
        }
        if (points.size < 2) return null
        return Route(prefs.getString(CLE_NOM, "") ?: "Parcours", points)
    }

    /** Oublie le parcours retenu. */
    fun clear(context: Context) {
        context.applicationContext.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
            .edit().clear().apply()
    }

    /** Corps JSON de la requete PUT /api/v1/live/route. */
    fun payload(device: String, route: Route): String {
        val points = route.points.joinToString(",") {
            String.format(Locale.ROOT, "[%.6f,%.6f]", it.first, it.second)
        }
        val nom = device.replace("\\", "").replace("\"", "")
        return "{\"device\":\"" + nom + "\",\"points\":[" + points + "]}"
    }
}
