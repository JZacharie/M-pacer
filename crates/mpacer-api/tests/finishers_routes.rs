//! Routes de la recherche Finishers : le routage est verifie sans base ni reseau.
//!
//! Un pool PostgreSQL *paresseux* suffit : les routes interrogees ici ne touchent
//! pas la base, et aucun appel ne part vers le calendrier Finishers (la source est
//! eteinte dans la configuration de test).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use mpacer_api::auth::google::UnconfiguredOidc;
use mpacer_api::config::Config;
use mpacer_api::routes;
use mpacer_api::state::AppState;
use std::sync::Arc;
use tower::ServiceExt;

/// Application de test : aucune connexion n'est ouverte.
fn app() -> Router {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://mpacer:mpacer@127.0.0.1:5432/mpacer")
        .expect("URL PostgreSQL de test");
    let config = Config::for_tests("http://localhost:8080", "postgresql://exemple");
    let state = AppState::with_oidc(pool, Arc::new(config), Arc::new(UnconfiguredOidc));
    routes::router(state)
}

async fn status(app: &Router, uri: &str) -> StatusCode {
    app.clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

/// Construire le routeur suffit a prouver qu'aucun chemin ne se confond avec
/// `/api/v1/races/{id}` ni avec `/courses/{id}` : axum panique sinon.
#[tokio::test]
async fn the_finishers_routes_are_registered() {
    let app = app();

    // API JSON : le jeton d'appareil est exige comme partout ailleurs.
    assert_eq!(
        status(&app, "/api/v1/races/search?q=marathon").await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(&app, "/api/v1/races/finishers/abc").await,
        StatusCode::UNAUTHORIZED
    );

    // Interface web : la recherche et le pre-remplissage renvoient a la connexion.
    assert_eq!(
        status(&app, "/courses/recherche").await,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        status(
            &app,
            "/courses/recherche?q=marathon&region=Occitanie&dmin=40"
        )
        .await,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        status(&app, "/courses/nouvelle?finishers=abc&distance=42195").await,
        StatusCode::SEE_OTHER
    );
}
