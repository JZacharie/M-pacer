package com.mpacer.watch.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathBuilder
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp

/**
 * Jeu d'icones de la montre M-pacer.
 *
 * Les tracés viennent des icones Material (Apache 2.0), graves ici en vectoriel
 * plutot que tires de material-icons-extended : la montre n'embarque ainsi que
 * les symboles qu'elle affiche vraiment, et le poids visuel reste homogene
 * (meme grille de 24 x 24, meme remplissage plein). Le trace est blanc et c'est
 * Icon qui applique la teinte voulue : un seul jeu, toutes les couleurs.
 *
 * Deux formes sont propres a M-pacer : la pause, aux barres elargies pour
 * rester lisible a 20 dp, et le carre d'arret, aux coins arrondis.
 */
object WatchIcons {

    /** Play */
    val Play: ImageVector = icone("Play", PathFillType.NonZero) {
        moveTo(8f, 5f)
        verticalLineToRelative(14f)
        lineToRelative(11f, -7f)
        close()
    }

    /** Pause */
    val Pause: ImageVector = icone("Pause", PathFillType.NonZero) {
        moveTo(6f, 5f)
        horizontalLineToRelative(4.6f)
        verticalLineToRelative(14f)
        horizontalLineTo(6f)
        close()
        moveTo(13.4f, 5f)
        horizontalLineTo(18f)
        verticalLineToRelative(14f)
        horizontalLineToRelative(-4.6f)
        close()
    }

    /** Stop */
    val Stop: ImageVector = icone("Stop", PathFillType.NonZero) {
        moveTo(8.4f, 5.4f)
        horizontalLineToRelative(7.2f)
        arcToRelative(3f, 3f, 0f, false, true, 3f, 3f)
        verticalLineToRelative(7.2f)
        arcToRelative(3f, 3f, 0f, false, true, -3f, 3f)
        horizontalLineToRelative(-7.2f)
        arcToRelative(3f, 3f, 0f, false, true, -3f, -3f)
        verticalLineToRelative(-7.2f)
        arcToRelative(3f, 3f, 0f, false, true, 3f, -3f)
        close()
    }

    /** Music */
    val Music: ImageVector = icone("Music", PathFillType.NonZero) {
        moveTo(12f, 3f)
        verticalLineToRelative(10.55f)
        curveToRelative(-0.59f, -0.34f, -1.27f, -0.55f, -2f, -0.55f)
        curveToRelative(-2.21f, 0f, -4f, 1.79f, -4f, 4f)
        reflectiveCurveToRelative(1.79f, 4f, 4f, 4f)
        reflectiveCurveToRelative(4f, -1.79f, 4f, -4f)
        verticalLineTo(7f)
        horizontalLineToRelative(4f)
        verticalLineTo(3f)
        horizontalLineToRelative(-6f)
        close()
    }

