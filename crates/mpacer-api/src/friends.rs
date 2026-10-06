//! Amis et partage de la position en direct.
//!
//! Le suivi en direct (docs/10) publie la position d'une montre sur un broker
//! MQTT, et le service garde en memoire la trace de chaque appareil
//! ([crate::live]). Ce module ajoute la seule chose qui manquait : **a qui**
//! appartient un appareil, et **qui** a le droit de voir la trace.
//!
//! ```text
//! montre/telephone                backend                        navigateur / ami
//!   |-- POST /live/register ------>|                              |
//!   |   (jeton d'appareil, "montre-a1b2")                         |
//!   |-- PUBLISH mpacer/live/... -->|  LiveStore (memoire)         |
//!                                  |<-- GET /api/v1/friends/live -|
//!                                  |--> position, si l'ami partage|
//! ```
//!
//! Trois regles, volontairement simples :
//!
//! 1. **Une amitie est mutuelle** et se noue en saisissant un code d'invitation
//!    a usage unique, transmis de la main a la main (comme le code d'appairage
//!    de la montre : aucun annuaire, aucun import de contacts).
//! 2. **Le partage se coupe** d'un interrupteur par compte (`users.share_live`).
//!    Coupe, aucune position n'est lue, meme par un ami.
//! 3. **Seule une seance en cours est partagee** : une trace terminee, un
//!    appareil muet depuis cinq minutes, ou un appareil jamais revendique ne
//!    sortent jamais du serveur.
//!
//! Rien de tout cela ne touche aux seances archivees : l'historique continue de
//! s'envoyer par `POST /api/v1/workouts`, et un ami ne voit jamais que la
//! position du moment.

use serde::Serialize;

use crate::auth::device::ALPHABET;
use crate::live::{LiveSessionView, LiveStore};
use crate::models::{FriendRow, LiveDeviceRow};
use crate::state::AppState;

/// Duree de validite d'un code d'invitation (24 h).
pub const INVITE_TTL_MS: i64 = 24 * 60 * 60 * 1000;

/// Nombre maximal d'amis par compte : au-dela, le cercle n'est plus un cercle,
/// et la page comme l'API deviendraient couteuses a rendre.
pub const MAX_FRIENDS: i64 = 200;

/// Nombre maximal d'appareils en direct par compte (telephone + montre + essais).
pub const MAX_DEVICES: usize = 8;

/// Age maximal d'une position encore consideree comme « en direct ».
///
/// Cinq minutes : au-dela, le coureur a coupe, la batterie est morte ou le
/// reseau est tombe ; afficher la derniere position comme si elle etait vivante
/// serait mensonger.
pub const LIVE_FRESH_MS: i64 = 5 * 60 * 1000;

/// Longueur maximale d'un nom de montre revendique (sujet MQTT).
pub const DEVICE_MAX_CHARS: usize = 32;

/// Genere un code d'invitation lisible du type `BCDF-GHJK`.
///
/// Meme alphabet que l'appairage de la montre : sans O/0 ni I/1, il se dicte au
/// telephone sans ambiguite.
pub fn generate_invite_code() -> String {
    let mut raw = String::with_capacity(8);
    let mut remaining = 8;
    while remaining > 0 {
        for byte in uuid::Uuid::new_v4().as_bytes() {
            if remaining == 0 {
                break;
            }
            raw.push(ALPHABET[(*byte as usize) % ALPHABET.len()] as char);
            remaining -= 1;
        }
    }
    format!("{}-{}", &raw[..4], &raw[4..])
}

/// Lien a envoyer a la personne a ajouter.
///
/// Il ouvre la page Amis avec le code deja saisi : un clic, et l'amitie est
/// faite (l'application telephone affiche le meme code, a recopier ou coller).
pub fn invite_url(public_url: &str, code: &str) -> String {
    format!(
        "{}/amis?code={}",
        public_url.trim_end_matches('/'),
        format_invite_code(code)
    )
}

