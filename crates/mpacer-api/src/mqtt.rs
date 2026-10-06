//! Client MQTT 3.1.1 minimal (abonne), sans dependance externe.
//!
//! Le suivi en direct ne demande qu'une chose au broker : recevoir les positions
//! publiees par la montre sur `<prefixe>/live/<montre>`. Un client complet
//! (QoS 2, will, TLS mutuel, session persistante) serait disproportionne : ce
//! module implemente exactement le sous-ensemble necessaire, et sa serialisation
//! MQTT est testee octet par octet.
//!
//! * `mqtt://hote:1883` : TCP simple (broker sur le reseau interne du cluster) ;
//! * `mqtts://hote:8883` : TLS avec les autorites racines publiques.
//!
//! Reconnexion automatique avec attente progressive (2 s -> 60 s), `PINGREQ`
//! periodique, `PUBACK` pour les messages QoS 1. Un seul task Tokio par
//! connexion, aucun tampon non borne : le suivi en direct ne doit rien couter
//! au reste du service.

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

/// Delai de vie negocie avec le broker (secondes). Le client envoie un
/// `PINGREQ` a la moitie de ce delai, ce qui suffit a detecter une connexion
/// morte en une minute.
pub const KEEP_ALIVE_S: u16 = 60;

/// Premiere attente avant reconnexion.
pub const RECONNECT_MIN: Duration = Duration::from_secs(2);
/// Attente maximale avant reconnexion (plafonnee : on veut reprendre vite).
pub const RECONNECT_MAX: Duration = Duration::from_secs(60);
/// Delai maximal accorde a la poignee de main (CONNECT / SUBSCRIBE).
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

// ------------------------------------------------------------- types de paquet

const CONNECT: u8 = 0x10;
const CONNACK: u8 = 0x20;
const PUBLISH: u8 = 0x30;
const SUBSCRIBE: u8 = 0x80;
const SUBACK: u8 = 0x90;
const PINGREQ: u8 = 0xC0;
const PINGRESP: u8 = 0xD0;
const DISCONNECT: u8 = 0xE0;