    /** Settings */
    val Settings: ImageVector = icone("Settings", PathFillType.EvenOdd) {
        moveTo(19.14f, 12.94f)
        curveToRelative(0.04f, -0.3f, 0.06f, -0.61f, 0.06f, -0.94f)
        curveToRelative(0f, -0.32f, -0.02f, -0.64f, -0.07f, -0.94f)
        lineToRelative(2.03f, -1.58f)
        curveToRelative(0.18f, -0.14f, 0.23f, -0.41f, 0.12f, -0.61f)
        lineToRelative(-1.92f, -3.32f)
        curveToRelative(-0.12f, -0.22f, -0.37f, -0.29f, -0.59f, -0.22f)
        lineToRelative(-2.39f, 0.96f)
        curveToRelative(-0.5f, -0.38f, -1.03f, -0.7f, -1.62f, -0.94f)
        lineToRelative(-0.36f, -2.54f)
        curveToRelative(-0.04f, -0.24f, -0.24f, -0.41f, -0.48f, -0.41f)
        horizontalLineToRelative(-3.84f)
        curveToRelative(-0.24f, 0f, -0.43f, 0.17f, -0.47f, 0.41f)
        lineToRelative(-0.36f, 2.54f)
        curveToRelative(-0.59f, 0.24f, -1.13f, 0.57f, -1.62f, 0.94f)
        lineToRelative(-2.39f, -0.96f)
        curveToRelative(-0.22f, -0.08f, -0.47f, 0f, -0.59f, 0.22f)
        lineTo(2.74f, 8.87f)
        curveToRelative(-0.12f, 0.21f, -0.08f, 0.47f, 0.12f, 0.61f)
        lineToRelative(2.03f, 1.58f)
        curveToRelative(-0.05f, 0.3f, -0.09f, 0.63f, -0.09f, 0.94f)
        reflectiveCurveToRelative(0.02f, 0.64f, 0.07f, 0.94f)
        lineToRelative(-2.03f, 1.58f)
        curveToRelative(-0.18f, 0.14f, -0.23f, 0.41f, -0.12f, 0.61f)
        lineToRelative(1.92f, 3.32f)
        curveToRelative(0.12f, 0.22f, 0.37f, 0.29f, 0.59f, 0.22f)
        lineToRelative(2.39f, -0.96f)
        curveToRelative(0.5f, 0.38f, 1.03f, 0.7f, 1.62f, 0.94f)
        lineToRelative(0.36f, 2.54f)
        curveToRelative(0.05f, 0.24f, 0.24f, 0.41f, 0.48f, 0.41f)
        horizontalLineToRelative(3.84f)
        curveToRelative(0.24f, 0f, 0.44f, -0.17f, 0.47f, -0.41f)
        lineToRelative(0.36f, -2.54f)
        curveToRelative(0.59f, -0.24f, 1.13f, -0.56f, 1.62f, -0.94f)
        lineToRelative(2.39f, 0.96f)
        curveToRelative(0.22f, 0.08f, 0.47f, 0f, 0.59f, -0.22f)
        lineToRelative(1.92f, -3.32f)
        curveToRelative(0.12f, -0.22f, 0.07f, -0.47f, -0.12f, -0.61f)
        lineToRelative(-2.01f, -1.58f)
        close()
        moveTo(12f, 15.6f)
        curveToRelative(-1.98f, 0f, -3.6f, -1.62f, -3.6f, -3.6f)
        reflectiveCurveToRelative(1.62f, -3.6f, 3.6f, -3.6f)
        reflectiveCurveToRelative(3.6f, 1.62f, 3.6f, 3.6f)
        reflectiveCurveToRelative(-1.62f, 3.6f, -3.6f, 3.6f)
        close()
    }

    /** Sync */
    val Sync: ImageVector = icone("Sync", PathFillType.NonZero) {
        moveTo(17.65f, 6.35f)
        curveTo(16.2f, 4.9f, 14.21f, 4f, 12f, 4f)
        curveToRelative(-4.42f, 0f, -7.99f, 3.58f, -8f, 8f)
        reflectiveCurveToRelative(3.58f, 8f, 8f, 8f)
        curveToRelative(3.73f, 0f, 6.84f, -2.55f, 7.73f, -6f)
        horizontalLineToRelative(-2.08f)
        curveToRelative(-0.82f, 2.33f, -3.04f, 4f, -5.65f, 4f)
        curveToRelative(-3.31f, 0f, -6f, -2.69f, -6f, -6f)
        reflectiveCurveToRelative(2.69f, -6f, 6f, -6f)
        curveToRelative(1.66f, 0f, 3.14f, 0.69f, 4.22f, 1.78f)
        lineTo(13f, 11f)
        horizontalLineToRelative(7f)
        verticalLineTo(4f)
        lineToRelative(-2.35f, 2.35f)
        close()
    }

    /** Shuffle : lecture melangee. */
    val Shuffle: ImageVector = icone("Shuffle", PathFillType.NonZero) {
        moveTo(10.59f, 9.17f)
        lineTo(5.41f, 4f)
        lineTo(4f, 5.41f)
        lineTo(9.17f, 10.59f)
        close()
        moveTo(14.5f, 4f)
        lineTo(16.54f, 6.04f)
        lineTo(3f, 19.59f)
        lineTo(4.41f, 21f)
        lineTo(18f, 7.41f)
        verticalLineTo(9.5f)
        horizontalLineTo(20f)
        verticalLineTo(4f)
        close()
        moveTo(14.83f, 13.41f)
        lineTo(13.42f, 14.82f)
        lineTo(16.54f, 17.94f)
        lineTo(14.5f, 20f)
        horizontalLineTo(20f)
        verticalLineTo(14.5f)
        lineTo(18f, 16.54f)
        close()
    }

