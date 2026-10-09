package com.mpacer.phone.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.dp

/**
 * Jeu d'icones de l'application telephone M-pacer.
 *
 * Meme parti pris que sur la montre : les traces Material (Apache 2.0) sont
 * gravees ici, dans le module qui les affiche, plutot que tirees de
 * `material-icons-extended`. L'application release n'active pas R8 : embarquer
 * le catalogue complet ajouterait plusieurs mega-octets a l'APK pour une
 * vingtaine de symboles reellement affiches. Ici, chaque icone separee est
 * inutile : rien de plus que ce que l'interface utilise.
 *
 * Le trace est blanc et c'est `Icon` qui applique la teinte voulue : un seul
 * jeu, toutes les couleurs -- meme principe que `WatchIcons` cote montre. Les
 * commandes du lecteur de musique (lecture, pistes, volume) vivent ici, avec
 * celles de la navigation et de l'ecran de course.
 *
 * Les chemins sont ecrits en SVG et analyses par [PathParser] : lisible, et
 * verifiable contre la source Material.
 */
object PhoneIcons {

    // ------------------------------------------------------------- navigation

    /** Course : la silhouette qui court. */
    val Run: ImageVector = icone(
        "Run",
        "M13.49,5.48 C14.59,5.48 15.49,4.58 15.49,3.48 C15.49,2.38 14.59,1.48 13.49,1.48 " +
            "C12.39,1.48 11.49,2.38 11.49,3.48 C11.49,4.58 12.39,5.48 13.49,5.48 Z " +
            "M9.89,19.38 L10.89,14.98 L12.99,16.98 L12.99,22.98 L14.99,22.98 L14.99,15.48 " +
            "L12.89,13.48 L13.49,10.48 C14.79,11.98 16.79,12.98 18.99,12.98 L18.99,10.98 " +
            "C17.09,10.98 15.49,9.98 14.69,8.58 L13.69,6.98 C13.29,6.38 12.69,5.98 11.99,5.98 " +
            "C11.69,5.98 11.49,6.08 11.19,6.08 L5.99,8.28 L5.99,12.98 L7.99,12.98 L7.99,9.58 " +
            "L9.79,8.88 L8.19,16.98 L3.29,15.98 L2.89,17.98 Z",
        PathFillType.NonZero,
    )

    /** Amis : les deux silhouettes du groupe. */
    val Group: ImageVector = icone(
        "Group",
        "M16,11 C17.66,11 18.99,9.66 18.99,8 C18.99,6.34 17.66,5 16,5 " +
            "C14.34,5 13,6.34 13,8 C13,9.66 14.34,11 16,11 Z " +
            "M8,11 C9.66,11 10.99,9.66 10.99,8 C10.99,6.34 9.66,5 8,5 " +
            "C6.34,5 5,6.34 5,8 C5,9.66 6.34,11 8,11 Z " +
            "M8,13 C5.67,13 1,14.17 1,16.5 L1,19 L15,19 L15,16.5 C15,14.17 10.33,13 8,13 Z " +
            "M16,13 C15.71,13 15.38,13.02 15.03,13.05 C16.19,13.89 17,15.02 17,16.5 L17,19 " +
            "L23,19 L23,16.5 C23,14.17 18.33,13 16,13 Z",
        PathFillType.NonZero,
    )

    /** Historique : trois barres, la lecture d'un volume par periode. */
    val Chart: ImageVector = icone(
        "Chart",
        "M5,9.2 L8,9.2 L8,19 L5,19 Z M10.6,5 L13.4,5 L13.4,19 L10.6,19 Z M16.2,13 L19,13 L19,19 L16.2,19 Z",
        PathFillType.NonZero,
    )

    /** Musique : la note, commune a l'onglet et au lecteur. */
    val Music: ImageVector = icone(
        "Music",
        "M12,3 L12,13.55 C11.41,13.21 10.73,13 10,13 C7.79,13 6,14.79 6,17 " +
            "C6,19.21 7.79,21 10,21 C12.21,21 14,19.21 14,17 L14,7 L18,7 L18,3 Z",
        PathFillType.NonZero,
    )

