//! Client Spotify : OAuth 2.0 + PKCE et API Web (lecture seule).
//!
//! M-pacer ne lit jamais l'audio Spotify (chiffre Widevine, reserve a
//! l'application Spotify) : il ne recupere que des **metadonnees** (fiche de
//! playlist, titres, durees, tempo) et laisse la montre telecommander
//! l'application Spotify installee.
//!
//! Depuis le 27/11/2024, l'endpoint `GET /v1/audio-features` (seule source
//! officielle du tempo) est refuse aux nouvelles applications. Un 403 ou un 404
//! n'est donc **jamais** une erreur bloquante : `audio_features` renvoie
//! `Ok(None)` et le BPM reste inconnu (balise, tap-tempo ou saisie manuelle
//! prennent le relais).

use crate::config::Config;
use crate::error::{AppError, AppResult};
use serde::Deserialize;

/// Points d'entree Spotify.
pub const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
pub const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
pub const API_BASE: &str = "https://api.spotify.com/v1";
/// Portees minimales : lecture des playlists privees et collaboratives.
pub const SCOPES: &str = "playlist-read-private playlist-read-collaborative";
/// Garde-fou de pagination : une playlist de course ne fait pas 1000 titres.
const MAX_PAGES: usize = 20;

/// Reponse du point d'echange de jeton.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub scope: Option<String>,
}

/// Playlist Spotify telle qu'affichee dans les resultats de recherche.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaylistRef {
    pub id: String,
    pub name: String,
    pub track_count: i64,
    pub cover_url: Option<String>,
    pub owner: Option<String>,
}

/// Piste Spotify (metadonnees seules).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrackRef {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_s: Option<f64>,
    pub spotify_uri: Option<String>,
    pub spotify_id: Option<String>,
}

/// Fiche complete d'une playlist Spotify.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaylistDetail {
    pub id: String,
    pub name: String,
    pub cover_url: Option<String>,
    pub tracks: Vec<TrackRef>,
}

/// Tempo renvoye par `audio-features`.
#[derive(Debug, Clone)]
pub struct AudioFeature {
    pub id: String,
    pub tempo: f64,
}

/// Profil du compte lie (nom affiche dans la page).
#[derive(Debug, Clone)]
pub struct Profile {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

// ---------------------------------------------------------------- reponses brutes

#[derive(Debug, Deserialize)]
struct RawImage {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawOwner {
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawArtist {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawAlbum {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTrack {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    duration_ms: Option<i64>,
    #[serde(default)]
    uri: Option<String>,
    #[serde(default)]
    artists: Vec<RawArtist>,
    #[serde(default)]
    album: Option<RawAlbum>,
    /// Piste locale (fichier du disque de l'utilisateur) : hors de portee de la
    /// telecommande Spotify, donc ignoree.
    #[serde(default)]
    is_local: bool,
}

#[derive(Debug, Deserialize)]
struct RawItem {
    #[serde(default)]
    track: Option<RawTrack>,
}

#[derive(Debug, Deserialize)]
struct RawTrackPage {
    #[serde(default)]
    items: Vec<RawItem>,
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    total: i64,
}

#[derive(Debug, Deserialize)]
struct RawPlaylist {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    images: Vec<RawImage>,
    #[serde(default)]
    tracks: Option<RawTrackPage>,
    #[serde(default)]
    owner: Option<RawOwner>,
}

#[derive(Debug, Deserialize)]
struct RawPlaylistPage {
    #[serde(default)]
    items: Vec<RawPlaylist>,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSearch {
    #[serde(default)]
    playlists: Option<RawPlaylistPage>,
}

#[derive(Debug, Deserialize)]
struct RawFeature {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    tempo: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawAudioFeatures {
    #[serde(default)]
    audio_features: Vec<Option<RawFeature>>,
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
}

// ---------------------------------------------------------------- OAuth

/// URL d'autorisation (OAuth 2.0 + PKCE, methode S256).
pub fn authorize_url(
    config: &Config,
    oauth_state: &str,
    code_challenge: &str,
) -> AppResult<String> {
    let client_id = config
        .spotify_client_id
        .clone()
        .filter(|_| config.spotify_configured())
        .ok_or_else(|| AppError::bad_request("Spotify n'est pas configure sur ce service"))?;
    let redirect_uri = config.spotify_redirect_uri();
    Ok(format!(
        "{AUTHORIZE_URL}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}\
         &code_challenge_method=S256&code_challenge={}&show_dialog=false",
        encode(&client_id),
        encode(&redirect_uri),
        encode(SCOPES),
        encode(oauth_state),
        encode(code_challenge)
    ))
}

/// Echange le code d'autorisation contre des jetons.
pub async fn exchange_code(
    http: &reqwest::Client,
    config: &Config,
    code: &str,
    verifier: &str,
) -> AppResult<TokenResponse> {
    let client_id = config.spotify_client_id.clone().unwrap_or_default();
    let client_secret = config.spotify_client_secret.clone().unwrap_or_default();
    let redirect_uri = config.spotify_redirect_uri();
    let response = http
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code_verifier", verifier),
        ])
        .send()
        .await?;
    token_response(response).await
}

/// Rafraichit un jeton d'acces arrive a expiration.
pub async fn refresh_token(
    http: &reqwest::Client,
    config: &Config,
    refresh_token: &str,
) -> AppResult<TokenResponse> {
    let client_id = config.spotify_client_id.clone().unwrap_or_default();
    let client_secret = config.spotify_client_secret.clone().unwrap_or_default();
    let response = http
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
        ])
        .send()
        .await?;
    token_response(response).await
}

async fn token_response(response: reqwest::Response) -> AppResult<TokenResponse> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!(
            "Spotify a refuse la demande de jeton (HTTP {}): {}",
            status.as_u16(),
            truncate(&body)
        )));
    }
    Ok(response.json::<TokenResponse>().await?)
}

