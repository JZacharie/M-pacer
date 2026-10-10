//! Decharge d'une rafale de positions : le chemin complet du suivi en direct.
//!
//! Ce test rejoue, sans broker externe, ce qui se produit quand la montre
//! retrouve le reseau apres une coupure : un faux broker MQTT en memoire
//! accepte le client de production ([mpacer_api::mqtt]), puis **decharge une
//! rafale de positions dans le desordre**. Les messages traversent le vrai
//! client MQTT et [mpacer_api::live::LiveStore], et la trace doit ressortir
//! ordonnee, complete et sans doublon.
//!
//! Aucun acces reseau externe : le broker ecoute sur 127.0.0.1.

use std::sync::Arc;
use std::time::Duration;

use mpacer_api::live::LiveStore;
use mpacer_api::mqtt::{self, MqttOptions, MqttStatus};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

/// Delai maximal accorde a la rafale.
const DELAI: Duration = Duration::from_secs(5);

/// Faux broker : poignee de main, puis positions dans l'ordre recu.
async fn faux_broker(ecoute: TcpListener, sujet: String, positions: Vec<i64>) {
    let (mut socket, _) = ecoute.accept().await.expect("connexion du client MQTT");
    let connect = mqtt::read_packet(&mut socket).await.expect("CONNECT");
    assert_eq!(connect.kind, 0x10, "CONNECT attendu");
    socket
        .write_all(&[0x20, 0x02, 0x00, 0x00])
        .await
        .expect("CONNACK");

    let subscribe = mqtt::read_packet(&mut socket).await.expect("SUBSCRIBE");
    assert_eq!(subscribe.kind, 0x80, "SUBSCRIBE attendu");
    let id = u16::from_be_bytes([subscribe.payload[0], subscribe.payload[1]]);
    socket
        .write_all(&[0x90, 0x03, (id >> 8) as u8, id as u8, 0x00])
        .await
        .expect("SUBACK");

    // La montre vide sa file : plusieurs points d'affilee, avec les horodatages
    // qu'ils avaient pendant la coupure. QoS 0 : un point de suivi perdu est
    // remplace par le suivant.
    for t_ms in positions {
        let charge =
            format!("{{\"t\":{t_ms},\"lat\":48.85,\"lon\":2.35,\"acc\":4.0,\"st\":\"run\"}}");
        socket
            .write_all(&mqtt::publish_packet(
                &sujet,
                charge.as_bytes(),
                0,
                false,
                0,
            ))
            .await
            .expect("PUBLISH");
        socket.flush().await.expect("vidage du tampon");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // Laisse le client lire la rafale avant de fermer la liaison.
    tokio::time::sleep(Duration::from_millis(300)).await;
}

#[tokio::test]
async fn a_burst_arriving_out_of_order_fills_an_ordered_trace() {
    let ecoute = TcpListener::bind("127.0.0.1:0").await.expect("port libre");
    let adresse = ecoute.local_addr().expect("adresse locale");
    let sujet = "mpacer/live/montre-burst".to_string();
    // Le reseau a reordonne la rafale : l'ingestion doit la remettre d'aplomb.
    let serveur = tokio::spawn(faux_broker(
        ecoute,
        sujet.clone(),
        vec![4_000, 1_000, 3_000, 2_000],
    ));

    let (sink, mut messages) = mpsc::channel(32);
    let status = Arc::new(MqttStatus::default());
    let abonne = mqtt::spawn(
        MqttOptions {
            url: format!("mqtt://{adresse}"),
            client_id: "mpacer-api-burst".to_string(),
            topics: vec![("mpacer/live/+".to_string(), 0)],
            keep_alive_s: 30,
            username: None,
            password: None,
        },
        sink,
        Arc::clone(&status),
    );

    let store = Arc::new(LiveStore::new());
    let magasin = Arc::clone(&store);
    let ingestion = tokio::spawn(async move {
        while let Some(message) = messages.recv().await {
            magasin
                .ingest(&message.topic, &message.payload)
                .expect("position valide");
        }
    });

    // Attend que les quatre points de la rafale soient arrives.
    let vue = tokio::time::timeout(DELAI, async {
        loop {
            if let Some(vue) = store.snapshot(10_000, 4).into_iter().next() {
                if vue.received >= 4 {
                    return vue;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("les quatre positions arrivent dans les cinq secondes");

    assert_eq!(vue.device, "montre-burst");
    assert_eq!(vue.received, 4, "aucun point de la rafale n'est perdu");
    assert_eq!(
        vue.trace.iter().map(|p| p.t_ms).collect::<Vec<_>>(),
        vec![1_000, 2_000, 3_000, 4_000],
        "la trace ressort dans l'ordre chronologique"
    );
    assert_eq!(
        vue.last_ms, 4_000,
        "la position courante est la plus recente"
    );
    assert_eq!(
        vue.last.as_ref().map(|p| p.t_ms),
        Some(4_000),
        "le dernier point du trace est la position courante"
    );
    assert!(status.connected(), "la liaison au broker est etablie");
    assert_eq!(status.messages(), 4);

    abonne.abort();
    ingestion.abort();
    serveur.await.expect("serveur de test");
}