    /** Reglages : l'engrenage, moyeu evide. */
    val Settings: ImageVector = icone(
        "Settings",
        "M19.14,12.94 C19.18,12.64 19.2,12.33 19.2,12 C19.2,11.68 19.18,11.36 19.13,11.06 " +
            "L21.16,9.48 C21.34,9.34 21.39,9.07 21.28,8.87 L19.36,5.55 " +
            "C19.24,5.33 18.99,5.26 18.77,5.33 L16.38,6.29 " +
            "C15.88,5.91 15.35,5.59 14.76,5.35 L14.4,2.81 " +
            "C14.36,2.57 14.16,2.4 13.92,2.4 L10.08,2.4 " +
            "C9.84,2.4 9.65,2.57 9.61,2.81 L9.25,5.35 " +
            "C8.66,5.59 8.12,5.92 7.63,6.29 L5.24,5.33 " +
            "C5.02,5.25 4.77,5.33 4.65,5.55 L2.74,8.87 " +
            "C2.62,9.08 2.66,9.34 2.86,9.48 L4.89,11.06 " +
            "C4.84,11.36 4.8,11.69 4.8,12 C4.8,12.31 4.82,12.64 4.87,12.94 " +
            "L2.84,14.52 C2.66,14.66 2.61,14.93 2.72,15.13 L4.64,18.45 " +
            "C4.76,18.67 5.01,18.74 5.23,18.67 L7.62,17.71 " +
            "C8.12,18.09 8.65,18.41 9.24,18.65 L9.6,21.19 " +
            "C9.65,21.43 9.84,21.6 10.08,21.6 L13.92,21.6 " +
            "C14.16,21.6 14.36,21.43 14.39,21.19 L14.75,18.65 " +
            "C15.34,18.41 15.88,18.09 16.37,17.71 L18.76,18.67 " +
            "C18.98,18.75 19.23,18.67 19.35,18.45 L21.27,15.13 " +
            "C21.39,14.91 21.34,14.66 21.15,14.52 Z " +
            "M12,15.6 C10.02,15.6 8.4,13.98 8.4,12 C8.4,10.02 10.02,8.4 12,8.4 " +
            "C13.98,8.4 15.6,10.02 15.6,12 C15.6,13.98 13.98,15.6 12,15.6 Z",
        PathFillType.EvenOdd,
    )

    // --------------------------------------------------------------- lecteur

    /** Lecture. */
    val Play: ImageVector = icone("Play", "M8,5 L19,12 L8,19 Z", PathFillType.NonZero)

    /** Pause : deux barres, elargies pour rester lisibles sur un petit bouton. */
    val Pause: ImageVector = icone(
        "Pause",
        "M6,5 L10.6,5 L10.6,19 L6,19 Z M13.4,5 L18,5 L18,19 L13.4,19 Z",
        PathFillType.NonZero,
    )

    /** Piste precedente : la barre de debut, puis le triangle. */
    val SkipPrevious: ImageVector = icone(
        "SkipPrevious",
        "M6,6 L8,6 L8,18 L6,18 Z M18,6 L18,18 L9.41,12 Z",
        PathFillType.NonZero,
    )

    /** Piste suivante : le triangle, puis la barre de fin. */
    val SkipNext: ImageVector = icone(
        "SkipNext",
        "M16,6 L18,6 L18,18 L16,18 Z M6,6 L14.59,12 L6,18 Z",
        PathFillType.NonZero,
    )

    /** Volume fort : le haut-parleur et ses ondes. */
    val VolumeUp: ImageVector = icone(
        "VolumeUp",
        "M3,9 L7,9 L12,4 L12,20 L7,15 L3,15 Z " +
            "M16.5,12 C16.5,10.23 15.48,8.71 14,7.97 L14,16.02 C15.48,15.29 16.5,13.77 16.5,12 Z " +
            "M14,3.23 L14,5.29 C16.89,6.15 19,8.83 19,12 C19,15.17 16.89,17.85 14,18.71 " +
            "L14,20.77 C18.01,19.86 21,16.28 21,12 C21,7.72 18.01,4.14 14,3.23 Z",
        PathFillType.NonZero,
    )