/// Forme affichee d'un code normalise : `BCDFGHJK` -> `BCDF-GHJK`.
///
/// C'est cette forme que l'utilisateur lit et dicte ; la base ne stocke que la
/// forme normalisee, comme pour les codes d'appairage.
pub fn format_invite_code(normalized: &str) -> String {
    if normalized.len() <= 4 {
        return normalized.to_string();
    }
    format!("{}-{}", &normalized[..4], &normalized[4..])
}

/// Normalise une saisie : majuscules, sans separateurs (`bcdf ghjk` -> `BCDFGHJK`).
///
/// La comparaison en base se fait sur la forme normalisee, comme pour les codes
/// d'appairage : l'utilisateur peut dicter le code avec ou sans tiret.
pub fn normalize_invite_code(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Vrai si la saisie a la forme d'un code d'invitation.
pub fn is_invite_code(input: &str) -> bool {
    let propre = normalize_invite_code(input);
    propre.len() == 8 && propre.bytes().all(|b| ALPHABET.contains(&b))
}

/// Nom d'appareil normalise pour le sujet MQTT : lettres, chiffres, tirets.
///
/// Le nom vient de l'utilisateur (reglages de la montre) : on le borne et on le
/// nettoie, car il finit dans un sujet MQTT et dans une cle primaire.
pub fn clean_device_name(input: &str) -> String {
    input
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .take(DEVICE_MAX_CHARS)
        .collect()
}

/// Resultat de l'acceptation d'un code d'invitation.
#[derive(Debug, Clone, PartialEq)]
pub enum InviteOutcome {
    /// Amitie creee (ou deja existante) : voici la fiche de l'ami.
    Accepted(Box<FriendRow>),
    /// Code inconnu, deja utilise, ou jamais emis.
    Unknown,
    /// Code reconnu mais expire.
    Expired,
    /// L'utilisateur a saisi son propre code.
    SelfInvite,
    /// Le cercle est plein ([MAX_FRIENDS]).
    TooMany,
}

/// Position d'un ami pendant sa seance.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FriendLive {
    /// Nom de l'appareil qui publie ("montre-a1b2", "pixel-8").
    pub device: String,
    /// Etat de la seance : `arm`, `run` ou `pause`.
    pub state: String,
    pub lat: f64,
    pub lon: f64,
    pub accuracy_m: Option<f64>,
    /// Horodatage de la derniere position recue (ms epoch).
    pub last_ms: i64,
    /// Age de cette position (s) : l'interface ecrit « il y a 12 s ».
    pub age_s: i64,
    pub distance_m: Option<f64>,
    pub pace_s_per_km: Option<f64>,
    pub heart_rate_bpm: Option<i64>,
    pub battery_percent: Option<i64>,
    pub lap: Option<i64>,
    /// Trace de la seance en cours, sous-echantillonnee : `[[lat, lon], ...]`.
    pub trace: Vec<[f64; 2]>,
}

impl FriendLive {
    /// Position partageable a partir d'une session en direct.
    ///
    /// Renvoie `None` si la seance est terminee (`stop`), si la derniere
    /// position est trop vieille, ou si la trace est vide : dans les trois cas,
    /// il n'y a rien d'honnete a montrer.
    pub fn from_session(session: &LiveSessionView, device: &str, now_ms: i64) -> Option<Self> {
        let dernier = session.last.as_ref()?;
        if !session.is_live(now_ms) || session.state == "stop" {
            return None;
        }
        if now_ms - session.last_ms > LIVE_FRESH_MS {
            return None;
        }
        Some(Self {
            device: device.to_string(),
            state: session.state.clone(),
            lat: dernier.lat,
            lon: dernier.lon,
            accuracy_m: dernier.accuracy_m,
            last_ms: session.last_ms,
            age_s: ((now_ms - session.last_ms).max(0)) / 1000,
            distance_m: session.distance_m,
            pace_s_per_km: session.pace_s_per_km,
            heart_rate_bpm: dernier.heart_rate_bpm,
            battery_percent: dernier.battery_percent,
            lap: dernier.lap,
            trace: session
                .trace
                .iter()
                .map(|point| [point.lat, point.lon])
                .collect(),
        })
    }
}

