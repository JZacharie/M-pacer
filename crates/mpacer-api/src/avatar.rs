//! Photo de profil Google : proxy serveur et pastille d'initiales.
//!
//! Le navigateur ne contacte jamais googleusercontent.com : GET /avatar sert les
//! octets recuperes par le service (conserves quelques heures en memoire), ou une
//! pastille SVG aux initiales du compte quand aucune photo n'est disponible.
//! Deux consequences utiles : aucune requete tierce depuis la page, et l'avatar
//! reste affiche meme si le poste ne peut pas joindre Google.

use crate::models::User;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Duree de conservation d'une photo en memoire.
const TTL: Duration = Duration::from_secs(6 * 60 * 60);
/// Nombre maximal de photos conservees (les plus anciennes sont evincees).
const MAX_ENTRIES: usize = 64;
/// Taille maximale acceptee pour une photo : un avatar Google pese quelques ko,
/// cette borne evite qu'une reponse inattendue gonfle la memoire du service.
const MAX_BYTES: usize = 512 * 1024;

/// Photo de profil prete a etre servie.
#[derive(Clone, Debug)]
pub struct AvatarImage {
    pub bytes: Arc<[u8]>,
    pub content_type: String,
}

struct Cached {
    image: AvatarImage,
    fetched_at: Instant,
}

/// Cache memoire des photos de profil, partage par toutes les requetes.
#[derive(Default)]
pub struct AvatarCache {
    entries: Mutex<HashMap<String, Cached>>,
}

impl AvatarCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Photo correspondant a une URL : celle du cache, sinon celle telechargee.
    ///
    /// Renvoie None si l'URL n'est pas une photo Google ou si le fournisseur ne
    /// repond pas : l'appelant sert alors la pastille d'initiales.
    pub async fn picture(&self, http: &reqwest::Client, url: &str) -> Option<AvatarImage> {
        if !is_google_picture_url(url) {
            tracing::debug!("photo de profil ignoree : hote non Google");
            return None;
        }
        if let Some(image) = self.cached(url) {
            return Some(image);
        }
        match download(http, url).await {
            Ok(image) => {
                self.store(url, image.clone());
                Some(image)
            }
            Err(error) => {
                tracing::warn!(%error, "photo de profil Google indisponible");
                None
            }
        }
    }

    fn cached(&self, url: &str) -> Option<AvatarImage> {
        let entries = self.entries.lock().ok()?;
        let entry = entries.get(url)?;
        (entry.fetched_at.elapsed() < TTL).then(|| entry.image.clone())
    }

    fn store(&self, url: &str, image: AvatarImage) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        if entries.len() >= MAX_ENTRIES && !entries.contains_key(url) {
            let oldest = entries
                .iter()
                .min_by_key(|(_, entry)| entry.fetched_at)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                entries.remove(&oldest);
            }
        }
        entries.insert(
            url.to_string(),
            Cached {
                image,
                fetched_at: Instant::now(),
            },
        );
    }
}

/// Telecharge une photo de profil, en refusant tout ce qui n'est pas une image
/// de taille raisonnable.
async fn download(http: &reqwest::Client, url: &str) -> anyhow::Result<AvatarImage> {
    let response = http.get(url).send().await?.error_for_status()?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("image/jpeg")
        .split(';')
        .next()
        .unwrap_or("image/jpeg")
        .trim()
        .to_ascii_lowercase();
    if !content_type.starts_with("image/") {
        anyhow::bail!("type de contenu inattendu pour une photo : {content_type}");
    }
    if response.content_length().unwrap_or(0) > MAX_BYTES as u64 {
        anyhow::bail!("photo de profil trop volumineuse");
    }
    let bytes = response.bytes().await?;
    if bytes.is_empty() {
        anyhow::bail!("photo de profil vide");
    }
    if bytes.len() > MAX_BYTES {
        anyhow::bail!("photo de profil trop volumineuse");
    }
    Ok(AvatarImage {
        bytes: Arc::from(bytes.to_vec()),
        content_type,
    })
}

/// Vrai si l'URL est une photo servie par Google.
///
/// La valeur vient de l'id_token Google, mais elle est stockee en base : la
/// restriction d'hote evite qu'une valeur detournee transforme le service en
/// proxy vers un hote arbitraire. Une autorite avec utilisateur@hote ou une
/// barre oblique inverse est refusee, pour ne pas dependre du parseur d'URL.
pub fn is_google_picture_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#', '\\']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    let host = authority
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if host.is_empty()
        || !host
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
    {
        return false;
    }
    host.strip_suffix(".googleusercontent.com")
        .map(|prefix| !prefix.is_empty())
        .unwrap_or(host == "googleusercontent.com")
}

