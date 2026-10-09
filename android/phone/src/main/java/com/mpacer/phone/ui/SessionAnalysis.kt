package com.mpacer.phone.ui

import com.mpacer.core.MpacerAnalysis
import org.json.JSONArray
import org.json.JSONObject

/**
 * Rapport d'analyse d'une seance, tel que le coeur Rust le produit.
 *
 * Rien n'est recalcule ici : [MpacerAnalysis] appelle mpacer-core, qui sait
 * mesurer une derive cardiaque, une allure ajustee a la pente ou une regularite.
 * Ce fichier ne fait que relire le JSON et lui donner des noms lisibles.
 */
data class AnalyseSeance(
    val distanceM: Double,
    val durationS: Double,
    val elapsedS: Double,
    val pausedS: Double,
    val pauseCount: Int,
    val averagePaceSPerKm: Double,
    val allureAjusteeSPerKm: Double?,
    val distanceEquivalenteM: Double?,
    val denivelePlusM: Double?,
    val deniveleMoinsM: Double?,
    val altitudeMinM: Double?,
    val altitudeMaxM: Double?,
    val profilAltitude: List<Couple>,
    val tours: List<Tour>,
    val cardio: Cardio?,
    val deriveCardiaquePercent: Double?,
    val courbeCardiaque: List<Couple>,
    val vitesseMaxMps: Double?,
    val regularite: Regularite?,
    val observations: List<Observation>,
    val aUneTrace: Boolean,
) {

    /** Un point de courbe : abscisse et ordonnee, chacune dans son unite. */
    data class Couple(val x: Double, val y: Double)

    /** Un temps de passage : un tour complet, ou le dernier troncon partiel. */
    data class Tour(
        val index: Int,
        val distanceM: Double,
        val durationS: Double,
        val allureSPerKm: Double,
        val cardioMoyen: Double?,
        val pentePercent: Double?,
        val allureAjusteeSPerKm: Double?,
        val ecartPlanS: Double?,
    )

    /** Bilan cardiaque : moyennes et temps passe dans chaque zone. */
    data class Cardio(
        val moyenne: Double,
        val max: Int,
        val min: Int,
        val secondesParZone: List<Double>,
        val sousZone1S: Double,
    ) {
        val totalS: Double get() = secondesParZone.sum() + sousZone1S

        fun partZone(index: Int): Double =
            if (totalS <= 0.0) 0.0 else secondesParZone.getOrElse(index) { 0.0 } / totalS
    }

    /** Regularite de l'allure et partage entre les deux moities. */
    data class Regularite(
        val toursComptes: Int,
        val ecartTypeAllureS: Double,
        val ecartTypePercent: Double,
        val allureRapideSPerKm: Double,
        val allureLenteSPerKm: Double,
        val negativeSplitPercent: Double,
    )

    /** Une observation du coeur : niveau "good", "watch" ou "info". */
    data class Observation(val niveau: String, val texte: String)
}

/**
 * Analyse une seance avec le coeur Rust.
 *
 * @param summary seance archivee (format .pac), telle qu'ecrite par le moteur.
 * @param maxBpm frequence cardiaque maximale de l'utilisateur : les zones n'ont
 *   de sens qu'avec elle.
 * @return le rapport, ou null si la seance est illisible (l'historique doit
 *   continuer de s'afficher).
 */
fun analyserSeance(
    summary: JSONObject,
    maxBpm: Int = MpacerAnalysis.MAX_BPM_DEFAUT,
): AnalyseSeance? {
    val rapport = MpacerAnalysis.report(summary, maxBpm)
    if (rapport.has("error")) return null
    if (rapport.optDouble("distance_m", -1.0) < 0.0) return null

    val ajustee = rapport.optJSONObject("grade_adjusted")
    val altitude = rapport.optJSONObject("elevation")
    val cardio = rapport.optJSONObject("heart_rate")
    val derive = rapport.optJSONObject("cardiac_drift")
    val vitesse = rapport.optJSONObject("speed_extremes")
    val regularite = rapport.optJSONObject("regularity")

    return AnalyseSeance(
        distanceM = rapport.optDouble("distance_m", 0.0),
        durationS = rapport.optDouble("duration_s", 0.0),
        elapsedS = rapport.optDouble("elapsed_s", 0.0),
        pausedS = rapport.optDouble("paused_s", 0.0),
        pauseCount = rapport.optInt("pause_count", 0),
        averagePaceSPerKm = rapport.optDouble("average_pace_s_per_km", 0.0),
        allureAjusteeSPerKm = ajustee?.nombre("pace_s_per_km"),
        distanceEquivalenteM = ajustee?.nombre("equivalent_distance_m"),
        denivelePlusM = altitude?.nombre("gain_m"),
        deniveleMoinsM = altitude?.nombre("loss_m"),
        altitudeMinM = altitude?.nombre("min_m"),
        altitudeMaxM = altitude?.nombre("max_m"),
        profilAltitude = rapport.couples("elevation_profile"),
        tours = rapport.tours(),
        cardio = cardio?.let {
            AnalyseSeance.Cardio(
                moyenne = it.optDouble("average_bpm", 0.0),
                max = it.optInt("max_bpm", 0),
                min = it.optInt("min_bpm", 0),
                secondesParZone = it.optJSONArray("zone_seconds").toDoubles(5),
                sousZone1S = it.optDouble("below_zone1_s", 0.0),
            )
        },
        deriveCardiaquePercent = derive?.nombre("decoupling_percent"),
        courbeCardiaque = rapport.couples("heart_rate_curve"),
        vitesseMaxMps = vitesse?.nombre("max_speed_mps"),
        regularite = regularite?.let {
            AnalyseSeance.Regularite(
                toursComptes = it.optInt("split_count", 0),
                ecartTypeAllureS = it.optDouble("pace_spread_s", 0.0),
                ecartTypePercent = it.optDouble("pace_spread_percent", 0.0),
                allureRapideSPerKm = it.optDouble("fastest_pace_s_per_km", 0.0),
                allureLenteSPerKm = it.optDouble("slowest_pace_s_per_km", 0.0),
                negativeSplitPercent = it.optDouble("negative_split_percent", 0.0),
            )
        },
        observations = rapport.observations(),
        aUneTrace = rapport.optBoolean("has_track", false),
    )
}

