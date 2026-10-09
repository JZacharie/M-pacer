//! Tests d'integration : flux complet montre <-> backend <-> navigateur.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use mpacer_api::auth::google::{ExchangeFuture, GoogleUser, OidcProvider};
use mpacer_api::config::Config;
use mpacer_api::routes;
use mpacer_api::state::AppState;
use std::str::FromStr;
use std::sync::Arc;
use tower::ServiceExt;

/// Fournisseur d'identite de test : aucun appel reseau.
struct FakeOidc;

impl OidcProvider for FakeOidc {
    fn authorize_url(&self, redirect_uri: &str, state: &str, code_challenge: &str) -> String {
        format!("https://fake.test/authorize?redirect_uri={redirect_uri}&state={state}&code_challenge={code_challenge}")
    }

    fn exchange<'a>(
        &'a self,
        code: &'a str,
        _verifier: &'a str,
        _redirect_uri: &'a str,
    ) -> ExchangeFuture<'a> {
        Box::pin(async move {
            if code == "bon-code" {
                Ok(GoogleUser {
                    sub: "google-sub-1".to_string(),
                    email: "coureur@example.org".to_string(),
                    name: Some("Coureur Test".to_string()),
                    picture: None,
                    email_verified: true,
                })
            } else {
                anyhow::bail!("code refuse par le faux fournisseur")
            }
        })
    }
}

/// Ignore le test si aucune base PostgreSQL n'est fournie.
///
/// ```text
/// MPACER_TEST_DATABASE_URL=postgresql://mpacer:motdepasse@127.0.0.1:15432/mpacer?sslmode=disable cargo test
/// ```
macro_rules! app_or_skip {
    ($value:expr) => {
        match $value {
            Some(pair) => pair,
            None => {
                eprintln!("MPACER_TEST_DATABASE_URL non defini : test ignore (PostgreSQL requis)");
                return;
            }
        }
    };
}

/// Nom de schema unique derive du nom du test (les tests tournent en parallele).
fn current_test_schema() -> String {
    let raw = std::thread::current()
        .name()
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "anon_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )
        });
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("test_{}", cleaned.chars().take(40).collect::<String>())
}

/// Application de test isolee : un schema PostgreSQL dedie par test.
///
/// Aucun droit particulier n'est requis (le role applicatif n'a pas `CREATEDB`) :
/// le schema est cree, utilise via `search_path`, puis recree a chaque execution.
async fn test_app(dev_auth: bool) -> Option<(Router, AppState)> {
    test_app_with(dev_auth, false).await
}

/// Application de test avec une application Deezer (OAuth) configuree.
///
/// Aucun appel reseau ne part tant qu'aucun compte n'est lie : la page propose
/// seulement la connexion et l'OAuth s'arrete a l'URL d'autorisation.
async fn test_app_deezer_oauth(dev_auth: bool) -> Option<(Router, AppState)> {
    test_app_with(dev_auth, true).await
}

