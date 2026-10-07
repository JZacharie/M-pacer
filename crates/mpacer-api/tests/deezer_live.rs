//! Test d'integration contre l'**API Deezer reelle**, par cookie `arl` (ignore
//! par defaut).
//!
//! Il exerce le chemin de production : `deezer::arl_session` ouvre la session,
//! puis les playlists du compte sont listees et la fiche d'une playlist est lue
//! avec l'identifiant Deezer de chaque piste — c'est cet identifiant qui
//! construit le lien Deemix affiche dans la liste des MP3. Le cookie se lit dans
//! l'environnement, jamais dans le code :
//!
//! ```text
//! MPACER_DEEZER_ARL="<cookie arl>" cargo test -p mpacer-api --test deezer_live -- --ignored --nocapture
//! ```
//!
//! Sans `MPACER_DEEZER_ARL`, le test se contente de l'annoncer : la CI n'a
//! jamais besoin d'un compte Deezer.

use mpacer_api::deezer;

/// Cookie `arl` de test, s'il est fourni par l'environnement.
fn arl() -> Option<String> {
    std::env::var("MPACER_DEEZER_ARL")
        .ok()
        .map(|valeur| valeur.trim().to_string())
        .filter(|valeur| valeur.len() >= 32)
}

#[tokio::test]
#[ignore = "acces reseau : necessite un cookie arl reel (MPACER_DEEZER_ARL)"]
async fn the_arl_cookie_lists_the_playlists_and_their_track_ids() {
    let Some(arl) = arl() else {
        eprintln!("MPACER_DEEZER_ARL non defini : test ignore");
        return;
    };
    let http = reqwest::Client::new();
    let mut session = deezer::arl_session(&http, &arl)
        .await
        .expect("session arl refusee");
    println!("compte {} ({})", session.user_id, session.label());

    let playlists = deezer::list_user_playlists_arl(&http, &mut session)
        .await
        .expect("playlists du compte");
    assert!(
        !playlists.is_empty(),
        "aucune playlist : le cookie est peut-etre un compte vide"
    );
    for playlist in playlists.iter().take(25) {
        println!(
            " - {} | {} | {} titres | {:?}",
            playlist.id, playlist.name, playlist.track_count, playlist.owner
        );
    }

    // La plus grande playlist : c'est elle qui exerce la pagination.
    let target = playlists
        .iter()
        .filter(|playlist| playlist.track_count > 0)
        .max_by_key(|playlist| playlist.track_count)
        .expect("aucune playlist non vide");
    let detail = deezer::get_playlist_arl(&http, &mut session, &target.id)
        .await
        .expect("fiche de playlist");
    assert!(!detail.tracks.is_empty(), "playlist sans piste lue");
    assert_eq!(
        detail.tracks.len() as i64,
        target.track_count,
        "toutes les pistes annoncees par Deezer doivent etre lues (pagination)"
    );
    assert!(
        detail.tracks.iter().all(|track| track.id.is_some()),
        "sans identifiant Deezer, aucun lien Deemix ne serait possible"
    );
    let first = &detail.tracks[0];
    println!(
        "playlist « {} » : {} pistes lues ; premiere : {} - {:?} ({:?} s, id {:?})",
        detail.name,
        detail.tracks.len(),
        first.title,
        first.artist,
        first.duration_s,
        first.id
    );
}

#[tokio::test]
#[ignore = "acces reseau : necessite un cookie arl reel (MPACER_DEEZER_ARL)"]
async fn the_arl_cookie_searches_public_playlists() {
    let Some(arl) = arl() else {
        eprintln!("MPACER_DEEZER_ARL non defini : test ignore");
        return;
    };
    let http = reqwest::Client::new();
    let mut session = deezer::arl_session(&http, &arl)
        .await
        .expect("session arl refusee");
    let results = deezer::search_playlists_arl(&http, &mut session, "running")
        .await
        .expect("recherche de playlists");
    assert!(!results.is_empty(), "aucun resultat pour « running »");
    println!(
        "{} resultat(s) de recherche pour « running »",
        results.len()
    );
    for playlist in results.iter().take(3) {
        println!(" - {} | {}", playlist.id, playlist.name);
    }
}