/// Erreurs du client MQTT.
#[derive(Debug, thiserror::Error)]
pub enum MqttError {
    #[error("adresse de broker invalide : {0}")]
    Url(String),
    #[error("paquet MQTT invalide : {0}")]
    Protocol(String),
    #[error("poignee de main refusee par le broker : {0}")]
    Refused(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

// ------------------------------------------------------------------- adresse

/// Adresse d'un broker MQTT, extraite de `MQTT_URL`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerUrl {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl BrokerUrl {
    /// Analyse `mqtt://[user[:motdepasse]@]hote[:port]` (et `mqtts://`).
    pub fn parse(url: &str) -> Result<Self, MqttError> {
        let (scheme, reste) = url.split_once("://").ok_or_else(|| {
            MqttError::Url(format!("schema attendu (mqtt:// ou mqtts://) : {url}"))
        })?;
        let tls = match scheme.to_ascii_lowercase().as_str() {
            "mqtt" | "tcp" => false,
            "mqtts" | "ssl" | "tls" => true,
            autre => return Err(MqttError::Url(format!("schema inconnu : {autre}"))),
        };

        // Les identifiants sont optionnels et precedes du dernier '@' (un mot de
        // passe peut contenir un '@' encode en %40).
        let (identifiants, hote_port) = match reste.rsplit_once('@') {
            Some((identifiants, hote_port)) => (Some(identifiants), hote_port),
            None => (None, reste),
        };
        let hote_port = hote_port.trim_end_matches('/');
        if hote_port.is_empty() {
            return Err(MqttError::Url("hote manquant".to_string()));
        }
        let (host, port) = match hote_port.rsplit_once(':') {
            Some((host, port)) => {
                let port = port
                    .parse::<u16>()
                    .map_err(|_| MqttError::Url(format!("port invalide : {port}")))?;
                (host.to_string(), port)
            }
            None => (hote_port.to_string(), if tls { 8883 } else { 1883 }),
        };
        if host.is_empty() {
            return Err(MqttError::Url("hote manquant".to_string()));
        }

        let (username, password) = match identifiants {
            Some(identifiants) => match identifiants.split_once(':') {
                Some((user, pass)) => (Some(percent_decode(user)), Some(percent_decode(pass))),
                None => {
                    let user = percent_decode(identifiants);
                    let user = if user.is_empty() { None } else { Some(user) };
                    (user, None)
                }
            },
            None => (None, None),
        };

        Ok(Self {
            host,
            port,
            tls,
            username,
            password,
        })
    }

    /// Adresse affichable, sans mot de passe.
    pub fn redacted(&self) -> String {
        let scheme = if self.tls { "mqtts" } else { "mqtt" };
        match &self.username {
            Some(user) => format!("{scheme}://{user}@{}:{}", self.host, self.port),
            None => format!("{scheme}://{}:{}", self.host, self.port),
        }
    }
}

/// Decodage des caracteres encodes dans une URL (RFC 3986).
fn percent_decode(value: &str) -> String {
    let octets = value.as_bytes();
    let mut sortie = Vec::with_capacity(octets.len());
    let mut index = 0;
    while index < octets.len() {
        if octets[index] == b'%' && index + 2 < octets.len() {
            let hex = std::str::from_utf8(&octets[index + 1..index + 3]).unwrap_or("");
            if let Ok(octet) = u8::from_str_radix(hex, 16) {
                sortie.push(octet);
                index += 3;
                continue;
            }
        }
        sortie.push(octets[index]);
        index += 1;
    }
    String::from_utf8_lossy(&sortie).into_owned()
}

// -------------------------------------------------------------- serialisation

/// Encodage de la longueur restante (1 a 4 octets, 7 bits par octet).
pub fn encode_remaining_length(mut longueur: usize) -> Result<Vec<u8>, MqttError> {
    if longueur > 268_435_455 {
        return Err(MqttError::Protocol("longueur de paquet hors bornes".into()));
    }
    let mut sortie = Vec::with_capacity(4);
    loop {
        let mut octet = (longueur % 128) as u8;
        longueur /= 128;
        if longueur > 0 {
            octet |= 0x80;
        }
        sortie.push(octet);
        if longueur == 0 {
            return Ok(sortie);
        }
    }
}

/// Chaine MQTT : longueur sur deux octets, puis UTF-8.
pub fn encode_string(value: &str) -> Vec<u8> {
    let octets = value.as_bytes();
    let mut sortie = Vec::with_capacity(octets.len() + 2);
    sortie.extend_from_slice(&(octets.len() as u16).to_be_bytes());
    sortie.extend_from_slice(octets);
    sortie
}

/// Assemble un paquet complet a partir de son type et de sa charge utile.
fn packet(kind: u8, charge: &[u8]) -> Vec<u8> {
    let mut sortie = Vec::with_capacity(charge.len() + 5);
    sortie.push(kind);
    // La longueur d'un paquet de suivi tient toujours sur un octet, mais on
    // passe par l'encodage complet : la fonction reste juste pour tout usage.
    sortie.extend_from_slice(&encode_remaining_length(charge.len()).unwrap_or_default());
    sortie.extend_from_slice(charge);
    sortie
}

/// Paquet `CONNECT` (3.1.1, session propre, sans will).
pub fn connect_packet(
    client_id: &str,
    username: Option<&str>,
    password: Option<&str>,
    keep_alive_s: u16,
) -> Vec<u8> {
    let mut charge = Vec::new();
    charge.extend_from_slice(&encode_string("MQTT"));
    charge.push(4); // niveau 3.1.1
    let mut drapeaux = 0x02; // session propre
    if username.is_some() {
        drapeaux |= 0x80;
    }
    if password.is_some() {
        drapeaux |= 0x40;
    }
    charge.push(drapeaux);
    charge.extend_from_slice(&keep_alive_s.to_be_bytes());
    charge.extend_from_slice(&encode_string(client_id));
    if let Some(username) = username {
        charge.extend_from_slice(&encode_string(username));
    }
    if let Some(password) = password {
        charge.extend_from_slice(&encode_string(password));
    }
    packet(CONNECT, &charge)
}

/// Paquet `SUBSCRIBE` (QoS demandee par filtre).
pub fn subscribe_packet(packet_id: u16, filtres: &[(String, u8)]) -> Vec<u8> {
    let mut charge = Vec::new();
    charge.extend_from_slice(&packet_id.to_be_bytes());
    for (filtre, qos) in filtres {
        charge.extend_from_slice(&encode_string(filtre));
        charge.push(*qos);
    }
    packet(SUBSCRIBE | 0x02, &charge)
}

/// Paquet `PUBLISH` (utilise par les tests et par une eventuelle republication).
pub fn publish_packet(
    topic: &str,
    payload: &[u8],
    qos: u8,
    retain: bool,
    packet_id: u16,
) -> Vec<u8> {
    let mut charge = Vec::new();
    charge.extend_from_slice(&encode_string(topic));
    if qos > 0 {
        charge.extend_from_slice(&packet_id.to_be_bytes());
    }
    charge.extend_from_slice(payload);
    let kind = PUBLISH | ((qos & 0x03) << 1) | u8::from(retain);
    packet(kind, &charge)
}

/// Paquet `PINGREQ`.
pub fn pingreq_packet() -> Vec<u8> {
    vec![PINGREQ, 0x00]
}

/// Paquet `PUBACK`.
pub fn puback_packet(packet_id: u16) -> Vec<u8> {
    let [haut, bas] = packet_id.to_be_bytes();
    vec![0x40, 0x02, haut, bas]
}

/// Paquet `DISCONNECT`.
pub fn disconnect_packet() -> Vec<u8> {
    vec![DISCONNECT, 0x00]
}

// ---------------------------------------------------------------- deserialisation

/// Paquet MQTT brut : type, drapeaux et charge utile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub kind: u8,
    pub flags: u8,
    pub payload: Vec<u8>,
}