    /** Volume faible : le haut-parleur seul, sans les ondes. */
    val VolumeDown: ImageVector = icone(
        "VolumeDown",
        "M3,9 L7,9 L12,4 L12,20 L7,15 L3,15 Z",
        PathFillType.NonZero,
    )

    /** Importer (USB) : la fleche qui monte vers le support. */
    val Upload: ImageVector = icone(
        "Upload",
        "M9,16 L15,16 L15,10 L19,10 L12,3 L5,10 L9,10 Z M5,18 L5,20 L19,20 L19,18 Z",
        PathFillType.NonZero,
    )

    /** Telecharger (serveur) : la fleche qui descend. */
    val Download: ImageVector = icone(
        "Download",
        "M19,9 L15,9 L15,3 L9,3 L9,9 L5,9 L12,16 Z M5,18 L5,20 L19,20 L19,18 Z",
        PathFillType.NonZero,
    )

    /** Supprimer : la corbeille. */
    val Delete: ImageVector = icone(
        "Delete",
        "M6,19 C6,20.1 6.9,21 8,21 L16,21 C17.1,21 18,20.1 18,19 L18,7 L6,7 Z " +
            "M19,4 L15.5,4 L14.5,3 L9.5,3 L8.5,4 L5,4 L5,6 L19,6 Z",
        PathFillType.NonZero,
    )

    // -------------------------------------------------------- course et outils

    /** Arreter : le carre de fin de seance. */
    val Stop: ImageVector = icone("Stop", "M6,6 L18,6 L18,18 L6,18 Z", PathFillType.NonZero)

    /** Annonce vocale / drapeau d'arrivee. */
    val Flag: ImageVector = icone(
        "Flag",
        "M14.4,6 L14,4 L5,4 L5,21 L7,21 L7,14 L12.6,14 L13,16 L20,16 L20,6 Z",
        PathFillType.NonZero,
    )

    /** Fenetre d'allure / rafraichir : la fleche circulaire. */
    val Refresh: ImageVector = icone(
        "Refresh",
        "M17.65,6.35 C16.2,4.9 14.21,4 12,4 C7.58,4 4.01,7.58 4,12 " +
            "C3.99,16.42 7.58,20 12,20 C15.73,20 18.84,17.45 19.73,14 " +
            "L17.65,14 C16.83,16.33 14.61,18 12,18 C8.69,18 6,15.31 6,12 " +
            "C6,8.69 8.69,6 12,6 C13.66,6 15.14,6.69 16.22,7.78 L13,11 L20,11 L20,4 Z",
        PathFillType.NonZero,
    )

    /** Chevron : ouvre une fiche depuis une liste. */
    val ChevronRight: ImageVector = icone(
        "ChevronRight",
        "M9.29,6.71 L13.58,11 L9.29,15.29 L10.7,16.7 L16.4,11 L10.7,5.3 Z",
        PathFillType.NonZero,
    )

    /** Position : l'epingle de carte. */
    val Location: ImageVector = icone(
        "Location",
        "M12,2 C8.13,2 5,5.13 5,9 C5,14.25 12,22 12,22 C12,22 19,14.25 19,9 C19,5.13 15.87,2 12,2 Z " +
            "M12,11.5 C10.62,11.5 9.5,10.38 9.5,9 C9.5,7.62 10.62,6.5 12,6.5 " +
            "C13.38,6.5 14.5,7.62 14.5,9 C14.5,10.38 13.38,11.5 12,11.5 Z",
        PathFillType.NonZero,
    )