    /** Back */
    val Back: ImageVector = icone("Back", PathFillType.NonZero) {
        moveTo(20f, 11f)
        horizontalLineTo(7.83f)
        lineToRelative(5.59f, -5.59f)
        lineTo(12f, 4f)
        lineToRelative(-8f, 8f)
        lineToRelative(8f, 8f)
        lineToRelative(1.41f, -1.41f)
        lineTo(7.83f, 13f)
        horizontalLineTo(20f)
        verticalLineToRelative(-2f)
        close()
    }

    /** Check */
    val Check: ImageVector = icone("Check", PathFillType.NonZero) {
        moveTo(9f, 16.17f)
        lineTo(4.83f, 12f)
        lineToRelative(-1.42f, 1.41f)
        lineTo(9f, 19f)
        lineTo(21f, 7f)
        lineToRelative(-1.41f, -1.41f)
        close()
    }

    /** Close */
    val Close: ImageVector = icone("Close", PathFillType.NonZero) {
        moveTo(19f, 6.41f)
        lineTo(17.59f, 5f)
        lineTo(12f, 10.59f)
        lineTo(6.41f, 5f)
        lineTo(5f, 6.41f)
        lineTo(10.59f, 12f)
        lineTo(5f, 17.59f)
        lineTo(6.41f, 19f)
        lineTo(12f, 13.41f)
        lineTo(17.59f, 19f)
        lineTo(19f, 17.59f)
        lineTo(13.41f, 12f)
        close()
    }

    /** Delete */
    val Delete: ImageVector = icone("Delete", PathFillType.NonZero) {
        moveTo(6f, 19f)
        curveToRelative(0f, 1.1f, 0.9f, 2f, 2f, 2f)
        horizontalLineToRelative(8f)
        curveToRelative(1.1f, 0f, 2f, -0.9f, 2f, -2f)
        verticalLineTo(7f)
        horizontalLineTo(6f)
        verticalLineToRelative(12f)
        close()
        moveTo(19f, 4f)
        horizontalLineToRelative(-3.5f)
        lineToRelative(-1f, -1f)
        horizontalLineToRelative(-5f)
        lineToRelative(-1f, 1f)
        horizontalLineTo(5f)
        verticalLineToRelative(2f)
        horizontalLineToRelative(14f)
        verticalLineTo(4f)
        close()
    }

    /** Next */
    val Next: ImageVector = icone("Next", PathFillType.NonZero) {
        moveTo(8.59f, 16.59f)
        lineTo(13.17f, 12f)
        lineTo(8.59f, 7.41f)
        lineTo(10f, 6f)
        lineToRelative(6f, 6f)
        lineToRelative(-6f, 6f)
        lineToRelative(-1.41f, -1.41f)
        close()
    }

    /** Previous */
    val Previous: ImageVector = icone("Previous", PathFillType.NonZero) {
        moveTo(15.41f, 16.59f)
        lineTo(10.83f, 12f)
        lineToRelative(4.58f, -4.59f)
        lineTo(14f, 6f)
        lineToRelative(-6f, 6f)
        lineToRelative(6f, 6f)
        lineToRelative(1.41f, -1.41f)
        close()
    }

    /** Plus : volume en hausse, ajout. */
    val Plus: ImageVector = icone("Plus", PathFillType.NonZero) {
        moveTo(19f, 13f)
        horizontalLineToRelative(-6f)
        verticalLineToRelative(6f)
        horizontalLineToRelative(-2f)
        verticalLineToRelative(-6f)
        horizontalLineTo(5f)
        verticalLineToRelative(-2f)
        horizontalLineToRelative(6f)
        verticalLineTo(5f)
        horizontalLineToRelative(2f)
        verticalLineToRelative(6f)
        horizontalLineToRelative(6f)
        close()
    }

    /** Minus : volume en baisse, retrait. */
    val Minus: ImageVector = icone("Minus", PathFillType.NonZero) {
        moveTo(5f, 11f)
        horizontalLineToRelative(14f)
        verticalLineToRelative(2f)
        horizontalLineTo(5f)
        close()
    }