// ---------------------------------------------------------------- API Web

/// Playlists de l'utilisateur (toutes pages confondues).
pub async fn list_user_playlists(
    http: &reqwest::Client,
    access_token: &str,
) -> AppResult<Vec<PlaylistRef>> {
    let mut url = format!("{API_BASE}/me/playlists?limit=50");
    let mut playlists = Vec::new();
    for _ in 0..MAX_PAGES {
        let page: RawPlaylistPage = get_json(http, &url, access_token).await?;
        playlists.extend(page.items.iter().filter_map(playlist_ref));
        match page.next {
            Some(next) => url = next,
            None => break,
        }
    }
    Ok(playlists)
}

/// Fiche complete d'une playlist (tous les titres, pages suivies).
pub async fn get_playlist(
    http: &reqwest::Client,
    access_token: &str,
    id: &str,
) -> AppResult<PlaylistDetail> {
    let url = format!("{API_BASE}/playlists/{id}");
    let raw: RawPlaylist = get_json(http, &url, access_token).await?;

    let mut tracks = Vec::new();
    let mut next = None;
    if let Some(page) = &raw.tracks {
        tracks.extend(page.items.iter().filter_map(track_ref));
        next = page.next.clone();
    }
    let mut pages = 0;
    while let Some(page_url) = next.take() {
        if pages >= MAX_PAGES {
            tracing::warn!(playlist = %id, "playlist Spotify tronquee a la pagination maximale");
            break;
        }
        pages += 1;
        let page: RawTrackPage = get_json(http, &page_url, access_token).await?;
        tracks.extend(page.items.iter().filter_map(track_ref));
        next = page.next;
    }

    Ok(PlaylistDetail {
        id: raw.id.clone().unwrap_or_else(|| id.to_string()),
        name: raw
            .name
            .clone()
            .unwrap_or_else(|| "Playlist Spotify".to_string()),
        cover_url: raw.images.first().and_then(|image| image.url.clone()),
        tracks,
    })
}

/// Recherche de playlists (20 premiers resultats).
pub async fn search_playlists(
    http: &reqwest::Client,
    access_token: &str,
    query: &str,
) -> AppResult<Vec<PlaylistRef>> {
    let response = http
        .get(format!("{API_BASE}/search"))
        .bearer_auth(access_token)
        .query(&[("q", query), ("type", "playlist"), ("limit", "20")])
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!(
            "recherche Spotify refusee (HTTP {}): {}",
            status.as_u16(),
            truncate(&body)
        )));
    }
    let payload: RawSearch = response.json().await?;
    Ok(payload
        .playlists
        .map(|page| page.items.iter().filter_map(playlist_ref).collect())
        .unwrap_or_default())
}