/// Un ami du cercle, avec sa position quand elle est partageable.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FriendView {
    pub id: String,
    pub name: String,
    pub email: String,
    pub picture_url: Option<String>,
    /// Date de l'amitie (ms epoch).
    pub since_ms: i64,
    /// Vrai si cet ami partage sa position en direct.
    pub sharing: bool,
    /// Position courante, `None` s'il ne partage pas ou ne court pas.
    pub live: Option<FriendLive>,
}

/// Assemble le cercle : profils, interrupteurs et positions courantes.
///
/// Fonction pure : elle ne fait que croiser des lignes deja lues et le magasin
/// en direct, ce qui la rend testable sans base de donnees.
pub fn circle(
    friends: &[FriendRow],
    devices: &[LiveDeviceRow],
    live: &LiveStore,
    now_ms: i64,
) -> Vec<FriendView> {
    friends
        .iter()
        .map(|ami| {
            let siens: Vec<&LiveDeviceRow> = devices
                .iter()
                .filter(|appareil| appareil.user_id == ami.id)
                .collect();
            let position = if ami.share_live {
                // L'appareil le plus recent gagne : un telephone et une montre
                // peuvent publier en meme temps, on montre la seance du moment.
                siens
                    .iter()
                    .filter_map(|appareil| {
                        live.session(&appareil.device).and_then(|session| {
                            FriendLive::from_session(&session, &appareil.device, now_ms)
                        })
                    })
                    .max_by_key(|position| position.last_ms)
            } else {
                None
            };
            FriendView {
                id: ami.id.clone(),
                name: ami.display_name(),
                email: ami.email.clone(),
                picture_url: ami.picture_url.clone(),
                since_ms: ami.since_ms,
                sharing: ami.share_live,
                live: position,
            }
        })
        .collect()
}

/// Nombre d'amis qui partagent une position en direct (bandeau de la page).
pub fn live_count(cercle: &[FriendView]) -> usize {
    cercle.iter().filter(|ami| ami.live.is_some()).count()
}

/// Cercle d'un compte : amis, appareils revendiques et positions courantes.
///
/// Seule fonction du module qui touche la base ; le reste est pur et testable
/// sans PostgreSQL.
pub async fn circle_for(state: &AppState, user_id: &str) -> Result<Vec<FriendView>, sqlx::Error> {
    let amis = crate::db::list_friends(&state.pool, user_id).await?;
    // Les appareils de l'utilisateur sont lus aussi : sa propre position sert a
    // se situer sur la carte, a cote de ses amis.
    let mut ids: Vec<String> = amis.iter().map(|ami| ami.id.clone()).collect();
    ids.push(user_id.to_string());
    let appareils = crate::db::list_live_devices(&state.pool, &ids).await?;
    Ok(circle(&amis, &appareils, &state.live, state.now_ms()))
}

/// Position en direct d'un compte, tous appareils confondus.
pub async fn own_live(state: &AppState, user_id: &str) -> Result<Option<FriendLive>, sqlx::Error> {
    let appareils = crate::db::list_live_devices(&state.pool, &[user_id.to_string()]).await?;
    let now_ms = state.now_ms();
    Ok(appareils
        .iter()
        .filter_map(|appareil| {
            state
                .live
                .session(&appareil.device)
                .and_then(|session| FriendLive::from_session(&session, &appareil.device, now_ms))
        })
        .max_by_key(|position| position.last_ms))
}

/// Charge utile partagee par l'API et la page Amis.
///
/// @param with_trace inclure la trace de la seance (carte) ; sans elle, la
///   reponse ne pese que quelques centaines d'octets par ami, ce qui permet un
///   rafraichissement toutes les dix secondes depuis un telephone.
#[derive(Debug, Clone, Serialize)]
pub struct CirclePayload {
    pub now_ms: i64,
    /// Adresse publique du service : l'application s'en sert pour ouvrir la
    /// carte du cercle dans un navigateur si besoin.
    pub public_url: String,
    /// Vrai si le compte partage sa position.
    pub share_live: bool,
    pub total: usize,
    /// Nombre d'amis actuellement en direct.
    pub live: usize,
    /// Ma propre position, pour se situer sur la carte.
    pub me: Option<FriendLive>,
    pub friends: Vec<FriendView>,
}

