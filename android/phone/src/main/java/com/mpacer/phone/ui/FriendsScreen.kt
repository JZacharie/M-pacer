package com.mpacer.phone.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Color as CouleurAndroid
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.mpacer.core.MpacerFormat
import com.mpacer.core.live.LiveSettings
import com.mpacer.core.social.Friend
import com.mpacer.core.social.FriendsClient
import com.mpacer.core.social.Invite
import com.mpacer.core.ui.Palette
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Onglet Amis : un cercle ferme, des codes courts, et les positions en direct
 * des copains sur un fond de carte OpenStreetMap.
 *
 * Tout vient du backend (cercle, codes, positions publiees en MQTT) ; l'ecran ne
 * fait que presenter et envoyer des intentions. La position publiee est celle de
 * l'appareil pendant la seance : elle s'active dans l'onglet Reglages (suivi en
 * direct) et se coupe d'un interrupteur ici.
 */
@Composable
fun FriendsScreen() {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val etat by FriendsClient.state.collectAsState()
    var code by remember { mutableStateOf("") }
    // Le partage ne montre quelque chose que si l'appareil publie deja.
    val suiviConfigure = remember { LiveSettings.load(context).configured }

    LaunchedEffect(Unit) {
        FriendsClient.load(context)
        FriendsClient.createInvite(context)
    }
    // Rafraichissement doux : les positions bougent, pas la structure du cercle.
    LaunchedEffect(Unit) {
        while (true) {
            delay(10_000)
            FriendsClient.load(context)
        }
    }

    val cercle = etat.circle
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text("Amis", fontSize = 22.sp, fontWeight = FontWeight.SemiBold, color = Palette.texte)
        Text(
            "Vos positions ne sortent que vers ce cercle, et seulement pendant une seance.",
            color = Palette.muted,
            fontSize = 12.sp,
        )

        // Sans broker MQTT, personne ne publie : le partage est actif mais vide.
        if (!suiviConfigure) {
            Text(
                "Le suivi en direct n'est pas configure : renseignez le broker MQTT " +
                    "(Reglages > Suivi en direct) pour que vos amis vous voient courir.",
                color = Palette.attention,
                fontSize = 12.sp,
            )
        }

        etat.error?.let { erreur ->
            Text(erreur, color = Palette.attention, fontSize = 13.sp)
        }
        etat.message?.let { message ->
            Text(message, color = Palette.ok, fontSize = 13.sp)
        }

        // ------------------------------------------------------------- partage
        Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text("Partager ma position", color = Palette.texte, fontWeight = FontWeight.Medium)
                    Switch(
                        checked = cercle?.shareLive ?: true,
                        onCheckedChange = { partage -> scope.launch { FriendsClient.setShare(context, partage) } },
                    )
                }
                Text(
                    if (cercle?.shareLive == false) {
                        "Coupe : aucune position ne sort de votre compte, meme pour vos amis."
                    } else {
                        "Actif : vos amis voient votre trace pendant vos seances, rien apres."
                    },
                    color = Palette.muted,
                    fontSize = 12.sp,
                )
            }
        }

        // ---------------------------------------------------------- invitation
        Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Text("Inviter un ami", color = Palette.muted, fontSize = 12.sp)
                val invitation = etat.invite
                if (invitation != null) {
                    Text(
                        invitation.code,
                        color = Palette.texte,
                        fontSize = 30.sp,
                        fontWeight = FontWeight.Bold,
                        fontFamily = FontFamily.Monospace,
                    )
                    Text(
                        "Valable " + duree(invitation.expiresInS) + ", usage unique.",
                        color = Palette.muted,
                        fontSize = 12.sp,
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        TextButton(onClick = {
                            copier(context, invitation.code)
                            Toast.makeText(context, "Code copie", Toast.LENGTH_SHORT).show()
                        }) { Text("Copier") }
                        TextButton(onClick = { partagerInvitation(context, invitation) }) {
                            Text("Partager")
                        }
                        TextButton(onClick = {
                            scope.launch { FriendsClient.createInvite(context, nouvelle = true) }
                        }) { Text("Nouveau code") }
                    }
                } else {
                    Text("Aucun code pour l'instant.", color = Palette.muted, fontSize = 13.sp)
                }
            }
        }

        // --------------------------------------------------------- ajout d'ami
        Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text("Ajouter un ami", color = Palette.muted, fontSize = 12.sp)
                OutlinedTextField(
                    value = code,
                    onValueChange = { saisie -> code = formater(saisie) },
                    label = { Text("Code recu (BCDF-GHJK)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Button(
                    onClick = {
                        scope.launch {
                            if (FriendsClient.accept(context, code)) code = ""
                        }
                    },
                    enabled = code.length >= 8,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = Palette.orange,
                        contentColor = Color.White,
                    ),
                ) {
                    Text("Ajouter")
                }
            }
        }

        // --------------------------------------------------------------- carte
        Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(12.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text("Carte", color = Palette.texte, fontWeight = FontWeight.Medium)
                    Text(
                        (cercle?.live ?: 0).toString() + " en direct",
                        color = if ((cercle?.live ?: 0) > 0) Palette.ok else Palette.muted,
                        fontSize = 12.sp,
                    )
                }
                if (etat.json.isBlank()) {
                    Text(
                        "La carte s'affiche des que le serveur repond.",
                        color = Palette.muted,
                        fontSize = 12.sp,
                    )
                } else {
                    CarteOpenStreetMap(
                        json = etat.json,
                        backend = backend(context),
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(320.dp),
                    )
                    Text(
                        "Fond de carte (c) OpenStreetMap contributeurs",
                        color = Palette.muted2,
                        fontSize = 10.sp,
                    )
                }
            }
        }

        // -------------------------------------------------------------- cercle
        Text("Mon cercle", color = Palette.texte, fontWeight = FontWeight.Medium)
        val amis = cercle?.friends.orEmpty()
        if (amis.isEmpty()) {
            Text(
                "Personne pour l'instant : envoyez le code ci-dessus.",
                color = Palette.muted,
                fontSize = 13.sp,
            )
        } else {
            amis.forEach { ami ->
                CarteAmi(ami = ami, onRemove = { scope.launch { FriendsClient.removeFriend(context, ami.id) } })
            }
        }
    }
}

