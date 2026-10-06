//! Suivi en direct : derniere position connue de chaque montre.
//!
//! La montre publie sa position sur un broker MQTT pendant la seance
//! (`<prefixe>/live/<montre>`) ; le service s'y abonne ([crate::mqtt]) et
//! conserve en memoire la trace de la seance en cours, ce qui alimente la page
//! `/live` et l'endpoint `/live.json`.
//!
//! Rien n'est ecrit en base : un suivi en direct est un etat volatil. Les
//! seances terminees, elles, arrivent par `POST /api/v1/workouts` et sont
//! archivees normalement. La memoire est bornee (un anneau de points par
//! montre, quelques centaines de kilo-octets au total) : le service ne peut pas
//! etre noye par un publisher trop bavard.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::mqtt::{self, Incoming, MqttOptions, MqttStatus};
use crate::state::AppState;

/// Nombre de points conserves par montre.
///
/// A une position toutes les dix secondes, 4096 points couvrent plus de onze
/// heures de course : au-dela, la trace n'a plus d'interet pour un suivi en
/// direct (le FIT et le `.pac` gardent la trace complete).
pub const MAX_POINTS: usize = 4096;

/// Nombre de points transmis a l'affichage : borne le cout de rendu.
pub const MAX_TRACE_POINTS: usize = 700;

/// Ecart au-dela duquel deux points appartiennent a deux seances differentes.
pub const SESSION_GAP_MS: i64 = 6 * 60 * 60 * 1000;

/// Duree de conservation d'une montre silencieuse.
pub const SESSION_TTL_MS: i64 = 24 * 60 * 60 * 1000;

/// Extension de sujet MQTT par defaut (le `+` couvre l'identifiant de montre).
pub const DEFAULT_TOPIC: &str = "mpacer/live/+";

/// Position publiee par la montre, au format JSON compact.
///
/// Les noms de champs sont courts : le message part sur le reseau toutes les
/// dix secondes, chaque octet compte pour la batterie de la montre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LivePoint {
    /// Horodatage de la mesure (ms epoch).
    #[serde(rename = "t")]
    pub t_ms: i64,
    pub lat: f64,
    pub lon: f64,
    /// Precision annoncee par le GPS (m).
    #[serde(rename = "acc", default, skip_serializing_if = "Option::is_none")]
    pub accuracy_m: Option<f64>,
    /// Distance parcourue depuis le depart (m).
    #[serde(rename = "dist", default, skip_serializing_if = "Option::is_none")]
    pub distance_m: Option<f64>,
    /// Allure lissee (s/km).
    #[serde(rename = "pace", default, skip_serializing_if = "Option::is_none")]
    pub pace_s_per_km: Option<f64>,
    /// Frequence cardiaque (bpm).
    #[serde(rename = "hr", default, skip_serializing_if = "Option::is_none")]
    pub heart_rate_bpm: Option<i64>,
    #[serde(rename = "lap", default, skip_serializing_if = "Option::is_none")]
    pub lap: Option<i64>,
    /// Niveau de batterie de la montre (%).
    #[serde(rename = "bat", default, skip_serializing_if = "Option::is_none")]
    pub battery_percent: Option<i64>,
    /// Etat de la seance : `arm`, `run`, `pause` ou `stop`.
    #[serde(rename = "st", default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Nom de la montre (sinon le dernier segment du sujet MQTT).
    #[serde(rename = "dev", default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

impl LivePoint {
    /// Verifie les bornes avant de conserver le point.
    pub fn is_valid(&self) -> bool {
        self.lat.is_finite()
            && self.lon.is_finite()
            && (-90.0..=90.0).contains(&self.lat)
            && (-180.0..=180.0).contains(&self.lon)
            && self.t_ms > 0
            && self.accuracy_m.is_none_or(|v| v.is_finite() && v >= 0.0)
    }
}

/// Trace en cours pour une montre.
#[derive(Debug, Clone)]
pub struct LiveSession {
    pub device: String,
    pub state: String,
    pub started_ms: i64,
    pub last_ms: i64,
    pub received: u64,
    points: VecDeque<LivePoint>,
}

impl LiveSession {
    fn new(device: &str, t_ms: i64, state: &str) -> Self {
        Self {
            device: device.to_string(),
            state: state.to_string(),
            started_ms: t_ms,
            last_ms: t_ms,
            received: 0,
            points: VecDeque::with_capacity(64),
        }
    }

    /// Faut-il repartir d'une trace vierge ?
    ///
    /// Une montre qui republie apres un `stop`, ou apres une longue
    /// interruption, commence une nouvelle seance.
    fn should_restart(&self, point: &LivePoint) -> bool {
        let etat = point.state.as_deref().unwrap_or("run");
        (self.state == "stop" && etat != "stop") || point.t_ms - self.last_ms > SESSION_GAP_MS
    }

