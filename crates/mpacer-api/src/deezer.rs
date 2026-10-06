//! Client Deezer : OAuth 2.0 et API Web (lecture seule des metadonnees).
//!
//! Deezer joue exactement le meme role que Spotify dans M-pacer : une **source
//! de metadonnees**. M-pacer ne telecharge jamais l'audio (le flux Deezer est
//! chiffre et reserve aux applications agreees) ; il lit la fiche des playlists
//! (titres, artistes, durees) et laisse les fichiers MP3 venir du disque de
//! l'utilisateur, copies sur la montre par l'outil local `mpacer-music`.
//!
//! Deux particularites par rapport a Spotify :
//! * l'echange du code se fait en `GET` et Deezer repond par une chaine de
//!   requete (ou du JSON avec `output=json`) : voir `parse_token_body` ;
//! * l'API Deezer n'expose **aucun tempo** : le BPM d'une playlist Deezer reste
//!   inconnu a l'import et se complete par la balise du fichier, le tap-tempo ou
//!   la saisie manuelle sur la page `/music`.
//!
//! Deezer renvoie ses erreurs applicatives en HTTP 200 avec un corps
//! `{"error":{...}}` : chaque reponse passe donc par `check_error`.

use crate::config::Config;
use crate::error::{AppError, AppResult};
use serde::Deserialize;

/// Points d'entree Deezer.
pub const AUTHORIZE_URL: &str = "https://connect.deezer.com/oauth/auth.php";
pub const TOKEN_URL: &str = "https://connect.deezer.com/oauth/access_token.php";
pub const API_BASE: &str = "https://api.deezer.com";
/// Portees minimales : profil du compte et lecture des playlists.
pub const PERMS: &str = "basic_access,email";
/// Garde-fou de pagination : une playlist de course ne fait pas 1000 titres.
const MAX_PAGES: usize = 20;
/// Erreur applicative Deezer (corps HTTP 200).
#[derive(Debug, Deserialize)]
struct RawErrorBody {
    #[serde(default)]
    error: Option<RawError>,
}

#[derive(Debug, Deserialize)]
struct RawError {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    code: Option<i64>,
}

/// Reponse du point d'echange de jeton.
#[derive(Debug, Clone)]
pub struct TokenResponse {
    pub access_token: String,
    /// Duree de vie en secondes ; `None` quand Deezer ne la communique pas.
    pub expires_in: Option<i64>,
}

/// Playlist Deezer telle qu'affichee dans les resultats.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaylistRef {
    pub id: String,
    pub name: String,
    pub track_count: i64,
    pub cover_url: Option<String>,
    pub owner: Option<String>,
}

/// Piste Deezer (metadonnees seules : aucun tempo n'est disponible).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrackRef {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_s: Option<f64>,
}

/// Fiche complete d'une playlist Deezer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaylistDetail {
    pub id: String,
    pub name: String,
    pub cover_url: Option<String>,
    pub tracks: Vec<TrackRef>,
}

