//! Metriques d'exploitation, exposees au format texte Prometheus sur `/metrics`.
//!
//! Le service tourne en un seul pod k3s : la supervision se resume a quatre
//! questions, celles du document 18.
//!
//!  * **les requetes repondent-elles ?** volume et latence par gestionnaire HTTP ;
//!  * **le pool PostgreSQL respire-t-il ?** connexions actives, inactives, plafond ;
//!  * **le suivi en direct tient-il ?** appareils publies, liaison au broker ;
//!  * **le volume audio deborde-t-il ?** octets stockes face au quota configure.
//!
//! Aucun second port et aucun second serveur : l'exportateur Prometheus est pris
//! **sans** son ecouteur HTTP (feature `http-listener` desactivee) et son texte
//! est rendu par le routeur Axum, sur le port de l'API. C'est le ServiceMonitor
//! du chart qui scrape le Service (ClusterIP) ; un operateur qui ne veut pas
//! exposer `/metrics` publiquement le bloque au niveau de l'ingress (voir
//! `deploy/README.md`).
//!
//! Cout : le middleware n'incremente que des compteurs atomiques. Les jauges
//! d'etat (pool, suivi, stockage) ne sont lues **qu'au scrape**, et la lecture
//! du stockage audio est bornee dans le temps : un scrape ne peut pas se
//! transformer en requete lente.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use axum::extract::{MatchedPath, Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

use crate::state::AppState;

/// Chemin scrape par Prometheus.
pub const ENDPOINT: &str = "/metrics";

/// Delai maximal accorde a la lecture du stockage audio pendant un scrape.
const DELAI_JAUGE: Duration = Duration::from_secs(2);

/// Poignee du recorder, installee une seule fois par processus.
static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Installe l'exportateur (idempotent) et renvoie sa poignee.
///
/// Sans recorder installe, les macros `metrics::*` seraient des no-op : le
/// service resterait sain, mais `/metrics` serait vide. L'installation est donc
/// explicite au demarrage (voir `main`), et refaite au premier scrape si elle a
/// ete oubliee (tests, binaires secondaires).
pub fn install() -> PrometheusHandle {
    HANDLE
        .get_or_init(|| {
            let handle = PrometheusBuilder::new()
                // Bornes adaptees a un service qui interroge PostgreSQL et un
                // broker MQTT : la quasi-totalite des requetes tient sous 50 ms.
                .set_buckets_for_metric(
                    Matcher::Suffix("_seconds".to_string()),
                    &[
                        0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
                    ],
                )
                .unwrap_or_else(|erreur| {
                    // Des bornes invalides seraient un bogue de programmation,
                    // pas une raison d'arreter le service : on garde celles par
                    // defaut.
                    tracing::error!(erreur = %erreur, "metriques : bornes d'histogramme refusees");
                    PrometheusBuilder::new()
                })
                .install_recorder()
                .unwrap_or_else(|erreur| {
                    panic!("metriques : recorder deja installe ou refuse ({erreur})")
                });
            describe();
            handle
        })
        .clone()
}

/// Description des metriques : elle apparait en commentaire `# HELP` dans le
/// texte Prometheus, ce qui rend le tableau de bord lisible sans la doc.
fn describe() {
    metrics::describe_counter!(
        "mpacer_http_requests_total",
        "Requetes HTTP servies, par gestionnaire et code de retour."
    );
    metrics::describe_histogram!(
        "mpacer_http_request_duration_seconds",
        metrics::Unit::Seconds,
        "Duree des requetes HTTP, par gestionnaire."
    );
    metrics::describe_gauge!(
        "mpacer_db_pool_active_connections",
        "Connexions PostgreSQL actuellement utilisees."
    );
    metrics::describe_gauge!(
        "mpacer_db_pool_idle_connections",
        "Connexions PostgreSQL ouvertes et inutilisees."
    );
    metrics::describe_gauge!(
        "mpacer_db_pool_max_connections",
        "Plafond du pool PostgreSQL."
    );
    metrics::describe_gauge!(
        "mpacer_active_live_devices",
        "Montres et telephones publiant une position de suivi en direct."
    );
    metrics::describe_gauge!(
        "mpacer_mqtt_connected",
        "1 si le service est abonne au broker MQTT, 0 sinon."
    );
    metrics::describe_counter!(
        "mpacer_mqtt_messages_total",
        "Messages de suivi en direct recus du broker depuis le demarrage."
    );
    metrics::describe_gauge!(
        "mpacer_media_storage_bytes",
        "Octets audio stockes sur le serveur, tous comptes confondus."
    );
    metrics::describe_gauge!(
        "mpacer_media_quota_bytes",
        "Quota audio configure par compte."
    );
}