    /** Location */
    val Location: ImageVector = icone("Location", PathFillType.NonZero) {
        moveTo(12f, 2f)
        curveTo(8.13f, 2f, 5f, 5.13f, 5f, 9f)
        curveToRelative(0f, 5.25f, 7f, 13f, 7f, 13f)
        reflectiveCurveToRelative(7f, -7.75f, 7f, -13f)
        curveToRelative(0f, -3.87f, -3.13f, -7f, -7f, -7f)
        close()
        moveTo(12f, 11.5f)
        curveToRelative(-1.38f, 0f, -2.5f, -1.12f, -2.5f, -2.5f)
        reflectiveCurveToRelative(1.12f, -2.5f, 2.5f, -2.5f)
        reflectiveCurveToRelative(2.5f, 1.12f, 2.5f, 2.5f)
        reflectiveCurveToRelative(-1.12f, 2.5f, -2.5f, 2.5f)
        close()
    }

    /** Flag */
    val Flag: ImageVector = icone("Flag", PathFillType.NonZero) {
        moveTo(14.4f, 6f)
        lineTo(14f, 4f)
        horizontalLineTo(5f)
        verticalLineToRelative(17f)
        horizontalLineToRelative(2f)
        verticalLineToRelative(-7f)
        horizontalLineToRelative(5.6f)
        lineToRelative(0.4f, 2f)
        horizontalLineToRelative(7f)
        verticalLineTo(6f)
        close()
    }

    /** Person */
    val Person: ImageVector = icone("Person", PathFillType.NonZero) {
        moveTo(12f, 12f)
        curveToRelative(2.21f, 0f, 4f, -1.79f, 4f, -4f)
        reflectiveCurveToRelative(-1.79f, -4f, -4f, -4f)
        reflectiveCurveToRelative(-4f, 1.79f, -4f, 4f)
        reflectiveCurveToRelative(1.79f, 4f, 4f, 4f)
        close()
        moveTo(12f, 14f)
        curveToRelative(-2.67f, 0f, -8f, 1.34f, -8f, 4f)
        verticalLineToRelative(2f)
        horizontalLineToRelative(16f)
        verticalLineToRelative(-2f)
        curveToRelative(0f, -2.66f, -5.33f, -4f, -8f, -4f)
        close()
    }

    /** Sound */
    val Sound: ImageVector = icone("Sound", PathFillType.NonZero) {
        moveTo(3f, 9f)
        verticalLineToRelative(6f)
        horizontalLineToRelative(4f)
        lineToRelative(5f, 5f)
        verticalLineTo(4f)
        lineTo(7f, 9f)
        horizontalLineTo(3f)
        close()
        moveTo(16.5f, 12f)
        curveToRelative(0f, -1.77f, -1.02f, -3.29f, -2.5f, -4.03f)
        verticalLineToRelative(8.05f)
        curveToRelative(1.48f, -0.73f, 2.5f, -2.25f, 2.5f, -4.02f)
        close()
        moveTo(14f, 3.23f)
        verticalLineToRelative(2.06f)
        curveToRelative(2.89f, 0.86f, 5f, 3.54f, 5f, 6.71f)
        reflectiveCurveToRelative(-2.11f, 5.85f, -5f, 6.71f)
        verticalLineToRelative(2.06f)
        curveToRelative(4.01f, -0.91f, 7f, -4.49f, 7f, -8.77f)
        reflectiveCurveToRelative(-2.99f, -7.86f, -7f, -8.77f)
        close()
    }

    /**
     * Fabrique commune : grille de 24 x 24, un seul sous-chemin plein.
     * Le remplissage pair-impair n'est necessaire que pour l'engrenage, dont le
     * moyeu est un trou : les autres icones se contentent du remplissage normal.
     */
    private fun icone(
        nom: String,
        remplissage: PathFillType,
        trace: PathBuilder.() -> Unit,
    ): ImageVector = ImageVector.Builder(
        name = nom,
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).path(
        fill = SolidColor(Color.White),
        pathFillType = remplissage,
        pathBuilder = trace,
    ).build()
}