/// Profil du compte lie.
#[derive(Debug, Clone)]
pub struct Profile {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

// ---------------------------------------------------------------- reponses brutes

#[derive(Debug, Deserialize)]
struct RawUser {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawArtist {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawAlbum {
    #[serde(default)]
    title: Option<String>,
}

/// Piste Deezer : seules les metadonnees utiles a M-pacer sont lues (l'`id`
/// Deezer n'apparait dans aucun contrat : le manifeste identifie une piste par
/// sa position et son titre).
#[derive(Debug, Deserialize)]
struct RawTrack {
    #[serde(default)]
    title: Option<String>,
    /// Duree en secondes (Deezer ne renvoie pas de millisecondes).
    #[serde(default)]
    duration: Option<i64>,
    #[serde(default)]
    artist: Option<RawArtist>,
    #[serde(default)]
    album: Option<RawAlbum>,
}

#[derive(Debug, Deserialize)]
struct RawTrackPage {
    #[serde(default)]
    data: Vec<RawTrack>,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawPlaylist {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    nb_tracks: Option<i64>,
    #[serde(default)]
    picture_medium: Option<String>,
    #[serde(default)]
    picture_bigger: Option<String>,
    #[serde(default)]
    user: Option<RawUser>,
    #[serde(default)]
    tracks: Option<RawTrackPage>,
    /// Deezer signale les playlists illisibles par un champ `error` dans l'objet.
    #[serde(default)]
    error: Option<RawError>,
}

#[derive(Debug, Deserialize)]
struct RawPlaylistPage {
    #[serde(default)]
    data: Vec<RawPlaylist>,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    error: Option<RawError>,
}

// ---------------------------------------------------------------- OAuth

/// URL d'autorisation Deezer (OAuth 2.0, code d'autorisation).
pub fn authorize_url(config: &Config, oauth_state: &str) -> AppResult<String> {
    let app_id = config
        .deezer_app_id
        .clone()
        .filter(|_| config.deezer_configured())
        .ok_or_else(|| AppError::bad_request("Deezer n'est pas configure sur ce service"))?;
    let redirect_uri = config.deezer_redirect_uri();
    Ok(format!(
        "{AUTHORIZE_URL}?app_id={}&redirect_uri={}&perms={}&state={}",
        encode(&app_id),
        encode(&redirect_uri),
        encode(PERMS),
        encode(oauth_state)
    ))
}

/// Echange le code d'autorisation contre un jeton d'acces.
pub async fn exchange_code(
    http: &reqwest::Client,
    config: &Config,
    code: &str,
) -> AppResult<TokenResponse> {
    let app_id = config.deezer_app_id.clone().unwrap_or_default();
    let secret = config.deezer_app_secret.clone().unwrap_or_default();
    let response = http
        .get(TOKEN_URL)
        .query(&[
            ("app_id", app_id.as_str()),
            ("secret", secret.as_str()),
            ("code", code),
            ("output", "json"),
        ])
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::internal(format!(
            "Deezer a refuse la demande de jeton (HTTP {}): {}",
            status.as_u16(),
            truncate(&body)
        )));
    }
    if let Some(error) = token_error(&body) {
        return Err(AppError::internal(format!(
            "Deezer a refuse la demande de jeton : {error}"
        )));
    }
    let (access_token, expires_in) = parse_token_body(&body).ok_or_else(|| {
        AppError::internal(format!(
            "reponse de jeton Deezer illisible : {}",
            truncate(&body)
        ))
    })?;
    Ok(TokenResponse {
        access_token,
        expires_in,
    })
}

/// Lit le corps du point d'echange de jeton : JSON (`output=json`) ou chaine de
/// requete (`access_token=...&expires=...`).
pub fn parse_token_body(body: &str) -> Option<(String, Option<i64>)> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('{') {
        let payload: serde_json::Value = serde_json::from_str(trimmed).ok()?;
        let token = payload.get("access_token")?.as_str()?;
        if token.is_empty() {
            return None;
        }
        let expires = payload.get("expires").and_then(|value| value.as_i64());
        return Some((token.to_string(), usable_expires(expires)));
    }
    let mut token: Option<String> = None;
    let mut expires: Option<i64> = None;
    for pair in trimmed.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        match key {
            "access_token" => token = Some(decode(value)),
            "expires" => expires = value.parse::<i64>().ok(),
            _ => {}
        }
    }
    token
        .filter(|token| !token.is_empty())
        .map(|token| (token, usable_expires(expires)))
}

/// `expires=0` signifie "jeton sans expiration" : on ne l'expose pas comme une
/// duree de vie, pour ne pas le traiter comme un jeton deja perime.
fn usable_expires(expires: Option<i64>) -> Option<i64> {
    expires.filter(|seconds| *seconds > 0)
}

/// Message d'erreur applicative dans un corps Deezer, s'il y en a un.
fn token_error(body: &str) -> Option<String> {
    let payload: RawErrorBody = serde_json::from_str(body.trim()).ok()?;
    error_message(payload.error.as_ref())
}

fn error_message(error: Option<&RawError>) -> Option<String> {
    let error = error?;
    let message = error.message.clone().unwrap_or_default();
    match error.code {
        Some(code) if !message.is_empty() => Some(format!("{message} (code {code})")),
        Some(code) => Some(format!("code {code}")),
        None if !message.is_empty() => Some(message),
        None => Some("erreur inconnue".to_string()),
    }
}

