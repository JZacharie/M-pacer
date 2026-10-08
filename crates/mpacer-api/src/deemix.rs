//! Client de l'instance **Deemix** : mettre les MP3 en file de telechargement.
//!
//! Deezer ne fournit pas les fichiers ; l'instance Deemix de l'utilisateur s'en
//! charge. M-pacer ne telecharge donc rien lui-meme : il **envoie** une playlist
//! ou une piste dans la file de Deemix, puis lit la file pour dire ou en est le
//! telechargement. Les fichiers arrivent dans le dossier `downloads` de
//! l'instance (volume NFS cote serveur), que `mpacer-music` apparie ensuite avec
//! le manifeste pour les copier sur la montre.
//!
//! Deemix n'a pas de documentation d'API : celle-ci est celle de son interface,
//! relevee et verifiee contre une instance reelle.
//!
//! | Methode | Chemin | Role |
//! |---|---|---|
//! | POST | `/api/loginArl` | ouvre la session Deezer de l'instance (`{"arl": "..."}`) |
//! | POST | `/api/addToQueue` | ajoute une reference Deezer (`{"url", "bitrate"}`) |
//! | GET | `/api/getQueue` | file de telechargement |
//!
//! Deux particularites :
//! * l'instance est protegee par une **authentification HTTP basique** (Traefik) :
//!   chaque appel porte `MPACER_DEEMIX_USER` / `MPACER_DEEMIX_PASSWORD` ;
//! * la session Deezer est portee par un **cookie** (`connect.sid`) : Deemix
//!   repond `NotLoggedIn` a `addToQueue` sans le cookie rendu par `loginArl`,
//!   meme quand le cookie `arl` est deja enregistre cote instance. Le service
//!   rouvre donc une session a chaque envoi.

use crate::config::Config;
use crate::error::{AppError, AppResult};
use serde::Deserialize;
use std::time::Duration;

/// Delai maximal accorde a un appel Deemix (instance locale, mais un arret ne
/// doit jamais bloquer la page /music).
pub const TIMEOUT: Duration = Duration::from_secs(8);

/// Reference Deezer d'une playlist, telle que Deemix l'accepte.
pub fn playlist_url(id: &str) -> String {
    format!("https://www.deezer.com/playlist/{id}")
}

/// Reference Deezer d'une piste, telle que Deemix l'accepte.
pub fn track_url(id: &str) -> String {
    format!("https://www.deezer.com/track/{id}")
}

/// Entree de la file de telechargement (vue simplifiee pour la page /music).
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueEntry {
    /// `track` | `album` | `playlist`.
    pub kind: String,
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    /// Nombre de fichiers attendus.
    pub size: i64,
    pub downloaded: i64,
    pub failed: i64,
    /// Progression annoncee par Deemix (0 a 100).
    pub progress: i64,
    pub errors: Vec<String>,
}

impl QueueEntry {
    /// Vrai quand tous les fichiers attendus sont arrives.
    pub fn is_done(&self) -> bool {
        self.size > 0 && self.downloaded >= self.size
    }

    /// Vrai quand Deemix a signale au moins un fichier en echec.
    pub fn has_failed(&self) -> bool {
        self.failed > 0 || !self.errors.is_empty()
    }
}

/// Ajoute une reference Deezer a la file de l'instance.
///
/// Renvoie le nombre d'entrees creees : Deemix accepte la demande
/// (`result: true`) mais peut ne rien ajouter quand la reference est deja en
/// file.
pub async fn add_to_queue(http: &reqwest::Client, config: &Config, url: &str) -> AppResult<usize> {
    let cookie = login(http, config).await?;
    let response = post_json(
        http,
        config,
        "addToQueue",
        &serde_json::json!({ "url": url, "bitrate": serde_json::Value::Null }),
        cookie.as_deref(),
    )
    .await?;
    let payload: RawAddResult = serde_json::from_value(response)
        .map_err(|error| AppError::internal(format!("reponse Deemix inattendue : {error}")))?;
    if !payload.result {
        let reason = payload
            .errid
            .unwrap_or_else(|| "raison inconnue".to_string());
        return Err(AppError::internal(format!(
            "Deemix a refuse la demande ({reason})"
        )));
    }
    Ok(payload
        .data
        .and_then(|data| data.obj)
        .map(|entries| entries.len())
        .unwrap_or(0))
}