/// Tempo des pistes demandees ; `Ok(None)` quand l'endpoint est refuse.
///
/// Un refus (403/404) est le cas **normal** depuis le 27/11/2024 : il ne doit
/// jamais faire echouer un import de playlist.
pub async fn audio_features(
    http: &reqwest::Client,
    access_token: &str,
    ids: &[String],
) -> AppResult<Option<Vec<AudioFeature>>> {
    if ids.is_empty() {
        return Ok(None);
    }
    let mut features = Vec::new();
    for chunk in ids.chunks(100) {
        let joined = chunk.join(",");
        let response = http
            .get(format!("{API_BASE}/audio-features"))
            .bearer_auth(access_token)
            .query(&[("ids", joined.as_str())])
            .send()
            .await?;
        let status = response.status();
        if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::NOT_FOUND {
            tracing::info!(
                code = status.as_u16(),
                "audio-features indisponible : le BPM restera inconnu"
            );
            return Ok(None);
        }
        if !status.is_success() {
            return Err(AppError::internal(format!(
                "audio-features Spotify HTTP {}",
                status.as_u16()
            )));
        }
        let payload: RawAudioFeatures = response.json().await?;
        for feature in payload.audio_features.into_iter().flatten() {
            if let (Some(id), Some(tempo)) = (feature.id, feature.tempo) {
                features.push(AudioFeature { id, tempo });
            }
        }
    }
    Ok(Some(features))
}

/// Profil du compte lie (nom affiche dans l'interface).
pub async fn current_user(http: &reqwest::Client, access_token: &str) -> AppResult<Profile> {
    let profile: RawProfile = get_json(http, &format!("{API_BASE}/me"), access_token).await?;
    Ok(Profile {
        id: profile.id,
        display_name: profile.display_name,
    })
}

// ---------------------------------------------------------------- utilitaires

/// Identifiant de playlist depuis une URL Spotify, une URI ou une reference brute.
pub fn playlist_id_from_ref(reference: &str) -> Option<String> {
    let trimmed = reference.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = if let Some(rest) = trimmed.strip_prefix("spotify:playlist:") {
        rest
    } else if let Some((_, rest)) = trimmed.split_once("/playlist/") {
        rest.split(['?', '#', '/']).next().unwrap_or(rest)
    } else if trimmed.contains('/') || trimmed.contains(':') {
        // URL d'un autre domaine ou URI d'un autre type : on refuse plutot que
        // d'inventer un identifiant.
        return None;
    } else {
        trimmed
    };
    let cleaned: String = candidate
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    (cleaned.len() >= 10).then_some(cleaned)
}

fn playlist_ref(raw: &RawPlaylist) -> Option<PlaylistRef> {
    let id = raw.id.clone()?;
    if id.is_empty() {
        return None;
    }
    Some(PlaylistRef {
        id,
        name: raw
            .name
            .clone()
            .unwrap_or_else(|| "Playlist sans nom".to_string()),
        track_count: raw.tracks.as_ref().map(|tracks| tracks.total).unwrap_or(0),
        cover_url: raw.images.first().and_then(|image| image.url.clone()),
        owner: raw
            .owner
            .as_ref()
            .and_then(|owner| owner.display_name.clone()),
    })
}

fn track_ref(item: &RawItem) -> Option<TrackRef> {
    let track = item.track.as_ref()?;
    if track.is_local {
        return None;
    }
    Some(TrackRef {
        title: track
            .name
            .clone()
            .unwrap_or_else(|| "Titre inconnu".to_string()),
        artist: track.artists.first().and_then(|artist| artist.name.clone()),
        album: track.album.as_ref().and_then(|album| album.name.clone()),
        duration_s: track.duration_ms.map(|ms| ms as f64 / 1000.0),
        spotify_uri: track.uri.clone(),
        spotify_id: track.id.clone(),
    })
}

async fn get_json<T: serde::de::DeserializeOwned>(
    http: &reqwest::Client,
    url: &str,
    access_token: &str,
) -> AppResult<T> {
    let response = http.get(url).bearer_auth(access_token).send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::internal(format!(
            "Spotify a repondu {} pour {}: {}",
            status.as_u16(),
            url,
            truncate(&body)
        )));
    }
    Ok(response.json::<T>().await?)
}

