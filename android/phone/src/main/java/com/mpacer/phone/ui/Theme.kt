package com.mpacer.phone.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import com.mpacer.core.ui.Palette

/**
 * Theme du telephone : Material 3 sur la palette commune, celle de la montre et
 * des pages d'analyse du backend. Un seul accent fort (l'orange), une encre
 * profonde, aucune teinte parasite.
 */
private val MpacerColorScheme = darkColorScheme(
    primary = Palette.orange,
    onPrimary = Color.White,
    primaryContainer = Palette.orangeProfond,
    onPrimaryContainer = Palette.texte,
    secondary = Palette.orange,
    onSecondary = Color.White,
    secondaryContainer = Color(0x26FC4C02),
    onSecondaryContainer = Palette.orange,
    background = Palette.encre,
    onBackground = Palette.texte,
    surface = Palette.surface,
    onSurface = Palette.texte,
    surfaceVariant = Palette.surface2,
    onSurfaceVariant = Palette.muted,
    outline = Color(0x24FFFFFF),
    outlineVariant = Color(0x14FFFFFF),
    error = Palette.danger,
    onError = Color.White,
)

@Composable
fun MpacerTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = MpacerColorScheme, content = content)
}
