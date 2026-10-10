//! L'endpoint `/metrics` : ce que Prometheus lira sur le pod.
//!
//! Le test monte le routeur complet (comme `tests/live_route.rs`) et verifie
//! deux choses : le texte est bien au format d'exposition Prometheus, et une
//! requete reelle y apparait, rattachee a sa route et a son code de retour.
//! Aucune base de donnees n'est necessaire : `/healthz` ne touche pas au pool,
//! et la jauge de stockage audio echec proprement (journalise, sans casser le
//! scrape) quand PostgreSQL n'est pas joignable.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use mpacer_api::auth::google::UnconfiguredOidc;
use mpacer_api::config::Config;
use mpacer_api::routes;
use mpacer_api::state::AppState;
use std::sync::Arc;
use tower::ServiceExt;

fn app() -> Router {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://mpacer:mpacer@127.0.0.1:5432/mpacer")
        .expect("URL PostgreSQL de test");
    let config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
    let state = AppState::with_oidc(pool, Arc::new(config), Arc::new(UnconfiguredOidc));
    routes::router(state)
}

async fn get(app: &Router, uri: &str) -> (StatusCode, String) {
    let reponse = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("requete valide"),
        )
        .await
        .expect("le routeur repond");
    let code = reponse.status();
    let octets = axum::body::to_bytes(reponse.into_body(), usize::MAX)
        .await
        .expect("corps lisible");
    (code, String::from_utf8_lossy(&octets).into_owned())
}

#[tokio::test]
async fn the_metrics_endpoint_renders_prometheus_text() {
    let app = app();

    // Une requete reelle, pour que le middleware ait quelque chose a compter.
    let (code, _) = get(&app, "/healthz").await;
    assert_eq!(code, StatusCode::OK);

    let (code, texte) = get(&app, "/metrics").await;
    assert_eq!(code, StatusCode::OK);

    // Format d'exposition Prometheus : type annonce pour chaque famille.
    assert!(
        texte.contains("# TYPE mpacer_http_requests_total counter"),
        "compteur de requetes annonce : {texte}"
    );
    assert!(
        texte.contains("# TYPE mpacer_http_request_duration_seconds histogram"),
        "histogramme de duree annonce : {texte}"
    );
    assert!(
        texte.contains("# TYPE mpacer_db_pool_active_connections gauge"),
        "jauge du pool annoncee : {texte}"
    );

    // La requete /healthz est comptee sous sa route (jamais l'URI brute).
    assert!(
        texte.contains(r#"handler="/healthz""#) && texte.contains(r#"code="200""#),
        "la requete est comptee par route et par code : {texte}"
    );

    // Les jauges d'etat sont relues a chaque scrape, sans base de donnees.
    assert!(
        texte.contains("mpacer_active_live_devices"),
        "jauges du suivi en direct : {texte}"
    );
    assert!(
        texte.contains("mpacer_db_pool_max_connections"),
        "plafond du pool expose : {texte}"
    );
    assert!(
        texte.contains("mpacer_media_quota_bytes"),
        "quota audio expose : {texte}"
    );
    assert!(
        texte.contains("mpacer_mqtt_connected"),
        "etat de la liaison MQTT expose : {texte}"
    );
}

#[tokio::test]
async fn a_missing_route_does_not_create_an_unbounded_series() {
    let app = app();

    let (code, _) = get(&app, "/cette-route-nexiste-pas").await;
    assert_eq!(code, StatusCode::NOT_FOUND);

    let (_, texte) = get(&app, "/metrics").await;
    // Un 404 est rattache au libelle fixe "inconnu" : aucun chemin arbitraire
    // ne doit apparaitre comme etiquette.
    assert!(
        texte.contains(r#"handler="inconnu""#),
        "les routes inconnues sont regroupees : {texte}"
    );
    assert!(
        !texte.contains("cette-route-nexiste-pas"),
        "l'URI brute ne doit jamais devenir une etiquette : {texte}"
    );
}
