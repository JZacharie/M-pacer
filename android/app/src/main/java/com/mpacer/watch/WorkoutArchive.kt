package com.mpacer.watch

import android.content.Context
import org.json.JSONObject
import java.io.File

/**
 * Historique local des seances.
 *
 * Une seance = un fichier JSON (format `.pac` versionne, produit par le coeur).
 * Aucun envoi reseau : la seance appartient a l'utilisateur.
 */
object WorkoutArchive {

    private const val DIRECTORY = "workouts"

    fun save(context: Context, summary: JSONObject) {
        val directory = File(context.filesDir, DIRECTORY).apply { mkdirs() }
        val startedAt = summary.optLong("started_at_ms", System.currentTimeMillis())
        File(directory, "workout-$startedAt.json").writeText(summary.toString())
    }

    fun list(context: Context): List<JSONObject> {
        val directory = File(context.filesDir, DIRECTORY)
        if (!directory.exists()) return emptyList()
        return directory.listFiles()
            ?.sortedByDescending { it.name }
            ?.mapNotNull { file -> runCatching { JSONObject(file.readText()) }.getOrNull() }
            ?: emptyList()
    }

    /** Contenu exportable (import/export entre montres, comme le fichier .pac d'origine). */
    fun exportAll(context: Context): String {
        val workouts = list(context)
        val root = JSONObject()
            .put("format", "mpacer.pac")
            .put("version", 1)
            .put("workouts", org.json.JSONArray(workouts))
        return root.toString(2)
    }
}
