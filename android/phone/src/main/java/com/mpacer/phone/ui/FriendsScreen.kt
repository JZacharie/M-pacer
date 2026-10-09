package com.mpacer.phone.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.widget.Toast
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.CircleShape
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.mpacer.core.MpacerFormat
import com.mpacer.core.live.LiveSettings
import com.mpacer.core.social.AvatarLoader
import com.mpacer.core.social.Friend
import com.mpacer.core.social.FriendRequest
import com.mpacer.core.social.FriendsClient
import com.mpacer.core.social.Invite
import com.mpacer.core.social.UserSummary
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
    var recherche by remember { mutableStateOf("") }
    // Le partage ne montre quelque chose que si l'appareil publie deja.
    val suiviConfigure = remember { LiveSettings.load(context).configured }

    LaunchedEffect(Unit) {
        FriendsClient.load(context)
        FriendsClient.loadRequests(context)
        FriendsClient.createInvite(context)
    }
    // Rafraichissement doux : les positions bougent, pas la structure du cercle.
    LaunchedEffect(Unit) {
        while (true) {
            delay(10_000)
            FriendsClient.load(context)
            FriendsClient.loadRequests(context)
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

        // ---------------------------------------------------- chercher un compte
        Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text("Chercher un compte", color = Palette.muted, fontSize = 12.sp)
                Text(
                    "Par adresse ou par nom : la demande part vers ce compte, et " +
                        "l'autre la valide.",
                    color = Palette.muted,
                    fontSize = 12.sp,
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    OutlinedTextField(
                        value = recherche,
                        onValueChange = { saisie -> recherche = saisie },
                        label = { Text("Adresse ou nom") },
                        singleLine = true,
                        modifier = Modifier.weight(1f),
                    )
                    Button(
                        onClick = { scope.launch { FriendsClient.search(context, recherche) } },
                        enabled = recherche.trim().length >= 2,
                        colors = ButtonDefaults.buttonColors(
                            containerColor = Palette.orange,
                            contentColor = Color.White,
                        ),
                    ) { Text("Chercher") }
                }
                etat.search.forEach { compte ->
                    CarteCompte(
                        compte = compte,
                        onAsk = { scope.launch { FriendsClient.sendRequest(context, compte.email) } },
                    )
                }
            }
        }

        // ------------------------------------------------------- demandes recues
        val demandes = etat.requests?.incoming.orEmpty()
        Card(colors = CardDefaults.cardColors(containerColor = Palette.surface)) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Text("Demandes recues", color = Palette.muted, fontSize = 12.sp)
                    if (demandes.isNotEmpty()) {
                        Text(
                            demandes.size.toString() + " en attente",
                            color = Palette.attention,
                            fontSize = 12.sp,
                        )
                    }
                }
                if (demandes.isEmpty()) {
                    Text(
                        "Personne ne vous a demande pour l'instant.",
                        color = Palette.muted,
                        fontSize = 13.sp,
                    )
                } else {
                    demandes.forEach { demande ->
                        CarteDemande(
                            demande = demande,
                            onAccept = {
                                scope.launch { FriendsClient.acceptRequest(context, demande.id) }
                            },
                            onDecline = {
                                scope.launch { FriendsClient.declineRequest(context, demande.id) }
                            },
                        )
                    }
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
                        majJson = etat.json,
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
                Row(
                    modifier = Modifier.weight(1f),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    PhotoProfil(id = ami.id, nom = ami.displayName, taille = 40.dp)
                    Column(modifier = Modifier.weight(1f)) {
                        Text(ami.displayName, color = Palette.texte, fontWeight = FontWeight.Medium)
                        Text(ami.email, color = Palette.muted, fontSize = 11.sp)
                    }
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

/** Fiche d'un compte trouve par la recherche : demander a etre ami. */
@Composable
private fun CarteCompte(compte: UserSummary, onAsk: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        PhotoProfil(id = compte.id, nom = compte.displayName, taille = 40.dp)
        Column(modifier = Modifier.weight(1f)) {
            Text(compte.displayName, color = Palette.texte, fontWeight = FontWeight.Medium)
            Text(compte.email, color = Palette.muted, fontSize = 11.sp)
        }
        Button(
            onClick = onAsk,
            colors = ButtonDefaults.buttonColors(
                containerColor = Palette.orange,
                contentColor = Color.White,
            ),
        ) { Text("Demander") }
    }
}

/** Fiche d'une demande recue : accepter, ou refuser. */
@Composable
private fun CarteDemande(demande: FriendRequest, onAccept: () -> Unit, onDecline: () -> Unit) {
    val compte = demande.autre
    val ecoule = ((System.currentTimeMillis() - demande.createdAtMs) / 1000).coerceAtLeast(0)
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
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                PhotoProfil(id = compte.id, nom = compte.displayName, taille = 40.dp)
                Column(modifier = Modifier.weight(1f)) {
                    Text(compte.displayName, color = Palette.texte, fontWeight = FontWeight.Medium)
                    Text(compte.email, color = Palette.muted, fontSize = 11.sp)
                }
            }
            demande.message?.takeIf { it.isNotBlank() }?.let { mot ->
                Text(mot, color = Palette.texte, fontSize = 13.sp)
            }
            Text("Demande recue il y a " + age(ecoule), color = Palette.muted, fontSize = 11.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(
                    onClick = onAccept,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = Palette.orange,
                        contentColor = Color.White,
                    ),
                ) { Text("Accepter") }
                TextButton(onClick = onDecline) {
                    Text("Refuser", color = Palette.danger)
                }
            }
        }
    }
}

/** Photo de profil d'un compte : la vraie si le service la sert, sinon les initiales. */
@Composable
private fun PhotoProfil(id: String, nom: String, taille: Dp) {
    val context = LocalContext.current
    var photo by remember(id) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(id) {
        photo = AvatarLoader.load(context, id)?.asImageBitmap()
    }
    val bitmap = photo
    if (bitmap != null) {
        Image(
            bitmap = bitmap,
            contentDescription = null,
            contentScale = ContentScale.Crop,
            modifier = Modifier
                .size(taille)
                .clip(CircleShape),
        )
    } else {
        Box(
            modifier = Modifier
                .size(taille)
                .clip(CircleShape)
                .background(Palette.orange),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                initiales(nom),
                color = Color.White,
                fontSize = 15.sp,
                fontWeight = FontWeight.SemiBold,
            )
        }
    }
}

/** Initiales d'un nom, pour la pastille sans photo. */
private fun initiales(nom: String): String {
    val mots = nom.split(' ', '.', '-', '_').filter { it.isNotBlank() }
    val lettres = mots.take(2).mapNotNull { it.firstOrNull() }
    if (lettres.isEmpty()) return "M"
    return lettres.joinToString("") { lettre -> lettre.uppercase() }
}

// La carte est celle de SessionMap.kt : meme script que le service web, memes
// styles, et le meme repli quand l'appareil n'a pas de moteur de rendu WebView.

// ------------------------------------------------------------------ helpers

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