// ---------------------------------------------------------------- API Web

/// Playlists du compte lie (toutes pages confondues).
pub async fn list_user_playlists(
    http: &reqwest::Client,
    access_token: &str,
) -> AppResult<Vec<PlaylistRef>> {
    let mut url = format!(
        "{API_BASE}/user/me/playlists?limit=50&access_token={}",
        encode(access_token)
    );
    let mut playlists = Vec::new();
    for _ in 0..MAX_PAGES {
        let page: RawPlaylistPage = get_json(http, &url).await?;
        playlists.extend(page.data.iter().filter_map(playlist_ref));
        match page.next {
            Some(next) => url = with_token(next, access_token),
            None => break,
        }
    }
    Ok(playlists)
}

/// Recherche de playlists (20 premiers resultats).
pub async fn search_playlists(
    http: &reqwest::Client,
    access_token: &str,
    query: &str,
) -> AppResult<Vec<PlaylistRef>> {
    let response = http
        .get(format!("{API_BASE}/search/playlist"))
        .query(&[
            ("q", query),
            ("limit", "20"),
            ("access_token", access_token),
        ])
        .send()
        .await?;
    let page: RawPlaylistPage = json_or_error(response).await?;
    Ok(page.data.iter().filter_map(playlist_ref).collect())
}

/// Fiche complete d'une playlist (tous les titres, pages suivies).
pub async fn get_playlist(
    http: &reqwest::Client,
    access_token: &str,
    id: &str,
) -> AppResult<PlaylistDetail> {
    let url = format!(
        "{API_BASE}/playlist/{id}?access_token={}",
        encode(access_token)
    );
    let raw: RawPlaylist = get_json(http, &url).await?;
    if let Some(message) = error_message(raw.error.as_ref()) {
        return Err(AppError::internal(format!(
            "playlist Deezer illisible : {message}"
        )));
    }

    let mut tracks = Vec::new();
    let mut next = raw
        .tracks
        .as_ref()
        .and_then(|page| page.next.clone())
        .map(|next| with_token(next, access_token));
    if let Some(page) = &raw.tracks {
        tracks.extend(page.data.iter().filter_map(track_ref));
    }
    let mut pages = 0;
    while let Some(page_url) = next.take() {
        if pages >= MAX_PAGES {
            tracing::warn!(playlist = %id, "playlist Deezer tronquee a la pagination maximale");
            break;
        }
        pages += 1;
        let page: RawTrackPage = get_json(http, &page_url).await?;
        tracks.extend(page.data.iter().filter_map(track_ref));
        next = page.next.map(|next| with_token(next, access_token));
    }

    Ok(PlaylistDetail {
        id: raw
            .id
            .map(|id| id.to_string())
            .unwrap_or_else(|| id.to_string()),
        name: raw
            .title
            .clone()
            .unwrap_or_else(|| "Playlist Deezer".to_string()),
        cover_url: raw
            .picture_bigger
            .clone()
            .or_else(|| raw.picture_medium.clone()),
        tracks,
    })
}

/// Profil du compte lie (nom affiche dans la page).
pub async fn current_user(http: &reqwest::Client, access_token: &str) -> AppResult<Profile> {
    let url = format!("{API_BASE}/user/me?access_token={}", encode(access_token));
    let raw: RawProfile = get_json(http, &url).await?;
    if let Some(message) = error_message(raw.error.as_ref()) {
        return Err(AppError::internal(format!(
            "profil Deezer illisible : {message}"
        )));
    }
    Ok(Profile {
        id: raw.id.map(|id| id.to_string()),
        display_name: raw.name,
    })
}

/// Identifiant de playlist extrait d'un lien, d'une URI ou d'un identifiant.
///
/// Accepte `https://www.deezer.com/fr/playlist/908622995`, l'URI
/// `deezer:playlist:908622995` et le nombre seul. Refuse tout le reste.
pub fn playlist_id_from_ref(reference: &str) -> Option<String> {
    let trimmed = reference.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().all(|character| character.is_ascii_digit()) {
        return Some(trimmed.to_string());
    }
    let lowered = trimmed.to_ascii_lowercase();
    let after_marker = lowered
        .split_once("/playlist/")
        .map(|(_, rest)| rest.to_string())
        .or_else(|| {
            lowered
                .strip_prefix("deezer:playlist:")
                .map(|rest| rest.to_string())
        })
        .or_else(|| {
            lowered
                .split_once("playlist/")
                .map(|(_, rest)| rest.to_string())
        })?;
    let digits: String = after_marker
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    (!digits.is_empty()).then_some(digits)
}

