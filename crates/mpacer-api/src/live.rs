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

/// Nombre maximal de points acceptes pour un parcours planifie.
///
/// Un parcours est publie une fois par seance, pas une fois par position : on
/// peut donc en garder plus qu'une trace, tout en bornant la memoire du service
/// (2000 points x 2 f64 = 32 ko par appareil) et la taille du JSON envoye.
pub const MAX_ROUTE_POINTS: usize = 2000;

/// Nombre de points du parcours transmis a la carte.
///
/// La forme du trace reste juste a cette resolution, et la charge utile d'une
/// page de suivi reste raisonnable.
pub const MAX_ROUTE_DISPLAY: usize = 400;

/// Duree de conservation d'un parcours planifie sans nouvelle seance.
pub const ROUTE_TTL_MS: i64 = 24 * 60 * 60 * 1000;

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
    /// Altitude (m) : elle alimente le profil de denivele de la page /live.
    #[serde(rename = "alt", default, skip_serializing_if = "Option::is_none")]
    pub altitude_m: Option<f64>,
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
            && self.altitude_m.is_none_or(|v| v.is_finite())
    }
}

/// Parcours planifie d'un appareil : la trace que le coureur compte suivre.
///
/// Le coureur le publie une fois au depart (PUT /api/v1/live/route) ; les
/// suiveurs voient alors, sur la meme carte, **le parcours prevu** et la
/// position courante, avec le pourcentage de parcours deja couvert.
#[derive(Debug, Clone)]
pub struct PlannedRoute {
    pub device: String,
    /// Points du parcours, dans l'ordre.
    pub points: Vec<[f64; 2]>,
    pub updated_ms: i64,
    /// Distance cumulee a chaque point (m), calculee a la reception.
    cumul_m: Vec<f64>,
    /// Longueur totale du parcours (m).
    pub total_m: f64,
}

/// Avancement d'une position sur un parcours planifie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteProgress {
    /// Distance parcourue le long du parcours (m).
    pub along_m: f64,
    /// Distance restante le long du parcours (m).
    pub remaining_m: f64,
    /// Ecart perpendiculaire au parcours (m) : dit si l'on suit bien le trace.
    pub off_m: f64,
    /// Pourcentage du parcours couvert (0 a 100).
    pub pct: f64,
    /// Rang du point de depart du segment courant (pour situer sur la trace).
    pub from_index: usize,
}

impl PlannedRoute {
    /// Valide et construit un parcours. `None` si les points sont inutilisables.
    pub fn new(device: &str, points: Vec<[f64; 2]>, updated_ms: i64) -> Option<Self> {
        if points.len() < 2 || points.len() > MAX_ROUTE_POINTS {
            return None;
        }
        if !points.iter().all(|point| {
            point[0].is_finite()
                && point[1].is_finite()
                && (-90.0..=90.0).contains(&point[0])
                && (-180.0..=180.0).contains(&point[1])
        }) {
            return None;
        }
        let mut cumul_m = Vec::with_capacity(points.len());
        let mut total = 0.0;
        cumul_m.push(0.0);
        for fenetre in points.windows(2) {
            total += mpacer_core::geo::haversine_m(
                mpacer_core::geo::Position::new(fenetre[0][0], fenetre[0][1]),
                mpacer_core::geo::Position::new(fenetre[1][0], fenetre[1][1]),
            );
            cumul_m.push(total);
        }
        if total <= 1.0 {
            // Un parcours de moins d'un metre n'est pas un parcours.
            return None;
        }
        Some(Self {
            device: device.to_string(),
            points,
            updated_ms,
            cumul_m,
            total_m: total,
        })
    }

    /// Parcours sous-echantillonne pour l'affichage.
    pub fn display_points(&self) -> Vec<[f64; 2]> {
        if self.points.len() <= MAX_ROUTE_DISPLAY {
            return self.points.clone();
        }
        let pas = self.points.len().div_ceil(MAX_ROUTE_DISPLAY);
        let mut sortie: Vec<[f64; 2]> = self.points.iter().step_by(pas).copied().collect();
        // Le dernier point compte : c'est l'arrivee du parcours.
        if let Some(dernier) = self.points.last() {
            if sortie.last() != Some(dernier) {
                sortie.push(*dernier);
            }
        }
        sortie
    }

