//! Test d'integration contre une instance **Deemix** reelle (ignore par defaut).
//!
//! Il exerce le chemin de production : le service ouvre la session Deezer de
//! l'instance avec le cookie `arl`, envoie une reference Deezer dans la file,
//! puis relit la file. Les valeurs se lisent dans l'environnement, jamais dans le
//! code :
//!
//! ```text
//! MPACER_DEEMIX_URL=https://deemix.p.zacharie.org \
//! MPACER_DEEMIX_USER=<utilisateur> MPACER_DEEMIX_PASSWORD=<mot de passe> \
//! MPACER_DEEZER_ARL=<cookie arl> \
//! MPACER_DEEMIX_TEST_URL=https://www.deezer.com/track/3402267041 \
//!   cargo test -p mpacer-api --test deemix_live -- --ignored --nocapture
//! ```
//!
//! Sans ces variables, le test se contente de l'annoncer : la CI n'a jamais
//! besoin d'une instance Deemix.

use mpacer_api::config::Config;
use mpacer_api::deemix;

/// Configuration Deemix de test, si l'environnement la fournit.
fn config() -> Option<Config> {
    let url = std::env::var("MPACER_DEEMIX_URL").ok()?;
    let user = std::env::var("MPACER_DEEMIX_USER").ok()?;
    let password = std::env::var("MPACER_DEEMIX_PASSWORD").ok()?;
    let arl = std::env::var("MPACER_DEEZER_ARL").ok()?;
    let mut config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
    config.deemix_url = Some(url);
    config.deemix_user = Some(user);
    config.deemix_password = Some(password);
    config.deezer_arl = Some(arl);
    Some(config)
}

#[tokio::test]
#[ignore = "acces reseau : necessite une instance Deemix reelle (MPACER_DEEMIX_*)"]
async fn the_deemix_queue_is_readable() {
    let Some(config) = config() else {
        eprintln!("MPACER_DEEMIX_* non definis : test ignore");
        return;
    };
    assert!(config.deemix_configured(), "configuration incomplete");
    let http = reqwest::Client::new();
    let entries = deemix::queue(&http, &config)
        .await
        .expect("file de Deemix illisible");
    println!("{} entree(s) dans la file Deemix", entries.len());
    for entry in entries.iter().take(10) {
        println!(
            " - {} | {} | {} | {}/{} | {} %",
            entry.kind,
            entry.title,
            entry.artist.clone().unwrap_or_default(),
            entry.downloaded,
            entry.size,
            entry.progress
        );
    }
}

#[tokio::test]
#[ignore = "acces reseau : necessite une instance Deemix reelle (MPACER_DEEMIX_*)"]
async fn a_deezer_reference_is_sent_to_the_deemix_queue() {
    let Some(config) = config() else {
        eprintln!("MPACER_DEEMIX_* non definis : test ignore");
        return;
    };
    let Some(url) = std::env::var("MPACER_DEEMIX_TEST_URL").ok() else {
        eprintln!("MPACER_DEEMIX_TEST_URL non defini : test ignore");
        return;
    };
    let http = reqwest::Client::new();
    let before = deemix::queue(&http, &config)
        .await
        .expect("file de Deemix illisible");
    let added = deemix::add_to_queue(&http, &config, &url)
        .await
        .expect("envoi dans Deemix refuse");
    let after = deemix::queue(&http, &config)
        .await
        .expect("file de Deemix illisible");
    println!(
        "{} entree(s) ajoutee(s) pour {url} ; file : {} -> {}",
        added,
        before.len(),
        after.len()
    );
    let reference = url.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    assert!(
        after.iter().any(|entry| entry.id == reference),
        "la reference envoyee ne figure pas dans la file : {after:?}"
    );
}