/// Assemble la charge utile d'un compte.
pub async fn payload(
    state: &AppState,
    user_id: &str,
    share_live: bool,
    with_trace: bool,
) -> Result<CirclePayload, sqlx::Error> {
    let mut amis = circle_for(state, user_id).await?;
    if !with_trace {
        for ami in &mut amis {
            if let Some(position) = ami.live.as_mut() {
                position.trace.clear();
            }
        }
    }
    let mut moi = own_live(state, user_id).await?;
    if !with_trace {
        if let Some(position) = moi.as_mut() {
            position.trace.clear();
        }
    }
    Ok(CirclePayload {
        now_ms: state.now_ms(),
        public_url: state.config.public_url.trim_end_matches('/').to_string(),
        share_live,
        total: amis.len(),
        live: live_count(&amis),
        me: moi,
        friends: amis,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::LivePoint;

    fn ami(id: &str, partage: bool) -> FriendRow {
        FriendRow {
            id: id.to_string(),
            name: Some(format!("Coureur {id}")),
            email: format!("{id}@example.org"),
            picture_url: None,
            share_live: partage,
            since_ms: 1_000,
        }
    }

    fn appareil(device: &str, user: &str) -> LiveDeviceRow {
        LiveDeviceRow {
            device: device.to_string(),
            user_id: user.to_string(),
            label: None,
            last_seen_ms: 1_000,
        }
    }

    fn point(t_ms: i64, lat: f64, lon: f64, state: &str) -> LivePoint {
        LivePoint {
            t_ms,
            lat,
            lon,
            accuracy_m: Some(4.0),
            distance_m: Some(1_500.0),
            pace_s_per_km: Some(300.0),
            heart_rate_bpm: Some(148),
            lap: Some(2),
            battery_percent: Some(76),
            state: Some(state.to_string()),
            device: None,
        }
    }

    fn charge(point: &LivePoint) -> Vec<u8> {
        serde_json::to_vec(point).unwrap()
    }

    #[test]
    fn invite_codes_are_readable_and_normalised() {
        let code = generate_invite_code();
        assert_eq!(code.len(), 9, "8 caracteres et un tiret");
        assert_eq!(code.chars().nth(4), Some('-'), "tiret au milieu : {code}");
        assert!(is_invite_code(&code), "code reconnu : {code}");
        // Dicte a voix haute, avec des espaces ou en minuscules.
        assert!(is_invite_code("bcdf ghjk"));
        assert_eq!(normalize_invite_code(&code), code.replace('-', ""));
        assert_eq!(normalize_invite_code(" bcdf-ghjk "), "BCDFGHJK");
    }

    #[test]
    fn anything_that_is_not_a_code_is_refused() {
        assert!(!is_invite_code(""));
        assert!(!is_invite_code("ABC"));
        assert!(!is_invite_code("BCDFGHJKLM"), "trop long");
        assert!(
            !is_invite_code("BCDFGHJ0"),
            "0 n'appartient pas a l'alphabet"
        );
        assert!(
            !is_invite_code("BCDFGHJI"),
            "I n'appartient pas a l'alphabet"
        );
    }

    #[test]
    fn device_names_are_cleaned_and_bounded() {
        assert_eq!(clean_device_name("  montre-a1b2  "), "montre-a1b2");
        assert_eq!(clean_device_name("mpacer/live/montre"), "mpacerlivemontre");
        assert_eq!(clean_device_name("a b c"), "abc");
        assert_eq!(clean_device_name(&"x".repeat(80)).len(), DEVICE_MAX_CHARS);
        assert_eq!(clean_device_name(""), "");
    }

    #[test]
    fn a_running_friend_is_shown_with_its_position() {
        let live = LiveStore::new();
        live.ingest(
            "mpacer/live/montre-a1b2",
            &charge(&point(1_000, 48.85, 2.35, "run")),
        )
        .unwrap();
        let cercle = circle(
            &[ami("u1", true)],
            &[appareil("montre-a1b2", "u1")],
            &live,
            12_000,
        );
        let position = cercle[0].live.as_ref().expect("position partagee");
        assert_eq!(position.device, "montre-a1b2");
        assert_eq!(position.lat, 48.85);
        assert_eq!(position.state, "run");
        assert_eq!(position.age_s, 11);
        assert_eq!(position.distance_m, Some(1_500.0));
        assert_eq!(position.heart_rate_bpm, Some(148));
        assert_eq!(position.trace, vec![[48.85, 2.35]]);
        assert_eq!(live_count(&cercle), 1);
    }

    #[test]
    fn a_friend_who_stopped_sharing_has_no_position() {
        let live = LiveStore::new();
        live.ingest(
            "mpacer/live/montre-a1b2",
            &charge(&point(1_000, 48.85, 2.35, "run")),
        )
        .unwrap();
        let cercle = circle(
            &[ami("u1", false)],
            &[appareil("montre-a1b2", "u1")],
            &live,
            12_000,
        );
        assert!(!cercle[0].sharing);
        assert!(cercle[0].live.is_none());
        assert_eq!(live_count(&cercle), 0);
    }

    #[test]
    fn a_finished_session_is_not_shared() {
        let live = LiveStore::new();
        live.ingest(
            "mpacer/live/montre-a1b2",
            &charge(&point(1_000, 48.85, 2.35, "run")),
        )
        .unwrap();
        live.ingest(
            "mpacer/live/montre-a1b2",
            &charge(&point(2_000, 48.86, 2.36, "stop")),
        )
        .unwrap();
        let cercle = circle(
            &[ami("u1", true)],
            &[appareil("montre-a1b2", "u1")],
            &live,
            3_000,
        );
        assert!(cercle[0].live.is_none(), "une seance terminee ne sort pas");
    }

    #[test]
    fn a_silent_device_is_not_shared() {
        let live = LiveStore::new();
        live.ingest(
            "mpacer/live/montre-a1b2",
            &charge(&point(1_000, 48.85, 2.35, "run")),
        )
        .unwrap();
        let cercle = circle(
            &[ami("u1", true)],
            &[appareil("montre-a1b2", "u1")],
            &live,
            1_000 + LIVE_FRESH_MS + 1,
        );
        assert!(cercle[0].live.is_none(), "position trop vieille");
    }

    #[test]
    fn only_the_claimed_devices_of_the_friend_are_read() {
        let live = LiveStore::new();
        // La montre d'un autre compte publie sous le meme prefixe : elle ne doit
        // jamais apparaitre dans le cercle, faute d'etre revendiquee par l'ami.
        live.ingest(
            "mpacer/live/montre-x",
            &charge(&point(1_000, 40.0, 3.0, "run")),
        )
        .unwrap();
        let cercle = circle(
            &[ami("u1", true)],
            &[appareil("montre-a1b2", "u1")],
            &live,
            2_000,
        );
        assert!(cercle[0].live.is_none());
    }

    #[test]
    fn the_most_recent_device_wins() {
        let live = LiveStore::new();
        live.ingest(
            "mpacer/live/montre",
            &charge(&point(1_000, 48.0, 2.0, "pause")),
        )
        .unwrap();
        live.ingest(
            "mpacer/live/telephone",
            &charge(&point(5_000, 49.0, 3.0, "run")),
        )
        .unwrap();
        let cercle = circle(
            &[ami("u1", true)],
            &[appareil("montre", "u1"), appareil("telephone", "u1")],
            &live,
            6_000,
        );
        let position = cercle[0].live.as_ref().unwrap();
        assert_eq!(position.device, "telephone");
        assert_eq!(position.lat, 49.0);
        assert_eq!(position.state, "run");
    }
}
