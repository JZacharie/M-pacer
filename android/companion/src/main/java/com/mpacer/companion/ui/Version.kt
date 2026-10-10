package com.mpacer.companion.ui

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import com.mpacer.companion.BuildConfig

/**
 * Version de l'application d'appoint et moment de compilation.
 *
 * L'application d'appoint n'embarque pas le socle `:core` (elle ne fait
 * qu'appeler l'API) : elle lit donc sa propre version dans le
 * `PackageManager` et son horodatage dans le champ de compilation de Gradle,
 * avec le meme affichage que la montre et le telephone.
 */
internal fun versionAffichee(context: Context): String =
    resumeVersion(nomInstalle(context), BuildConfig.BUILD_DATE)

/**
 * Composition du resume, par exemple « Version 0.2.0 - compile le 10/10/2026 07:42 ».
 *
 * Separee de la lecture du paquet : c'est la seule partie qui merite un test,
 * et elle n'a besoin ni d'Android ni d'un gestionnaire de paquets.
 */
internal fun resumeVersion(nom: String, horodatageIso: String): String {
    val nomMontre = nom.trim().ifBlank { "?" }
    val horodatage = horodatageLisible(horodatageIso)
    return if (horodatage.isBlank()) {
        "Version $nomMontre"
    } else {
        "Version $nomMontre - compile le $horodatage"
    }
}

/**
 * `AAAA-MM-JJ HH:MM` devient `JJ/MM/AAAA HH:MM` (comme les dates de
 * l'application) ; une valeur plus ancienne qui ne porte que le jour reste
 * lisible, et une valeur ecrite autrement — variable d'environnement mal
 * formee — est rendue sans transformation plutot que de disparaitre.
 */
internal fun horodatageLisible(valeur: String): String {
    val propre = valeur.trim()
    if (propre.isEmpty()) return ""
    val avecHeure = propre.length > 10
    val analyse = java.text.SimpleDateFormat(
        if (avecHeure) "yyyy-MM-dd HH:mm" else "yyyy-MM-dd",
        java.util.Locale.ROOT,
    )
    analyse.isLenient = false
    val instant = try {
        analyse.parse(propre)?.time ?: return propre
    } catch (_: Exception) {
        return propre
    }
    val affichage = Format.date(instant)
    return if (avecHeure) affichage else affichage.substringBefore(' ')
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
        // Installation en cours, profil de travail... : l'horodatage reste juste.
        "?"
    }
}