/// Message applicatif recu du broker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming {
    pub topic: String,
    pub payload: Vec<u8>,
}

/// Lecture d'un paquet complet depuis un flux.
///
/// Volontairement non annulable (pas de `select!` sur cette future) : une
/// lecture interrompue au milieu d'un paquet desynchroniserait le flux.
pub async fn read_packet<R>(reader: &mut R) -> Result<Packet, MqttError>
where
    R: AsyncRead + Unpin,
{
    let mut entete = [0u8; 1];
    reader.read_exact(&mut entete).await?;
    let mut longueur: usize = 0;
    let mut multiplicateur: usize = 1;
    loop {
        let mut octet = [0u8; 1];
        reader.read_exact(&mut octet).await?;
        longueur += usize::from(octet[0] & 0x7F) * multiplicateur;
        if octet[0] & 0x80 == 0 {
            break;
        }
        multiplicateur = multiplicateur.saturating_mul(128);
        if multiplicateur > 128 * 128 * 128 {
            return Err(MqttError::Protocol("longueur restante trop grande".into()));
        }
    }
    let mut payload = vec![0u8; longueur];
    reader.read_exact(&mut payload).await?;
    Ok(Packet {
        kind: entete[0] & 0xF0,
        flags: entete[0] & 0x0F,
        payload,
    })
}

/// Contenu d'un `PUBLISH` decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub packet_id: u16,
}

/// Decode la charge utile d'un `PUBLISH`.
pub fn parse_publish(flags: u8, payload: &[u8]) -> Result<Published, MqttError> {
    if payload.len() < 2 {
        return Err(MqttError::Protocol("PUBLISH tronque".into()));
    }
    let taille_topic = usize::from(u16::from_be_bytes([payload[0], payload[1]]));
    if payload.len() < 2 + taille_topic {
        return Err(MqttError::Protocol("sujet de PUBLISH tronque".into()));
    }
    let topic = String::from_utf8_lossy(&payload[2..2 + taille_topic]).into_owned();
    let qos = (flags >> 1) & 0x03;
    let mut curseur = 2 + taille_topic;
    let mut packet_id = 0u16;
    if qos > 0 {
        if payload.len() < curseur + 2 {
            return Err(MqttError::Protocol("identifiant de paquet manquant".into()));
        }
        packet_id = u16::from_be_bytes([payload[curseur], payload[curseur + 1]]);
        curseur += 2;
    }
    if qos == 3 {
        return Err(MqttError::Protocol("QoS 3 invalide".into()));
    }
    Ok(Published {
        topic,
        payload: payload[curseur..].to_vec(),
        qos,
        packet_id,
    })
}

