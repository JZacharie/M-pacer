package com.mpacer.watch.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.wear.compose.material.Colors
import androidx.wear.compose.material.Icon
import androidx.wear.compose.material.MaterialTheme
import androidx.wear.compose.material.Text
import com.mpacer.core.ui.Palette

/**
 * Vocabulaire visuel commun aux ecrans de la montre.
 *
 * L'ecran rond impose une contrainte simple : rien de large en bas du cadran,
 * ou les coins sont coupes. D'ou un petit nombre de briques, utilisees partout :
 *
 *  * [RoundButton]     -- une commande est un rond avec une icone, jamais un mot ;
 *  * [StatusPill]      -- un etat est une pastille teintee, une seule couleur ;
 *  * [SectionTitle]    -- un intertitre separe deux familles de reglages ;
 *  * [SettingRow]      -- une ligne par reglage, l'etat porte par la couleur ;
 *  * [SecondaryScreen] -- le contenu defile, le retour ne defile pas.
 *
 * Les icones vivent dans [WatchIcons]. Aucun ecran ne redessine un composant :
 * c'est ce qui garantit que la montre reste coherente d'un ecran a l'autre.
 */

/** Couleurs Wear, alignees sur la palette M-pacer commune au telephone et au site. */
private val WatchColors = Colors(
    primary = Palette.orange,
    primaryVariant = Palette.orangeProfond,
    secondary = Palette.muted,
    secondaryVariant = Palette.muted2,
    background = Palette.encre,
    surface = Palette.surface,
    error = Palette.danger,
    onPrimary = Color.White,
    onSecondary = Palette.encre,
    onBackground = Palette.texte,
    onSurface = Palette.texte,
    onSurfaceVariant = Palette.muted,
    onError = Color.White,
)

/**
 * Theme de la montre. Il ne change pas l'habillage -- chaque ecran pose ses
 * couleurs -- mais il fixe la couleur de contenu par defaut, sans laquelle un
 * `Text` sans couleur explicite s'ecrirait en noir sur l'encre.
 */
@Composable
fun MpacerWatchTheme(content: @Composable () -> Unit) {
    MaterialTheme(colors = WatchColors, content = content)
}

/**
 * Commande ronde : une icone, un appui.
 *
 * Les libelles a rallonge de l'ancien ecran (« Demarrer », « Reglages »)
 * occupaient toute la largeur et se tronquaient des que la taille de police du
 * systeme augmentait. Une icone, elle, ne change pas de largeur : les six
 * commandes de la montre tiennent dans un rond de 40 a 60 dp, sans texte.
 *
 * @param label description lue par le lecteur d'ecran, et non affichee.
 */
@Composable
fun RoundButton(
    icon: ImageVector,
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 44.dp,
    iconSize: Dp = size * 0.5f,
    background: Color = Palette.surface3,
    contentColor: Color = Palette.texte,
    enabled: Boolean = true,
) {
    val interaction = remember { MutableInteractionSource() }
    val presse by interaction.collectIsPressedAsState()
    // Seul retour visuel disponible sans ripple : le rond se retracte sous le doigt.
    val echelle by animateFloatAsState(
        targetValue = if (presse) 0.88f else 1f,
        label = "appui",
    )
    Box(
        modifier = modifier
            .size(size)
            .scale(echelle)
            .clip(CircleShape)
            .background(if (enabled) background else background.copy(alpha = 0.35f))
            .clickable(
                interactionSource = interaction,
                indication = null,
                enabled = enabled,
                onClickLabel = label,
                role = Role.Button,
                onClick = onClick,
            ),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            modifier = Modifier.size(iconSize),
            tint = if (enabled) contentColor else contentColor.copy(alpha = 0.35f),
        )
    }
}

/**
 * Pastille d'etat : un texte court sur un fond de sa propre couleur, diluee.
 * Elle porte une information (sur le plan, en avance, en retard), jamais une
 * decoration ; l'icone est optionnelle.
 */