/// Middleware : compte chaque requete et sa duree, par gestionnaire et par code.
///
/// Le nom vient de [MatchedPath] (« /api/v1/music/playlists/{id} ») et jamais de
/// l'URI brute : les identifiants contenus dans les chemins ne doivent pas
/// exploser la cardinalite des series Prometheus.
pub async fn track(request: Request, next: Next) -> Response {
    let gestionnaire = request
        .extensions()
        .get::<MatchedPath>()
        .map(|chemin| chemin.as_str().to_owned())
        .unwrap_or_else(|| "inconnu".to_string());
    let debut = Instant::now();
    let reponse = next.run(request).await;
    metrics::counter!(
        "mpacer_http_requests_total",
        "handler" => gestionnaire.clone(),
        "code" => reponse.status().as_u16().to_string(),
    )
    .increment(1);
    metrics::histogram!(
        "mpacer_http_request_duration_seconds",
        "handler" => gestionnaire,
    )
    .record(debut.elapsed().as_secs_f64());
    reponse
}

/// Gestionnaire de `GET /metrics` : jauges d'etat, puis texte Prometheus.
pub async fn exporter(State(state): State<AppState>) -> impl IntoResponse {
    // Le recorder avant les jauges : sans lui, les macros seraient des no-op et
    // le scrape rendrait un texte vide (cas d'un binaire qui n'a pas installe
    // l'exportateur au demarrage, comme les tests du routeur).
    let poignee = install();
    collect(&state).await;
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        poignee.render(),
    )
}

/// Renseigne les jauges d'etat au moment du scrape.
pub async fn collect(state: &AppState) {
    let inactives = state.pool.num_idle() as f64;
    let ouvertes = f64::from(state.pool.size());
    metrics::gauge!("mpacer_db_pool_active_connections").set((ouvertes - inactives).max(0.0));
    metrics::gauge!("mpacer_db_pool_idle_connections").set(inactives);
    metrics::gauge!("mpacer_db_pool_max_connections")
        .set(f64::from(state.pool.options().get_max_connections()));

    metrics::gauge!("mpacer_active_live_devices").set(state.live.len() as f64);
    metrics::gauge!("mpacer_media_quota_bytes").set(state.config.media_quota_bytes as f64);

    let mqtt = state.live.status();
    metrics::gauge!("mpacer_mqtt_connected").set(if mqtt.connected() { 1.0 } else { 0.0 });
    // Compteur absolu : la valeur est deja tenue par le client MQTT depuis le
    // demarrage, on la reprend telle quelle au lieu de la recompter.
    metrics::counter!("mpacer_mqtt_messages_total").absolute(mqtt.messages());

    match tokio::time::timeout(
        DELAI_JAUGE,
        crate::db::music_storage_bytes_total(&state.pool),
    )
    .await
    {
        Ok(Ok(octets)) => metrics::gauge!("mpacer_media_storage_bytes").set(octets as f64),
        Ok(Err(erreur)) => {
            // Un scrape ne doit pas echouer parce que la base tousse : la jauge
            // garde sa derniere valeur et la cause part au journal.
            tracing::debug!(erreur = %erreur, "metriques : stockage audio illisible");
        }
        Err(_) => tracing::debug!(
            delai_ms = DELAI_JAUGE.as_millis(),
            "metriques : stockage audio trop lent a lire, jauge inchangee"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    #[tokio::test]
    async fn every_request_is_counted_under_its_route() {
        install();
        let app = Router::new()
            .route("/ok", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(track));

        let reponse = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/ok")
                    .body(Body::empty())
                    .expect("requete valide"),
            )
            .await
            .expect("le routeur repond");
        assert_eq!(reponse.status(), StatusCode::OK);

        let texte = install().render();
        assert!(
            texte.contains("# HELP mpacer_http_requests_total")
                && texte.contains("# TYPE mpacer_http_requests_total counter"),
            "la metrique est decrite : {texte}"
        );
        assert!(
            texte.contains(r#"handler="/ok""#) && texte.contains(r#"code="200""#),
            "la requete est comptee sous sa route et son code : {texte}"
        );
        assert!(
            texte.contains("mpacer_http_request_duration_seconds_bucket"),
            "la duree est un histogramme : {texte}"
        );
    }
}
