package com.mpacer.phone.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.ui.draw.clip
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.core.MpacerFormat
import com.mpacer.core.SyncClient
import com.mpacer.core.SyncState
import com.mpacer.core.WorkoutArchive
import com.mpacer.core.ui.Palette
import com.mpacer.phone.PhoneSettingsStore
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Historique local du telephone.
 *
 * La liste ne calcule rien : elle affiche le resume tel que le moteur l'a
 * ecrit. La fiche d'une seance, elle, demande son rapport au coeur Rust
 * ([analyserSeance]) : zones cardiaques, allure ajustee a la pente, derive
 * cardiaque, regularite. Aucun de ces calculs n'est fait ici, et la fiche
 * s'ouvre meme si le rapport echoue.
 */
@Composable
fun HistoryScreen() {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val sync by SyncClient.state.collectAsState()
    var seances by remember { mutableStateOf(WorkoutArchive.list(context)) }
    var ouverte by remember { mutableStateOf<JSONObject?>(null) }
    val maxBpm = remember { PhoneSettingsStore.load(context).heartRateMax }

    // Reprise automatique : une seance courue avec la montre est deja sur le
    // backend, elle arrive ici sans qu'on ait rien a demander.
    LaunchedEffect(Unit) { SyncClient.pullInBackground(context) }
    // La liste se relit des qu'une reprise a eu lieu, quelle qu'en soit l'issue.
    LaunchedEffect(sync.lastPullMs) {
        if (sync.lastPullMs != null) seances = WorkoutArchive.list(context)
    }

    val detail = ouverte
    if (detail != null) {
        FicheSeance(
            summary = detail,
            maxBpm = maxBpm,
            onBack = { ouverte = null },
            onDelete = {
                WorkoutArchive.delete(context, detail.optLong("started_at_ms"))
                seances = WorkoutArchive.list(context)
                ouverte = null
            },
            onShare = { partager(context, detail) },
        )
        return
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("Historique", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, color = Palette.texte)
        Text(
            text = seances.size.toString() + " seance(s) sur le telephone",
            color = Palette.muted,
            fontSize = 13.sp,
        )
        CarteMontre(sync) {
            scope.launch {
                SyncClient.pullWorkouts(context)
                seances = WorkoutArchive.list(context)
            }
        }
        if (seances.isEmpty()) {
            Text("Aucune seance : lancez une course depuis l'onglet Course.", color = Palette.muted)
            return@Column
        }
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(seances) { seance ->
                CarteSeance(seance) { ouverte = seance }
            }
        }
    }
}

/**
 * Etat de la reprise des seances du compte.
 *
 * Le telephone ne fabrique rien : il affiche ce qu'a rapporte la derniere
 * reprise et propose de la relancer. Sans backend appaire, il le dit au lieu de
 * laisser croire que rien n'arrive.
 */
@Composable
private fun CarteMontre(sync: SyncState, onPull: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = Palette.surface),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            // Copie locale : une propriete delegatee ne se laisse pas smart-caster.
            val reprise = sync.pullMessage
            Text("Seances de la montre", color = Palette.texte, fontWeight = FontWeight.Medium)
            Text(
                text = when {
                    !sync.paired ->
                        "Backend non appaire : rien ne peut etre repris. Reglages > Synchronisation."
                    sync.pulling -> "Recherche des seances de la montre..."
                    reprise != null -> reprise
                    else -> "Reprise automatique des que le telephone est en ligne."
                },
                color = if (sync.paired) Palette.muted else Palette.attention,
                fontSize = 12.sp,
            )
            sync.lastPullMs?.let { instant ->
                Text(
                    text = (if (sync.received > 0) "Derniere reprise il y a " else "Derniere recherche il y a ") +
                        age(instant),
                    color = Palette.muted2,
                    fontSize = 11.sp,
                )
            }
            if (sync.paired) {
                TextButton(onClick = onPull) {
                    Icon(PhoneIcons.Refresh, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Chercher maintenant")
                }
            }
        }
    }
}

/** Age en clair : deux unites suffisent pour situer une reprise. */
private fun age(instantMs: Long): String {
    val secondes = ((System.currentTimeMillis() - instantMs) / 1000).coerceAtLeast(0)
    return when {
        secondes < 60 -> secondes.toString() + " s"
        secondes < 3600 -> (secondes / 60).toString() + " min"
        else -> (secondes / 3600).toString() + " h"
    }
}