@Composable
fun StatusPill(
    text: String,
    color: Color,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    fontSize: TextUnit = 12.sp,
) {
    Row(
        modifier = modifier
            .clip(CircleShape)
            .background(Palette.pastille(color))
            .padding(horizontal = 12.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, modifier = Modifier.size(13.dp), tint = color)
        }
        Text(
            text = text,
            color = color,
            fontSize = fontSize,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
        )
    }
}

/** Nom de l'ecran secondaire : une ligne, en haut du cadran. */
@Composable
fun ScreenTitle(text: String, modifier: Modifier = Modifier) {
    Text(
        text = text,
        modifier = modifier,
        color = Palette.texte,
        fontSize = 14.sp,
        fontWeight = FontWeight.SemiBold,
        maxLines = 1,
    )
}

/** Intertitre : court, en majuscules espacees, il separe deux blocs de reglages. */
@Composable
fun SectionTitle(text: String, modifier: Modifier = Modifier) {
    Text(
        text = text.uppercase(),
        modifier = modifier,
        color = Palette.muted2,
        fontSize = 10.sp,
        fontWeight = FontWeight.SemiBold,
        letterSpacing = 1.5.sp,
        maxLines = 1,
    )
}

/**
 * Ligne de reglage ou d'action : un libelle, une icone optionnelle, et l'etat
 * porte par la couleur -- orange quand l'option est active, surface neutre
 * sinon. Une seule ligne par reglage : l'ecran tient sans defiler.
 */
@Composable
fun SettingRow(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    selected: Boolean = false,
    enabled: Boolean = true,
) {
    val interaction = remember { MutableInteractionSource() }
    val presse by interaction.collectIsPressedAsState()
    val fond = when {
        !enabled -> Palette.surface
        selected -> if (presse) Palette.orangeProfond else Palette.orange
        presse -> Palette.surface3
        else -> Palette.surface2
    }
    val contenu = when {
        !enabled -> Palette.muted2
        selected -> Color.White
        else -> Palette.texte
    }
    Row(
        modifier = modifier
            .clip(CircleShape)
            .background(fond)
            .clickable(
                interactionSource = interaction,
                indication = null,
                enabled = enabled,
                onClickLabel = label,
                role = Role.Button,
                onClick = onClick,
            )
            .padding(horizontal = 14.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (icon != null) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                modifier = Modifier.size(16.dp),
                tint = contenu,
            )
        }
        Text(
            text = label,
            color = contenu,
            fontSize = 12.sp,
            fontWeight = FontWeight.Medium,
            maxLines = 1,
        )
    }
}

/** Retour : un rond, toujours au meme endroit, en bas du cadran. */
@Composable
fun BackButton(onClick: () -> Unit, modifier: Modifier = Modifier) {
    RoundButton(
        icon = WatchIcons.Back,
        label = "Retour",
        onClick = onClick,
        modifier = modifier,
        size = 40.dp,
        iconSize = 20.dp,
        background = Palette.surface2,
        contentColor = Palette.muted,
    )
}

/** Fond commun : l'encre, sous tous les ecrans. */
@Composable
fun WatchSurface(content: @Composable BoxScope.() -> Unit) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Palette.encre),
        content = content,
    )
}

/**
 * Ecran secondaire (reglages, synchronisation, musique) : le contenu defile,
 * le retour ne defile pas. Sur un cadran rond, une commande qu'on doit aller
 * chercher en fin de liste est une commande qu'on ne trouve pas.
 *
 * Le contenu doit reserver la hauteur du bouton : `bottom = 58.dp`.
 */
@Composable
fun SecondaryScreen(
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    WatchSurface {
        Column(
            modifier = modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(start = 12.dp, end = 12.dp, top = 10.dp, bottom = 58.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(6.dp),
            content = content,
        )
        // Le contenu defile sous le bouton : un degrade de l'encre le fait
        // disparaitre avant, plutot que de laisser une ligne passer dessous.
        Box(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .background(
                    Brush.verticalGradient(
                        colors = listOf(Color.Transparent, Palette.encre, Palette.encre),
                    )
                )
                .padding(top = 12.dp, bottom = 8.dp),
            contentAlignment = Alignment.Center,
        ) {
            BackButton(onClick = onBack)
        }
    }
}
