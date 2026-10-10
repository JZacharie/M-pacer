// Build racine : les plugins sont declares (et non appliques) ici.
// Chaque module choisit ceux dont il a besoin via le catalogue gradle/libs.versions.toml.
plugins {
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.library) apply false
    alias(libs.plugins.kotlin.android) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.kotlin.serialization) apply false
}

// --- Date de compilation -----------------------------------------------------
// Les trois applications affichent leur version ET le jour ou elles ont ete
// compilees, dans l'ecran Reglages : deux APK d'une meme version restent ainsi
// discernables, et un correctif se verifie a l'oeil sur la montre.
//
// Trois sources, dans cet ordre :
//   1. MPACER_BUILD_DATE (variable d'environnement ou propriete Gradle -P) :
//      la chaine de publication y met la date du tag, donc celle de la release ;
//   2. SOURCE_DATE_EPOCH (secondes UNIX), variable normalisee de
//      reproductibilite : un build rejoue a la meme date qu'original ;
//   3. l'horloge de la machine, en **UTC** — comme le fait build.rs cote Rust,
//      pour que poste, CI et image donnent le meme jour.
val dateDeCompilation: String = (findProperty("mpacer.buildDate") as String?)
    ?: System.getenv("MPACER_BUILD_DATE")
    ?: System.getenv("SOURCE_DATE_EPOCH")?.trim()?.takeIf { it.isNotEmpty() }?.let { epoch ->
        java.time.Instant.ofEpochSecond(epoch.toLong())
            .atZone(java.time.ZoneOffset.UTC)
            .toLocalDate()
            .toString()
    }
    ?: java.time.LocalDate.now(java.time.ZoneOffset.UTC).toString()

// Le champ est pose une seule fois ici : chaque module Android ajoute ensuite
// buildConfigField("String", "BUILD_DATE", "\"" + dateDeCompilation + "\"").
extra["mpacerBuildDate"] = dateDeCompilation