/** Trace GPS d'une seance, dans l'ordre du parcours. */
fun traceDeSeance(summary: JSONObject): List<PointCarte> {
    val points = summary.optJSONArray("track") ?: return emptyList()
    return (0 until points.length()).mapNotNull { index ->
        val point = points.optJSONObject(index) ?: return@mapNotNull null
        if (point.isNull("lat") || point.isNull("lon")) return@mapNotNull null
        PointCarte(point.optDouble("lat"), point.optDouble("lon"))
    }
}

/**
 * Un repere tous les pasM metres, plus les bornes kilometriques rondes.
 *
 * Sur une sortie longue, un repere par kilometre encombrerait la carte : on
 * espace les reperes au-dela de 10 km.
 */
fun reperesDeSeance(summary: JSONObject, pasM: Double): List<RepereCarte> {
    val points = summary.optJSONArray("track") ?: return emptyList()
    val reperes = mutableListOf<RepereCarte>()
    var prochain = pasM
    for (index in 0 until points.length()) {
        val point = points.optJSONObject(index) ?: continue
        if (point.isNull("lat") || point.isNull("lon")) continue
        val distance = point.optDouble("dist_m", 0.0)
        while (distance >= prochain) {
            val kilometres = prochain / 1000.0
            reperes += RepereCarte(
                lat = point.optDouble("lat"),
                lon = point.optDouble("lon"),
                nom = if (kilometres == kilometres.toInt().toDouble()) {
                    kilometres.toInt().toString() + " km"
                } else {
                    String.format(java.util.Locale.ROOT, "%.1f km", kilometres)
                },
            )
            prochain += pasM
        }
    }
    return reperes
}

/** Vrai si la seance porte des mesures de frequence cardiaque. */
fun seanceAPulsations(summary: JSONObject): Boolean =
    (summary.optJSONArray("heart_rate")?.length() ?: 0) > 0

// ------------------------------------------------------------------ lecture JSON

private fun JSONObject.nombre(cle: String): Double? =
    if (isNull(cle)) null else optDouble(cle).takeIf { !it.isNaN() }

private fun JSONArray?.toDoubles(taille: Int): List<Double> {
    if (this == null) return List(taille) { 0.0 }
    return (0 until taille).map { index -> optDouble(index, 0.0) }
}

private fun JSONObject.couples(cle: String): List<AnalyseSeance.Couple> {
    val tableau = optJSONArray(cle) ?: return emptyList()
    return (0 until tableau.length()).mapNotNull { index ->
        val couple = tableau.optJSONArray(index) ?: return@mapNotNull null
        if (couple.length() < 2) null
        else AnalyseSeance.Couple(couple.optDouble(0), couple.optDouble(1))
    }
}

private fun JSONObject.tours(): List<AnalyseSeance.Tour> {
    val tableau = optJSONArray("splits") ?: return emptyList()
    return (0 until tableau.length()).mapNotNull { index ->
        val tour = tableau.optJSONObject(index) ?: return@mapNotNull null
        AnalyseSeance.Tour(
            index = tour.optInt("index", index + 1),
            distanceM = tour.optDouble("distance_m", 0.0),
            durationS = tour.optDouble("duration_s", 0.0),
            allureSPerKm = tour.optDouble("pace_s_per_km", 0.0),
            cardioMoyen = tour.nombre("heart_rate_avg"),
            pentePercent = tour.nombre("grade_percent"),
            allureAjusteeSPerKm = tour.nombre("gap_pace_s_per_km"),
            ecartPlanS = tour.nombre("plan_delta_s"),
        )
    }
}

private fun JSONObject.observations(): List<AnalyseSeance.Observation> {
    val tableau = optJSONArray("highlights") ?: return emptyList()
    return (0 until tableau.length()).mapNotNull { index ->
        val ligne = tableau.optJSONObject(index) ?: return@mapNotNull null
        val texte = ligne.optString("text")
        if (texte.isBlank()) null
        else AnalyseSeance.Observation(ligne.optString("level", "info"), texte)
    }
}