/// Initiales affichees quand le compte n'a pas de photo (1 ou 2 lettres).
pub fn initials(name: Option<&str>, email: &str) -> String {
    let source = name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(email);
    let mut letters: Vec<char> = source
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .filter_map(|word| word.chars().next())
        .collect();
    letters.truncate(2);
    if letters.is_empty() {
        // Aucune lettre exploitable (compte sans nom ni adresse lisible).
        return "M".to_string();
    }
    letters
        .iter()
        .flat_map(|letter| letter.to_uppercase())
        .collect()
}

/// Pastille SVG aux initiales, teinte stable derivee du compte.
pub fn monogram_svg(user: &User) -> String {
    let letters = escape(&initials(user.name.as_deref(), &user.email));
    let hue = hue_for(&user.id);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\" width=\"64\" height=\"64\" role=\"img\">\
<defs><linearGradient id=\"avatar-fond\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\">\
<stop offset=\"0\" stop-color=\"hsl({hue} 58% 52%)\"/>\
<stop offset=\"1\" stop-color=\"hsl({hue} 58% 38%)\"/></linearGradient></defs>\
<rect width=\"64\" height=\"64\" rx=\"18\" fill=\"url(#avatar-fond)\"/>\
<text x=\"32\" y=\"41\" text-anchor=\"middle\" font-family=\"system-ui,-apple-system,'Segoe UI',Roboto,sans-serif\" font-size=\"26\" font-weight=\"600\" fill=\"#ffffff\">{letters}</text>\
</svg>"
    )
}

/// Teinte (0-359) stable pour un compte, sans dependance de hachage.
fn hue_for(seed: &str) -> u64 {
    // FNV-1a 64 bits.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in seed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash % 360
}

/// Echappe le texte insere dans le SVG (les initiales viennent du compte).
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(name: Option<&str>, email: &str, id: &str) -> User {
        User {
            id: id.to_string(),
            google_sub: None,
            share_live: true,
            email: email.to_string(),
            name: name.map(str::to_string),
            picture_url: None,
            created_at_ms: 0,
            last_seen_ms: 0,
        }
    }

    #[test]
    fn initials_come_from_the_name_then_the_email() {
        assert_eq!(initials(Some("Coureur Test"), "x@example.org"), "CT");
        assert_eq!(initials(Some("Developpeur"), "x@example.org"), "D");
        assert_eq!(initials(Some("  "), "coureur@example.org"), "CE");
        assert_eq!(initials(None, "@"), "M");
        assert_eq!(initials(Some("ana maria silva"), "x@example.org"), "AM");
    }

    #[test]
    fn a_monogram_stays_within_the_svg() {
        let svg = monogram_svg(&user(Some("<b>Bruno</b>"), "coureur@example.org", "u-1"));
        assert!(svg.starts_with("<svg"), "{svg}");
        // Les initiales sont tirees des mots "b" et "Bruno" : le balisage du nom
        // ne se retrouve jamais dans le document.
        assert!(svg.contains(">BB</text>"), "{svg}");
        assert!(!svg.contains("<b>"), "{svg}");
    }

    #[test]
    fn svg_text_is_escaped() {
        assert_eq!(escape("<&>\"'"), "&lt;&amp;&gt;&quot;&apos;");
    }

    #[test]
    fn a_monogram_is_stable_for_a_given_account() {
        let first = user(None, "coureur@example.org", "u-1");
        let same = user(None, "coureur@example.org", "u-1");
        let other = user(None, "coureur@example.org", "u-2");
        assert_eq!(monogram_svg(&first), monogram_svg(&same));
        assert_ne!(monogram_svg(&first), monogram_svg(&other));
    }

    #[test]
    fn only_google_pictures_are_proxied() {
        assert!(is_google_picture_url(
            "https://lh3.googleusercontent.com/a/ACg8ocK=s96-c"
        ));
        assert!(is_google_picture_url(
            "https://googleusercontent.com/a/photo.jpg"
        ));
        // Protocole, hote voisin, usurpation d'autorite et chemin trompeur.
        assert!(!is_google_picture_url(
            "http://lh3.googleusercontent.com/a/photo.jpg"
        ));
        assert!(!is_google_picture_url("https://evil.com/photo.jpg"));
        assert!(!is_google_picture_url(
            "https://lh3.googleusercontent.com.evil.com/a/photo.jpg"
        ));
        assert!(!is_google_picture_url(
            "https://evilgoogleusercontent.com/a/x"
        ));
        assert!(!is_google_picture_url(
            "https://lh3.googleusercontent.com@evil.com/a/photo.jpg"
        ));
        assert!(!is_google_picture_url(
            "https://evil.com\\@lh3.googleusercontent.com/a/photo.jpg"
        ));
        assert!(!is_google_picture_url(""));
    }
}
