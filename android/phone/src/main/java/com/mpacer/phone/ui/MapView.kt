package com.mpacer.phone.ui

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Color as CouleurAndroid
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.mpacer.core.ui.Palette

/** Point d'une trace, en degres decimaux. */
data class PointCarte(val lat: Double, val lon: Double)

/** Repere pose sur la carte : un kilometre, le depart, l'arrivee. */
data class RepereCarte(val lat: Double, val lon: Double, val nom: String)

/**
 * Carte OpenStreetMap, sans dependance externe.
 *
 * Le script (/static/map.js) et la feuille de style (app.css) sont ceux du
 * service web : une seule implementation pour le navigateur et le telephone,
 * donc un seul endroit ou corriger un fond de carte. La tache Gradle
 * copierCarteWeb les embarque en assets, si bien que la carte s'affiche hors
 * ligne : seules les tuiles demandent le reseau, et la trace se dessine sans
 * elles.
 *
 * @param trace polyligne du parcours, dans l'ordre.
 * @param reperes marqueurs etiquetes (un par kilometre, par exemple).
 * @param majJson charge utile JSON poussee dans la carte apres chargement
 *   (positions du cercle d'amis) ; nulle pour une trace figee.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun CarteOpenStreetMap(
    trace: List<PointCarte> = emptyList(),
    reperes: List<RepereCarte> = emptyList(),
    majJson: String? = null,
    modifier: Modifier = Modifier,
) {
    val contexte = LocalContext.current
    if (!carteDisponible()) {
        Box(modifier, contentAlignment = Alignment.Center) {
            Text(
                "Carte indisponible : cet appareil n'a pas de moteur de rendu WebView.",
                color = Palette.muted2,
                fontSize = 11.sp,
            )
        }
        return
    }
    val page = remember(trace, reperes) { pageCarte(contexte, trace, reperes) }
    AndroidView(
        modifier = modifier,
        factory = { contexteVue ->
            WebView(contexteVue).apply {
                setBackgroundColor(CouleurAndroid.TRANSPARENT)
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = false
                webViewClient = object : WebViewClient() {
                    override fun onPageFinished(vue: WebView, url: String?) {
                        majJson?.let { vue.injecter(it) }
                    }
                }
                // Tout est inline : aucune requete n'est faite pour le script.
                loadDataWithBaseURL(null, page, "text/html", "utf-8", null)
            }
        },
        update = { web -> majJson?.let { web.injecter(it) } },
    )
}

/**
 * Vrai si l'appareil sait afficher une page web.
 *
 * Toutes les ROM n'embarquent pas de moteur de rendu WebView : une image
 * systeme nue, par exemple, n'en a aucun. Sans ce test, construire la vue leve
 * une UnsupportedOperationException qui emporte l'ecran entier -- carte ou pas
 * carte, l'utilisateur veut d'abord voir sa seance.
 */
fun carteDisponible(): Boolean =
    runCatching { WebView.getCurrentWebViewPackage() != null }.getOrDefault(false)

/** Pousse une charge utile du serveur dans la carte affichee. */
private fun WebView.injecter(json: String) {
    if (json.isBlank()) return
    runCatching {
        evaluateJavascript(
            "window.MpacerCartes && window.MpacerCartes[0] && window.MpacerCartes[0].mettreAJour(" +
                json + ");",
            null,
        )
    }
}

/**
 * Page minimale : la carte occupe toute la vue, script et style compris.
 *
 * Les donnees sont posees en attributs (data-trace, data-reperes) : c'est ce
 * que lit map.js au chargement, sans injection JavaScript.
 */
private fun pageCarte(
    contexte: Context,
    trace: List<PointCarte>,
    reperes: List<RepereCarte>,
): String {
    val script = lireAsset(contexte, "map.js")
    val style = lireAsset(contexte, "app.css")
    val attributTrace = trace.joinToString(",", "[", "]") { "[" + it.lat + "," + it.lon + "]" }
    val attributReperes = reperes.joinToString(",", "[", "]") {
        "{\"lat\":" + it.lat + ",\"lon\":" + it.lon + ",\"nom\":\"" + echapper(it.nom) + "\"}"
    }
    return "<!DOCTYPE html><html lang=\"fr\"><head><meta charset=\"utf-8\">" +
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">" +
        "<style>" + style + "\n" +
        "html,body{margin:0;height:100%;background:#0b0d10;overflow:hidden}" +
        "#carte{position:absolute;inset:0}</style></head><body>" +
        "<div id=\"carte\" data-carte=\"1\" data-zoom=\"14\" " +
        "data-trace='" + attributTrace + "' data-reperes='" + attributReperes + "'></div>" +
        "<script>" + script + "</script></body></html>"
}

/** Lit un asset embarque ; une carte absente ne doit pas faire planter l'ecran. */
private fun lireAsset(contexte: Context, nom: String): String =
    runCatching {
        contexte.assets.open(nom).bufferedReader().use { it.readText() }
    }.getOrElse { "" }

/** Echappe ce qui fermerait l'attribut HTML ou casserait le JSON. */
private fun echapper(texte: String): String =
    texte.replace("\\", "\\\\").replace("\"", "\\\"").replace("'", "&#39;").replace("<", "&lt;")