// ------------------------------------------------------------------- etat

/// Etat de la liaison avec le broker, expose par la page de suivi.
#[derive(Debug, Default)]
pub struct MqttStatus {
    connecte: AtomicBool,
    connexions: AtomicU64,
    messages: AtomicU64,
    dernier_message_ms: AtomicI64,
}

impl MqttStatus {
    /// Vrai si la liaison est etablie.
    pub fn connected(&self) -> bool {
        self.connecte.load(Ordering::Relaxed)
    }

    /// Nombre de connexions reussies depuis le demarrage.
    pub fn connections(&self) -> u64 {
        self.connexions.load(Ordering::Relaxed)
    }

    /// Nombre de messages recus depuis le demarrage.
    pub fn messages(&self) -> u64 {
        self.messages.load(Ordering::Relaxed)
    }

    /// Instant du dernier message recu (ms epoch), 0 si aucun.
    pub fn last_message_ms(&self) -> i64 {
        self.dernier_message_ms.load(Ordering::Relaxed)
    }

    fn set_connected(&self, value: bool) {
        self.connecte.store(value, Ordering::Relaxed);
    }

    fn record_message(&self, t_ms: i64) {
        self.messages.fetch_add(1, Ordering::Relaxed);
        self.dernier_message_ms.store(t_ms, Ordering::Relaxed);
    }
}

// ------------------------------------------------------------------- client

/// Parametres de l'abonnement.
#[derive(Debug, Clone)]
pub struct MqttOptions {
    /// Adresse complete (`mqtt://hote:1883`).
    pub url: String,
    /// Identifiant de client : doit etre unique par connexion.
    pub client_id: String,
    /// Filtres a souscrire, avec la QoS demandee.
    pub topics: Vec<(String, u8)>,
    pub keep_alive_s: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl MqttOptions {
    /// Analyse l'adresse et applique les identifiants de l'URL.
    pub fn broker(&self) -> Result<BrokerUrl, MqttError> {
        let mut broker = BrokerUrl::parse(&self.url)?;
        if broker.username.is_none() {
            broker.username = self.username.clone().filter(|v| !v.is_empty());
        }
        if broker.password.is_none() {
            broker.password = self.password.clone().filter(|v| !v.is_empty());
        }
        Ok(broker)
    }
}

/// Lance l'abonnement en tache de fond. La tache vit aussi longtemps que le
/// service : elle se reconnecte toute seule et ne panique jamais.
pub fn spawn(
    options: MqttOptions,
    sink: mpsc::Sender<Incoming>,
    status: Arc<MqttStatus>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        run(options, sink, status).await;
    })
}

/// Boucle de connexion : elle ne rend jamais la main.
pub async fn run(options: MqttOptions, sink: mpsc::Sender<Incoming>, status: Arc<MqttStatus>) {
    let mut attente = RECONNECT_MIN;
    loop {
        match connecter(&options).await {
            Ok((broker, transport)) => {
                status.set_connected(true);
                status.connexions.fetch_add(1, Ordering::Relaxed);
                tracing::info!(
                    broker = %broker.redacted(),
                    filtres = ?options.topics.iter().map(|(t, _)| t.clone()).collect::<Vec<_>>(),
                    "suivi en direct : abonne au broker MQTT"
                );
                attente = RECONNECT_MIN;
                let resultat = session(transport, &options, &sink, &status).await;
                status.set_connected(false);
                match resultat {
                    Ok(()) => tracing::warn!("suivi en direct : liaison MQTT terminee"),
                    Err(erreur) => {
                        tracing::warn!(erreur = %erreur, "suivi en direct : liaison MQTT perdue")
                    }
                }
            }
            Err(erreur) => {
                status.set_connected(false);
                tracing::warn!(erreur = %erreur, "suivi en direct : connexion au broker impossible");
            }
        }
        tokio::time::sleep(attente).await;
        attente = (attente * 2).min(RECONNECT_MAX);
    }
}