@Composable
private fun CarteSeance(summary: JSONObject, onOpen: () -> Unit) {
    val trace = remember(summary) { traceDeSeance(summary) }
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = androidx.compose.foundation.shape.RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = Palette.surface),
        border = androidx.compose.foundation.BorderStroke(1.dp, Color(0x1FFFFFFF)),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = date(summary.optLong("started_at_ms")),
                        color = Palette.texte,
                        fontSize = 15.sp,
                        fontWeight = FontWeight.SemiBold,
                    )
                    val capteurs = etiquettes(summary)
                    if (capteurs.isNotEmpty()) {
                        Text(
                            text = capteurs.joinToString(" • "),
                            color = Palette.muted2,
                            fontSize = 11.sp,
                        )
                    }
                }
                TextButton(onClick = onOpen) {
                    Text("Détail", color = Palette.orange, fontWeight = FontWeight.Medium)
                    Spacer(Modifier.width(2.dp))
                    Icon(PhoneIcons.ChevronRight, contentDescription = null, tint = Palette.orange, modifier = Modifier.size(16.dp))
                }
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                // Métriques principales en grand (Style Strava)
                Row(
                    modifier = Modifier.weight(1f),
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    Column {
                        Text("Distance", color = Palette.muted, fontSize = 11.sp)
                        Text(
                            text = MpacerFormat.distance(summary.optDouble("distance_m")),
                            color = Palette.texte,
                            fontSize = 18.sp,
                            fontWeight = FontWeight.Bold,
                        )
                    }
                    Column {
                        Text("Allure", color = Palette.muted, fontSize = 11.sp)
                        Text(
                            text = MpacerFormat.pace(summary.optDouble("average_pace_s_per_km")) + " /km",
                            color = Palette.texte,
                            fontSize = 18.sp,
                            fontWeight = FontWeight.Bold,
                        )
                    }
                    Column {
                        Text("Temps", color = Palette.muted, fontSize = 11.sp)
                        Text(
                            text = MpacerFormat.duration(summary.optDouble("duration_s")),
                            color = Palette.texte,
                            fontSize = 18.sp,
                            fontWeight = FontWeight.Bold,
                        )
                    }
                }

                // Mini silhouette GPS vectorielle si trace disponible
                if (trace.size >= 2) {
                    Box(
                        modifier = Modifier
                            .size(width = 80.dp, height = 60.dp)
                            .clip(RoundedCornerShape(8.dp))
                            .background(Palette.surface2),
                    ) {
                        MiniTraceGPX(
                            points = trace,
                            couleur = Palette.orange,
                            modifier = Modifier.fillMaxSize(),
                        )
                    }
                }
            }
        }
    }
}

/** Ce que la seance porte, sans rien calculer : cardio, trace, tours. */
private fun etiquettes(summary: JSONObject): List<String> = buildList {
    if (seanceAPulsations(summary)) add("cardio")
    if ((summary.optJSONArray("track")?.length() ?: 0) > 0) add("trace GPS")
    if ((summary.optJSONArray("laps")?.length() ?: 0) > 0) {
        add(summary.optJSONArray("laps").length().toString() + " tours")
    }
    if ((summary.optJSONArray("pauses")?.length() ?: 0) > 0) add("pauses")
}

// ------------------------------------------------------------------ la fiche

