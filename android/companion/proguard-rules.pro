# Regles R8 (minification desactivee par defaut : voir build.gradle.kts).
# A completer le jour ou isMinifyEnabled passe a true.
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.**
-keepclassmembers class com.mpacer.companion.** {
    *** Companion;
}
-keepclasseswithmembers class com.mpacer.companion.** {
    kotlinx.serialization.KSerializer serializer(...);
}
