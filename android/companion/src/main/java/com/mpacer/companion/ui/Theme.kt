package com.mpacer.companion.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** Palette sombre commune, alignee sur les couleurs d etat du coeur (feu GPS). */
private val MpacerColorScheme = darkColorScheme(
    primary = Color(0xFF2ECC71),
    onPrimary = Color(0xFF07230F),
    secondary = Color(0xFF7F8C8D),
    background = Color(0xFF101418),
    surface = Color(0xFF171C21),
    onBackground = Color(0xFFECF0F1),
    onSurface = Color(0xFFECF0F1),
)

@Composable
fun MpacerTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = MpacerColorScheme, content = content)
}