@Composable
private fun FicheSeance(
    summary: JSONObject,
    maxBpm: Int,
    onBack: () -> Unit,
    onDelete: () -> Unit,
    onShare: () -> Unit,
) {
    val context = LocalContext.current
    // Une seule analyse, a l'ouverture : elle traverse le JNI une fois.
    val analyse = remember(summary, maxBpm) {
        runCatching { analyserSeance(summary, maxBpm) }.getOrNull()
    }
    val trace = remember(summary) { traceDeSeance(summary) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text(
            date(summary.optLong("started_at_ms")),
            fontSize = 20.sp,
            fontWeight = FontWeight.SemiBold,
            color = Palette.texte,
        )
        Text(
            MpacerFormat.distance(summary.optDouble("distance_m")) + "   " +
                MpacerFormat.duration(summary.optDouble("duration_s")) + "   " +
                MpacerFormat.pace(summary.optDouble("average_pace_s_per_km")) + " /km",
            color = Palette.muted,
        )

        // 1. Ce qu'il faut retenir : les observations du coeur, deja redigees.
        if (analyse != null && analyse.observations.isNotEmpty()) {
            Bloc("A retenir") {
                analyse.observations.forEach { observation ->
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.Top,
                    ) {
                        Text(
                            text = if (observation.niveau == "good") "\u2713" else "\u2022",
                            color = couleurObservation(observation.niveau),
                            fontSize = 14.sp,
                            fontWeight = FontWeight.Bold,
                        )
                        Text(
                            text = observation.texte,
                            color = couleurObservation(observation.niveau),
                            fontSize = 14.sp,
                        )
                    }
                }
            }
        }

        // 2. La carte : la trace du parcours et un repere par kilometre.
        if (trace.size >= 2) {
            Bloc("Carte") {
                if (carteDisponible()) {
                    val pas = if (analyse != null && analyse.distanceM > 10_000.0) 5000.0 else 1000.0
                    CarteOpenStreetMap(
                        trace = trace,
                        reperes = remember(summary, pas) { reperesDeSeance(summary, pas) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(260.dp),
                    )
                    Text(
                        "Fond de carte (c) OpenStreetMap contributeurs",
                        color = Palette.muted2,
                        fontSize = 10.sp,
                    )
                } else {
                    Text(
                        "Cet appareil n'a pas de moteur de rendu WebView : la trace " +
                            "reste consultable sur openstreetmap.org.",
                        color = Palette.muted2,
                        fontSize = 11.sp,
                    )
                }
                TextButton(onClick = { ouvrirCarteExterne(context, summary) }) {
                    Icon(PhoneIcons.Location, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Ouvrir sur OpenStreetMap")
                }
            }
        }

        if (analyse == null) {
            Text(
                "Analyse indisponible pour cette seance.",
                color = Palette.attention,
                fontSize = 13.sp,
            )
        }

        // 3. Cardio : ce que la montre a enregistre, et ce qu'on en tire.
        analyse?.cardio?.let { cardio ->
            Bloc("Frequence cardiaque") {
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    Chiffre("Moyenne", cardio.moyenne.toInt().toString() + " bpm", Palette.texte)
                    Chiffre("Maximale", cardio.max.toString() + " bpm", Palette.danger)
                    Chiffre("Minimale", cardio.min.toString() + " bpm", Palette.texte)
                }
                if (analyse.courbeCardiaque.size >= 2) {
                    Courbe(
                        points = analyse.courbeCardiaque,
                        couleur = Palette.danger,
                        moyenne = cardio.moyenne,
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(120.dp),
                    )
                    Text(
                        "Pouls en fonction de la distance",
                        color = Palette.muted2,
                        fontSize = 10.sp,
                    )
                }
                BarresZones(cardio)
                analyse.deriveCardiaquePercent?.let { derive ->
                    Ligne(
                        "Derive cardiaque",
                        String.format(Locale.ROOT, "%+.1f %%", derive),
                        couleurDerive(derive),
                    )
                    Text(
                        "Ecart de rendement entre les deux moities. Au-dela de 5 %, " +
                            "la fatigue ou la chaleur se sont fait sentir.",
                        color = Palette.muted2,
                        fontSize = 10.sp,
                    )
                }
            }
        }

        // 4. Energie : ce que le terrain a coute, et ce qu'il a rendu.
        analyse?.let { rapport ->
            if (rapport.denivelePlusM != null || rapport.allureAjusteeSPerKm != null) {
                Bloc("Energie et terrain") {
                    Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                        rapport.denivelePlusM?.let {
                            Chiffre("Denivele +", it.toInt().toString() + " m", Palette.ok)
                        }
                        rapport.deniveleMoinsM?.let {
                            Chiffre("Denivele -", it.toInt().toString() + " m", Palette.muted)
                        }
                        rapport.altitudeMaxM?.let {
                            Chiffre("Altitude max", it.toInt().toString() + " m", Palette.muted)
                        }
                    }
                    rapport.allureAjusteeSPerKm?.let { gap ->
                        Ligne("Allure ajustee (GAP)", MpacerFormat.pace(gap) + " /km")
                    }
                    rapport.distanceEquivalenteM?.let { equivalente ->
                        Ligne(
                            "Distance equivalente a plat",
                            MpacerFormat.distance(equivalente),
                        )
                        Text(
                            "Meme effort sur terrain plat. L'ecart avec l'allure reelle " +
                                "mesure le cout du relief.",
                            color = Palette.muted2,
                            fontSize = 10.sp,
                        )
                    }
                    if (rapport.profilAltitude.size >= 2) {
                        Courbe(
                            points = rapport.profilAltitude,
                            couleur = Palette.ok,
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(90.dp),
                        )
                        Text("Profil altimetrique", color = Palette.muted2, fontSize = 10.sp)
                    }
                }
            }
        }

        // 5. Temps de passage : le coeur de la gestion d'allure.
        if (analyse != null && analyse.tours.isNotEmpty()) {
            Bloc("Temps de passage") {
                val reference = analyse.averagePaceSPerKm
                analyse.tours.forEach { tour ->
                    LigneTour(tour = tour, allureReference = reference)
                }
                Text(
                    "Barre verte : plus rapide que l'allure moyenne. La colonne de " +
                        "droite donne le pouls moyen du tour.",
                    color = Palette.muted2,
                    fontSize = 10.sp,
                )
                analyse.regularite?.let { regularite ->
                    Ligne(
                        "Ecart entre les tours",
                        String.format(
                            Locale.ROOT,
                            "%.0f s/km (%.1f %%)",
                            regularite.ecartTypeAllureS,
                            regularite.ecartTypePercent,
                        ),
                    )
                    Ligne(
                        "Deuxieme moitie",
                        if (regularite.negativeSplitPercent >= 0.0) {
                            String.format(Locale.ROOT, "%.1f %% plus rapide", regularite.negativeSplitPercent)
                        } else {
                            String.format(Locale.ROOT, "%.1f %% plus lente", -regularite.negativeSplitPercent)
                        },
                        if (regularite.negativeSplitPercent >= 0.0) Palette.ok else Palette.attention,
                    )
                    Ligne(
                        "Allure du meilleur tour",
                        MpacerFormat.pace(regularite.allureRapideSPerKm) + " /km",
                    )
                }
                analyse.vitesseMaxMps?.let {
                    Ligne("Vitesse maximale", String.format(Locale.ROOT, "%.1f km/h", it * 3.6))
                }
            }
        }

        // 6. Meilleures distances, telles que le moteur les a relevees.
        val meilleures = summary.optJSONArray("best_efforts")
        if (meilleures != null && meilleures.length() > 0) {
            Bloc("Meilleures distances") {
                for (index in 0 until meilleures.length()) {
                    val effort = meilleures.optJSONObject(index) ?: continue
                    Ligne(effort.optString("label"), MpacerFormat.duration(effort.optDouble("time_s")))
                }
            }
        }

        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            TextButton(onClick = onBack) {
                Icon(PhoneIcons.Back, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(4.dp))
                Text("Retour")
            }
            TextButton(onClick = onShare) {
                Icon(PhoneIcons.Share, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("Partager (.pac)")
            }
            TextButton(onClick = onDelete) {
                Icon(PhoneIcons.Delete, contentDescription = null, tint = Palette.danger, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text("Supprimer", color = Palette.danger)
            }
        }
        Spacer(Modifier.height(8.dp))
    }
}