    fn restart(&mut self, t_ms: i64, state: &str) {
        self.points.clear();
        self.state = state.to_string();
        self.started_ms = t_ms;
        self.last_ms = t_ms;
        self.received = 0;
    }

    fn push(&mut self, point: LivePoint) {
        if let Some(etat) = point.state.as_deref() {
            self.state = etat.to_string();
        }
        self.last_ms = self.last_ms.max(point.t_ms);
        self.received += 1;
        self.points.push_back(point);
        while self.points.len() > MAX_POINTS {
            self.points.pop_front();
        }
    }

    /// Trace sous-echantillonnee, pour un rendu borne.
    pub fn trace(&self) -> Vec<LivePoint> {
        let points: Vec<LivePoint> = self.points.iter().cloned().collect();
        if points.len() <= MAX_TRACE_POINTS {
            return points;
        }
        let pas = points.len().div_ceil(MAX_TRACE_POINTS);
        let mut sortie: Vec<LivePoint> = points.iter().step_by(pas).cloned().collect();
        // Le dernier point est le plus important (position courante) : on le
        // force, meme si le pas l'a saute.
        if let Some(dernier) = points.last() {
            if sortie.last().map(|p| p.t_ms) != Some(dernier.t_ms) {
                sortie.push(dernier.clone());
            }
        }
        sortie
    }
}

/// Resume d'une montre, pret a etre affiche.
#[derive(Debug, Clone, Serialize)]
pub struct LiveSessionView {
    pub device: String,
    pub state: String,
    pub started_ms: i64,
    pub last_ms: i64,
    pub received: u64,
    pub points: usize,
    pub trace: Vec<LivePoint>,
    pub distance_m: Option<f64>,
    pub duration_s: Option<f64>,
    pub pace_s_per_km: Option<f64>,
    pub last: Option<LivePoint>,
}

impl LiveSessionView {
    /// Vrai si la montre publiait encore il y a peu.
    pub fn is_live(&self, now_ms: i64) -> bool {
        self.state != "stop" && now_ms - self.last_ms < 5 * 60 * 1000
    }
}

/// Magasin des traces en direct, partage par tout le service.
#[derive(Debug)]
pub struct LiveStore {
    sessions: RwLock<HashMap<String, LiveSession>>,
    status: Arc<MqttStatus>,
}

