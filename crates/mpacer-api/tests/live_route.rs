//! Route du parcours planifie : le routage est verifie sans base ni reseau.
//!
//! Le nom d'appareil doit etre revendique avant qu'un parcours puisse y etre
//! accroche ; cette verification-la demande la base et vit dans les tests
//! d'integration. Ici, on prouve seulement que la route existe, qu'elle n'accepte
//! que PUT et qu'elle exige un jeton, comme le reste de l'API.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
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

async fn status(app: &Router, methode: Method, uri: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method(methode)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn the_planned_route_route_is_registered_and_guarded() {
    let app = app();

    // Sans jeton d'appareil, la publication d'un parcours est refusee.
    assert_eq!(
        status(&app, Method::PUT, "/api/v1/live/route").await,
        StatusCode::UNAUTHORIZED
    );

    // GET n'est pas la bonne methode : 405 (route connue) et non 404.
    assert_eq!(
        status(&app, Method::GET, "/api/v1/live/route").await,
        StatusCode::METHOD_NOT_ALLOWED
    );

    // Un chemin voisin reste introuvable : les deux cas ne se confondent pas.
    assert_eq!(
        status(&app, Method::GET, "/api/v1/live/inconnu").await,
        StatusCode::NOT_FOUND
    );
}