// ------------------------------------------------------------------ morceaux

/** Bloc titré de la fiche : une carte, un intertitre, un contenu. */
@Composable
private fun Bloc(titre: String, contenu: @Composable ColumnScope.() -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = Palette.surface),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = titre.uppercase(),
                color = Palette.muted2,
                fontSize = 11.sp,
                fontWeight = FontWeight.SemiBold,
            )
            contenu()
        }
    }
}

@Composable
private fun Chiffre(label: String, valeur: String, couleur: Color = Palette.texte) {
    Column {
        Text(label, color = Palette.muted2, fontSize = 10.sp)
        Text(valeur, color = couleur, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
private fun Ligne(gauche: String, droite: String, couleur: Color = Palette.texte) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(gauche, color = Palette.muted, fontSize = 14.sp)
        Text(droite, color = couleur, fontSize = 14.sp, fontWeight = FontWeight.Medium)
    }
}

private fun couleurObservation(niveau: String): Color = when (niveau) {
    "good" -> Palette.ok
    "watch" -> Palette.attention
    else -> Palette.muted
}

private fun couleurDerive(derive: Double): Color = when {
    derive >= 5.0 -> Palette.attention
    derive <= 3.0 -> Palette.ok
    else -> Palette.texte
}

// ------------------------------------------------------------------ actions

private fun date(startedAtMs: Long): String =
    SimpleDateFormat("EEEE d MMMM yyyy, HH:mm", Locale.FRENCH).format(Date(startedAtMs))

/** Partage l'archive complete (.pac) : relecture sur un autre appareil ou le PC. */
private fun partager(context: Context, summary: JSONObject) {
    val contenu = summary.toString(2)
    val intention = Intent(Intent.ACTION_SEND).apply {
        type = "application/json"
        putExtra(Intent.EXTRA_TEXT, contenu)
        putExtra(Intent.EXTRA_SUBJECT, "Seance M-pacer")
    }
    context.startActivity(Intent.createChooser(intention, "Partager la seance"))
}

/**
 * Ouvre la trace sur openstreetmap.org.
 *
 * La carte embarquee suffit a lire le parcours ; ce lien sert a zoomer a
 * l'infini, a changer de fond de carte ou a partager le lieu a quelqu'un.
 */
private fun ouvrirCarteExterne(context: Context, summary: JSONObject) {
    val trace = traceDeSeance(summary)
    if (trace.isEmpty()) return
    val latitude = trace.map { it.lat }.average()
    val longitude = trace.map { it.lon }.average()
    val lien = Uri.parse(
        "https://www.openstreetmap.org/?mlat=" + latitude + "&mlon=" + longitude +
            "#map=14/" + latitude + "/" + longitude
    )
    runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, lien)) }
}