impl Default for LiveStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveStore {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            status: Arc::new(MqttStatus::default()),
        }
    }

    /// Etat de la liaison avec le broker.
    pub fn status(&self) -> &Arc<MqttStatus> {
        &self.status
    }

    /// Enregistre un message MQTT (sujet + charge utile JSON).
    ///
    /// Renvoie une erreur lisible pour les messages inexploitables : ils sont
    /// ignores et journalises, jamais fatals.
    pub fn ingest(&self, topic: &str, payload: &[u8]) -> Result<(), String> {
        let point: LivePoint = serde_json::from_slice(payload)
            .map_err(|erreur| format!("JSON illisible : {erreur}"))?;
        if !point.is_valid() {
            return Err("position hors bornes".to_string());
        }
        let device = point
            .device
            .clone()
            .filter(|nom| !nom.is_empty())
            .unwrap_or_else(|| device_from_topic(topic));
        if device.is_empty() {
            return Err("montre inconnue (sujet MQTT sans identifiant)".to_string());
        }
        let etat = point.state.clone().unwrap_or_else(|| "run".to_string());
        // Le nom de la montre vit dans la session : le repetent sur chaque point,
        // il alourdirait l'anneau (24 octets x 4096) et le JSON de la page.
        let mut point = point;
        point.device = None;

        // Un verrou empoisonne ne doit pas arreter le suivi : la donnee est
        // volatile, on reprend l'ecriture.
        let mut sessions = self
            .sessions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let session = sessions
            .entry(device.clone())
            .or_insert_with(|| LiveSession::new(&device, point.t_ms, &etat));
        if session.should_restart(&point) {
            session.restart(point.t_ms, &etat);
        }
        session.push(point);
        Ok(())
    }

    /// Oublie les montres silencieuses depuis plus de [SESSION_TTL_MS].
    pub fn prune(&self, now_ms: i64) -> usize {
        let mut sessions = self
            .sessions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let avant = sessions.len();
        sessions.retain(|_, session| now_ms - session.last_ms <= SESSION_TTL_MS);
        avant - sessions.len()
    }

    /// Instantanes des montres, la plus recente d'abord.
    pub fn snapshot(&self, now_ms: i64, max_sessions: usize) -> Vec<LiveSessionView> {
        self.prune(now_ms);
        let sessions = self
            .sessions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut vues: Vec<LiveSessionView> = sessions.values().map(resume).collect();
        vues.sort_by_key(|vue| std::cmp::Reverse(vue.last_ms));
        vues.truncate(max_sessions);
        vues
    }

    /// Nombre de montres suivies.
    pub fn len(&self) -> usize {
        self.sessions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Resume affichable d'une trace.
fn resume(session: &LiveSession) -> LiveSessionView {
    let trace = session.trace();
    let distance_m = session
        .points
        .iter()
        .filter_map(|point| point.distance_m)
        .next_back();
    let duree_s = (session.last_ms - session.started_ms) as f64 / 1000.0;
    let allure = match distance_m {
        Some(distance) if distance > 50.0 && duree_s > 0.0 => Some(duree_s / distance * 1000.0),
        _ => None,
    };
    LiveSessionView {
        device: session.device.clone(),
        state: session.state.clone(),
        started_ms: session.started_ms,
        last_ms: session.last_ms,
        received: session.received,
        points: session.points.len(),
        last: session.points.back().cloned(),
        trace,
        distance_m,
        duration_s: Some(duree_s),
        pace_s_per_km: allure,
    }
}

/// Identifiant de montre a partir du sujet : dernier segment.
pub fn device_from_topic(topic: &str) -> String {
    topic
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Demarre l'abonnement MQTT si `MPACER_MQTT_URL` est renseigne.
///
/// Sans broker configure, la fonction ne fait rien : le suivi en direct est une
/// fonctionnalite optionnelle, elle ne doit rien couter quand elle est eteinte.
pub fn spawn(state: AppState) -> Option<tokio::task::JoinHandle<()>> {
    let url = state.config.mqtt_url.clone()?;
    let topics = vec![(state.config.mqtt_topic.clone(), 1)];
    let options = MqttOptions {
        url,
        client_id: format!("mpacer-api-{}", std::process::id()),
        topics,
        keep_alive_s: mqtt::KEEP_ALIVE_S,
        username: state.config.mqtt_username.clone(),
        password: state.config.mqtt_password.clone(),
    };
    let (sink, mut messages) = tokio::sync::mpsc::channel::<Incoming>(256);
    mqtt::spawn(options, sink, Arc::clone(state.live.status()));
    let live = Arc::clone(&state.live);
    Some(tokio::spawn(async move {
        while let Some(message) = messages.recv().await {
            match live.ingest(&message.topic, &message.payload) {
                Ok(()) => {}
                Err(erreur) => {
                    tracing::debug!(sujet = %message.topic, erreur = %erreur, "suivi en direct : message ignore");
                }
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(t_ms: i64, lat: f64, lon: f64, dist: Option<f64>) -> LivePoint {
        LivePoint {
            t_ms,
            lat,
            lon,
            accuracy_m: Some(5.0),
            distance_m: dist,
            pace_s_per_km: Some(300.0),
            heart_rate_bpm: Some(140),
            lap: Some(1),
            battery_percent: Some(80),
            state: Some("run".to_string()),
            device: None,
        }
    }

    fn charge(point: &LivePoint) -> Vec<u8> {
        serde_json::to_vec(point).unwrap()
    }

    #[test]
    fn a_published_point_is_stored_under_its_device() {
        let store = LiveStore::new();
        let payload = br#"{"t":1000,"lat":48.85,"lon":2.35,"acc":4.2,"dist":120.0,"st":"run"}"#;
        store.ingest("mpacer/live/montre-a1b2", payload).unwrap();

        let vues = store.snapshot(2000, 4);
        assert_eq!(vues.len(), 1);
        assert_eq!(vues[0].device, "montre-a1b2");
        assert_eq!(vues[0].state, "run");
        assert_eq!(vues[0].points, 1);
        assert_eq!(vues[0].last.as_ref().unwrap().lat, 48.85);
        assert_eq!(vues[0].distance_m, Some(120.0));
        // Le nom de la montre n'est pas duplique sur chaque point.
        assert_eq!(vues[0].last.as_ref().unwrap().device, None);
    }

    #[test]
    fn the_device_can_come_from_the_payload() {
        let store = LiveStore::new();
        let mut p = point(1_000, 48.85, 2.35, None);
        p.device = Some("fenix".to_string());
        store.ingest("mpacer/live/inconnu", &charge(&p)).unwrap();
        assert_eq!(store.snapshot(1_000, 4)[0].device, "fenix");
    }

    #[test]
    fn unusable_messages_are_rejected_without_panicking() {
        let store = LiveStore::new();
        assert!(store.ingest("mpacer/live/m", b"{pas du json").is_err());
        assert!(
            store.ingest("mpacer/live/m", b"{}").is_err(),
            "horodatage absent"
        );
        assert!(
            store
                .ingest("mpacer/live/m", br#"{"t":1,"lat":91.0,"lon":2.0}"#)
                .is_err(),
            "latitude hors bornes"
        );
        assert!(
            store
                .ingest("mpacer/live/m", br#"{"t":1,"lat":48.0,"lon":181.0}"#)
                .is_err(),
            "longitude hors bornes"
        );
        assert!(store
            .ingest("", br#"{"t":1,"lat":48.0,"lon":2.0}"#)
            .is_err());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn the_ring_buffer_is_bounded() {
        let store = LiveStore::new();
        for index in 0..(MAX_POINTS + 250) {
            let p = point(1_000 + index as i64, 48.0, 2.0, None);
            store.ingest("mpacer/live/m", &charge(&p)).unwrap();
        }
        let vue = &store.snapshot(MAX_POINTS as i64 + 2_000, 4)[0];
        assert_eq!(vue.points, MAX_POINTS);
        assert_eq!(vue.received, (MAX_POINTS + 250) as u64);
        assert!(vue.trace.len() <= MAX_TRACE_POINTS + 1);
    }

    #[test]
    fn a_new_session_starts_after_a_stop() {
        let store = LiveStore::new();
        let mut premier = point(1_000, 48.0, 2.0, Some(500.0));
        premier.state = Some("stop".to_string());
        store.ingest("mpacer/live/m", &charge(&premier)).unwrap();
        store
            .ingest("mpacer/live/m", &charge(&point(2_000, 48.1, 2.1, None)))
            .unwrap();

        let vue = &store.snapshot(3_000, 4)[0];
        assert_eq!(vue.points, 1, "la trace repart de zero");
        assert_eq!(
            vue.distance_m, None,
            "la distance de la seance precedente est oubliee"
        );
        assert_eq!(vue.state, "run");
    }

    #[test]
    fn a_long_gap_starts_a_new_session() {
        let store = LiveStore::new();
        store
            .ingest(
                "mpacer/live/m",
                &charge(&point(1_000, 48.0, 2.0, Some(1_000.0))),
            )
            .unwrap();
        let tard = 1_000 + SESSION_GAP_MS + 1;
        store
            .ingest("mpacer/live/m", &charge(&point(tard, 48.5, 2.5, None)))
            .unwrap();
        let vue = &store.snapshot(tard, 4)[0];
        assert_eq!(vue.points, 1);
        assert_eq!(vue.started_ms, tard);
    }

    #[test]
    fn the_summary_computes_distance_duration_and_pace() {
        let store = LiveStore::new();
        store
            .ingest("mpacer/live/m", &charge(&point(1, 48.0, 2.0, Some(0.0))))
            .unwrap();
        store
            .ingest(
                "mpacer/live/m",
                &charge(&point(600_001, 48.1, 2.1, Some(2_000.0))),
            )
            .unwrap();
        let vue = &store.snapshot(600_002, 4)[0];
        assert_eq!(vue.distance_m, Some(2_000.0));
        assert_eq!(vue.duration_s, Some(600.0));
        assert_eq!(vue.pace_s_per_km, Some(300.0));
    }

    #[test]
    fn silent_devices_are_forgotten() {
        let store = LiveStore::new();
        store
            .ingest(
                "mpacer/live/vieille",
                &charge(&point(1_000, 48.0, 2.0, None)),
            )
            .unwrap();
        store
            .ingest(
                "mpacer/live/recente",
                &charge(&point(10_000, 48.0, 2.0, None)),
            )
            .unwrap();
        assert_eq!(store.len(), 2);

        let vues = store.snapshot(1_000 + SESSION_TTL_MS + 1, 4);
        assert_eq!(vues.len(), 1);
        assert_eq!(vues[0].device, "recente");
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn the_trace_is_downsampled_but_keeps_the_last_point() {
        let store = LiveStore::new();
        for index in 0..2_000i64 {
            store
                .ingest(
                    "mpacer/live/m",
                    &charge(&point(1_000 + index, 48.0, 2.0, None)),
                )
                .unwrap();
        }
        let vue = &store.snapshot(3_000, 4)[0];
        assert_eq!(vue.points, 2_000);
        assert!(vue.trace.len() <= MAX_TRACE_POINTS + 1);
        assert_eq!(vue.trace.last().unwrap().t_ms, 1_000 + 1_999);
    }

    #[test]
    fn topics_without_device_are_ignored() {
        assert_eq!(device_from_topic("mpacer/live/montre"), "montre");
        assert_eq!(device_from_topic("montre"), "montre");
        assert_eq!(device_from_topic("mpacer/live/"), "");
    }
}