/// Ajoute le jeton d'acces a une URL de pagination renvoyee par Deezer.
fn with_token(url: String, access_token: &str) -> String {
    if url.contains("access_token=") {
        return url;
    }
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}access_token={}", encode(access_token))
}

fn playlist_ref(raw: &RawPlaylist) -> Option<PlaylistRef> {
    let id = raw.id?;
    Some(PlaylistRef {
        id: id.to_string(),
        name: raw
            .title
            .clone()
            .unwrap_or_else(|| format!("Playlist {id}")),
        track_count: raw.nb_tracks.unwrap_or(0).max(0),
        cover_url: raw
            .picture_medium
            .clone()
            .or_else(|| raw.picture_bigger.clone()),
        owner: raw.user.as_ref().and_then(|user| user.name.clone()),
    })
}

fn track_ref(raw: &RawTrack) -> Option<TrackRef> {
    let title = raw.title.clone().filter(|title| !title.is_empty())?;
    Some(TrackRef {
        title,
        artist: raw.artist.as_ref().and_then(|artist| artist.name.clone()),
        album: raw.album.as_ref().and_then(|album| album.title.clone()),
        duration_s: raw
            .duration
            .filter(|seconds| *seconds > 0)
            .map(|seconds| seconds as f64),
    })
}

async fn get_json<T: for<'de> Deserialize<'de>>(http: &reqwest::Client, url: &str) -> AppResult<T> {
    let response = http.get(url).send().await?;
    json_or_error(response).await
}

/// Decode un corps JSON en refusant les erreurs applicatives Deezer.
async fn json_or_error<T: for<'de> Deserialize<'de>>(response: reqwest::Response) -> AppResult<T> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::internal(format!(
            "Deezer a refuse la demande (HTTP {}): {}",
            status.as_u16(),
            truncate(&body)
        )));
    }
    let value: serde_json::Value = serde_json::from_str(body.trim()).map_err(|error| {
        AppError::internal(format!(
            "reponse Deezer illisible : {error} ({})",
            truncate(&body)
        ))
    })?;
    if let Ok(payload) = serde_json::from_value::<RawErrorBody>(value.clone()) {
        if let Some(message) = error_message(payload.error.as_ref()) {
            return Err(AppError::internal(format!(
                "Deezer a refuse la demande : {message}"
            )));
        }
    }
    serde_json::from_value(value)
        .map_err(|error| AppError::internal(format!("reponse Deezer inattendue : {error}")))
}

/// Encodage pourcentage minimal (RFC 3986) pour les valeurs d'URL.
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Decodage pourcentage (`%2C`, `+`) d'une valeur de formulaire.
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = String::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        output.push(byte as char);
                        index += 3;
                    }
                    Err(_) => {
                        output.push('%');
                        index += 1;
                    }
                }
            }
            b'+' => {
                output.push(' ');
                index += 1;
            }
            other => {
                output.push(other as char);
                index += 1;
            }
        }
    }
    output
}

