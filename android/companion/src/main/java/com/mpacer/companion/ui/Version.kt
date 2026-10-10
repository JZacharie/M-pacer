package com.mpacer.companion.ui

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import com.mpacer.companion.BuildConfig

/**
 * Version de l'application d'appoint et jour de compilation.
 *
 * L'application d'appoint n'embarque pas le socle `:core` (elle ne fait
 * qu'appeler l'API) : elle lit donc sa propre version dans le
 * `PackageManager` et sa date dans le champ de compilation de Gradle, avec le
 * meme affichage que la montre et le telephone.
 */
internal fun versionAffichee(context: Context): String =
    resumeVersion(nomInstalle(context), BuildConfig.BUILD_DATE)

/**
 * Composition du resume, par exemple « Version 0.2.0 - compile le 10/10/2026 ».
 *
 * Separee de la lecture du paquet : c'est la seule partie qui merite un test,
 * et elle n'a besoin ni d'Android ni d'un gestionnaire de paquets.
 */
internal fun resumeVersion(nom: String, jourIso: String): String {
    val nomMontre = nom.trim().ifBlank { "?" }
    val jour = jourLisible(jourIso)
    return if (jour.isBlank()) "Version $nomMontre" else "Version $nomMontre - compile le $jour"
}

/**
 * `AAAA-MM-JJ` devient `JJ/MM/AAAA` (comme les dates de l'application) ; une
 * valeur ecrite autrement — variable d'environnement mal formee — est rendue
 * sans transformation plutot que de faire disparaitre la date.
 */
internal fun jourLisible(jour: String): String {
    val propre = jour.trim()
    if (propre.isEmpty()) return ""
    val analyse = java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.ROOT)
    analyse.isLenient = false
    val minute = try {
        analyse.parse(propre)?.time ?: return propre
    } catch (_: Exception) {
        return propre
    }
    return Format.date(minute).substringBefore(' ')
}

/** Nom de version de l'APK installe, ou « ? » si le paquet est illisible. */
private fun nomInstalle(context: Context): String {
    val gestionnaire = context.packageManager ?: return "?"
    return try {
        val paquet = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            gestionnaire.getPackageInfo(
                context.packageName,
                PackageManager.PackageInfoFlags.of(0L),
            )
        } else {
            @Suppress("DEPRECATION")
            gestionnaire.getPackageInfo(context.packageName, 0)
        }
        paquet?.versionName ?: "?"
    } catch (_: Exception) {
        // Installation en cours, profil de travail... : la date reste juste.
        "?"
    }
}