    /// Position la plus proche du parcours et avancement correspondant.
    ///
    /// Projection en plan local (metres) autour de la position : sur quelques
    /// centaines de metres, l'ecart avec la sphere est negligeable, et c'est le
    /// seul calcul dont la carte a besoin.
    pub fn nearest(&self, lat: f64, lon: f64) -> RouteProgress {
        let lat0 = lat.to_radians();
        let metres_par_degre_lat = mpacer_core::geo::EARTH_RADIUS_M * std::f64::consts::PI / 180.0;
        let metres_par_degre_lon = metres_par_degre_lat * lat0.cos().abs().max(0.02);
        let vers_plan = |point: [f64; 2]| {
            (
                (point[1] - lon) * metres_par_degre_lon,
                (point[0] - lat) * metres_par_degre_lat,
            )
        };
        let (px, py) = (0.0_f64, 0.0_f64);
        let mut meilleur = RouteProgress {
            along_m: 0.0,
            remaining_m: self.total_m,
            off_m: f64::INFINITY,
            pct: 0.0,
            from_index: 0,
        };
        for (index, fenetre) in self.points.windows(2).enumerate() {
            let (ax, ay) = vers_plan(fenetre[0]);
            let (bx, by) = vers_plan(fenetre[1]);
            let (dx, dy) = (bx - ax, by - ay);
            let longueur2 = dx * dx + dy * dy;
            let t = if longueur2 <= 1e-9 {
                0.0
            } else {
                (((px - ax) * dx + (py - ay) * dy) / longueur2).clamp(0.0, 1.0)
            };
            let (cx, cy) = (ax + t * dx, ay + t * dy);
            let distance = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            if distance < meilleur.off_m {
                let segment_m = self.cumul_m[index + 1] - self.cumul_m[index];
                let along_m = self.cumul_m[index] + t * segment_m;
                meilleur = RouteProgress {
                    along_m,
                    remaining_m: (self.total_m - along_m).max(0.0),
                    off_m: distance,
                    pct: (along_m / self.total_m * 100.0).clamp(0.0, 100.0),
                    from_index: index,
                };
            }
        }
        meilleur
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
    /// Vrai depuis qu'une nouvelle seance a remplace la precedente.
    ///
    /// Sert a ecarter un point de l'ancienne seance arrive apres coup : sans
    /// cette marque, un message retarde pourrait relancer une trace close.
    restarted: bool,
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
            restarted: false,
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
        self.restarted = true;
    }

