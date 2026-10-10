package com.mpacer.core

import android.content.Context
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import android.os.Build
import java.text.SimpleDateFormat
import java.util.Locale

/**
 * Version de l'application et moment de compilation, pour l'ecran Reglages.
 *
 * Le numero vient du `PackageManager` : c'est le `versionName` de l'APK
 * reellement installe, donc celui que l'utilisateur peut comparer a la
 * publication GitHub. Il n'est pas recopie dans le code, ou il finirait par
 * mentir au premier oubli lors d'une release.
 *
 * L'horodatage, lui, est pose a la compilation par Gradle
 * (`BuildConfig.BUILD_DATE`, voir `android/build.gradle.kts`) : l'APK porte le
 * moment ou il a ete construit — celui du tag lors d'une release — et non le
 * moment ou l'application a ete ouverte. La precision est la minute : deux
 * compilations du meme jour restent discernables.
 *
 * Ce module est partage par les trois applications (`:app`, `:phone`,
 * `:companion`) : le socle ne connait aucun nom d'ecran.
 */
object BuildInfo {

    /** Renseigne une fois pour toutes : le paquet ne change pas en cours de route. */
    @Volatile
    private var resumeMemorise: String? = null

    /** Resume affichable, par exemple « Version 0.2.0 - compile le 10/10/2026 07:42 ». */
    fun resume(context: Context): String {
        resumeMemorise?.let { return it }
        val texte = composer(context.applicationContext)
        resumeMemorise = texte
        return texte
    }

    /** Horodatage de compilation (`AAAA-MM-JJ HH:MM`), tel qu'inscrit dans l'APK. */
    fun dateDeCompilation(): String = BuildConfig.BUILD_DATE

    private fun composer(context: Context): String {
        val paquet = infos(context)
        val nom = paquet?.versionName?.takeIf { it.isNotBlank() } ?: "?"
        val texte = StringBuilder("Version ").append(nom)

        // Les trois applications sont compilees ensemble : le meme horodatage
        // vaut pour toutes, et suffit a distinguer deux compilations.
        dateDeCompilation().takeIf { it.isNotBlank() }?.let { horodatage ->
            texte.append(" - compile le ").append(horodatageLisible(horodatage))
        }

        // Numero de build Android : utile pour comparer deux APK de meme version.
        numeroDeVersion(paquet)?.let { code ->
            texte.append(" (build ").append(code).append(')')
        }
        return texte.toString()
    }

    /**
     * Informations du paquet de l'application qui affiche la version.
     *
     * Le socle ne lit pas son propre paquet : une bibliotheque partagee
     * n'a pas de version propre a l'ecran, ce sont les applications qui en ont une.
     */
    private fun infos(context: Context): PackageInfo? {
        val gestionnaire = context.packageManager ?: return null
        return try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                gestionnaire.getPackageInfo(
                    context.packageName,
                    PackageManager.PackageInfoFlags.of(0L),
                )
            } else {
                @Suppress("DEPRECATION")
                gestionnaire.getPackageInfo(context.packageName, 0)
            }
        } catch (_: Exception) {
            // Paquet illisible (installation en cours, profil de travail...) :
            // l'ecran montre l'horodatage seul plutot que d'echouer.
            null
        }
    }

    /** Numero de build, quelle que soit la version d'Android. */
    private fun numeroDeVersion(paquet: PackageInfo?): Long? {
        paquet ?: return null
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            paquet.longVersionCode
        } else {
            @Suppress("DEPRECATION")
            paquet.versionCode.toLong()
        }
    }

    /**
     * `AAAA-MM-JJ HH:MM` devient `JJ/MM/AAAA HH:MM`. Un APK plus ancien, qui ne
     * porte que le jour, reste lisible ; une valeur ecrite autrement (variable
     * d'environnement mal formee) est rendue sans transformation.
     */
    private fun horodatageLisible(valeur: String): String {
        val propre = valeur.trim()
        val avecHeure = propre.length > 10
        val analyse = SimpleDateFormat(if (avecHeure) "yyyy-MM-dd HH:mm" else "yyyy-MM-dd", Locale.ROOT)
        analyse.isLenient = false
        val affichage = if (avecHeure) "dd/MM/yyyy HH:mm" else "dd/MM/yyyy"
        return try {
            SimpleDateFormat(affichage, Locale.FRANCE).format(analyse.parse(propre)!!)
        } catch (_: Exception) {
            propre
        }
    }
}