/// File de telechargement de l'instance, dans l'ordre annonce par Deemix.
pub async fn queue(http: &reqwest::Client, config: &Config) -> AppResult<Vec<QueueEntry>> {
    let response = get_json(http, config, "getQueue").await?;
    let payload: RawQueue = serde_json::from_value(response)
        .map_err(|error| AppError::internal(format!("reponse Deemix inattendue : {error}")))?;
    Ok(payload
        .queue
        .unwrap_or_default()
        .into_iter()
        .map(|(uuid, item)| QueueEntry {
            kind: item.kind.unwrap_or_else(|| "track".to_string()),
            id: item.id.unwrap_or(uuid),
            title: item.title.unwrap_or_else(|| "Sans titre".to_string()),
            artist: item.artist,
            size: item.size.unwrap_or(0),
            downloaded: item.downloaded.unwrap_or(0),
            failed: item.failed.unwrap_or(0),
            progress: item.progress.unwrap_or(0),
            errors: item.errors.unwrap_or_default(),
        })
        .collect())
}

/// Ouvre une session Deezer sur l'instance et rend le cookie `connect.sid`.
///
/// Sans cookie `arl` configure cote service, la session de l'instance est
/// utilisee telle quelle (`None`) : elle suffit quand quelqu'un s'est connecte
/// a la main dans l'interface Deemix.
async fn login(http: &reqwest::Client, config: &Config) -> AppResult<Option<String>> {
    let Some(arl) = config
        .deezer_arl
        .as_deref()
        .map(str::trim)
        .filter(|arl| !arl.is_empty())
    else {
        return Ok(None);
    };
    let response = post_response(
        http,
        config,
        "loginArl",
        &serde_json::json!({ "arl": arl }),
        None,
    )
    .await?;
    let cookie = session_cookie(&response);
    let body = response.text().await.unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(body.trim())
        .map_err(|error| AppError::internal(format!("reponse Deemix illisible : {error}")))?;
    // L'instance repond `{"status":1,...}` quand la session est ouverte ; un
    // mot de passe Deezer change fait repondre une erreur applicative.
    if value.get("status").and_then(serde_json::Value::as_i64) != Some(1) {
        let reason = value
            .get("error")
            .or_else(|| value.get("errid"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("cookie arl refuse");
        return Err(AppError::internal(format!(
            "Deemix n'a pas ouvert de session Deezer : {reason}"
        )));
    }
    Ok(cookie)
}

/// Cookie de session rendu par une reponse (`connect.sid`).
fn session_cookie(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|header| {
            let pair = header.split(';').next()?.trim();
            let (name, _) = pair.split_once('=')?;
            name.trim()
                .eq_ignore_ascii_case("connect.sid")
                .then(|| pair.to_string())
        })
}

async fn get_json(
    http: &reqwest::Client,
    config: &Config,
    endpoint: &str,
) -> AppResult<serde_json::Value> {
    let response = request(http, config, reqwest::Method::GET, endpoint)
        .send()
        .await
        .map_err(unreachable)?;
    read_json(response).await
}

async fn post_json(
    http: &reqwest::Client,
    config: &Config,
    endpoint: &str,
    body: &serde_json::Value,
    cookie: Option<&str>,
) -> AppResult<serde_json::Value> {
    let response = post_response(http, config, endpoint, body, cookie).await?;
    read_json(response).await
}

async fn post_response(
    http: &reqwest::Client,
    config: &Config,
    endpoint: &str,
    body: &serde_json::Value,
    cookie: Option<&str>,
) -> AppResult<reqwest::Response> {
    let mut request = request(http, config, reqwest::Method::POST, endpoint).json(body);
    if let Some(cookie) = cookie {
        request = request.header(reqwest::header::COOKIE, cookie);
    }
    request.send().await.map_err(unreachable)
}

/// Requete authentifiee vers l'instance (authentification basique + delai).
fn request(
    http: &reqwest::Client,
    config: &Config,
    method: reqwest::Method,
    endpoint: &str,
) -> reqwest::RequestBuilder {
    let builder = http
        .request(
            method,
            format!("{}/api/{endpoint}", config.deemix_base_url()),
        )
        .timeout(TIMEOUT);
    match (&config.deemix_user, &config.deemix_password) {
        (Some(user), Some(password)) => builder.basic_auth(user, Some(password)),
        _ => builder,
    }
}

/// Message d'erreur quand l'instance ne repond pas.
fn unreachable(error: reqwest::Error) -> AppError {
    AppError::internal(format!("Deemix est injoignable : {error}"))
}

/// Decode un corps JSON en refusant une reponse d'erreur HTTP.
async fn read_json(response: reqwest::Response) -> AppResult<serde_json::Value> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::internal(format!(
            "Deemix a refuse la demande (HTTP {})",
            status.as_u16()
        )));
    }
    serde_json::from_str(body.trim())
        .map_err(|error| AppError::internal(format!("reponse Deemix illisible : {error}")))
}