/// Tronque un corps d'erreur pour le journal (jamais de fuite de jeton).
fn truncate(body: &str) -> String {
    let cleaned = body.trim();
    let mut text: String = cleaned.chars().take(300).collect();
    if cleaned.chars().count() > 300 {
        text.push('…');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_body_is_read_from_json_or_query_string() {
        assert_eq!(
            parse_token_body(r#"{"access_token":"abc","expires":3600}"#),
            Some(("abc".to_string(), Some(3600)))
        );
        // expires = 0 : jeton sans expiration, aucune duree de vie annoncee.
        assert_eq!(
            parse_token_body(r#"{"access_token":"abc","expires":0}"#),
            Some(("abc".to_string(), None))
        );
        assert_eq!(
            parse_token_body("access_token=abc&expires=3600"),
            Some(("abc".to_string(), Some(3600)))
        );
        assert_eq!(parse_token_body("error_reason=user_denied"), None);
        assert_eq!(parse_token_body(""), None);
        assert_eq!(parse_token_body("   "), None);
    }

    #[test]
    fn application_errors_are_detected_in_a_successful_body() {
        let body = r#"{"error":{"type":"OAuthException","message":"Invalid token","code":300}}"#;
        assert_eq!(
            token_error(body).as_deref(),
            Some("Invalid token (code 300)")
        );
        assert_eq!(token_error(r#"{"data":[]}"#), None);
        assert_eq!(token_error("pas du json"), None);
    }

    #[test]
    fn playlist_reference_accepts_links_uris_and_ids() {
        assert_eq!(
            playlist_id_from_ref("https://www.deezer.com/fr/playlist/908622995"),
            Some("908622995".to_string())
        );
        assert_eq!(
            playlist_id_from_ref("https://www.deezer.com/playlist/123?utm_source=x"),
            Some("123".to_string())
        );
        assert_eq!(
            playlist_id_from_ref("deezer:playlist:456"),
            Some("456".to_string())
        );
        assert_eq!(playlist_id_from_ref(" 789 "), Some("789".to_string()));
        assert_eq!(playlist_id_from_ref(""), None);
        assert_eq!(
            playlist_id_from_ref("https://open.spotify.com/playlist/abc"),
            None
        );
    }

    #[test]
    fn playlist_payloads_are_mapped_to_metadata_only() {
        let payload: RawPlaylist = serde_json::from_str(
            r#"{
                "id": 42,
                "title": "Run 170",
                "nb_tracks": 3,
                "picture_medium": "https://e-cdns-images.dzcdn.net/cover.jpg",
                "user": { "name": "Zach" },
                "tracks": {
                    "data": [
                        { "id": 1, "title": "Wake me up", "duration": 249,
                          "artist": { "name": "Avicii" }, "album": { "title": "True" } },
                        { "id": 2, "title": "Sans duree", "artist": { "name": "X" } }
                    ],
                    "next": "https://api.deezer.com/playlist/42/tracks?index=25"
                }
            }"#,
        )
        .expect("charge utile Deezer valide");

        let reference = playlist_ref(&payload).expect("playlist mappee");
        assert_eq!(reference.id, "42");
        assert_eq!(reference.name, "Run 170");
        assert_eq!(reference.track_count, 3);
        assert_eq!(reference.owner.as_deref(), Some("Zach"));

        let page = payload.tracks.expect("pistes presentes");
        let tracks: Vec<TrackRef> = page.data.iter().filter_map(track_ref).collect();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "Wake me up");
        assert_eq!(tracks[0].artist.as_deref(), Some("Avicii"));
        assert_eq!(tracks[0].album.as_deref(), Some("True"));
        assert_eq!(tracks[0].duration_s, Some(249.0));
        assert_eq!(tracks[1].duration_s, None);
        assert!(page.next.is_some());
    }

    #[test]
    fn pagination_urls_receive_the_access_token_once() {
        assert_eq!(
            with_token(
                "https://api.deezer.com/user/me/playlists?index=50".to_string(),
                "tok"
            ),
            "https://api.deezer.com/user/me/playlists?index=50&access_token=tok"
        );
        assert_eq!(
            with_token(
                "https://api.deezer.com/user/me/playlists?access_token=tok".to_string(),
                "tok"
            ),
            "https://api.deezer.com/user/me/playlists?access_token=tok"
        );
    }

    #[test]
    fn authorization_url_requires_configuration() {
        let mut config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
        assert!(
            authorize_url(&config, "etat").is_err(),
            "sans identifiants, refus"
        );

        config.deezer_app_id = Some("123456".to_string());
        config.deezer_app_secret = Some("secret".to_string());
        let url = authorize_url(&config, "etat-1").expect("URL d'autorisation");
        assert!(url.starts_with(AUTHORIZE_URL));
        assert!(url.contains("app_id=123456"));
        assert!(url.contains("state=etat-1"));
        assert!(
            url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8080%2Fauth%2Fdeezer%2Fcallback")
        );
    }
}