    /// Ajoute un point a sa place chronologique.
    ///
    /// Le reseau ne garantit pas l'ordre d'arrivee : la montre decharge sa file
    /// en rafale a la reconnexion, MQTT est en QoS 0 et un message retenu peut
    /// etre republie. Un point plus ancien que le dernier recu est donc insere a
    /// sa date au lieu d'etre colle en fin de trace (ce qui ferait zigzaguer le
    /// trace et fausserait la position courante).
    fn push(&mut self, point: LivePoint) {
        // Apres une reprise, un point anterieur au debut de la seance courante
        // appartient a la precedente (il arrive apres un stop, en retard) : le
        // meler a cette trace n'aurait aucun sens.
        if self.restarted && point.t_ms < self.started_ms {
            return;
        }
        self.received += 1;
        // Un point en retard peut preciser le debut reel de la trace.
        self.started_ms = self.started_ms.min(point.t_ms);
        // L'etat et la date de fin suivent le point le plus recent, pas le
        // dernier message arrive : un retardataire ne fait pas reculer la montre.
        if point.t_ms >= self.last_ms {
            if let Some(etat) = point.state.as_deref() {
                self.state = etat.to_string();
            }
            self.last_ms = point.t_ms;
        }
        match self.points.binary_search_by(|p| p.t_ms.cmp(&point.t_ms)) {
            // Meme horodatage : c'est une rediffusion, on remplace le point au
            // lieu d'empiler un doublon sur la trace.
            Ok(index) => self.points[index] = point,
            Err(index) => self.points.insert(index, point),
        }
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
    /// Parcours planifie du coureur, sous-echantillonne (vide s'il n'en a pas).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route: Vec<[f64; 2]>,
    /// Longueur du parcours planifie (m).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_total_m: Option<f64>,
    /// Pourcentage du parcours planifie deja couvert (0 a 100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_pct: Option<f64>,
    /// Distance restante sur le parcours planifie (m).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_remaining_m: Option<f64>,
    /// Ecart au parcours planifie (m) : dit si le coureur suit bien son trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub off_route_m: Option<f64>,
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
    /// Parcours planifies, par nom d'appareil : publies une fois au depart.
    routes: RwLock<HashMap<String, PlannedRoute>>,
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
            routes: RwLock::new(HashMap::new()),
            status: Arc::new(MqttStatus::default()),
        }
    }

    /// Etat de la liaison avec le broker.
    pub fn status(&self) -> &Arc<MqttStatus> {
        &self.status
    }

    // ------------------------------------------------------- parcours planifie

    /// Enregistre le parcours planifie d'un appareil.
    ///
    /// Renvoie une erreur lisible quand le trace est inexploitable : le
    /// coureur le saura tout de suite, au lieu de croire qu'il partage.
    pub fn set_route(
        &self,
        device: &str,
        points: Vec<[f64; 2]>,
        now_ms: i64,
    ) -> Result<PlannedRoute, String> {
        if device.trim().is_empty() {
            return Err("appareil sans nom".to_string());
        }
        if points.len() > MAX_ROUTE_POINTS {
            return Err(format!(
                "parcours trop long : {} points (maximum {})",
                points.len(),
                MAX_ROUTE_POINTS
            ));
        }
        let route = PlannedRoute::new(device, points, now_ms)
            .ok_or_else(|| "parcours inexploitable : deux points valides au minimum".to_string())?;
        let mut routes = self
            .routes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        routes.insert(device.to_string(), route.clone());
        Ok(route)
    }

    /// Oublie le parcours d'un appareil (fin de seance, changement de trace).
    pub fn clear_route(&self, device: &str) -> bool {
        let mut routes = self
            .routes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        routes.remove(device).is_some()
    }

    /// Parcours planifie d'un appareil, s'il en a publie un.
    pub fn route(&self, device: &str) -> Option<PlannedRoute> {
        let routes = self
            .routes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        routes.get(device).cloned()
    }

    /// Nombre de parcours planifies en memoire (diagnostic).
    pub fn routes_len(&self) -> usize {
        self.routes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Vue d'une session, parcours planifie et avancement compris.
    fn vue(&self, session: &LiveSession) -> LiveSessionView {
        let mut vue = resume(session);
        let Some(route) = self.route(&session.device) else {
            return vue;
        };
        vue.route = route.display_points();
        vue.route_total_m = Some(route.total_m);
        if let Some(dernier) = session.points.back() {
            let avancement = route.nearest(dernier.lat, dernier.lon);
            vue.progress_pct = Some(avancement.pct);
            vue.route_remaining_m = Some(avancement.remaining_m);
            vue.off_route_m = Some(avancement.off_m);
        }
        vue
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
        // Un parcours planifie suit la meme duree de vie que la seance : au-dela
        // de 24 h sans publication, il n'interesse plus personne.
        let mut routes = self
            .routes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        routes.retain(|_, route| now_ms - route.updated_ms <= ROUTE_TTL_MS);
        drop(routes);
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
        let mut vues: Vec<LiveSessionView> =
            sessions.values().map(|session| self.vue(session)).collect();
        vues.sort_by_key(|vue| std::cmp::Reverse(vue.last_ms));
        vues.truncate(max_sessions);
        vues
    }

    /// Trace en cours d'un appareil precis, si elle existe.
    ///
    /// Sert au partage entre amis ([crate::friends]) : le service ne lit que les
    /// appareils revendiques par un compte, jamais tout le magasin.
    pub fn session(&self, device: &str) -> Option<LiveSessionView> {
        let sessions = self
            .sessions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sessions.get(device).map(|session| self.vue(session))
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
        // Remplis par `vue()` quand l'appareil a publie un parcours planifie.
        route: Vec::new(),
        route_total_m: None,
        progress_pct: None,
        route_remaining_m: None,
        off_route_m: None,
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
            altitude_m: Some(120.0),
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
    fn the_altitude_is_kept_for_the_elevation_profile() {
        let store = LiveStore::new();
        store
            .ingest(
                "mpacer/live/montre-a1b2",
                br#"{"t":1000,"lat":48.85,"lon":2.35,"alt":152.5,"st":"run"}"#,
            )
            .unwrap();
        let vue = &store.snapshot(2_000, 4)[0];
        assert_eq!(vue.last.as_ref().unwrap().altitude_m, Some(152.5));
    }

    #[test]
    fn an_altitude_that_is_not_a_number_is_rejected() {
        let store = LiveStore::new();
        // Un JSON ne peut pas porter NaN, mais une altitude infinie arriverait
        // d'un publish mal forme : le point est ecarte, pas la montre.
        assert!(store
            .ingest(
                "mpacer/live/montre",
                br#"{"t":1,"lat":48.0,"lon":2.0,"alt":1e400}"#,
            )
            .is_err());
        assert_eq!(store.len(), 0);
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
    fn a_planned_route_gives_the_position_and_the_percentage() {
        // Deux kilometres plein est : le point median doit tomber a mi-parcours.
        let route = PlannedRoute::new("montre", vec![[48.0, 2.0], [48.0, 2.02]], 1_000)
            .expect("parcours valide");
        assert!(route.total_m > 1_000.0, "total = {}", route.total_m);

        let milieu = route.nearest(48.0, 2.01);
        assert!((milieu.pct - 50.0).abs() < 3.0, "pct = {}", milieu.pct);
        assert!(milieu.off_m < 30.0, "ecart = {}", milieu.off_m);
        assert!((milieu.along_m + milieu.remaining_m - route.total_m).abs() < 1.0);

        let debut = route.nearest(48.0, 2.0);
        assert!(debut.pct < 2.0, "debut = {}", debut.pct);
        let fin = route.nearest(48.0, 2.02);
        assert!(fin.pct > 98.0, "fin = {}", fin.pct);
    }

    #[test]
    fn a_run_that_leaves_the_route_is_measured_against_it() {
        let route = PlannedRoute::new("montre", vec![[48.0, 2.0], [48.0, 2.02]], 1_000).unwrap();
        // 300 m au nord du trace, a mi-parcours : le pourcentage reste juste,
        // et l'ecart est signale.
        let ecarte = route.nearest(48.0027, 2.01);
        assert!((ecarte.pct - 50.0).abs() < 3.0, "pct = {}", ecarte.pct);
        assert!(ecarte.off_m > 250.0, "ecart = {}", ecarte.off_m);
    }

    #[test]
    fn unusable_routes_are_rejected() {
        assert!(PlannedRoute::new("m", vec![], 1).is_none());
        assert!(PlannedRoute::new("m", vec![[48.0, 2.0]], 1).is_none());
        assert!(PlannedRoute::new("m", vec![[91.0, 2.0], [48.0, 2.0]], 1).is_none());
        assert!(PlannedRoute::new("m", vec![[48.0, 2.0], [48.0, 2.0]], 1).is_none());
        let trop = vec![[48.0, 2.0], [48.0, 2.02]]
            .into_iter()
            .cycle()
            .take(MAX_ROUTE_POINTS + 1)
            .collect();
        assert!(PlannedRoute::new("m", trop, 1).is_none());
    }

    #[test]
    fn the_store_keeps_the_route_and_computes_the_progress() {
        let store = LiveStore::new();
        assert!(store.routes_len() == 0);
        let route = store
            .set_route("montre-a", vec![[48.0, 2.0], [48.0, 2.02]], 1_000)
            .expect("parcours accepte");
        assert!(route.total_m > 1_000.0);
        assert_eq!(store.routes_len(), 1);

        store
            .ingest(
                "mpacer/live/montre-a",
                br#"{"t":1000,"lat":48.0,"lon":2.01,"st":"run"}"#,
            )
            .unwrap();
        let vue = &store.snapshot(2_000, 4)[0];
        assert!(!vue.route.is_empty(), "le parcours voyage avec la session");
        assert_eq!(vue.route_total_m, Some(route.total_m));
        let pct = vue.progress_pct.expect("pourcentage calcule");
        assert!((pct - 50.0).abs() < 3.0, "pct = {pct}");
        assert!(vue.route_remaining_m.unwrap() > 300.0);
        assert!(vue.off_route_m.unwrap() < 30.0);

        // Sans position, le parcours reste visible mais sans pourcentage.
        let seule = LiveStore::new();
        seule
            .set_route("montre-b", vec![[48.0, 2.0], [48.0, 2.02]], 1_000)
            .unwrap();
        seule
            .ingest("mpacer/live/montre-b", br#"{"t":1,"lat":48.0,"lon":2.0}"#)
            .unwrap();
        let vue_b = &seule.snapshot(2, 4)[0];
        assert!(vue_b.progress_pct.is_some());

        assert!(store.clear_route("montre-a"));
        assert_eq!(store.routes_len(), 0);
        let apres = &store.snapshot(3_000, 4)[0];
        assert!(apres.route.is_empty(), "parcours efface");
        assert!(apres.progress_pct.is_none());
    }

    #[test]
    fn a_stale_route_is_forgotten_like_a_stale_session() {
        let store = LiveStore::new();
        store
            .set_route("montre", vec![[48.0, 2.0], [48.0, 2.02]], 1_000)
            .unwrap();
        store.prune(1_000 + ROUTE_TTL_MS + 1);
        assert_eq!(store.routes_len(), 0);
    }

    #[test]
    fn out_of_order_points_keep_the_trace_in_time_order() {
        let store = LiveStore::new();
        store
            .ingest(
                "mpacer/live/m",
                &charge(&point(2_000, 48.02, 2.0, Some(20.0))),
            )
            .unwrap();
        // La montre decharge sa file en rafale : un point peut arriver apres
        // son suivant.
        store
            .ingest(
                "mpacer/live/m",
                &charge(&point(1_000, 48.01, 2.0, Some(10.0))),
            )
            .unwrap();
        store
            .ingest(
                "mpacer/live/m",
                &charge(&point(3_000, 48.03, 2.0, Some(30.0))),
            )
            .unwrap();

        let vue = &store.snapshot(4_000, 4)[0];
        let horodatages: Vec<i64> = vue.trace.iter().map(|p| p.t_ms).collect();
        assert_eq!(
            horodatages,
            vec![1_000, 2_000, 3_000],
            "le trace reste ordonne"
        );
        assert_eq!(vue.points, 3);
        assert_eq!(vue.last.as_ref().unwrap().t_ms, 3_000, "position courante");
        assert_eq!(vue.last_ms, 3_000);
        assert_eq!(vue.started_ms, 1_000, "le debut reel est retenu");
    }

    #[test]
    fn a_late_point_does_not_move_the_clock_backwards() {
        let store = LiveStore::new();
        store
            .ingest("mpacer/live/m", &charge(&point(5_000, 48.05, 2.0, None)))
            .unwrap();
        let mut retardataire = point(4_000, 48.04, 2.0, None);
        retardataire.state = Some("pause".to_string());
        store
            .ingest("mpacer/live/m", &charge(&retardataire))
            .unwrap();

        let vue = &store.snapshot(5_001, 4)[0];
        assert_eq!(vue.last_ms, 5_000, "la montre ne recule pas");
        assert_eq!(
            vue.state, "run",
            "un vieux point ne change pas l'etat courant"
        );
        assert_eq!(vue.last.as_ref().unwrap().t_ms, 5_000);
        assert_eq!(vue.points, 2, "le point en retard reste sur la trace");
    }

    #[test]
    fn a_retransmitted_point_replaces_instead_of_duplicating() {
        let store = LiveStore::new();
        store
            .ingest("mpacer/live/m", &charge(&point(1_000, 48.0, 2.0, None)))
            .unwrap();
        // Meme horodatage, position corrigee : c'est une rediffusion (message
        // retenu republie a la reconnexion), pas un nouveau point.
        store
            .ingest("mpacer/live/m", &charge(&point(1_000, 48.5, 2.5, None)))
            .unwrap();

        let vue = &store.snapshot(2_000, 4)[0];
        assert_eq!(vue.points, 1);
        assert_eq!(vue.last.as_ref().unwrap().lat, 48.5);
    }

    #[test]
    fn a_late_point_from_a_previous_session_is_ignored() {
        let store = LiveStore::new();
        let mut fin = point(1_000, 48.0, 2.0, Some(900.0));
        fin.state = Some("stop".to_string());
        store.ingest("mpacer/live/m", &charge(&fin)).unwrap();
        store
            .ingest("mpacer/live/m", &charge(&point(2_000, 48.1, 2.1, None)))
            .unwrap();
        // Un point de la seance precedente arrive apres la nouvelle.
        store
            .ingest(
                "mpacer/live/m",
                &charge(&point(1_500, 48.05, 2.05, Some(950.0))),
            )
            .unwrap();

        let vue = &store.snapshot(3_000, 4)[0];
        assert_eq!(vue.points, 1);
        assert_eq!(vue.started_ms, 2_000);
        assert_eq!(vue.distance_m, None, "la seance precedente ne revient pas");
    }

    #[test]
    fn topics_without_device_are_ignored() {
        assert_eq!(device_from_topic("mpacer/live/montre"), "montre");
        assert_eq!(device_from_topic("montre"), "montre");
        assert_eq!(device_from_topic("mpacer/live/"), "");
    }
}