/// Flux MQTT etabli, avant decoupage en lecture / ecriture.
enum Transport {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::client::TlsStream<TcpStream>>),
}

/// Ouvre la connexion et realise la poignee de main MQTT.
async fn connecter(options: &MqttOptions) -> Result<(BrokerUrl, Transport), MqttError> {
    let broker = options.broker()?;
    // La poignee de main est bornee : un broker injoignable ne doit pas laisser
    // une tache endormie sur une socket a moitie ouverte.
    let transport = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
        let mut transport = if broker.tls {
            Transport::Tls(Box::new(tls_stream(&broker).await?))
        } else {
            Transport::Plain(tcp_stream(&broker).await?)
        };
        match &mut transport {
            Transport::Plain(stream) => handshake(stream, options, &broker).await?,
            Transport::Tls(stream) => handshake(stream.as_mut(), options, &broker).await?,
        }
        Ok::<Transport, MqttError>(transport)
    })
    .await
    .map_err(|_| MqttError::Refused("delai de poignee de main depasse".to_string()))??;
    Ok((broker, transport))
}

async fn tcp_stream(broker: &BrokerUrl) -> Result<TcpStream, MqttError> {
    let stream = TcpStream::connect((broker.host.as_str(), broker.port)).await?;
    stream.set_nodelay(true)?;
    Ok(stream)
}

/// Connexion TLS (autorites racines publiques) pour `mqtts://`.
async fn tls_stream(
    broker: &BrokerUrl,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>, MqttError> {
    use tokio_rustls::rustls::pki_types::ServerName;
    use tokio_rustls::rustls::{ClientConfig, RootCertStore};
    use tokio_rustls::TlsConnector;

    let racines: RootCertStore = webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();
    let fournisseur = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(fournisseur)
        .with_safe_default_protocol_versions()
        .map_err(|erreur| MqttError::Protocol(format!("TLS indisponible : {erreur}")))?
        .with_root_certificates(racines)
        .with_no_client_auth();
    let connecteur = TlsConnector::from(Arc::new(config));
    let nom = ServerName::try_from(broker.host.clone())
        .map_err(|_| MqttError::Url(format!("nom d'hote invalide : {}", broker.host)))?;
    let tcp = tcp_stream(broker).await?;
    let flux = connecteur.connect(nom, tcp).await?;
    Ok(flux)
}

/// Poignee de main MQTT sur un flux deja ouvert (CONNECT, puis SUBSCRIBE).
async fn handshake<S>(
    stream: &mut S,
    options: &MqttOptions,
    broker: &BrokerUrl,
) -> Result<(), MqttError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let connect = connect_packet(
        &options.client_id,
        broker.username.as_deref(),
        broker.password.as_deref(),
        options.keep_alive_s,
    );
    stream.write_all(&connect).await?;
    let connack = read_packet(&mut *stream).await?;
    if connack.kind != CONNACK {
        return Err(MqttError::Protocol(format!(
            "CONNACK attendu, recu 0x{:02X}",
            connack.kind
        )));
    }
    let code = connack.payload.last().copied().unwrap_or(0xFF);
    if code != 0 {
        return Err(MqttError::Refused(refusal_label(code)));
    }

    stream
        .write_all(&subscribe_packet(1, &options.topics))
        .await?;
    let suback = read_packet(&mut *stream).await?;
    if suback.kind != SUBACK {
        return Err(MqttError::Protocol(format!(
            "SUBACK attendu, recu 0x{:02X}",
            suback.kind
        )));
    }
    // SUBACK : identifiant de paquet (2 octets) puis un code par filtre ;
    // 0x80 signale un filtre refuse.
    if suback.payload.len() >= 3 && suback.payload[2..].contains(&0x80) {
        return Err(MqttError::Refused(
            "un filtre d'abonnement a ete refuse".to_string(),
        ));
    }
    Ok(())
}