    /** Chercher : la loupe. */
    val Search: ImageVector = icone(
        "Search",
        "M15.5,14 L14.71,14 L14.43,13.73 C15.41,12.59 16,11.11 16,9.5 " +
            "C16,5.91 13.09,3 9.5,3 C5.91,3 3,5.91 3,9.5 C3,13.09 5.91,16 9.5,16 " +
            "C11.11,16 12.59,15.41 13.73,14.43 L14,14.71 L14,15.5 L19,20.49 L20.49,19 Z " +
            "M9.5,14 C7.01,14 5,11.99 5,9.5 C5,7.01 7.01,5 9.5,5 C11.99,5 14,7.01 14,9.5 " +
            "C14,11.99 11.99,14 9.5,14 Z",
        PathFillType.NonZero,
    )

    /** Valider : la coche. */
    val Check: ImageVector = icone(
        "Check",
        "M9,16.17 L4.83,12 L3.41,13.41 L9,19 L21,7 L19.59,5.59 Z",
        PathFillType.NonZero,
    )

    /** Refuser / annuler : la croix. */
    val Close: ImageVector = icone(
        "Close",
        "M19,6.41 L17.59,5 L12,10.59 L6.41,5 L5,6.41 L10.59,12 L5,17.59 L6.41,19 " +
            "L12,13.41 L17.59,19 L19,17.59 L13.41,12 Z",
        PathFillType.NonZero,
    )

    /** Envoyer : l'avion de papier. */
    val Send: ImageVector = icone(
        "Send",
        "M2.01,21 L23,12 L2.01,3 L2,10 L17,12 L2,14 Z",
        PathFillType.NonZero,
    )

    /** Copier : les deux feuilles. */
    val Copy: ImageVector = icone(
        "Copy",
        "M16,1 L4,1 C2.9,1 2,1.9 2,3 L2,17 L4,17 L4,3 L16,3 Z " +
            "M19,5 L8,5 C6.9,5 6,5.9 6,7 L6,21 C6,22.1 6.9,23 8,23 L19,23 C20.1,23 21,22.1 21,21 " +
            "L21,7 C21,5.9 20.1,5 19,5 Z M19,21 L8,21 L8,7 L19,7 Z",
        PathFillType.NonZero,
    )

    /** Retour : la fleche vers la gauche. */
    val Back: ImageVector = icone(
        "Back",
        "M20,11 L7.83,11 L13.42,5.41 L12,4 L4,12 L12,20 L13.41,18.59 L7.83,13 L20,13 Z",
        PathFillType.NonZero,
    )

    /** Partager : les trois points relies. */
    val Share: ImageVector = icone(
        "Share",
        "M18,16.08 C17.24,16.08 16.56,16.38 16.04,16.85 L8.91,12.7 " +
            "C8.96,12.47 9,12.24 9,12 C9,11.76 8.96,11.53 8.91,11.3 " +
            "L15.96,7.19 C16.5,7.69 17.21,8 18,8 C19.66,8 21,6.66 21,5 " +
            "C21,3.34 19.66,2 18,2 C16.34,2 15,3.34 15,5 C15,5.24 15.04,5.47 15.09,5.7 " +
            "L8.04,9.81 C7.5,9.31 6.79,9 6,9 C4.34,9 3,10.34 3,12 " +
            "C3,13.66 4.34,15 6,15 C6.79,15 7.5,14.69 8.04,14.19 " +
            "L15.16,18.34 C15.11,18.55 15.08,18.77 15.08,19 C15.08,20.61 16.39,21.92 18,21.92 " +
            "C19.61,21.92 20.92,20.61 20.92,19 C20.92,17.39 19.61,16.08 18,16.08 Z",
        PathFillType.NonZero,
    )

    /**
     * Fabrique commune : grille de 24 x 24, un trace SVG, un seul remplissage.
     * Le remplissage pair-impair n'est necessaire que pour l'engrenage, dont le
     * moyeu est un trou.
     */
    private fun icone(
        nom: String,
        chemin: String,
        remplissage: PathFillType,
    ): ImageVector = ImageVector.Builder(
        name = nom,
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).addPath(
        pathData = PathParser().parsePathString(chemin).toNodes(),
        pathFillType = remplissage,
        fill = SolidColor(Color.White),
    ).build()
}