async fn test_app_with(dev_auth: bool, deezer_oauth: bool) -> Option<(Router, AppState)> {
    let url = std::env::var("MPACER_TEST_DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())?;
    let schema = current_test_schema();

    let admin = sqlx::PgPool::connect(&url)
        .await
        .expect("PostgreSQL de test injoignable (verifiez MPACER_TEST_DATABASE_URL)");
    sqlx::query(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"))
        .execute(&admin)
        .await
        .expect("nettoyage du schema de test");
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .expect("creation du schema de test");
    admin.close().await;

    // Le schema est place en tete du search_path : le DDL du coeur s'y applique.
    // (`options` est un parametre de connexion PostgreSQL, passe dans l'URL.)
    let separator = if url.contains('?') { '&' } else { '?' };
    let scoped_url = format!("{url}{separator}options=-c%20search_path%3D{schema}");
    let options =
        sqlx::postgres::PgConnectOptions::from_str(&scoped_url).expect("URL PostgreSQL invalide");
    let pool = mpacer_api::db::connect_with_options(options)
        .await
        .expect("initialisation du schema applicatif");

    let mut config = Config::for_tests("http://localhost:8080", &url);
    config.dev_auth = dev_auth;
    if deezer_oauth {
        config.deezer_app_id = Some("123456".to_string());
        config.deezer_app_secret = Some("secret-de-test".to_string());
    }
    let state = AppState::with_oidc(pool, Arc::new(config), Arc::new(FakeOidc));
    Some((routes::router(state.clone()), state))
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8_lossy(&bytes).to_string()
}

fn json_request(method: &str, uri: &str, payload: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn get_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap()
}

/// Seance de reference, identique a ce que produit `mpacer-core`.
fn sample_workout() -> serde_json::Value {
    serde_json::json!({
        "id": "1700000000000",
        "started_at_ms": 1_700_000_000_000_i64,
        "duration_s": 3000.0,
        "distance_m": 10000.0,
        "average_pace_s_per_km": 300.0,
        "laps": [
            { "index": 1, "distance_m": 1000.0, "duration_s": 306.0, "pace_s_per_km": 306.0 },
            { "index": 2, "distance_m": 1000.0, "duration_s": 300.0, "pace_s_per_km": 300.0 }
        ],
        "best_efforts": [
            { "label": "1 km", "distance_m": 1000.0, "time_s": 300.0, "start_dist_m": 1000.0 }
        ],
        "track": [
            { "t_ms": 0, "dist_m": 0.0, "lat": 45.0, "lon": 3.0, "elevation_m": 300.0 },
            { "t_ms": 300000, "dist_m": 1000.0, "lat": 45.009, "lon": 3.0, "elevation_m": null }
        ],
        "unit_system": "Metric"
    })
}

#[tokio::test]
async fn health_and_readiness_report_database_state() {
    let (app, _state) = app_or_skip!(test_app(false).await);
    let response = app.clone().oneshot(get("/healthz")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("\"status\":\"ok\""), "{body}");

    let response = app.oneshot(get("/readyz")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let ready = body_text(response).await;
    assert!(ready.contains("\"database\":true"), "readyz = {ready}");
}

#[tokio::test]
async fn device_flow_pairs_a_watch_and_allows_upload() {
    let (app, state) = app_or_skip!(test_app(false).await);

    // 1. La montre demande un code.
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/device/code",
            r#"{"label":"Pixel Watch 3"}"#,
        ))
        .await
        .unwrap();
    let status = response.status();
    let body = body_text(response).await;
    assert_eq!(status, StatusCode::OK, "corps = {body}");
    let device: serde_json::Value = serde_json::from_str(&body).unwrap();
    let user_code = device["user_code"].as_str().unwrap().to_string();
    let device_code = device["device_code"].as_str().unwrap().to_string();
    assert_eq!(user_code.len(), 9);
    assert!(device["verification_uri"]
        .as_str()
        .unwrap()
        .ends_with("/link"));

    // 2. Tant que personne n'a approuve : "authorization_pending".
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/device/token",
            &format!(r#"{{"device_code":"{device_code}"}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(response).await.contains("authorization_pending"));

    // 3. L'utilisateur (cree par sa connexion Google) approuve le code.
    let user = mpacer_api::db::upsert_user(
        &state.pool,
        Some("google-sub-1"),
        "coureur@example.org",
        Some("Coureur"),
        None,
        state.now_ms(),
    )
    .await
    .unwrap();
    mpacer_api::auth::device::approve(&state, &user_code, &user.id)
        .await
        .unwrap();

    // 4. La montre echange le code contre un jeton d'appareil.
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/device/token",
            &format!(r#"{{"device_code":"{device_code}"}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let token: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    let access_token = token["access_token"].as_str().unwrap().to_string();
    assert_eq!(token["token_type"], "Bearer");
    assert_eq!(token["label"], "Pixel Watch 3");

    // 5. Le jeton identifie l'utilisateur.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/me")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("coureur@example.org"));

    // 6. Envoi d'une seance.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workouts")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::from(sample_workout().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let receipt: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(receipt["replaced"], false);

    // 7. Un second envoi remplace la seance (idempotence : la montre peut reessayer).
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workouts")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::from(sample_workout().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let receipt: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(receipt["replaced"], true);

    // 8. Listes, statistiques et export GPX.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workouts")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(list["total"], 1);
    assert_eq!(list["items"][0]["distance_m"], 10000.0);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                // Fenetre large : la seance de test est datee (novembre 2023).
                .uri("/api/v1/stats?days=3650")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let stats: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(stats["workout_count"], 1);
    assert_eq!(stats["total_distance_m"], 10000.0);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workouts/1700000000000/gpx")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .contains("gpx"));
    let gpx = body_text(response).await;
    assert!(gpx.contains("<trkpt lat=\"45.0000000\""), "{gpx}");
    assert!(gpx.contains("</gpx>"));

    // 9. Suppression.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/workouts/1700000000000")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/workouts/1700000000000")
                .header(header::AUTHORIZATION, format!("Bearer {access_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn dashboard_works_with_an_empty_history() {
    let (app, _state) = app_or_skip!(test_app(true).await);
    // Historique vide : les statistiques doivent renvoyer des zeros, pas une erreur.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/dev-login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_string();
    let session = cookie.split(';').next().unwrap().to_string();

    let response = app.oneshot(get_with_cookie("/", &session)).await.unwrap();
    let status = response.status();
    let body = body_text(response).await;
    assert_eq!(status, StatusCode::OK, "corps = {body}");
    assert!(body.contains("0 seances"), "{body}");
    assert!(body.contains("Aucune seance"), "{body}");
}

#[tokio::test]
async fn upload_requires_a_token() {
    let (app, _state) = app_or_skip!(test_app(false).await);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workouts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(sample_workout().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn invalid_workout_is_rejected() {
    let (app, state) = app_or_skip!(test_app(false).await);
    let user = mpacer_api::db::upsert_user(
        &state.pool,
        None,
        "dev@localhost",
        None,
        None,
        state.now_ms(),
    )
    .await
    .unwrap();
    let token = mpacer_api::auth::new_device_token();
    mpacer_api::db::insert_api_token(
        &state.pool,
        &user.id,
        &mpacer_api::auth::hash_token(&token),
        "test",
        state.now_ms(),
    )
    .await
    .unwrap();

    let mut workout = sample_workout();
    workout["distance_m"] = serde_json::json!(900_000.0);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workouts")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(workout.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(response).await.contains("invraisemblable"));
}

#[tokio::test]
async fn google_login_creates_a_session_and_dashboard() {
    let (app, _state) = app_or_skip!(test_app(false).await);

    // 1. /auth/google/start redirige vers le fournisseur avec un etat signe.
    let response = app
        .clone()
        .oneshot(get("/auth/google/start"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        location.starts_with("https://fake.test/authorize"),
        "{location}"
    );
    let state_param = location
        .split("state=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap()
        .to_string();

    // 2. Le callback echange le code, cree l'utilisateur et pose le cookie de session.
    let response = app
        .clone()
        .oneshot(get(&format!(
            "/auth/google/callback?code=bon-code&state={state_param}"
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.starts_with("mpacer_session="), "{cookie}");
    assert!(cookie.contains("HttpOnly"));
    let session = cookie.split(';').next().unwrap().to_string();

    // 3. Le tableau de bord est accessible avec la session.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/", &session))
        .await
        .unwrap();
    let status = response.status();
    let body = body_text(response).await;
    assert_eq!(status, StatusCode::OK, "corps = {body}");
    assert!(body.contains("Vos seances"), "{body}");
    assert!(body.contains("Coureur Test"));

    // 3b. Le lien "Telecharger le GPX" de la page detail doit repondre.
    // La seance appartient a l'utilisateur connecte (cree par le callback Google).
    let owner = mpacer_api::db::upsert_user(
        &_state.pool,
        Some("google-sub-1"),
        "coureur@example.org",
        Some("Coureur Test"),
        None,
        _state.now_ms(),
    )
    .await
    .unwrap();
    let upload = mpacer_api::models::WorkoutUpload {
        id: "1700000000000".into(),
        started_at_ms: 1_700_000_000_000,
        duration_s: 3000.0,
        distance_m: 10_000.0,
        average_pace_s_per_km: 300.0,
        laps: serde_json::json!([]),
        best_efforts: serde_json::json!([]),
        track: vec![],
        unit_system: Some(mpacer_core::units::UnitSystem::Metric),
        elapsed_s: 0.0,
        pauses: vec![],
        heart_rate: vec![],
        plan: None,
    };
    let payload = serde_json::to_string(&upload).unwrap();
    mpacer_api::db::upsert_workout(&_state.pool, &owner.id, &upload, &payload, _state.now_ms())
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(get_with_cookie("/workouts/1700000000000/gpx", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("</gpx>"));

    // 3 bis. Le meme trace part en KML, pour Google Earth.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/workouts/1700000000000/kml", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let kml = body_text(response).await;
    assert!(kml.contains("</kml>"), "{kml}");
    assert!(kml.contains("<LineString>"), "{kml}");

    // 4. Les appareils appaires sont listes dans les reglages.
    let response = app
        .oneshot(get_with_cookie("/settings", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("Appareils appaires"));
}

#[tokio::test]
async fn unknown_oauth_state_is_rejected() {
    let (app, _state) = app_or_skip!(test_app(false).await);
    let response = app
        .oneshot(get("/auth/google/callback?code=bon-code&state=inconnu"))
        .await
        .unwrap();
    let status = response.status();
    let body = body_text(response).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "corps = {body}");
}

#[tokio::test]
async fn landing_page_and_login_are_public() {
    let (app, _state) = app_or_skip!(test_app(false).await);
    let response = app.clone().oneshot(get("/")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("M-pacer"));

    let response = app.clone().oneshot(get("/login")).await.unwrap();
    assert!(body_text(response).await.contains("Continuer avec Google"));

    // Une page protegee redirige vers la connexion.
    let response = app.oneshot(get("/link")).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/login");
}

#[tokio::test]
async fn the_account_picture_is_served_by_the_service() {
    let (app, state) = app_or_skip!(test_app(true).await);

    // Connexion de developpement : le compte n'a aucune photo Google.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/dev-login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let session = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    // L'interface affiche la pastille de compte, y compris sur la page de reglages.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/", &session))
        .await
        .unwrap();
    let body = body_text(response).await;
    assert!(body.contains("class=\"user-chip\""), "{body}");
    assert!(body.contains("src=\"/avatar\""), "{body}");

    let response = app
        .clone()
        .oneshot(get_with_cookie("/settings", &session))
        .await
        .unwrap();
    let body = body_text(response).await;
    assert!(body.contains("class=\"card profile\""), "{body}");
    assert!(body.contains("Aucune photo Google"), "{body}");

    // Sans photo, le service sert une pastille aux initiales du compte.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/avatar", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "image/svg+xml; charset=utf-8"
    );
    assert!(response.headers()[header::CACHE_CONTROL]
        .to_str()
        .unwrap()
        .contains("private"));
    let body = body_text(response).await;
    assert!(body.starts_with("<svg"), "{body}");
    assert!(body.contains(">D</text>"), "{body}");

    // Une URL qui ne vient pas de Google n'est jamais suivie par le service :
    // la pastille d'initiales reste servie, sans requete sortante.
    mpacer_api::db::upsert_user(
        &state.pool,
        None,
        "dev@localhost",
        None,
        Some("https://exemple.invalid/photo.jpg"),
        state.now_ms(),
    )
    .await
    .unwrap();
    let response = app
        .oneshot(get_with_cookie("/avatar", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "image/svg+xml; charset=utf-8"
    );
    let body = body_text(response).await;
    assert!(body.contains(">D</text>"), "{body}");
}

/// Verification du proxy de photo Google, a lancer explicitement :
///
///     cargo test -p mpacer-api --test api -- --ignored
///
/// Le test est ignore par defaut : il exige un acces reseau a
/// googleusercontent.com, ce qui n'a pas sa place dans une suite hors ligne.
#[tokio::test]
#[ignore = "necessite un acces reseau a googleusercontent.com"]
async fn a_real_google_picture_is_proxied() {
    let (app, state) = app_or_skip!(test_app(false).await);
    const PHOTO: &str = "https://lh3.googleusercontent.com/-XdUIqdMkCWA/AAAAAAAAAAI/AAAAAAAAAAA/4252rscbv5M/photo.jpg";

    let now = state.now_ms();
    let user = mpacer_api::db::upsert_user(
        &state.pool,
        None,
        "photo@localhost",
        Some("Coureur Photo"),
        Some(PHOTO),
        now,
    )
    .await
    .unwrap();
    let session = format!(
        "{}={}",
        mpacer_api::auth::SESSION_COOKIE,
        mpacer_api::auth::issue_session(&state.config, &user, now).unwrap()
    );

    let response = app
        .oneshot(get_with_cookie("/avatar", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .to_string();
    let body = body_text(response).await;
    assert!(
        content_type.starts_with("image/") && !content_type.contains("svg"),
        "la photo Google doit etre servie ({content_type})"
    );
    assert!(body.len() > 100, "photo vide");
}

#[tokio::test]
async fn web_link_page_approves_a_device_code() {
    let (app, state) = app_or_skip!(test_app(false).await);
    let user = mpacer_api::db::upsert_user(
        &state.pool,
        None,
        "dev@localhost",
        None,
        None,
        state.now_ms(),
    )
    .await
    .unwrap();
    let session = mpacer_api::auth::issue_session(&state.config, &user, state.now_ms()).unwrap();

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/device/code",
            r#"{"label":"Montre test"}"#,
        ))
        .await
        .unwrap();
    let device: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    let user_code = device["user_code"].as_str().unwrap().to_string();

    // Saisie du code dans le navigateur (formulaire HTML).
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/link")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, format!("mpacer_session={session}"))
                .body(Body::from(format!("user_code={user_code}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/link?ok=1");

    // La montre peut alors recuperer son jeton.
    let device_code = device["device_code"].as_str().unwrap().to_string();
    let response = app
        .oneshot(json_request(
            "POST",
            "/api/v1/device/token",
            &format!(r#"{{"device_code":"{device_code}"}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn device_login_is_disabled_unless_enabled() {
    let (app, _state) = app_or_skip!(test_app(false).await);
    let request = Request::builder()
        .method("POST")
        .uri("/auth/dev-login")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let (app, _state) = app_or_skip!(test_app(true).await);
    let request = Request::builder()
        .method("POST")
        .uri("/auth/dev-login")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .starts_with("mpacer_session="));
}

#[tokio::test]
async fn workout_filters_and_pac_export() {
    let (app, state) = app_or_skip!(test_app(false).await);
    let user = mpacer_api::db::upsert_user(
        &state.pool,
        None,
        "coureur@example.org",
        Some("Coureur"),
        None,
        state.now_ms(),
    )
    .await
    .unwrap();
    let token = mpacer_api::auth::new_device_token();
    mpacer_api::db::insert_api_token(
        &state.pool,
        &user.id,
        &mpacer_api::auth::hash_token(&token),
        "test",
        state.now_ms(),
    )
    .await
    .unwrap();

    // Deux seances : une ancienne (2020) et une recente (2025).
    for (id, started) in [
        ("ancienne", 1_600_000_000_000_i64),
        ("recente", 1_750_000_000_000_i64),
    ] {
        let mut workout = sample_workout();
        workout["id"] = serde_json::json!(id);
        workout["started_at_ms"] = serde_json::json!(started);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workouts")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(workout.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Filtre par date : seule la seance recente est renvoyee.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workouts?from=1700000000000")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_text(response).await;
    let list: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(list["total"], 1, "corps = {body}");
    assert_eq!(list["items"][0]["id"], "recente");

    // Plage inversee : refusee.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workouts?from=2000&to=1000")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Export .pac : le fichier contient les deux seances.
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/export")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .contains(".pac"));
    let pac = body_text(response).await;
    assert!(pac.contains("mpacer.pac"), "{pac}");
    assert!(pac.contains("ancienne") && pac.contains("recente"));
}

#[tokio::test]
async fn stats_page_renders_weekly_volume() {
    let (app, state) = app_or_skip!(test_app(true).await);

    // Connexion de developpement : le cookie porte la session.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/dev-login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_string();
    let session = cookie.split(';').next().unwrap().to_string();

    // Une seance vieille de trois jours (donc dans la semaine en cours).
    let user = mpacer_api::db::upsert_user(
        &state.pool,
        None,
        "dev@localhost",
        None,
        None,
        state.now_ms(),
    )
    .await
    .unwrap();
    let upload = mpacer_api::models::WorkoutUpload {
        id: "1700000000000".into(),
        started_at_ms: state.now_ms() - 3 * 24 * 3600 * 1000,
        duration_s: 1800.0,
        distance_m: 6000.0,
        average_pace_s_per_km: 300.0,
        laps: serde_json::json!([]),
        best_efforts: serde_json::json!([]),
        track: vec![],
        unit_system: Some(mpacer_core::units::UnitSystem::Metric),
        elapsed_s: 0.0,
        pauses: vec![],
        heart_rate: vec![],
        plan: None,
    };
    let payload = serde_json::to_string(&upload).unwrap();
    mpacer_api::db::upsert_workout(&state.pool, &user.id, &upload, &payload, state.now_ms())
        .await
        .unwrap();

    let response = app
        .oneshot(get_with_cookie("/stats", &session))
        .await
        .unwrap();
    let status = response.status();
    let body = body_text(response).await;
    assert_eq!(status, StatusCode::OK, "corps = {body}");
    assert!(body.contains("Statistiques"), "{body}");
    assert!(body.contains("Volume hebdomadaire"), "{body}");
    assert!(body.contains("6.00 km"), "{body}");
}

// ------------------------------------------------------------------ courses

/// Encodage minimal d'un corps de formulaire (application/x-www-form-urlencoded).
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Requete de formulaire HTML portant la session du navigateur.
fn form_request(method: &str, uri: &str, body: &str, session: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, session)
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Utilisateur de developpement et cookie de session associe.
async fn dev_user_session(state: &AppState) -> (mpacer_api::models::User, String) {
    let user = mpacer_api::db::upsert_user(
        &state.pool,
        None,
        "dev@localhost",
        None,
        None,
        state.now_ms(),
    )
    .await
    .unwrap();
    let session = mpacer_api::auth::issue_session(&state.config, &user, state.now_ms()).unwrap();
    (user, format!("mpacer_session={session}"))
}

/// Date locale (YYYY-MM-DD) a J+days.
fn local_date_in_days(now_ms: i64, days: i64) -> String {
    chrono::DateTime::from_timestamp_millis(now_ms + days * 86_400_000)
        .unwrap()
        .with_timezone(&chrono::Local)
        .format("%Y-%m-%d")
        .to_string()
}

/// Corps de formulaire complet decrivant une course.
fn race_form_body(date: &str, extra: &[(&str, &str)]) -> String {
    let mut fields: Vec<(String, String)> = vec![
        ("name".into(), "Marathon de Lyon".into()),
        ("date".into(), date.to_string()),
        ("time".into(), "09:30".into()),
        ("distance_km".into(), "42.195".into()),
        ("discipline".into(), "Route".into()),
        ("location".into(), "Lyon".into()),
        ("start_location".into(), "Place Bellecour".into()),
        ("bib_number".into(), "1234".into()),
        ("bib_pickup_date".into(), date.to_string()),
        ("bib_pickup_time".into(), "15:00".into()),
        ("bib_pickup_location".into(), "Village depart".into()),
        (
            "live_url".into(),
            "https://live.example.org/coureur/1234".into(),
        ),
        (
            "registration_url".into(),
            "https://marathon.example.org/inscription".into(),
        ),
        ("hotel_name".into(), "Ibis Lyon Centre".into()),
        ("hotel_address".into(), "12 rue de la Paix, Lyon".into()),
        ("hotel_booked".into(), "1".into()),
        ("hotel_check_in".into(), date.to_string()),
        ("lodging_notes".into(), "Camping possible a 5 km".into()),
        ("nutrition_notes".into(), "Ravitos tous les 5 km".into()),
        (
            "important_info".into(),
            "Certificat medical obligatoire".into(),
        ),
        ("notes".into(), "Depart en train la veille".into()),
        ("goal_time".into(), "3:30:00".into()),
    ];
    // Une cle deja presente est remplacee, jamais repetee : serde_urlencoded
    // refuse un champ duplique (422), exactement comme un navigateur n'envoyait
    // qu'une seule valeur par champ texte.
    for (key, value) in extra {
        let key = (*key).to_string();
        let value = (*value).to_string();
        match fields.iter().position(|(existing, _)| existing == &key) {
            Some(index) => fields[index].1 = value,
            None => fields.push((key, value)),
        }
    }
    fields
        .iter()
        .map(|(key, value)| format!("{key}={}", encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Cree une course par le formulaire web et renvoie son identifiant.
async fn create_race_through_web(app: &Router, session: &str, date: &str) -> String {
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            "/courses/nouvelle",
            &race_form_body(date, &[]),
            session,
        ))
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(header::LOCATION)
        .map(|value| value.to_str().unwrap().to_string());
    let body = body_text(response).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "corps = {body}");
    location
        .expect("redirection vers la fiche")
        .rsplit('/')
        .next()
        .expect("identifiant de course dans la redirection")
        .to_string()
}

#[tokio::test]
async fn race_sheet_holds_everything_a_runner_needs() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (_user, session) = dev_user_session(&state).await;
    let now = state.now_ms();
    let date = local_date_in_days(now, 30);

    // Etat initial : aucune course.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/courses", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Aucune course enregistree"), "{body}");

    let race_id = create_race_through_web(&app, &session, &date).await;

    // La carte de course resume l'essentiel sans ouvrir la fiche.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/courses", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Marathon de Lyon"), "{body}");
    assert!(body.contains("Dossard 1234"), "{body}");
    assert!(body.contains("Hotel reserve"), "{body}");
    assert!(body.contains("42.20 km"), "{body}");

    // La fiche detaillee reprend tous les elements de suivi demandes.
    let response = app
        .clone()
        .oneshot(get_with_cookie(&format!("/courses/{race_id}"), &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    for expected in [
        "Marathon de Lyon",
        "Place Bellecour",
        "09:30",
        "1234",
        "15:00",
        "Village depart",
        "https://live.example.org/coureur/1234",
        "Ibis Lyon Centre",
        "12 rue de la Paix, Lyon",
        "Camping possible a 5 km",
        "Ravitos tous les 5 km",
        "Certificat medical obligatoire",
        "Depart en train la veille",
        "3:30:00",
        "Suivi des elements importants",
        "Dossard retire",
    ] {
        assert!(
            body.contains(expected),
            "« {expected} » absent de la fiche : {body}"
        );
    }
    assert!(body.contains("0 / 8 prets"), "{body}");

    // Le suivi se coche.
    let tasks = mpacer_api::db::list_race_tasks(&state.pool, &_user.id, &race_id)
        .await
        .unwrap();
    assert_eq!(
        tasks.len(),
        8,
        "les elements de suivi par defaut sont crees"
    );
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/courses/{race_id}/suivi/{}", tasks[0].id),
            "done=1",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(&format!("/courses/{race_id}"), &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("1 / 8 prets"), "{body}");

    // Un element de suivi peut etre ajoute puis retire.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/courses/{race_id}/suivi"),
            &format!(
                "label={}&due={}",
                encode("Reconnaissance du parcours"),
                date
            ),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let tasks = mpacer_api::db::list_race_tasks(&state.pool, &_user.id, &race_id)
        .await
        .unwrap();
    assert_eq!(tasks.len(), 9);
    assert!(tasks
        .iter()
        .any(|task| task.label == "Reconnaissance du parcours" && task.due_at_ms.is_some()));

    // La fiche se modifie.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/courses/{race_id}/modifier"),
            &race_form_body(
                &date,
                &[("bib_number", "4321"), ("name", "Marathon de Lyon 2027")],
            ),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(&format!("/courses/{race_id}"), &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("4321"), "{body}");
    assert!(body.contains("Marathon de Lyon 2027"), "{body}");

    // Suppression : la course et son suivi disparaissent.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/courses/{race_id}/supprimer"),
            "",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/courses", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(!body.contains("Marathon de Lyon 2027"), "{body}");
    assert!(
        mpacer_api::db::list_race_tasks(&state.pool, &_user.id, &race_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn planning_lists_upcoming_races_in_order() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (_user, session) = dev_user_session(&state).await;
    let now = state.now_ms();

    // Rien au planning tant qu'aucune course n'est enregistree.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/courses/planning", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Rien au planning"), "{body}");

    let date = local_date_in_days(now, 30);
    create_race_through_web(&app, &session, &date).await;

    let response = app
        .clone()
        .oneshot(get_with_cookie("/courses/planning", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("Depart - Marathon de Lyon"), "{body}");
    assert!(
        body.contains("Prise de dossard - Marathon de Lyon"),
        "{body}"
    );
    assert!(
        body.contains("Arrivee a l'hotel - Ibis Lyon Centre"),
        "{body}"
    );
    assert!(body.contains("Place Bellecour"), "{body}");
    // Le depart (09:30) precede la prise de dossard (15:00) le meme jour.
    let depart = body.find("Depart - Marathon de Lyon").unwrap();
    let dossard = body.find("Prise de dossard - Marathon de Lyon").unwrap();
    assert!(depart < dossard, "ordre du planning : {body}");

    // Une course passee ne figure plus au planning.
    let past_date = local_date_in_days(now, -10);
    create_race_through_web(&app, &session, &past_date).await;
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/courses", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Deja courues"), "{body}");
}

#[tokio::test]
async fn race_form_rejects_a_link_that_is_not_http() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (user, session) = dev_user_session(&state).await;
    let date = local_date_in_days(state.now_ms(), 20);

    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            "/courses/nouvelle",
            &race_form_body(&date, &[("live_url", "javascript:alert(1)")]),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_text(response).await;
    assert!(body.contains("http:// ou https://"), "{body}");
    // Rien n'a ete enregistre.
    assert!(mpacer_api::db::list_races(&state.pool, &user.id)
        .await
        .unwrap()
        .is_empty());
}

// ------------------------------------------------------------------ analyse de seance

/// Seance complete : trace, cardio, pause et plan de course.
fn analysed_workout() -> serde_json::Value {
    let mut track = Vec::new();
    let mut heart_rate = Vec::new();
    for i in 0..=600 {
        // Une pause de 60 s interrompt la trace apres 200 s : aucun point GPS
        // n'est enregistre pendant l'arret, exactement comme sur la montre.
        let t_ms = if i > 200 { i * 1000 + 60_000 } else { i * 1000 };
        track.push(serde_json::json!({
            "t_ms": t_ms,
            "dist_m": i as f64 * 3.0,
            "lat": 45.0 + i as f64 * 2.0e-5,
            "lon": 3.0,
            "elevation_m": 300.0 + i as f64 * 0.02,
        }));
        // Une mesure toutes les 10 s : c'est la cadence minimale pour que les
        // zones et la derive cardiaque soient exploitables.
        if i % 10 == 0 {
            heart_rate.push(serde_json::json!({
                "t_ms": t_ms,
                "bpm": 140 + (i / 120) as u16,
            }));
        }
    }
    serde_json::json!({
        "id": "1700000000123",
        "started_at_ms": 1_700_000_000_000_i64,
        "duration_s": 600.0,
        "elapsed_s": 660.0,
        "distance_m": 1800.0,
        "average_pace_s_per_km": 333.33,
        "laps": [
            { "index": 1, "distance_m": 1000.0, "duration_s": 333.33, "pace_s_per_km": 333.33 }
        ],
        "best_efforts": [],
        "track": track,
        "unit_system": "Metric",
        "heart_rate": heart_rate,
        "pauses": [
            { "at_s": 200.0, "at_distance_m": 600.0, "duration_s": 60.0, "automatic": false }
        ],
        "plan": {
            "distance_m": 1800.0,
            "target_time_s": 540.0,
            "negative_split": { "enabled": true, "ratio": 0.03 }
        }
    })
}

#[tokio::test]
async fn workout_page_shows_plan_cardio_pauses_and_acceleration() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (user, session) = dev_user_session(&state).await;

    let upload: mpacer_api::models::WorkoutUpload =
        serde_json::from_value(analysed_workout()).expect("seance analysee relue");
    assert_eq!(upload.heart_rate.len(), 61);
    assert_eq!(upload.pauses.len(), 1);
    assert!(upload.plan.is_some());
    let payload = serde_json::to_string(&upload).unwrap();
    mpacer_api::db::upsert_workout(&state.pool, &user.id, &upload, &payload, state.now_ms())
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(get_with_cookie("/workouts/1700000000123", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    for expected in [
        "Temps en mouvement",
        "Temps ecoule",
        "Allure ajustee (GAP)",
        "Pente",
        "FC moyenne",
        "Graphique",
        "polyline class=\"trace pace\"",
        "polyline class=\"trace cardio\"",
        "Plan de course",
        "Ecart au finish",
        "Frequence cardiaque",
        "Derive cardiaque",
        "Temps de passage",
        "Chronologie",
        "Pause 1",
        "Acceleration",
        "Reprise",
        // Analyse de trace a la VisuGPX : carte, profil, statistiques.
        "carte-seance",
        "data-trace=",
        "data-reperes=",
        "Profil altimetrique",
        "Altitude min / max",
        "Vitesse max",
        "Depart / arrivee",
        "Points GPS",
        "Denivele horaire des portions",
    ] {
        assert!(
            body.contains(expected),
            "« {expected} » absent de la page : {body}"
        );
    }
    // La pause est situee et qualifiee.
    assert!(body.contains("manuelle"), "{body}");
    // Le plan se compare au realise : cible 9:00 pour 1.8 km.
    assert!(body.contains("9:00"), "{body}");
    // La pause du kilometre apparait dans le tableau des temps de passage.
    assert!(body.contains("<th>Pause</th>"), "{body}");
    // Les deux exports sont proposes depuis la fiche.
    assert!(body.contains("/workouts/1700000000123/gpx"), "{body}");
    assert!(body.contains("/workouts/1700000000123/kml"), "{body}");

    // L'API renvoie les memes donnees brutes.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/api/v1/workouts/1700000000123", &session))
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(
        detail["summary"]["heart_rate"].as_array().unwrap().len(),
        61
    );
    assert_eq!(detail["summary"]["pauses"].as_array().unwrap().len(), 1);
    assert_eq!(
        detail["summary"]["plan"]["target_time_s"],
        serde_json::json!(540.0)
    );
}

#[tokio::test]
async fn an_implausible_heart_rate_is_rejected() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (_user, session) = dev_user_session(&state).await;

    let mut workout = analysed_workout();
    workout["heart_rate"] = serde_json::json!([{ "t_ms": 0, "bpm": 400 }]);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workouts")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &session)
                .body(Body::from(workout.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_text(response).await;
    assert!(body.contains("frequence cardiaque"), "{body}");
}

// ------------------------------------------------------------------ musique

/// Jeton d'appareil pour l'utilisateur donne.
async fn device_token(state: &AppState, user_id: &str) -> String {
    let token = mpacer_api::auth::new_device_token();
    mpacer_api::db::insert_api_token(
        &state.pool,
        user_id,
        &mpacer_api::auth::hash_token(&token),
        "montre de test",
        state.now_ms(),
    )
    .await
    .unwrap();
    token
}

fn bearer(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn music_api_publishes_playlists_and_the_transfer_manifest() {
    let (app, state) = app_or_skip!(test_app(false).await);
    let (user, _session) = dev_user_session(&state).await;
    let token = device_token(&state, &user.id).await;

    // Une playlist de metadonnees, comme apres un import Deezer.
    let playlist = mpacer_api::db::insert_music_playlist(
        &state.pool,
        &user.id,
        &mpacer_api::models::MusicPlaylistInput {
            name: "Run 170".into(),
            source: "deezer".into(),
            deezer_id: Some("1924357302".into()),
            cover_url: None,
            target_bpm: Some(170.0),
        },
        state.now_ms(),
    )
    .await
    .unwrap();

    for (position, (title, duration_s, bpm)) in
        [("Wake me up", 249.0, 124.0), ("Levels", 200.0, 126.0)]
            .iter()
            .enumerate()
    {
        mpacer_api::db::insert_music_track(
            &state.pool,
            &user.id,
            &playlist.id,
            &mpacer_api::models::MusicTrackInput {
                position: position as i32,
                title: (*title).to_string(),
                artist: Some("Avicii".into()),
                album: Some("True".into()),
                duration_s: Some(*duration_s),
                bpm: Some(*bpm),
                bpm_source: Some("tag".into()),
                deezer_track_id: Some(format!("42732470{position}")),
            },
            state.now_ms(),
        )
        .await
        .unwrap();
    }

    // 1. La liste resume les playlists : plus aucun compteur d'octets.
    let response = app
        .clone()
        .oneshot(bearer("/api/v1/music/playlists", &token))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let list: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    let entry = &list["playlists"][0];
    assert_eq!(entry["name"], "Run 170");
    assert_eq!(entry["source"], "deezer");
    assert_eq!(entry["target_bpm"], 170.0);
    assert_eq!(entry["track_count"], 2);
    assert_eq!(entry["duration_s"], 449.0);
    assert!(entry["updated_at_ms"].is_i64(), "{entry}");
    assert!(
        entry.get("total_bytes").is_none() && entry.get("ready_track_count").is_none(),
        "plus de compteur d'audio : {entry}"
    );

    // 2. La fiche publie les metadonnees et l'adresse du manifeste.
    let response = app
        .clone()
        .oneshot(bearer(
            &format!("/api/v1/music/playlists/{}", playlist.id),
            &token,
        ))
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(
        detail["manifest_url"],
        format!("/api/v1/music/playlists/{}/manifest", playlist.id)
    );
    assert_eq!(detail["tracks"][0]["title"], "Wake me up");
    assert_eq!(detail["tracks"][0]["duration_s"], 249.0);
    assert_eq!(detail["tracks"][0]["bpm"], 124.0);
    assert_eq!(detail["tracks"][0]["bpm_source"], "tag");
    assert!(
        detail["tracks"][0].get("download_url").is_none()
            && detail["tracks"][0].get("size_bytes").is_none()
            && detail["tracks"][0].get("mime").is_none(),
        "la fiche ne doit plus exposer d'audio : {detail}"
    );

    // 3. Le manifeste de transfert suit le schema gele (section 3.1).
    let response = app
        .clone()
        .oneshot(bearer(
            &format!("/api/v1/music/playlists/{}/manifest", playlist.id),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("application/json"));
    let disposition = response.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(disposition.starts_with("attachment"), "{disposition}");
    assert!(disposition.contains("run-170.json"), "{disposition}");

    let manifest: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    // serde_json trie les cles d'un Value : on compare l'ensemble des champs
    // (l'ordre du texte JSON est verifie par le test unitaire du modele).
    let mut keys: Vec<&str> = manifest
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "name",
            "playlist_id",
            "source",
            "target_bpm",
            "tracks",
            "version"
        ]
    );
    assert_eq!(manifest["version"], 1);
    assert_eq!(manifest["playlist_id"], playlist.id.as_str());
    assert_eq!(manifest["name"], "Run 170");
    assert_eq!(manifest["target_bpm"], 170.0);
    let mut track_keys: Vec<&str> = manifest["tracks"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    track_keys.sort_unstable();
    assert_eq!(
        track_keys,
        vec![
            "album",
            "artist",
            "bpm",
            "duration_s",
            "id",
            "position",
            "title"
        ],
        "file et size_bytes ne sont ecrits que par mpacer-music"
    );
    // Position 1-based : la base numerote depuis 0, la montre depuis 1.
    assert_eq!(manifest["tracks"][0]["position"], 1);
    assert_eq!(manifest["tracks"][1]["position"], 2);

    // 4. Playlist inconnue et jeton obligatoire.
    let response = app
        .clone()
        .oneshot(bearer("/api/v1/music/playlists/inconnue/manifest", &token))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app
        .clone()
        .oneshot(get(&format!(
            "/api/v1/music/playlists/{}/manifest",
            playlist.id
        )))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app.oneshot(get("/api/v1/music/playlists")).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn deemix_list_exports_the_download_links() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (user, session) = dev_user_session(&state).await;

    // Une playlist Deezer importee : chaque piste garde son identifiant Deezer.
    let playlist = mpacer_api::db::insert_music_playlist(
        &state.pool,
        &user.id,
        &mpacer_api::models::MusicPlaylistInput {
            name: "Rock Workout".into(),
            source: "deezer".into(),
            deezer_id: Some("1924357302".into()),
            cover_url: None,
            target_bpm: None,
        },
        state.now_ms(),
    )
    .await
    .unwrap();
    for (position, (title, track_id)) in [("Wake me up", "4273247042"), ("Levels", "999")]
        .iter()
        .enumerate()
    {
        mpacer_api::db::insert_music_track(
            &state.pool,
            &user.id,
            &playlist.id,
            &mpacer_api::models::MusicTrackInput {
                position: position as i32,
                title: (*title).to_string(),
                artist: Some("Avicii".into()),
                album: None,
                duration_s: Some(249.0),
                bpm: None,
                bpm_source: None,
                deezer_track_id: Some((*track_id).to_string()),
            },
            state.now_ms(),
        )
        .await
        .unwrap();
    }

    // 1. La page propose le telechargement de la playlist dans Deemix.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(
                &format!("/music?playlist={}", playlist.id),
                &session,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(
        body.contains("https://deemix.p.zacharie.org/#/playlist/1924357302"),
        "{body}"
    );
    assert!(body.contains("Deemix"), "{body}");

    // 2. La liste .txt porte un lien Deemix par piste.
    let response = app
        .clone()
        .oneshot(get_with_cookie(
            &format!("/music/playlists/{}/deemix", playlist.id),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let disposition = response.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        disposition.contains("rock-workout-deemix.txt"),
        "{disposition}"
    );
    let body = body_text(response).await;
    assert!(
        body.contains("# playlist entiere : https://deemix.p.zacharie.org/#/playlist/1924357302"),
        "{body}"
    );
    assert!(
        body.contains(
            "01 - Avicii - Wake me up.mp3	https://deemix.p.zacharie.org/#/track/4273247042"
        ),
        "{body}"
    );
    assert!(
        body.contains("https://deemix.p.zacharie.org/#/track/999"),
        "{body}"
    );

    // 3. Une playlist inconnue reste un 404.
    let response = app
        .oneshot(get_with_cookie(
            "/music/playlists/inconnue/deemix",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn music_page_has_six_blocks_and_downloads_the_manifest() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (_user, session) = dev_user_session(&state).await;

    // La page vide presente les six blocs de la maquette v4 (docs/11) et ne
    // propose toujours aucun televersement audio cote serveur.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/music", &session))
            .await
            .unwrap(),
    )
    .await;
    for expected in [
        "1. Source des playlists",
        "2. Playlists preparees",
        "3. Titres (playlist selectionnee)",
        "4. Fichiers a preparer (MP3)",
        "5. Transfert vers la montre (USB)",
        "6. Assez de musique pour la course ?",
        "Deezer n'est pas configure",
        "Aucune playlist pour l'instant",
        "Selectionnez une playlist dans le bloc 2",
    ] {
        assert!(
            body.contains(expected),
            "« {expected} » absent de /music : {body}"
        );
    }
    assert!(!body.contains("multipart/form-data"), "{body}");
    assert!(!body.contains("Envoyer sur la montre"), "{body}");

    // Une playlist importee alimente les blocs 2 a 6.
    let playlist = mpacer_api::db::insert_music_playlist(
        &state.pool,
        &_user.id,
        &mpacer_api::models::MusicPlaylistInput {
            name: "Run 170".into(),
            source: "deezer".into(),
            deezer_id: Some("1924357302".into()),
            cover_url: None,
            target_bpm: None,
        },
        state.now_ms(),
    )
    .await
    .unwrap();
    let track_id = mpacer_api::db::insert_music_track(
        &state.pool,
        &_user.id,
        &playlist.id,
        &mpacer_api::models::MusicTrackInput {
            position: 0,
            title: "Wake me up".into(),
            artist: Some("Avicii".into()),
            album: None,
            duration_s: Some(249.0),
            bpm: None,
            bpm_source: None,
            deezer_track_id: Some("4273247042".into()),
        },
        state.now_ms(),
    )
    .await
    .unwrap();

    let page_url = format!("/music?playlist={}", playlist.id);
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(&page_url, &session))
            .await
            .unwrap(),
    )
    .await;
    for expected in [
        "Run 170",
        "Wake me up",
        "4:09",
        "inconnu",
        "tapper",
        "saisir",
        "Ouvrir",
        "Renommer",
        "Telecharger le manifeste",
        "run-170.json",
        "mpacer-music transfer",
        "Brancher la montre en USB",
        "Importer (USB)",
        "BPM cible",
        // Bloc 4 : le nom de fichier attendu sur la montre.
        "01 - Avicii - Wake me up.mp3",
        "Telecharger la liste (.txt)",
        // Bloc 6 : sans duree de course, le verdict reste en attente.
        "coverage-warn",
        "coverage-gauge-fill",
        "Duree playlist",
    ] {
        assert!(body.contains(expected), "« {expected} » absent : {body}");
    }

    // BPM saisi a la main, puis tap-tempo (median des intervalles).
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{}/track-bpm", playlist.id),
            &format!("track_id={track_id}&bpm=181&source=manual"),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{}/track-bpm", playlist.id),
            &format!("track_id={track_id}&taps=0,500,1000,1500,2000"),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let tracks = mpacer_api::db::list_music_tracks(&state.pool, &_user.id, &playlist.id)
        .await
        .unwrap();
    assert_eq!(tracks[0].bpm, Some(120.0));
    assert_eq!(tracks[0].bpm_source.as_deref(), Some("tap"));

    // Un BPM hors bornes est refuse sans rien ecrire.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{}/track-bpm", playlist.id),
            &format!("track_id={track_id}&bpm=1000"),
            &session,
        ))
        .await
        .unwrap();
    assert!(response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .contains("erreur=bpm_invalide"));

    // Consigne de tempo de la playlist.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{}/target", playlist.id),
            "target_bpm=170",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    // Le manifeste se telecharge aussi depuis le navigateur (session).
    let response = app
        .clone()
        .oneshot(get_with_cookie(
            &format!("/music/playlists/{}/manifest", playlist.id),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .contains("run-170.json"));
    let manifest: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(manifest["name"], "Run 170");
    assert_eq!(manifest["target_bpm"], 170.0);
    assert_eq!(manifest["tracks"][0]["bpm"], 120.0);

    // La liste des MP3 a mettre en place se telecharge en texte (session).
    let response = app
        .clone()
        .oneshot(get_with_cookie(
            &format!("/music/playlists/{}/files", playlist.id),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .contains("run-170.txt"));
    let listing = body_text(response).await;
    assert!(
        listing.contains("# Run 170 : 1 fichier(s) MP3 a mettre en place"),
        "{listing}"
    );
    assert!(
        listing.contains("01 - Avicii - Wake me up.mp3"),
        "{listing}"
    );

    // Suppression : la playlist disparait.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{}/delete", playlist.id),
            "",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/music", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(!body.contains(">Run 170</a>"), "{body}");
    assert!(mpacer_api::db::list_music_playlists(&state.pool, &_user.id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn deezer_stays_optional_without_credentials() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (_user, session) = dev_user_session(&state).await;

    // Sans configuration, /auth/deezer renvoie vers la page avec un message clair
    // (aucun appel reseau).
    let response = app
        .clone()
        .oneshot(get_with_cookie("/auth/deezer", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/music?erreur=deezer_non_configure"
    );

    // La recherche affiche le meme message sans contacter Deezer.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/music/search?q=running", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Deezer n'est pas configure"), "{body}");

    // L'import est refuse proprement.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            "/music/import",
            "ref=https%3A%2F%2Fwww.deezer.com%2Ffr%2Fplaylist%2F1924357302&target_bpm=",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/music?erreur=deezer_non_configure"
    );

    // Le compte n'existe pas : rien n'a ete ecrit.
    assert!(mpacer_api::db::get_deezer_account(&state.pool, &_user.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn deezer_oauth_starts_when_configured() {
    let (app, state) = app_or_skip!(test_app_deezer_oauth(true).await);
    let (_user, session) = dev_user_session(&state).await;

    // La page propose la connexion quand le service est configure en OAuth.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/music", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Connecter Deezer"), "{body}");
    assert!(!body.contains("Deezer n'est pas configure"), "{body}");

    // /auth/deezer part vers Deezer, sans aucun appel reseau.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/auth/deezer", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        location.starts_with("https://connect.deezer.com/oauth/auth.php"),
        "{location}"
    );
    assert!(location.contains("app_id=123456"), "{location}");
    assert!(location.contains("perms=basic_access"), "{location}");

    // La recherche demande d'abord de connecter le compte.
    let body = body_text(
        app.oneshot(get_with_cookie("/music/search?q=running", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Connectez votre compte Deezer"), "{body}");
}

#[tokio::test]
async fn races_api_creates_reads_updates_and_deletes() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (user, session) = dev_user_session(&state).await;
    let start = state.now_ms() + 60 * 24 * 3600 * 1000;

    // Creation.
    let payload = serde_json::json!({
        "name": "Trail des volcans",
        "start_at_ms": start,
        "distance_m": 25000.0,
        "bib_number": "77",
        "start_location": "Le Mont-Dore",
        "live_url": "https://live.example.org/volcans/77",
        "hotel_booked": true,
    })
    .to_string();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/races")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &session)
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let race: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    let race_id = race["id"].as_str().unwrap().to_string();
    assert_eq!(race["hotel_booked"], serde_json::json!(true));
    // Les elements de suivi accompagnent la course.
    assert_eq!(
        mpacer_api::db::list_race_tasks(&state.pool, &user.id, &race_id)
            .await
            .unwrap()
            .len(),
        8
    );

    // Lecture de la liste et du detail.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/api/v1/races", &session))
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(list["total"], serde_json::json!(1));
    assert_eq!(
        list["items"][0]["name"],
        serde_json::json!("Trail des volcans")
    );

    let response = app
        .clone()
        .oneshot(get_with_cookie(
            &format!("/api/v1/races/{race_id}"),
            &session,
        ))
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(detail["race"]["bib_number"], serde_json::json!("77"));
    assert_eq!(detail["tasks"].as_array().unwrap().len(), 8);

    // Mise a jour.
    let payload =
        serde_json::json!({ "name": "Trail des volcans", "bib_number": "88" }).to_string();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/races/{race_id}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &session)
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(updated["bib_number"], serde_json::json!("88"));
    assert_eq!(updated["distance_m"], serde_json::Value::Null);

    // Validation : un nom vide est refuse.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/races")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &session)
                .body(Body::from(r#"{"name":"   "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Suppression.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/races/{race_id}"))
                .header(header::COOKIE, &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = app
        .oneshot(get_with_cookie(
            &format!("/api/v1/races/{race_id}"),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ------------------------------------------- courses de reference et commentaires

/// Corps multipart portant un seul fichier (champ file).
///
/// Le corps est construit sur place : ces tests ne dependent d'aucun helper
/// partage avec les autres televersements.
fn upload_file_body(boundary: &str, filename: &str, bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: application/gpx+xml\r\n\r\n");
    body.extend_from_slice(bytes);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// Requete multipart d'import, avec ou sans jeton d'appareil.
fn import_request(uri: &str, boundary: &str, body: Vec<u8>, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("POST").uri(uri).header(
        header::CONTENT_TYPE,
        format!("multipart/form-data; boundary={boundary}"),
    );
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    builder.body(Body::from(body)).unwrap()
}

/// Requete multipart portant la session du navigateur.
fn upload_request(uri: &str, boundary: &str, body: Vec<u8>, session: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header(header::COOKIE, session)
        .body(Body::from(body))
        .unwrap()
}

/// Export GPX minimal d'une sortie Strava : 30 s, environ 110 m.
fn strava_gpx_export() -> Vec<u8> {
    let gpx = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
        <gpx version=\"1.1\" creator=\"StravaGPX\" xmlns=\"http://www.topografix.com/GPX/1/1\">\
        <metadata><name>10 km de nuit</name></metadata>\
        <trk><name>10 km de nuit</name><trkseg>\
        <trkpt lat=\"45.0\" lon=\"3.0\"><ele>300.0</ele><time>2026-05-01T19:00:00Z</time></trkpt>\
        <trkpt lat=\"45.0009\" lon=\"3.0\"><ele>305.0</ele><time>2026-05-01T19:00:30Z</time></trkpt>\
        </trkseg></trk></gpx>";
    gpx.as_bytes().to_vec()
}

#[tokio::test]
async fn a_strava_export_becomes_a_reference_race() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (user, session) = dev_user_session(&state).await;

    // Depot du fichier depuis le navigateur.
    let boundary = "----mpacer-import-web";
    let body = upload_file_body(boundary, "10km-de-nuit.gpx", &strava_gpx_export());
    let response = app
        .clone()
        .oneshot(upload_request(
            "/courses/importer",
            boundary,
            body,
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(location.contains("ok=course_importee"), "{location}");
    let race_id = location
        .split('/')
        .nth(2)
        .and_then(|reste| reste.split('?').next())
        .expect("identifiant de course")
        .to_string();

    // La fiche porte l'origine, les mesures et la trace du fichier.
    let race = mpacer_api::db::get_race(&state.pool, &user.id, &race_id)
        .await
        .unwrap()
        .expect("course importee");
    assert!(race.is_reference);
    assert_eq!(race.source.as_deref(), Some("strava"));
    assert_eq!(race.name, "10 km de nuit");
    assert!(race.start_at_ms.is_some());
    let distance = race.distance_m.expect("distance");
    assert!((90.0..130.0).contains(&distance), "distance = {distance}");
    assert_eq!(race.moving_time_s, Some(30.0));
    assert_eq!(race.elapsed_time_s, Some(30.0));
    assert_eq!(race.elevation_gain_m, Some(5.0));

    // Une course importee n'a pas d'elements de suivi : elle est deja courue.
    assert!(
        mpacer_api::db::list_race_tasks(&state.pool, &user.id, &race_id)
            .await
            .unwrap()
            .is_empty()
    );

    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(&format!("/courses/{race_id}"), &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Course de reference"), "{body}");
    assert!(body.contains("Strava"), "{body}");
    assert!(body.contains("Telecharger le GPX"), "{body}");

    // La trace conservee est reexportable.
    let response = app
        .clone()
        .oneshot(get_with_cookie(
            &format!("/courses/{race_id}/trace.gpx"),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let trace = body_text(response).await;
    assert!(trace.contains("<trkpt"), "{trace}");

    // La course rejoint les courses deja courues, marquee comme reference.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/courses", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("10 km de nuit"), "{body}");
    assert!(body.contains("Reference"), "{body}");

    // Le meme import par l'API, avec le jeton d'appareil.
    let token = device_token(&state, &user.id).await;
    let boundary = "----mpacer-import-api";
    let body = upload_file_body(boundary, "sortie.gpx", &strava_gpx_export());
    let response = app
        .clone()
        .oneshot(import_request(
            "/api/v1/races/import",
            boundary,
            body,
            Some(&token),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let imported: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(imported["is_reference"], serde_json::json!(true));
    assert_eq!(imported["source"], serde_json::json!("strava"));
    assert_eq!(imported["moving_time_s"], serde_json::json!(30.0));

    // Sans jeton ni session, l'import est ferme.
    let boundary = "----mpacer-import-anonyme";
    let body = upload_file_body(boundary, "10km.gpx", &strava_gpx_export());
    let response = app
        .clone()
        .oneshot(import_request("/api/v1/races/import", boundary, body, None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Un fichier qui n'est pas une trace revient sur la page avec un message clair.
    let boundary = "----mpacer-import-refus";
    let body = upload_file_body(boundary, "connexion.html", b"<html>Connexion</html>");
    let response = app
        .clone()
        .oneshot(upload_request(
            "/courses/importer",
            boundary,
            body,
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let body = body_text(response).await;
    assert!(body.contains("GPX ou TCX"), "{body}");
}

#[tokio::test]
async fn a_workout_comment_is_saved_and_shown() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (user, session) = dev_user_session(&state).await;
    let token = device_token(&state, &user.id).await;
    let workout_id = "1700000000000";

    // Une seance arrive depuis la montre.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workouts")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(sample_workout().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // La fiche propose le commentaire meme quand il n'y en a pas encore.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(
                &format!("/workouts/{workout_id}"),
                &session,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Commentaire de course"), "{body}");

    // Le coureur l'ecrit depuis le navigateur.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/workouts/{workout_id}/comment"),
            &format!("comment={}", encode("Belle sortie, jambes legeres.")),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let (row, _) = mpacer_api::db::get_workout(&state.pool, &user.id, workout_id)
        .await
        .unwrap()
        .expect("seance");
    assert_eq!(
        row.comment.as_deref(),
        Some("Belle sortie, jambes legeres.")
    );

    // Il s'affiche sur la fiche et dans l'historique.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(
                &format!("/workouts/{workout_id}"),
                &session,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Belle sortie, jambes legeres."), "{body}");
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("row-comment"), "{body}");

    // Une nouvelle synchronisation de la montre ne l'efface pas.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workouts")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(sample_workout().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (row, _) = mpacer_api::db::get_workout(&state.pool, &user.id, workout_id)
        .await
        .unwrap()
        .expect("seance");
    assert_eq!(
        row.comment.as_deref(),
        Some("Belle sortie, jambes legeres.")
    );

    // L'API appareil peut aussi l'ecrire, et un texte vide l'efface.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/workouts/{workout_id}/comment"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({ "comment": "Revue par l'API" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (row, _) = mpacer_api::db::get_workout(&state.pool, &user.id, workout_id)
        .await
        .unwrap()
        .expect("seance");
    assert_eq!(row.comment.as_deref(), Some("Revue par l'API"));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/workouts/{workout_id}/comment"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &session)
                .body(Body::from(
                    serde_json::json!({ "comment": "   " }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let (row, _) = mpacer_api::db::get_workout(&state.pool, &user.id, workout_id)
        .await
        .unwrap()
        .expect("seance");
    assert!(row.comment.is_none());

    // Seance inconnue : 404, jamais un commentaire orphelin.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/workouts/inconnue/comment")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, &session)
                .body(Body::from(
                    serde_json::json!({ "comment": "x" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ------------------------------------------------------------ tableaux de bord

/// Seance de reference des tableaux de bord : deux tours, un meilleur effort,
/// du cardio et une trace GPS.
fn dashboard_workout() -> serde_json::Value {
    let mut workout = analysed_workout();
    workout["laps"] = serde_json::json!([
        { "index": 1, "distance_m": 1000.0, "duration_s": 320.0, "pace_s_per_km": 320.0 },
        { "index": 2, "distance_m": 800.0, "duration_s": 280.0, "pace_s_per_km": 350.0 }
    ]);
    workout["best_efforts"] = serde_json::json!([
        { "label": "1 km", "distance_m": 1000.0, "time_s": 320.0, "start_dist_m": 0.0 }
    ]);
    workout
}

/// Depose la seance de reference et renvoie son identifiant.
async fn store_dashboard_workout(state: &AppState, user: &mpacer_api::models::User) -> String {
    let value = dashboard_workout();
    let upload: mpacer_api::models::WorkoutUpload =
        serde_json::from_value(value.clone()).expect("seance des tableaux de bord");
    let payload = serde_json::to_string(&value).expect("payload JSON");
    mpacer_api::db::upsert_workout(&state.pool, &user.id, &upload, &payload, state.now_ms())
        .await
        .unwrap();
    upload.id
}

fn widget_kinds(widgets: &[mpacer_api::models::DashboardWidget]) -> Vec<String> {
    widgets.iter().map(|widget| widget.kind.clone()).collect()
}

#[tokio::test]
async fn dashboard_builder_composes_reorders_and_deletes_widgets() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (user, session) = dev_user_session(&state).await;
    let _workout_id = store_dashboard_workout(&state, &user).await;

    // 1. Aucun tableau au depart : la page propose d'en creer un.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/dashboards", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("Aucun tableau de bord"), "{body}");

    // 2. Le formulaire de creation liste les widgets du catalogue et les gabarits.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/dashboards/nouveau", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("Pace Control"), "{body}");
    assert!(body.contains("Analyse de seance"), "{body}");
    assert!(body.contains("value=\"meilleures-distances\""), "{body}");

    // 3. Creation depuis le gabarit « Pace Control » : aucune case cochee, le
    //    serveur applique le modele.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            "/dashboards/nouveau",
            &format!("name={}&template=pace-control", encode("Pace Control")),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(location.starts_with("/dashboards/"), "{location}");
    let dashboard_id = location.rsplit('/').next().unwrap().to_string();

    let widgets = mpacer_api::db::list_dashboard_widgets(&state.pool, &user.id, &dashboard_id)
        .await
        .unwrap();
    assert_eq!(
        widget_kinds(&widgets),
        vec!["allure", "tours", "meilleures-distances"]
    );
    assert_eq!(
        widgets
            .iter()
            .map(|widget| widget.position)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    // 4. Le tableau affiche est alimente par la derniere seance.
    let response = app
        .clone()
        .oneshot(get_with_cookie(&location, &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("Allure"), "{body}");
    assert!(body.contains("Tours"), "{body}");
    assert!(body.contains("Meilleures distances"), "{body}");
    assert!(body.contains("1 km"), "{body}");
    // L'allure du dernier tour (320 s/km) est affichee.
    assert!(body.contains("5:20"), "{body}");

    // 5. Monter le deuxieme widget : l'ordre change, les positions restent contigues.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!(
                "/dashboards/{dashboard_id}/widgets/{}/monter",
                widgets[1].id
            ),
            "",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let widgets = mpacer_api::db::list_dashboard_widgets(&state.pool, &user.id, &dashboard_id)
        .await
        .unwrap();
    assert_eq!(
        widget_kinds(&widgets),
        vec!["tours", "allure", "meilleures-distances"]
    );
    assert_eq!(
        widgets
            .iter()
            .map(|widget| widget.position)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    // 6. Monter le premier widget ne fait rien : la page reste utilisable.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!(
                "/dashboards/{dashboard_id}/widgets/{}/monter",
                widgets[0].id
            ),
            "",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let widgets = mpacer_api::db::list_dashboard_widgets(&state.pool, &user.id, &dashboard_id)
        .await
        .unwrap();
    assert_eq!(
        widget_kinds(&widgets),
        vec!["tours", "allure", "meilleures-distances"]
    );

    // 7. Ajouter un widget : il arrive en fin de tableau.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/dashboards/{dashboard_id}/widgets/ajouter"),
            "kind=cardio",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let widgets = mpacer_api::db::list_dashboard_widgets(&state.pool, &user.id, &dashboard_id)
        .await
        .unwrap();
    assert_eq!(
        widget_kinds(&widgets),
        vec!["tours", "allure", "meilleures-distances", "cardio"]
    );

    // 8. Un widget inconnu est refuse.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/dashboards/{dashboard_id}/widgets/ajouter"),
            "kind=telepathie",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // 9. Retirer le deuxieme widget : les suivants remontent.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!(
                "/dashboards/{dashboard_id}/widgets/{}/retirer",
                widgets[1].id
            ),
            "",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let widgets = mpacer_api::db::list_dashboard_widgets(&state.pool, &user.id, &dashboard_id)
        .await
        .unwrap();
    assert_eq!(
        widget_kinds(&widgets),
        vec!["tours", "meilleures-distances", "cardio"]
    );
    assert_eq!(
        widgets
            .iter()
            .map(|widget| widget.position)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    // 10. Renommer.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/dashboards/{dashboard_id}/modifier"),
            &format!("name={}", encode("  Ma course   du dimanche ")),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let dashboard = mpacer_api::db::get_dashboard(&state.pool, &user.id, &dashboard_id)
        .await
        .unwrap()
        .expect("tableau toujours present");
    assert_eq!(dashboard.name, "Ma course du dimanche");

    // 11. Un nom vide est refuse.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/dashboards/{dashboard_id}/modifier"),
            "name=+++",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // 12. Suppression : le tableau et ses widgets disparaissent.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/dashboards/{dashboard_id}/supprimer"),
            "",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/dashboards");
    assert!(
        mpacer_api::db::get_dashboard(&state.pool, &user.id, &dashboard_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        mpacer_api::db::list_dashboard_widgets(&state.pool, &user.id, &dashboard_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_dashboard_belongs_to_its_owner() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (user, session) = dev_user_session(&state).await;
    store_dashboard_workout(&state, &user).await;

    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            "/dashboards/nouveau",
            &format!("name={}&widgets=allure&widgets=tours", encode("Prive")),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    let dashboard_id = location.rsplit('/').next().unwrap().to_string();

    // Le proprietaire voit son tableau.
    let response = app
        .clone()
        .oneshot(get_with_cookie(&location, &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Un autre utilisateur ne le voit pas et ne peut pas le modifier.
    let other = mpacer_api::db::upsert_user(
        &state.pool,
        None,
        "autre@localhost",
        None,
        None,
        state.now_ms(),
    )
    .await
    .unwrap();
    let other_session = format!(
        "mpacer_session={}",
        mpacer_api::auth::issue_session(&state.config, &other, state.now_ms()).unwrap()
    );

    let response = app
        .clone()
        .oneshot(get_with_cookie(&location, &other_session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    for uri in [
        format!("/dashboards/{dashboard_id}/modifier"),
        format!("/dashboards/{dashboard_id}/widgets/ajouter"),
        format!("/dashboards/{dashboard_id}/supprimer"),
    ] {
        let response = app
            .clone()
            .oneshot(form_request(
                "POST",
                &uri,
                "name=vole&kind=allure",
                &other_session,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
    }

    // Sans session, tout ramene a la connexion.
    let response = app.clone().oneshot(get("/dashboards")).await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/login");
}

// ------------------------------------------------------------------ musique v3

#[tokio::test]
async fn music_page_validates_the_coverage_and_renames_a_playlist() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (_user, session) = dev_user_session(&state).await;

    // Une playlist confortable : 20 titres de 5 min (1 h 40) a 150 BPM.
    let playlist = mpacer_api::db::insert_music_playlist(
        &state.pool,
        &_user.id,
        &mpacer_api::models::MusicPlaylistInput {
            name: "Run 170".into(),
            source: "deezer".into(),
            deezer_id: Some("1924357302".into()),
            cover_url: None,
            target_bpm: Some(170.0),
        },
        state.now_ms(),
    )
    .await
    .unwrap();
    for position in 0..20 {
        mpacer_api::db::insert_music_track(
            &state.pool,
            &_user.id,
            &playlist.id,
            &mpacer_api::models::MusicTrackInput {
                position,
                title: format!("Titre {position}"),
                artist: Some("Artiste".into()),
                album: None,
                duration_s: Some(300.0),
                bpm: Some(150.0),
                bpm_source: Some("manual".into()),
                deezer_track_id: None,
            },
            state.now_ms(),
        )
        .await
        .unwrap();
    }

    // Course de 10 km a 5:00/km (50 min) : la musique couvre largement.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(
                &format!("/music?playlist={}&distance_km=10&allure=5:00", playlist.id),
                &session,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(
        body.contains("6. Assez de musique pour la course ?"),
        "{body}"
    );
    assert!(body.contains("coverage-ok"), "{body}");
    assert!(body.contains("OK,"), "{body}");
    assert!(body.contains("coverage-gauge-fill"), "{body}");
    assert!(body.contains("bpm-gauge-marker"), "{body}");
    assert!(body.contains("1:40:00"), "{body}");
    assert!(body.contains("BPM moyen 150"), "{body}");
    // Le formulaire est pre-rempli avec ce qui a ete soumis.
    assert!(body.contains(r#"value="10""#), "{body}");

    // Une playlist trop courte ne couvre pas un marathon.
    let short = mpacer_api::db::insert_music_playlist(
        &state.pool,
        &_user.id,
        &mpacer_api::models::MusicPlaylistInput {
            name: "Courte".into(),
            source: "manual".into(),
            deezer_id: None,
            cover_url: None,
            target_bpm: None,
        },
        state.now_ms(),
    )
    .await
    .unwrap();
    mpacer_api::db::insert_music_track(
        &state.pool,
        &_user.id,
        &short.id,
        &mpacer_api::models::MusicTrackInput {
            position: 0,
            title: "Unique".into(),
            artist: None,
            album: None,
            duration_s: Some(300.0),
            bpm: None,
            bpm_source: None,
            deezer_track_id: None,
        },
        state.now_ms(),
    )
    .await
    .unwrap();
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(
                &format!(
                    "/music?playlist={}&distance_km=42.195&allure=5:00",
                    short.id
                ),
                &session,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("coverage-bad"), "{body}");
    assert!(body.contains("insuffisant"), "{body}");

    // Le total des durees est affiche dans la liste des playlists.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/music", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Total"), "{body}");
    assert!(body.contains("Ouvrir"), "{body}");
    assert!(body.contains("Renommer"), "{body}");

    // Renommage d'une playlist.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{}/rename", playlist.id),
            "name=Sortie+longue",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let renamed = mpacer_api::db::get_music_playlist(&state.pool, &_user.id, &playlist.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(renamed.name, "Sortie longue");

    // Un nom vide est refuse sans rien changer.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{}/rename", playlist.id),
            "name=",
            &session,
        ))
        .await
        .unwrap();
    assert!(response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .contains("erreur=nom_invalide"));
    assert_eq!(
        mpacer_api::db::get_music_playlist(&state.pool, &_user.id, &playlist.id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Sortie longue"
    );
}

#[tokio::test]
async fn settings_gathers_profile_devices_pairing_and_logout() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (user, session) = dev_user_session(&state).await;

    // Un appareil appaire, pour verifier la liste et la revocation.
    let token = mpacer_api::auth::new_device_token();
    mpacer_api::db::insert_api_token(
        &state.pool,
        &user.id,
        &mpacer_api::auth::hash_token(&token),
        "Pixel Watch",
        state.now_ms(),
    )
    .await
    .unwrap();

    let response = app
        .clone()
        .oneshot(get_with_cookie("/settings", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    for expected in [
        "Reglages",
        "Profil",
        "Appareils appaires",
        "Pixel Watch",
        "Revoquer",
        "Appairer une montre",
        "BCDF-GHJK",
        "action=\"/link\"",
        "Deconnexion",
        "action=\"/logout\"",
    ] {
        assert!(
            body.contains(expected),
            "« {expected} » absent de /settings : {body}"
        );
    }
    // Reglages est un onglet de la navigation, et le menu deroulant a disparu.
    assert!(body.contains("href=\"/settings\""), "{body}");
    assert!(!body.contains("menu-panel"), "{body}");
    // Appairer et les jetons vivent dans le menu des reglages (ancres), et
    // l'en-tete ne declare plus seulement la meta iOS depreciee.
    for expected in [
        "settings-menu",
        "href=\"#appareils\"",
        "href=\"#appairer\"",
        "name=\"mobile-web-app-capable\"",
    ] {
        assert!(
            body.contains(expected),
            "« {expected} » absent de /settings : {body}"
        );
    }

    // La route /link reste servie, meme si elle quitte la navigation.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/link", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("Appairer une montre"));
}

// ------------------------------------------------------------------ amis

/// Compte de test avec l'email donne (deux comptes distincts = deux emails).
async fn compte(state: &AppState, email: &str) -> mpacer_api::models::User {
    mpacer_api::db::upsert_user(&state.pool, None, email, Some(email), None, state.now_ms())
        .await
        .unwrap()
}

/// Requete JSON portant un jeton d'appareil.
fn json_bearer(method: &str, uri: &str, payload: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(payload.to_string()))
        .unwrap()
}

/// Point de suivi en direct, tel que la montre le publie sur le broker.
fn point_live(lat: f64, lon: f64) -> String {
    format!(
        r#"{{"t":{},"lat":{lat},"lon":{lon},"acc":4.0,"dist":1200.0,"pace":300.0,"hr":148,"st":"run"}}"#,
        chrono::Utc::now().timestamp_millis()
    )
}

/// Amis : invitation, cercle, partage et position en direct de bout en bout.
#[tokio::test]
async fn friends_share_a_live_position_between_two_accounts() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let alice = compte(&state, "alice@example.org").await;
    let bob = compte(&state, "bob@example.org").await;
    let jeton_alice = device_token(&state, &alice.id).await;
    let jeton_bob = device_token(&state, &bob.id).await;

    // Bob genere un code, Alice le saisit : l'amitie se fait dans les deux sens.
    let reponse = app
        .clone()
        .oneshot(json_bearer(
            "POST",
            "/api/v1/friends/invite",
            "{}",
            &jeton_bob,
        ))
        .await
        .unwrap();
    assert_eq!(reponse.status(), StatusCode::OK);
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    let code = corps["code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 9, "code lisible du type BCDF-GHJK : {code}");
    assert!(
        corps["url"]
            .as_str()
            .unwrap()
            .ends_with(&format!("?code={code}")),
        "lien d'invitation : {corps}"
    );

    let correlation = format!(r#"{{"code":"{code}"}}"#);
    let ajout = json_bearer("POST", "/api/v1/friends/accept", &correlation, &jeton_alice);
    let reponse = app.clone().oneshot(ajout).await.unwrap();
    assert_eq!(
        reponse.status(),
        StatusCode::OK,
        "{}",
        body_text(reponse).await
    );

    // Le code est a usage unique.
    let rejoue = json_bearer("POST", "/api/v1/friends/accept", &correlation, &jeton_alice);
    let reponse = app.clone().oneshot(rejoue).await.unwrap();
    assert_eq!(reponse.status(), StatusCode::BAD_REQUEST);

    // Alice voit Bob dans son cercle (et reciproquement).
    let cercles = |jeton: String| json_bearer("GET", "/api/v1/friends", "", &jeton);
    let reponse = app
        .clone()
        .oneshot(cercles(jeton_alice.clone()))
        .await
        .unwrap();
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    assert_eq!(corps["total"], 1, "{corps}");
    assert_eq!(corps["friends"][0]["email"], "bob@example.org");
    assert_eq!(corps["share_live"], true, "le partage est actif par defaut");

    // Alice revendique son appareil puis publie une position.
    let enregistrement = json_bearer(
        "POST",
        "/api/v1/live/register",
        r#"{"device":"montre-alice","label":"Montre d'Alice"}"#,
        &jeton_alice,
    );
    let reponse = app.clone().oneshot(enregistrement).await.unwrap();
    assert_eq!(
        reponse.status(),
        StatusCode::OK,
        "{}",
        body_text(reponse).await
    );
    state
        .live
        .ingest(
            "mpacer/live/montre-alice",
            point_live(48.85, 2.35).as_bytes(),
        )
        .unwrap();

    let reponse = app
        .clone()
        .oneshot(json_bearer(
            "GET",
            "/api/v1/friends/live?trace=1",
            "",
            &jeton_bob,
        ))
        .await
        .unwrap();
    let statut = reponse.status();
    let texte = body_text(reponse).await;
    let corps: serde_json::Value = serde_json::from_str(&texte)
        .unwrap_or_else(|erreur| panic!("statut={statut} corps={texte} erreur={erreur}"));
    assert_eq!(corps["live"], 1, "{corps}");
    assert_eq!(corps["friends"][0]["live"]["lat"], 48.85, "{corps}");
    assert_eq!(
        corps["friends"][0]["live"]["heart_rate_bpm"], 148,
        "{corps}"
    );
    assert_eq!(corps["friends"][0]["live"]["device"], "montre-alice");
    assert_eq!(corps["friends"][0]["live"]["state"], "run");

    // Un nom d'appareil deja revendique ne peut pas etre pris par un autre compte.
    let vol = json_bearer(
        "POST",
        "/api/v1/live/register",
        r#"{"device":"montre-alice"}"#,
        &jeton_bob,
    );
    let reponse = app.clone().oneshot(vol).await.unwrap();
    assert_eq!(reponse.status(), StatusCode::CONFLICT);

    // Alice coupe le partage : plus aucune position ne sort, meme pour un ami.
    let coupe = json_bearer(
        "PUT",
        "/api/v1/friends/share",
        r#"{"share_live":false}"#,
        &jeton_alice,
    );
    let reponse = app.clone().oneshot(coupe).await.unwrap();
    assert_eq!(reponse.status(), StatusCode::OK);
    let reponse = app
        .clone()
        .oneshot(json_bearer("GET", "/api/v1/friends/live", "", &jeton_bob))
        .await
        .unwrap();
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    assert_eq!(corps["live"], 0, "partage coupe : {corps}");
    assert_eq!(corps["friends"][0]["sharing"], false);
    assert!(corps["friends"][0]["live"].is_null(), "{corps}");

    // Le retrait de l'amitie efface le cercle des deux cotes.
    let retrait = Request::builder()
        .method("DELETE")
        .uri(format!("/api/v1/friends/{}", bob.id))
        .header(header::AUTHORIZATION, format!("Bearer {jeton_alice}"))
        .body(Body::empty())
        .unwrap();
    let reponse = app.clone().oneshot(retrait).await.unwrap();
    assert_eq!(reponse.status(), StatusCode::NO_CONTENT);
    let reponse = app
        .clone()
        .oneshot(json_bearer("GET", "/api/v1/friends", "", &jeton_bob))
        .await
        .unwrap();
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    assert_eq!(corps["total"], 0, "{corps}");
}

/// La page Amis demande une session et affiche la carte et le code.
#[tokio::test]
async fn the_friends_page_shows_the_code_and_the_map() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (_user, session) = dev_user_session(&state).await;

    // Sans session : redirection vers la connexion.
    let reponse = app.clone().oneshot(get("/amis")).await.unwrap();
    assert_eq!(reponse.status(), StatusCode::SEE_OTHER);

    let reponse = app
        .clone()
        .oneshot(get_with_cookie("/amis", &session))
        .await
        .unwrap();
    assert_eq!(reponse.status(), StatusCode::OK);
    let page = body_text(reponse).await;
    assert!(page.contains("Amis"), "{page}");
    assert!(page.contains("Inviter un ami"), "{page}");
    assert!(page.contains("data-carte"), "carte OpenStreetMap : {page}");
    assert!(page.contains("/static/map.js"), "{page}");
    assert!(page.contains("OpenStreetMap"), "attribution : {page}");

    // Le JSON de la carte est servi a la session.
    let reponse = app
        .clone()
        .oneshot(get_with_cookie("/amis.json", &session))
        .await
        .unwrap();
    assert_eq!(reponse.status(), StatusCode::OK);
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    assert!(corps["friends"].is_array(), "{corps}");
    assert!(corps["public_url"].is_string(), "{corps}");

    // Le script de la carte est public (l'application telephone le charge).
    let reponse = app.clone().oneshot(get("/static/map.js")).await.unwrap();
    assert_eq!(reponse.status(), StatusCode::OK);
    assert!(body_text(reponse).await.contains("MpacerCartes"));
}

/// Amis : une demande ne cree l'amitie qu'apres acceptation de l'autre compte.
#[tokio::test]
async fn a_friend_request_waits_for_the_other_account() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let alice = compte(&state, "demande-alice@example.org").await;
    let bob = compte(&state, "demande-bob@example.org").await;
    let jeton_alice = device_token(&state, &alice.id).await;
    let jeton_bob = device_token(&state, &bob.id).await;

    // La recherche porte sur l'identite du compte (adresse ou nom), pas sur un
    // nom de montre ou d'appareil.
    let reponse = app
        .clone()
        .oneshot(json_bearer(
            "GET",
            "/api/v1/friends/search?q=demande-bob",
            "",
            &jeton_alice,
        ))
        .await
        .unwrap();
    assert_eq!(reponse.status(), StatusCode::OK);
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    assert_eq!(
        corps["users"][0]["email"], "demande-bob@example.org",
        "{corps}"
    );

    // Alice demande : l'amitie n'existe pas encore, la demande attend.
    let reponse = app
        .clone()
        .oneshot(json_bearer(
            "POST",
            "/api/v1/friends/requests",
            r#"{"email":"demande-bob@example.org","message":"On court ?"}"#,
            &jeton_alice,
        ))
        .await
        .unwrap();
    assert_eq!(
        reponse.status(),
        StatusCode::OK,
        "{}",
        body_text(reponse).await
    );
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    let id = corps["request"]["id"].as_str().unwrap().to_string();
    assert_eq!(corps["request"]["direction"], "out", "{corps}");
    assert_eq!(corps["status"], "pending", "{corps}");

    // Le cercle reste vide, mais la demande est annoncee.
    let reponse = app
        .clone()
        .oneshot(json_bearer("GET", "/api/v1/friends", "", &jeton_bob))
        .await
        .unwrap();
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    assert_eq!(
        corps["total"], 0,
        "pas d'amitie avant acceptation : {corps}"
    );
    assert_eq!(corps["pending_requests"], 1, "{corps}");

    // Bob voit la demande, avec le mot et la fiche d'Alice.
    let reponse = app
        .clone()
        .oneshot(json_bearer(
            "GET",
            "/api/v1/friends/requests",
            "",
            &jeton_bob,
        ))
        .await
        .unwrap();
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    assert_eq!(corps["incoming"][0]["id"], id, "{corps}");
    assert_eq!(corps["incoming"][0]["message"], "On court ?", "{corps}");
    assert_eq!(corps["incoming"][0]["direction"], "in", "{corps}");
    assert_eq!(
        corps["incoming"][0]["from"]["email"], "demande-alice@example.org",
        "{corps}"
    );

    // Bob accepte : l'amitie apparait des deux cotes, la demande disparait.
    let reponse = app
        .clone()
        .oneshot(json_bearer(
            "POST",
            &format!("/api/v1/friends/requests/{id}/accept"),
            "",
            &jeton_bob,
        ))
        .await
        .unwrap();
    assert_eq!(
        reponse.status(),
        StatusCode::OK,
        "{}",
        body_text(reponse).await
    );
    let reponse = app
        .clone()
        .oneshot(json_bearer("GET", "/api/v1/friends", "", &jeton_alice))
        .await
        .unwrap();
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    assert_eq!(corps["total"], 1, "{corps}");
    assert_eq!(
        corps["friends"][0]["email"], "demande-bob@example.org",
        "{corps}"
    );
    assert_eq!(corps["pending_requests"], 0, "{corps}");

    // La photo de profil de l'ami est servie par le service, avec le jeton.
    let reponse = app
        .clone()
        .oneshot(json_bearer(
            "GET",
            &format!("/avatar/{}", bob.id),
            "",
            &jeton_alice,
        ))
        .await
        .unwrap();
    assert_eq!(reponse.status(), StatusCode::OK, "avatar d'un ami");
}

/// Amis : une demande refusee ne cree aucune amitie.
#[tokio::test]
async fn declining_a_friend_request_leaves_no_friendship() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let claire = compte(&state, "refus-claire@example.org").await;
    let david = compte(&state, "refus-david@example.org").await;
    let jeton_claire = device_token(&state, &claire.id).await;
    let jeton_david = device_token(&state, &david.id).await;

    let reponse = app
        .clone()
        .oneshot(json_bearer(
            "POST",
            "/api/v1/friends/requests",
            r#"{"email":"refus-david@example.org"}"#,
            &jeton_claire,
        ))
        .await
        .unwrap();
    assert_eq!(reponse.status(), StatusCode::OK);
    let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
    let id = corps["request"]["id"].as_str().unwrap().to_string();

    let reponse = app
        .clone()
        .oneshot(json_bearer(
            "POST",
            &format!("/api/v1/friends/requests/{id}/decline"),
            "",
            &jeton_david,
        ))
        .await
        .unwrap();
    assert_eq!(reponse.status(), StatusCode::NO_CONTENT);

    // Rien des deux cotes : ni ami, ni demande.
    for jeton in [&jeton_claire, &jeton_david] {
        let reponse = app
            .clone()
            .oneshot(json_bearer("GET", "/api/v1/friends", "", jeton))
            .await
            .unwrap();
        let corps: serde_json::Value = serde_json::from_str(&body_text(reponse).await).unwrap();
        assert_eq!(corps["total"], 0, "{corps}");
        assert_eq!(corps["pending_requests"], 0, "{corps}");
    }
}