/// Session etablie : lecture des messages jusqu'a la perte du lien.
async fn session(
    transport: Transport,
    options: &MqttOptions,
    sink: &mpsc::Sender<Incoming>,
    status: &MqttStatus,
) -> Result<(), MqttError> {
    match transport {
        Transport::Plain(stream) => {
            let (lecture, ecriture) = tokio::io::split(stream);
            run_streams(lecture, ecriture, options, sink, status).await
        }
        Transport::Tls(stream) => {
            let (lecture, ecriture) = tokio::io::split(*stream);
            run_streams(lecture, ecriture, options, sink, status).await
        }
    }
}

/// Boucle de lecture, avec une tache d'ecriture pour les `PINGREQ` et `PUBACK`.
async fn run_streams<R, W>(
    mut lecture: R,
    ecriture: W,
    options: &MqttOptions,
    sink: &mpsc::Sender<Incoming>,
    status: &MqttStatus,
) -> Result<(), MqttError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (sortie, entree) = mpsc::channel::<Vec<u8>>(32);
    let keep_alive = Duration::from_secs(u64::from(options.keep_alive_s.max(5)));
    let pings = tokio::spawn(ecriture_loop(ecriture, entree, keep_alive));

    let resultat = loop {
        let paquet = match read_packet(&mut lecture).await {
            Ok(paquet) => paquet,
            Err(erreur) => break Err(erreur),
        };
        match paquet.kind {
            PUBLISH => {
                let message = match parse_publish(paquet.flags, &paquet.payload) {
                    Ok(message) => message,
                    Err(erreur) => break Err(erreur),
                };
                if message.qos == 1 {
                    // Accuse de reception : sans lui, le broker rejoue le message.
                    let _ = sortie.send(puback_packet(message.packet_id)).await;
                }
                status.record_message(now_ms());
                if sink
                    .send(Incoming {
                        topic: message.topic,
                        payload: message.payload,
                    })
                    .await
                    .is_err()
                {
                    break Ok(());
                }
            }
            PINGRESP | SUBACK | CONNACK => {}
            autre => {
                // Les autres paquets (PUBACK, UNSUBACK...) sont ignores : le
                // client est abonne et ne publie pas.
                tracing::trace!(kind = autre, "paquet MQTT ignore");
            }
        }
    };

    // L'arret de la tache d'ecriture ferme la socket (FIN) : inutile d'envoyer
    // un DISCONNECT que personne n'ecrirait.
    pings.abort();
    resultat
}

/// Tache d'ecriture : `PINGREQ` periodique et paquets ponctuels.
async fn ecriture_loop<W>(mut flux: W, mut entree: mpsc::Receiver<Vec<u8>>, keep_alive: Duration)
where
    W: AsyncWrite + Unpin,
{
    let mut battement = tokio::time::interval(keep_alive / 2);
    battement.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // Le premier tick est immediat : on le consomme pour ne pas envoyer un
    // PINGREQ juste apres la poignee de main.
    battement.tick().await;
    loop {
        tokio::select! {
            paquet = entree.recv() => {
                match paquet {
                    Some(paquet) => {
                        if flux.write_all(&paquet).await.is_err() {
                            return;
                        }
                        if flux.flush().await.is_err() {
                            return;
                        }
                    }
                    None => return,
                }
            }
            _ = battement.tick() => {
                if flux.write_all(&pingreq_packet()).await.is_err() {
                    return;
                }
            }
        }
    }
}

/// Libelle d'un code de refus CONNACK.
fn refusal_label(code: u8) -> String {
    let texte = match code {
        1 => "version de protocole refusee",
        2 => "identifiant de client refuse",
        3 => "service indisponible",
        4 => "identifiants refuses",
        5 => "non autorise",
        _ => "code inconnu",
    };
    format!("{texte} ({code})")
}

