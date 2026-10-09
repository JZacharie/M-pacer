//! Test d'integration contre l'index public de Finishers (reseau requis).
//!
//! Ignore par defaut, comme les tests MQTT et Deezer :
//!
//! ```text
//! cargo test -p mpacer-api --test finishers_live -- --ignored
//! ```

use mpacer_api::config::Config;
use mpacer_api::finishers::{Finishers, RaceSearchParams, RaceSearchQuery};

/// Client active : l'index public de Finishers, en lecture seule.
fn client() -> Finishers {
    let mut config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
    config.finishers_enabled = true;
    Finishers::from_config(reqwest::Client::new(), &config)
}

#[tokio::test]
#[ignore = "reseau requis : interroge l'index public de Finishers"]
async fn the_french_calendar_answers_a_filtered_search() {
    let client = client();
    let query = RaceSearchQuery::parse(&RaceSearchParams {
        region: Some("Occitanie".to_string()),
        discipline: Some("trail".to_string()),
        dmin: Some("40".to_string()),
        taille: Some("5".to_string()),
        ..Default::default()
    })
    .unwrap();

    let results = client.search(&query).await.expect("recherche Finishers");
    assert_eq!(results.source, "finishers.com");
    assert!(results.total > 0, "le calendrier doit repondre");
    assert!(!results.items.is_empty(), "au moins une course sur la page");

    for race in &results.items {
        assert!(!race.name.is_empty());
        assert_eq!(race.region.as_deref(), Some("Occitanie"), "{}", race.name);
        assert_eq!(race.discipline.as_deref(), Some("trail"), "{}", race.name);
        assert!(
            race.url.starts_with("https://www.finishers.com/course/"),
            "{}",
            race.url
        );
        assert!(
            race.distances_m
                .iter()
                .any(|distance| *distance >= 40_000.0),
            "{} : {:?}",
            race.name,
            race.distances_m
        );
    }

    // La fiche par identifiant rend exactement la meme epreuve.
    let first = &results.items[0];
    let found = client
        .event(&first.id)
        .await
        .expect("fiche Finishers")
        .expect("epreuve connue");
    assert_eq!(found.id, first.id);
    assert_eq!(found.name, first.name);
}

#[tokio::test]
#[ignore = "reseau requis : interroge l'index public de Finishers"]
async fn the_next_edition_of_a_known_race_is_dated() {
    let client = client();
    let query = RaceSearchQuery::parse(&RaceSearchParams {
        q: Some("Marathon de Paris".to_string()),
        ..Default::default()
    })
    .unwrap();
    let results = client.search(&query).await.expect("recherche Finishers");
    let race = results.items.first().expect("le marathon de Paris existe");
    assert!(
        race.start_date.is_some() && race.start_at_ms.is_some(),
        "{} doit avoir une date d'edition",
        race.name
    );
    assert!(race.city.is_some(), "{} doit avoir une ville", race.name);
}
