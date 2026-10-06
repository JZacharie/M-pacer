package com.mpacer.phone.ui

import android.content.Context
import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.mpacer.core.MpacerFormat
import com.mpacer.core.WorkoutArchive
import com.mpacer.core.ui.Palette
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Historique local du telephone : les seances enregistrees par le socle, telles
 * que le moteur les a produites (format .pac). Aucun recalcul : les totaux, tours
 * et meilleures distances sont ceux du resume.
 *
 * Le fichier peut etre partage (export .pac) ou supprime ; l'envoi vers le
 * backend se fait depuis l'onglet Synchronisation.
 */
@Composable
fun HistoryScreen() {
    val context = LocalContext.current
    var seances by remember { mutableStateOf(WorkoutArchive.list(context)) }
    var ouverte by remember { mutableStateOf<JSONObject?>(null) }

    val detail = ouverte
    if (detail != null) {
        WorkoutDetail(
            summary = detail,
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
        if (seances.isEmpty()) {
            Text(
                "Aucune seance : lancez une course depuis l'onglet Course.",
                color = Palette.muted,
            )
            return@Column
        }
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(seances) { seance ->
                CarteSeance(seance) { ouverte = seance }
            }
        }
    }
}

@Composable
private fun CarteSeance(summary: JSONObject, onOpen: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = Palette.surface),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 14.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(date(summary.optLong("started_at_ms")), color = Palette.texte, fontSize = 15.sp)
                Text(
                    MpacerFormat.distance(summary.optDouble("distance_m")) + "  " +
                        MpacerFormat.duration(summary.optDouble("duration_s")) + "  " +
                        MpacerFormat.pace(summary.optDouble("average_pace_s_per_km")) + " /km",
                    color = Palette.muted,
                    fontSize = 13.sp,
                )
            }
            TextButton(onClick = onOpen) { Text("Ouvrir") }
        }
    }
}

@Composable
private fun WorkoutDetail(
    summary: JSONObject,
    onBack: () -> Unit,
    onDelete: () -> Unit,
    onShare: () -> Unit,
) {
    val tours = summary.optJSONArray("laps")
    val meilleures = summary.optJSONArray("best_efforts")
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(date(summary.optLong("started_at_ms")), fontSize = 20.sp, fontWeight = FontWeight.SemiBold, color = Palette.texte)
        Text(
            MpacerFormat.distance(summary.optDouble("distance_m")) + "  " +
                MpacerFormat.duration(summary.optDouble("duration_s")) + "  " +
                MpacerFormat.pace(summary.optDouble("average_pace_s_per_km")) + " /km",
            color = Palette.muted,
        )

        if (tours != null && tours.length() > 0) {
            Text("Tours", color = Palette.texte, fontWeight = FontWeight.Medium)
            LazyColumn(modifier = Modifier.weight(1f, fill = false)) {
                items((0 until tours.length()).toList()) { index ->
                    val tour = tours.optJSONObject(index) ?: return@items
                    Ligne(
                        gauche = "Tour " + tour.optInt("index", index + 1),
                        droite = MpacerFormat.distance(tour.optDouble("distance_m")) + "  " +
                            MpacerFormat.duration(tour.optDouble("duration_s")) + "  " +
                            MpacerFormat.pace(tour.optDouble("pace_s_per_km")),
                    )
                }
            }
        }

        if (meilleures != null && meilleures.length() > 0) {
            Text("Meilleures distances", color = Palette.texte, fontWeight = FontWeight.Medium)
            for (index in 0 until meilleures.length()) {
                val effort = meilleures.optJSONObject(index) ?: continue
                Ligne(
                    gauche = effort.optString("label"),
                    droite = MpacerFormat.duration(effort.optDouble("time_s")),
                )
            }
        }

        Spacer(Modifier.height(4.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            TextButton(onClick = onBack) { Text("Retour") }
            TextButton(onClick = onShare) { Text("Partager (.pac)") }
            TextButton(onClick = onDelete) { Text("Supprimer", color = Palette.danger) }
        }
    }
}

@Composable
private fun Ligne(gauche: String, droite: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text(gauche, color = Palette.muted, fontSize = 14.sp)
        Text(droite, color = Palette.texte, fontSize = 14.sp)
    }
}

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