/// Horodatage courant en millisecondes.
fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
// ------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn remaining_length_is_encoded_on_seven_bits() {
        assert_eq!(encode_remaining_length(0).unwrap(), vec![0x00]);
        assert_eq!(encode_remaining_length(127).unwrap(), vec![0x7F]);
        assert_eq!(encode_remaining_length(128).unwrap(), vec![0x80, 0x01]);
        assert_eq!(encode_remaining_length(16_383).unwrap(), vec![0xFF, 0x7F]);
        assert_eq!(
            encode_remaining_length(16_384).unwrap(),
            vec![0x80, 0x80, 0x01]
        );
        assert!(encode_remaining_length(300_000_000).is_err());
    }

    #[test]
    fn connect_packet_matches_the_specification() {
        let paquet = connect_packet("mpacer-api", None, None, 60);
        assert_eq!(paquet[0], 0x10, "type CONNECT");
        assert_eq!(paquet[1] as usize, paquet.len() - 2, "longueur restante");
        // En-tete variable : "MQTT", niveau 3.1.1, session propre, keep alive.
        assert_eq!(&paquet[2..9], b"\x00\x04MQTT\x04");
        assert_eq!(paquet[9], 0x02);
        assert_eq!(&paquet[10..12], &[0x00, 0x3C]);
        // Charge utile : identifiant de client.
        assert_eq!(&paquet[12..14], &[0x00, 0x0A]);
        assert_eq!(&paquet[14..], b"mpacer-api");
    }

    #[test]
    fn connect_packet_declares_credentials() {
        let paquet = connect_packet("montre", Some("coureur"), Some("secret"), 30);
        assert_eq!(paquet[9], 0xC2, "drapeaux identifiant + mot de passe");
        // Le mot de passe ferme la charge utile (6 octets + sa longueur).
        assert_eq!(&paquet[paquet.len() - 8..], b"\x00\x06secret");
    }

    #[test]
    fn subscribe_packet_carries_filters_and_qos() {
        let paquet = subscribe_packet(1, &[("mpacer/live/+".to_string(), 1)]);
        assert_eq!(paquet[0], 0x82, "type SUBSCRIBE avec les drapeaux imposes");
        assert_eq!(&paquet[4..6], &[0x00, 0x0D], "taille du filtre");
        assert_eq!(&paquet[6..19], b"mpacer/live/+");
        assert_eq!(paquet[paquet.len() - 1], 0x01, "QoS demandee");
    }

    #[test]
    fn publish_packets_round_trip_through_the_decoder() {
        let charge = br#"{"t":1,"lat":45.0,"lon":4.0}"#;
        for (qos, attendu_id) in [(0u8, 0u16), (1, 42)] {
            let paquet = publish_packet("mpacer/live/montre", charge, qos, true, 42);
            let lu = read_packet_sync(&paquet);
            assert_eq!(lu.kind, PUBLISH);
            assert_eq!(lu.flags & 0x01, 0x01, "indicateur retain");
            let publie = parse_publish(lu.flags, &lu.payload).unwrap();
            assert_eq!(publie.topic, "mpacer/live/montre");
            assert_eq!(publie.payload, charge);
            assert_eq!(publie.qos, qos);
            assert_eq!(publie.packet_id, attendu_id);
        }
    }

    #[test]
    fn truncated_publish_is_rejected() {
        assert!(parse_publish(0, &[]).is_err());
        assert!(parse_publish(0, &[0x00, 0x05, b'a']).is_err());
        assert!(parse_publish(0b0110, &[0x00, 0x01, b'a']).is_err(), "QoS 3");
    }

    #[test]
    fn broker_url_parsing() {
        let broker = BrokerUrl::parse("mqtt://broker.local").unwrap();
        assert_eq!(broker.host, "broker.local");
        assert_eq!(broker.port, 1883);
        assert!(!broker.tls);
        assert!(broker.username.is_none());

        let broker = BrokerUrl::parse("mqtts://coureur:mo%40t@mqtt.exemple.org:8884").unwrap();
        assert!(broker.tls);
        assert_eq!(broker.port, 8884);
        assert_eq!(broker.username.as_deref(), Some("coureur"));
        assert_eq!(broker.password.as_deref(), Some("mo@t"));
        assert_eq!(broker.redacted(), "mqtts://coureur@mqtt.exemple.org:8884");

        assert!(BrokerUrl::parse("http://broker.local").is_err());
        assert!(BrokerUrl::parse("mqtt://broker.local:port").is_err());
        assert!(BrokerUrl::parse("mqtt://").is_err());
    }

    #[test]
    fn options_fall_back_on_configured_credentials() {
        let options = MqttOptions {
            url: "mqtt://broker.local:1883".to_string(),
            client_id: "mpacer-api".to_string(),
            topics: vec![("mpacer/live/+".to_string(), 1)],
            keep_alive_s: KEEP_ALIVE_S,
            username: Some("coureur".to_string()),
            password: Some("secret".to_string()),
        };
        let broker = options.broker().unwrap();
        assert_eq!(broker.username.as_deref(), Some("coureur"));
        assert_eq!(broker.password.as_deref(), Some("secret"));
    }

    /// Decodage synchrone d'un paquet deja en memoire (tests unitaires).
    fn read_packet_sync(octets: &[u8]) -> Packet {
        let mut curseur = 0usize;
        let kind = octets[curseur] & 0xF0;
        let flags = octets[curseur] & 0x0F;
        curseur += 1;
        let mut longueur = 0usize;
        let mut multiplicateur = 1usize;
        loop {
            let octet = octets[curseur];
            curseur += 1;
            longueur += usize::from(octet & 0x7F) * multiplicateur;
            if octet & 0x80 == 0 {
                break;
            }
            multiplicateur *= 128;
        }
        Packet {
            kind,
            flags,
            payload: octets[curseur..curseur + longueur].to_vec(),
        }
    }

    /// Faux broker : accepte une connexion, repond CONNACK et SUBACK, publie un
    /// message QoS 1, puis verifie le PUBACK.
    async fn faux_broker(ecoute: TcpListener) -> Vec<u8> {
        let (mut socket, _) = ecoute.accept().await.unwrap();
        let connect = read_packet(&mut socket).await.unwrap();
        assert_eq!(connect.kind, CONNECT);
        socket
            .write_all(&[CONNACK, 0x02, 0x00, 0x00])
            .await
            .unwrap();

        let subscribe = read_packet(&mut socket).await.unwrap();
        assert_eq!(subscribe.kind, SUBSCRIBE);
        let id = u16::from_be_bytes([subscribe.payload[0], subscribe.payload[1]]);
        socket
            .write_all(&[SUBACK, 0x03, (id >> 8) as u8, id as u8, 0x01])
            .await
            .unwrap();

        let charge = br#"{"t":7,"lat":48.85,"lon":2.35,"st":"run"}"#;
        socket
            .write_all(&publish_packet(
                "mpacer/live/montre-a1b2",
                charge,
                1,
                true,
                7,
            ))
            .await
            .unwrap();

        let ack = read_packet(&mut socket).await.unwrap();
        ack.payload
    }

    #[tokio::test]
    async fn subscriber_receives_a_publish_and_acknowledges_it() {
        let ecoute = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let adresse = ecoute.local_addr().unwrap();
        let serveur = tokio::spawn(faux_broker(ecoute));

        let (sink, mut messages) = mpsc::channel(8);
        let status = Arc::new(MqttStatus::default());
        let tache = spawn(
            MqttOptions {
                url: format!("mqtt://{adresse}"),
                client_id: "mpacer-api-test".to_string(),
                topics: vec![("mpacer/live/+".to_string(), 1)],
                keep_alive_s: 30,
                username: None,
                password: None,
            },
            sink,
            Arc::clone(&status),
        );

        let message = tokio::time::timeout(Duration::from_secs(5), messages.recv())
            .await
            .expect("message attendu dans les cinq secondes")
            .expect("canal ouvert");
        assert_eq!(message.topic, "mpacer/live/montre-a1b2");
        assert!(String::from_utf8_lossy(&message.payload).contains("\"lat\":48.85"));
        assert!(status.connected());
        assert_eq!(status.messages(), 1);
        assert!(status.last_message_ms() > 0);

        tache.abort();
        let puback = serveur.await.unwrap();
        assert_eq!(puback, vec![0x00, 0x07], "PUBACK du message QoS 1");
    }
}