/// Encodage pourcent d'une valeur de parametre d'URL (RFC 3986).
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Tronque un corps d'erreur : il ne doit pas polluer les journaux.
fn truncate(body: &str) -> String {
    body.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_reference_accepts_links_uris_and_ids() {
        let id = "37i9dQZF1DXcBWIGoYBM5M";
        assert_eq!(
            playlist_id_from_ref(&format!("https://open.spotify.com/playlist/{id}?si=abc")),
            Some(id.to_string())
        );
        assert_eq!(
            playlist_id_from_ref(&format!("spotify:playlist:{id}")),
            Some(id.to_string())
        );
        assert_eq!(playlist_id_from_ref(id), Some(id.to_string()));
        assert_eq!(
            playlist_id_from_ref("https://exemple.org/playlist/abc"),
            None
        );
        assert_eq!(playlist_id_from_ref("court"), None);
        assert_eq!(playlist_id_from_ref("   "), None);
    }

    #[test]
    fn authorize_url_carries_pkce_parameters() {
        let mut config = Config::for_tests("https://mpacer.exemple.org", "postgresql://exemple");
        config.spotify_client_id = Some("client-123".to_string());
        config.spotify_client_secret = Some("secret-123".to_string());
        let url = authorize_url(&config, "etat-1", "defi-1").expect("URL construite");
        assert!(url.starts_with(AUTHORIZE_URL), "{url}");
        assert!(url.contains("client_id=client-123"), "{url}");
        assert!(url.contains("code_challenge=defi-1"), "{url}");
        assert!(url.contains("code_challenge_method=S256"), "{url}");
        assert!(
            url.contains(
                "redirect_uri=https%3A%2F%2Fmpacer.exemple.org%2Fauth%2Fspotify%2Fcallback"
            ),
            "{url}"
        );
        assert!(url.contains("playlist-read-private"), "{url}");
        assert!(url.contains("state=etat-1"), "{url}");
    }

    #[test]
    fn authorize_url_refuses_an_unconfigured_service() {
        let config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
        assert!(authorize_url(&config, "etat", "defi").is_err());
    }

    #[test]
    fn playlist_payloads_are_mapped_to_metadata_only() {
        let raw: RawPlaylist = serde_json::from_str(
            r#"{
                "id": "p1",
                "name": "Run 170",
                "images": [{ "url": "https://img/cover.jpg" }],
                "tracks": { "total": 2, "items": [
                    { "track": {
                        "id": "t1", "name": "Wake me up", "duration_ms": 249000,
                        "uri": "spotify:track:t1",
                        "artists": [{ "name": "Avicii" }],
                        "album": { "name": "True" }
                    }},
                    { "track": {
                        "id": "local", "name": "Fichier local", "is_local": true,
                        "artists": [], "duration_ms": 1000
                    }}
                ]}
            }"#,
        )
        .expect("charge utile Spotify lue");

        let reference = playlist_ref(&raw).expect("reference");
        assert_eq!(reference.name, "Run 170");
        assert_eq!(reference.track_count, 2);
        assert_eq!(
            reference.cover_url.as_deref(),
            Some("https://img/cover.jpg")
        );

        let page = raw.tracks.as_ref().expect("page de titres");
        let tracks: Vec<TrackRef> = page.items.iter().filter_map(track_ref).collect();
        assert_eq!(tracks.len(), 1, "une piste locale est ignoree");
        assert_eq!(tracks[0].title, "Wake me up");
        assert_eq!(tracks[0].artist.as_deref(), Some("Avicii"));
        assert_eq!(tracks[0].album.as_deref(), Some("True"));
        assert_eq!(tracks[0].duration_s, Some(249.0));
        assert_eq!(tracks[0].spotify_id.as_deref(), Some("t1"));
    }

    #[test]
    fn audio_features_payload_tolerates_nulls() {
        let payload: RawAudioFeatures = serde_json::from_str(
            r#"{"audio_features":[{"id":"t1","tempo":172.5},null,{"id":"t2","tempo":null}]}"#,
        )
        .expect("charge utile audio-features lue");
        let tempos: Vec<(String, f64)> = payload
            .audio_features
            .into_iter()
            .flatten()
            .filter_map(|feature| feature.id.zip(feature.tempo))
            .collect();
        assert_eq!(tempos, vec![("t1".to_string(), 172.5)]);
    }
}