/** Fiche d'un ami : etat, chiffres de la seance et retrait. */
@Composable
private fun CarteAmi(ami: Friend, onRemove: () -> Unit) {
    val position = ami.live
    Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(ami.displayName, color = Palette.texte, fontWeight = FontWeight.Medium)
                    Text(ami.email, color = Palette.muted, fontSize = 11.sp)
                }
                Text(
                    libelleEtat(ami),
                    color = couleurEtat(ami),
                    fontSize = 12.sp,
                )
            }
            if (position != null) {
                Text(
                    MpacerFormat.distance(position.distanceM ?: 0.0) + "   " +
                        (position.paceSPerKm?.let { MpacerFormat.pace(it) + " /km" } ?: "--") +
                        (position.heartRateBpm?.let { "   " + it + " bpm" } ?: "") +
                        (position.lap?.let { "   tour " + it } ?: ""),
                    color = Palette.texte,
                    fontSize = 14.sp,
                )
                Text(
                    position.device + "   il y a " + age(position.ageS),
                    color = Palette.muted,
                    fontSize = 11.sp,
                )
            }
            TextButton(onClick = onRemove) { Text("Retirer", color = Palette.danger) }
        }
    }
}

/**
 * Carte OpenStreetMap dans une WebView.
 *
 * Le script de la carte (/static/map.js) est servi par le backend : une seule
 * implementation pour le navigateur et le telephone, donc un seul endroit ou
 * corriger un fond de carte. Les positions arrivent du client d'amis et sont
 * poussees dans la page : aucune session navigateur n'est necessaire.
 */
@Composable
private fun CarteOpenStreetMap(json: String, backend: String, modifier: Modifier = Modifier) {
    var vue by remember { mutableStateOf<WebView?>(null) }
    AndroidView(
        modifier = modifier,
        factory = { contexte ->
            WebView(contexte).apply {
                setBackgroundColor(CouleurAndroid.TRANSPARENT)
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = false
                webViewClient = object : WebViewClient() {
                    override fun onPageFinished(vue: WebView, url: String?) {
                        vue.injecter(json)
                    }
                }
                loadDataWithBaseURL(backend, PAGE_CARTE, "text/html", "utf-8", null)
                vue = this
            }
        },
        update = { web -> web.injecter(json) },
    )
}

/** Pousse une charge utile du serveur dans la carte affichee. */
private fun WebView.injecter(json: String) {
    if (json.isBlank()) return
    runCatching {
        evaluateJavascript(
            "window.MpacerCartes && window.MpacerCartes[0] && window.MpacerCartes[0].mettreAJour(" + json + ");",
            null,
        )
    }
}

/** Page minimale : la carte occupe toute la vue. */
private const val PAGE_CARTE = "<!DOCTYPE html><html lang=\"fr\"><head>" +
    "<meta charset=\"utf-8\">" +
    "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">" +
    "<style>html,body{margin:0;height:100%;background:#0b0d10}" +
    "#carte{position:absolute;inset:0}</style></head><body>" +
    "<div id=\"carte\" data-carte=\"1\" data-zoom=\"13\"></div>" +
    "<script src=\"/static/map.js\"></script></body></html>"

// ------------------------------------------------------------------ helpers

private fun backend(context: Context): String = com.mpacer.core.SyncClient.baseUrl(context)

private fun libelleEtat(ami: Friend): String = when {
    ami.live == null && ami.sharing -> "au repos"
    ami.live == null -> "ne partage pas"
    ami.live?.state == "pause" -> "en pause"
    ami.live?.state == "arm" -> "pret"
    else -> "en direct"
}

private fun couleurEtat(ami: Friend): Color = when {
    ami.live == null -> Palette.muted
    ami.live?.state == "pause" -> Palette.attention
    else -> Palette.ok
}

/** "3 min", "12 s" : age d'une position. */
private fun age(secondes: Long): String = when {
    secondes < 60 -> secondes.toString() + " s"
    secondes < 3600 -> (secondes / 60).toString() + " min"
    else -> (secondes / 3600).toString() + " h"
}

/** "24 h", "45 min" : validite d'un code. */
private fun duree(secondes: Long): String = when {
    secondes >= 3600 -> (secondes / 3600).toString() + " h"
    secondes >= 60 -> (secondes / 60).toString() + " min"
    else -> secondes.toString() + " s"
}

/** Saisie d'un code : majuscules, tiret au milieu, huit caracteres. */
private fun formater(saisie: String): String {
    val brut = saisie.uppercase().filter { it.isLetterOrDigit() }.take(8)
    return if (brut.length > 4) brut.substring(0, 4) + "-" + brut.substring(4) else brut
}

private fun copier(context: Context, texte: String) {
    val pressePapiers = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
    pressePapiers?.setPrimaryClip(ClipData.newPlainText("Code M-pacer", texte))
}

private fun partagerInvitation(context: Context, invitation: Invite) {
    val message = "Rejoins-moi sur M-pacer : " + invitation.url +
        " (code " + invitation.code + ")"
    val intention = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_TEXT, message)
    }
    context.startActivity(Intent.createChooser(intention, "Inviter un ami"))
}
