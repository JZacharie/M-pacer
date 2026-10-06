//! Test d'integration contre un **vrai** broker MQTT (ignore par defaut).
//!
//! Il rejoue le chemin de production : le client de \`mpacer_api::mqtt\` s'abonne,
//! puis un second client minimal publie une position telle que la montre
//! l'envoie, et le message doit revenir intact. Les valeurs se lisent dans
//! l'environnement, jamais dans le code (voir \`.env.example\`) :
//!
//! \`\`\`text
//! MPACER_MQTT_TEST_URL="mqtt://utilisateur:motdepasse@192.168.0.115:1883" \
//!   cargo test -p mpacer-api --test mqtt_live -- --ignored --nocapture
//! \`\`\`
//!
//! Sans \`MPACER_MQTT_TEST_URL\`, le test se contente de l'annoncer : la CI n'a
//! jamais besoin d'un broker.

use mpacer_api::mqtt::{self, MqttOptions, MqttStatus};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

/// Delai maximal accorde a la liaison avec le broker.
const DELAI_CONNEXION: Duration = Duration::from_secs(10);
/// Delai maximal accorde au message.
const DELAI_MESSAGE: Duration = Duration::from_secs(10);

#[tokio::test]
#[ignore = "acces reseau : necessite un broker MQTT reel (MPACER_MQTT_TEST_URL)"]
async fn a_real_broker_carries_the_watch_position() {
    let Some(url) = url_de_test() else {
        eprintln!("MPACER_MQTT_TEST_URL non defini : test ignore");
        return;
    };
    let topic = std::env::var("MPACER_MQTT_TEST_TOPIC")
        .unwrap_or_else(|_| "mpacer/live/mpacer-test".to_string());
    // Charge utile unique : un message retenu d'une execution precedente ne peut
    // pas se faire passer pour la reponse attendue.
    let charge = format!(
        "{{\"t\":{},\"lat\":48.856600,\"lon\":2.352200,\"acc\":4.0,\"st\":\"run\",\"dev\":\"test-{}\"}}",
        chrono::Utc::now().timestamp_millis(),
        std::process::id()
    )
    .into_bytes();

    let (sink, mut messages) = mpsc::channel(8);
    let status = Arc::new(MqttStatus::default());
    let abonne = mqtt::spawn(
        MqttOptions {
            url: url.clone(),
            client_id: format!("mpacer-test-{}", std::process::id()),
            topics: vec![(topic.clone(), 1)],
            keep_alive_s: 30,
            username: None,
            password: None,
        },
        sink,
        Arc::clone(&status),
    );

    // 1. La liaison s'etablit (CONNECT/CONNACK puis SUBSCRIBE/SUBACK).
    let connecte = tokio::time::timeout(DELAI_CONNEXION, async {
        while !status.connected() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(connecte.is_ok(), "le broker n'a pas accepte la liaison");
    assert_eq!(status.connections(), 1);

    // 2. Un client minimal publie une position, comme le fait la montre.
    publier(&url, &topic, &charge).await;

    // 3. Le client de production la recoit, octet pour octet.
    let message = tokio::time::timeout(DELAI_MESSAGE, async {
        while let Some(message) = messages.recv().await {
            if message.payload == charge {
                return message;
            }
        }
        panic!("canal ferme avant le message attendu");
    })
    .await
    .expect("message attendu dans les dix secondes");
    assert_eq!(message.topic, topic);
    assert!(status.messages() >= 1);

    // 4. Menage : le message retenu de test est efface du broker.
    publier(&url, &topic, b"").await;
    abonne.abort();
}

/// URL du broker de test, ou None si le test doit etre ignore.
fn url_de_test() -> Option<String> {
    std::env::var("MPACER_MQTT_TEST_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

/// Client MQTT minimal : CONNECT puis PUBLISH retenu, puis fermeture.
async fn publier(url: &str, topic: &str, charge: &[u8]) {
    let broker = mqtt::BrokerUrl::parse(url).expect("adresse de broker exploitable");
    let mut flux = TcpStream::connect((broker.host.as_str(), broker.port))
        .await
        .expect("connexion TCP au broker");
    flux.write_all(&mqtt::connect_packet(
        "mpacer-test-pub",
        broker.username.as_deref(),
        broker.password.as_deref(),
        30,
    ))
    .await
    .expect("envoi du CONNECT");
    let connack = mqtt::read_packet(&mut flux).await.expect("CONNACK");
    assert_eq!(connack.kind, 0x20, "CONNACK attendu");
    assert_eq!(
        connack.payload.last().copied(),
        Some(0),
        "le broker a refuse les identifiants"
    );
    flux.write_all(&mqtt::publish_packet(topic, charge, 0, true, 0))
        .await
        .expect("envoi du PUBLISH");
    flux.flush().await.expect("vidage du tampon");
    let _ = flux.shutdown().await;
}