// ------------------------------------------------------------- reponses brutes

#[derive(Debug, Deserialize)]
struct RawAddResult {
    #[serde(default)]
    result: bool,
    #[serde(default)]
    errid: Option<String>,
    #[serde(default)]
    data: Option<RawAddData>,
}

#[derive(Debug, Deserialize)]
struct RawAddData {
    #[serde(default)]
    obj: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct RawQueue {
    #[serde(default)]
    queue: Option<std::collections::BTreeMap<String, RawQueueItem>>,
}

#[derive(Debug, Deserialize)]
struct RawQueueItem {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    size: Option<i64>,
    #[serde(default)]
    downloaded: Option<i64>,
    #[serde(default)]
    failed: Option<i64>,
    #[serde(default)]
    progress: Option<i64>,
    #[serde(default)]
    errors: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deezer_references_follow_the_urls_deemix_accepts() {
        assert_eq!(
            playlist_url("1924357302"),
            "https://www.deezer.com/playlist/1924357302"
        );
        assert_eq!(
            track_url("3402267041"),
            "https://www.deezer.com/track/3402267041"
        );
    }

    #[test]
    fn a_successful_add_reports_the_created_entries() {
        let payload: RawAddResult = serde_json::from_str(
            r#"{"result":true,"data":{"url":["https://www.deezer.com/track/1"],
                "bitrate":1,"obj":[{"type":"track","id":"1","title":"NEVER ENOUGH"}]}}"#,
        )
        .expect("reponse d'ajout valide");
        assert!(payload.result);
        assert_eq!(payload.data.and_then(|data| data.obj).unwrap().len(), 1);
    }

    #[test]
    fn a_refused_add_keeps_the_reason() {
        let payload: RawAddResult =
            serde_json::from_str(r#"{"result":false,"errid":"NotLoggedIn","data":{"url":["x"]}}"#)
                .expect("reponse de refus valide");
        assert!(!payload.result);
        assert_eq!(payload.errid.as_deref(), Some("NotLoggedIn"));
    }

    #[test]
    fn the_queue_is_read_with_its_progress() {
        let payload: RawQueue = serde_json::from_str(
            r#"{"queue":{
                "album_12752508_1":{"type":"album","id":"12752508","title":"Automatic For The People",
                    "artist":"R.E.M.","size":12,"downloaded":12,"failed":0,"progress":98,"errors":[]},
                "track_3402267041_1":{"type":"track","id":"3402267041","title":"NEVER ENOUGH",
                    "artist":"Turnstile","size":1,"downloaded":0,"failed":1,"progress":0,
                    "errors":["Fichier introuvable"]}
            }}"#,
        )
        .expect("file valide");
        let mut entries: Vec<QueueEntry> = payload
            .queue
            .unwrap_or_default()
            .into_iter()
            .map(|(uuid, item)| QueueEntry {
                kind: item.kind.unwrap_or_else(|| "track".to_string()),
                id: item.id.unwrap_or(uuid),
                title: item.title.unwrap_or_else(|| "Sans titre".to_string()),
                artist: item.artist,
                size: item.size.unwrap_or(0),
                downloaded: item.downloaded.unwrap_or(0),
                failed: item.failed.unwrap_or(0),
                progress: item.progress.unwrap_or(0),
                errors: item.errors.unwrap_or_default(),
            })
            .collect();
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Automatic For The People");
        assert!(entries[0].is_done());
        assert!(!entries[0].has_failed());
        assert_eq!(entries[0].progress, 98);
        assert!(!entries[1].is_done());
        assert!(entries[1].has_failed());
        assert_eq!(entries[1].artist.as_deref(), Some("Turnstile"));
    }

    #[test]
    fn a_session_cookie_is_taken_from_set_cookie() {
        // Le cookie compte : Deemix repond `NotLoggedIn` sans lui.
        let header = "connect.sid=s%3Aabc.def; Path=/; HttpOnly";
        let pair = header.split(';').next().unwrap().trim();
        let (name, _) = pair.split_once('=').unwrap();
        assert!(name.eq_ignore_ascii_case("connect.sid"));
        assert_eq!(pair, "connect.sid=s%3Aabc.def");
    }
}
