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

/// Application de test avec un client Spotify configure.
///
/// Aucun appel reseau ne part tant qu'aucun compte n'est lie : la page propose
/// seulement la connexion et l'OAuth s'arrete a l'URL d'autorisation.
async fn test_app_spotify(dev_auth: bool) -> Option<(Router, AppState)> {
    test_app_with(dev_auth, true).await
}

async fn test_app_with(dev_auth: bool, spotify: bool) -> Option<(Router, AppState)> {
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
    // Les fichiers audio sont de vrais fichiers : chaque test a son dossier.
    config.media_dir = media_test_dir(&schema);
    if spotify {
        config.spotify_client_id = Some("client-de-test".to_string());
        config.spotify_client_secret = Some("secret-de-test".to_string());
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

    // 4. Les jetons d'appareil sont listes dans les reglages.
    let response = app
        .oneshot(get_with_cookie("/settings", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("Jetons d'appareil"));
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

/// Dossier de medias isole par test (les fichiers audio sont reels sur disque).
///
/// Il vit sous le dossier `target` du depot, toujours inscriptible : le dossier
/// temporaire du systeme peut manquer ou etre refuse selon l'environnement.
fn media_test_dir(schema: &str) -> std::path::PathBuf {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("test-media")
        .join(schema);
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

// ------------------------------------------------------------------ musique

/// MP3 minimal portant une balise ID3v2.3 `TBPM` (172 BPM ici).
///
/// Le corps d'une trame texte ID3v2.3 commence par l'octet d'encodage
/// (0x00 = ISO-8859-1) et se termine par un NUL.
fn tagged_mp3(bpm: &str) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.extend_from_slice(b"TBPM");
    frame.extend_from_slice(&(bpm.len() as u32 + 2).to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.push(0);
    frame.extend_from_slice(bpm.as_bytes());
    frame.push(0);

    let mut tag = Vec::new();
    tag.extend_from_slice(b"ID3");
    tag.extend_from_slice(&[3, 0, 0]);
    let size = frame.len() as u32;
    tag.push(((size >> 21) & 0x7f) as u8);
    tag.push(((size >> 14) & 0x7f) as u8);
    tag.push(((size >> 7) & 0x7f) as u8);
    tag.push((size & 0x7f) as u8);
    tag.extend_from_slice(&frame);
    // Quelques octets d'audio apres la balise, comme dans un vrai fichier.
    tag.extend_from_slice(&[0xff, 0xfb, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00]);
    tag
}

fn push_text(body: &mut Vec<u8>, text: &str) {
    body.extend_from_slice(text.as_bytes());
}

/// Corps `multipart/form-data` minimal (aucune dependance de test supplementaire).
fn multipart_body(boundary: &str, playlist_name: &str, files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    multipart_body_named(boundary, "playlist_name", playlist_name, files)
}

/// Meme corps, avec le nom du champ libre (le navigateur envoie
/// `playlist_name`, l'API appareil `name`).
fn multipart_body_named(
    boundary: &str,
    field: &str,
    playlist_name: &str,
    files: &[(&str, Vec<u8>)],
) -> Vec<u8> {
    let mut body = Vec::new();
    push_text(&mut body, &format!("--{boundary}\r\n"));
    push_text(
        &mut body,
        &format!("Content-Disposition: form-data; name=\"{field}\"\r\n\r\n"),
    );
    push_text(&mut body, playlist_name);
    push_text(&mut body, "\r\n");
    for (filename, bytes) in files {
        push_text(&mut body, &format!("--{boundary}\r\n"));
        push_text(
            &mut body,
            &format!("Content-Disposition: form-data; name=\"files\"; filename=\"{filename}\"\r\n"),
        );
        push_text(&mut body, "Content-Type: audio/mpeg\r\n\r\n");
        body.extend_from_slice(bytes);
        push_text(&mut body, "\r\n");
    }
    push_text(&mut body, &format!("--{boundary}--\r\n"));
    body
}

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
async fn music_api_serves_playlists_tracks_files_and_plans() {
    let (app, state) = app_or_skip!(test_app(false).await);
    let (user, _session) = dev_user_session(&state).await;
    let token = device_token(&state, &user.id).await;

    // Une playlist televersee : les octets sont sur le disque du serveur.
    let playlist = mpacer_api::db::insert_music_playlist(
        &state.pool,
        &user.id,
        &mpacer_api::models::MusicPlaylistInput {
            name: "Ma course 10 km".into(),
            source: "upload".into(),
            spotify_id: None,
            cover_url: None,
            target_bpm: Some(170.0),
        },
        state.now_ms(),
    )
    .await
    .unwrap();

    let bytes = tagged_mp3("172");
    let relative = format!("{}/titre-test.mp3", user.id);
    let path = state.config.media_dir.join(&relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &bytes).unwrap();

    let track_id = mpacer_api::db::insert_music_track(
        &state.pool,
        &user.id,
        &playlist.id,
        &mpacer_api::models::MusicTrackInput {
            position: 0,
            title: "Wake me up".into(),
            artist: Some("Avicii".into()),
            album: None,
            duration_s: Some(249.0),
            bpm: Some(172.0),
            bpm_source: Some("tag".into()),
            spotify_uri: None,
            mime: Some("audio/mpeg".into()),
            size_bytes: Some(bytes.len() as i64),
            storage_path: Some(relative.clone()),
        },
        state.now_ms(),
    )
    .await
    .unwrap();

    // 1. La liste resume la playlist.
    let response = app
        .clone()
        .oneshot(bearer("/api/v1/music/playlists", &token))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let list: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(list["playlists"][0]["name"], "Ma course 10 km");
    assert_eq!(list["playlists"][0]["track_count"], 1);
    assert_eq!(list["playlists"][0]["total_bytes"], bytes.len() as i64);
    assert_eq!(list["playlists"][0]["ready_track_count"], 1);
    assert_eq!(list["playlists"][0]["target_bpm"], 170.0);

    // 2. La fiche publie le chemin relatif de telechargement.
    let response = app
        .clone()
        .oneshot(bearer(
            &format!("/api/v1/music/playlists/{}", playlist.id),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let detail: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(detail["source"], "upload");
    assert_eq!(detail["tracks"][0]["title"], "Wake me up");
    assert_eq!(detail["tracks"][0]["bpm"], 172.0);
    assert_eq!(detail["tracks"][0]["bpm_source"], "tag");
    assert_eq!(
        detail["tracks"][0]["download_url"],
        format!("/api/v1/music/tracks/{track_id}/file")
    );

    // 3. Le fichier est servi entier, puis par tranches (reprise du telechargement).
    let response = app
        .clone()
        .oneshot(bearer(
            &format!("/api/v1/music/tracks/{track_id}/file"),
            &token,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "audio/mpeg");
    assert_eq!(response.headers()[header::ACCEPT_RANGES], "bytes");
    assert_eq!(
        response.headers()[header::CONTENT_LENGTH],
        bytes.len().to_string()
    );
    let downloaded = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(downloaded.as_ref(), bytes.as_slice());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/music/tracks/{track_id}/file"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::RANGE, "bytes=0-9")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response.headers()[header::CONTENT_RANGE],
        format!("bytes 0-9/{}", bytes.len())
    );
    let partial = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(partial.as_ref(), &bytes[..10]);

    // 4. Accuse de la montre.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/music/playlists/{}/ack", playlist.id))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({ "track_ids": [track_id] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let ack: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(ack["playlist_id"], playlist.id.as_str());
    assert_eq!(ack["downloaded"], 1);

    // 5. Plan de telechargement : absent, cree, acquitte.
    let response = app
        .clone()
        .oneshot(bearer("/api/v1/music/prepare", &token))
        .await
        .unwrap();
    let prepared: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert!(prepared["plan"].is_null(), "{prepared}");

    let plan = mpacer_api::db::insert_music_plan(
        &state.pool,
        &user.id,
        &playlist.id,
        None,
        Some(176.0),
        state.now_ms(),
    )
    .await
    .unwrap();
    let response = app
        .clone()
        .oneshot(bearer("/api/v1/music/prepare", &token))
        .await
        .unwrap();
    let prepared: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(prepared["plan"]["id"], plan.id.as_str());
    assert_eq!(prepared["plan"]["name"], "Ma course 10 km");
    assert_eq!(prepared["plan"]["target_bpm"], 176.0);
    assert_eq!(prepared["plan"]["tracks"][0]["id"], track_id.as_str());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/music/prepare/ack")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({ "plan_id": plan.id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("\"ok\":true"));

    let response = app
        .clone()
        .oneshot(bearer("/api/v1/music/prepare", &token))
        .await
        .unwrap();
    let prepared: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert!(
        prepared["plan"].is_null(),
        "le plan acquitte disparait : {prepared}"
    );

    // 6. Meme garde que /api/v1/workouts : sans jeton, rien n'est accessible.
    let response = app.oneshot(get("/api/v1/music/playlists")).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn music_page_has_four_blocks_and_prepares_a_race() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (_user, session) = dev_user_session(&state).await;

    // La page vide presente les quatre blocs de la maquette.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/music", &session))
            .await
            .unwrap(),
    )
    .await;
    for expected in [
        "1. Source de musique",
        "2. Playlists preparees",
        "3. Titres (playlist selectionnee)",
        "4. Preparation de la prochaine course",
        "Fichiers personnels",
        "Choisir des fichiers MP3/OGG/M4A",
        "Nom de playlist",
        "Importer sur le serveur",
        "BPM cible",
        "Envoyer sur la montre",
        "aucun plan en attente",
        "Aucune playlist pour l'instant",
    ] {
        assert!(
            body.contains(expected),
            "« {expected} » absent de /music : {body}"
        );
    }
    // Spotify reste explicitement optionnel.
    assert!(body.contains("Spotify n'est pas configure"), "{body}");

    // Televersement multipart de deux fichiers, dont un porte un BPM en balise.
    let boundary = "----mpacer-test-boundary";
    let files = vec![
        ("Wake_me-up.mp3", tagged_mp3("172")),
        (
            "Titre_sans_bpm.mp3",
            vec![
                0x49, 0x44, 0x33, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02,
            ],
        ),
    ];
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/music/upload")
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header(header::COOKIE, &session)
                .body(Body::from(multipart_body(
                    boundary,
                    "Ma course 10 km",
                    &files,
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(location.contains("ok=fichiers_importes"), "{location}");
    let playlist_id = location
        .split("playlist=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap()
        .to_string();

    // Les titres apparaissent avec leur BPM de balise.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(
                &format!("/music?playlist={playlist_id}"),
                &session,
            ))
            .await
            .unwrap(),
    )
    .await;
    for expected in [
        "Ma course 10 km",
        "Wake me up",
        "Titre sans bpm",
        "172",
        "tapper",
        "saisir",
        "Preparer",
    ] {
        assert!(body.contains(expected), "« {expected} » absent : {body}");
    }

    // Consigne de tempo de la playlist.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{playlist_id}/target"),
            "target_bpm=170",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    // BPM saisi a la main, puis tap-tempo.
    let tracks = mpacer_api::db::list_music_tracks(&state.pool, &_user.id, &playlist_id)
        .await
        .unwrap();
    assert_eq!(tracks.len(), 2);
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{playlist_id}/track-bpm"),
            &format!("track_id={}&bpm=181&source=manual", tracks[1].id),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{playlist_id}/track-bpm"),
            &format!("track_id={}&taps=0,500,1000,1500,2000", tracks[0].id),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let tracks = mpacer_api::db::list_music_tracks(&state.pool, &_user.id, &playlist_id)
        .await
        .unwrap();
    assert_eq!(tracks[1].bpm, Some(181.0));
    assert_eq!(tracks[1].bpm_source.as_deref(), Some("manual"));
    assert_eq!(tracks[0].bpm, Some(120.0));
    assert_eq!(tracks[0].bpm_source.as_deref(), Some("tap"));

    // Un BPM hors bornes est refuse sans rien ecrire.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{playlist_id}/track-bpm"),
            &format!("track_id={}&bpm=1000", tracks[1].id),
            &session,
        ))
        .await
        .unwrap();
    assert!(response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .contains("erreur=bpm_invalide"));

    // Preparation de la course : le plan attend la montre.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            "/music/prepare",
            &format!("playlist_id={playlist_id}&race_id="),
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie(
                &format!("/music?playlist={playlist_id}"),
                &session,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("en attente de la montre"), "{body}");
    assert!(body.contains("Annuler le plan"), "{body}");
    assert!(body.contains("value=\"170\""), "BPM cible affiche : {body}");

    let response = app
        .clone()
        .oneshot(get_with_cookie("/api/v1/music/prepare", &session))
        .await
        .unwrap();
    let prepared: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(prepared["plan"]["playlist_id"], playlist_id.as_str());
    assert_eq!(prepared["plan"]["target_bpm"], 170.0);

    let response = app
        .clone()
        .oneshot(form_request("POST", "/music/prepare/cancel", "", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let response = app
        .clone()
        .oneshot(get_with_cookie("/api/v1/music/prepare", &session))
        .await
        .unwrap();
    let prepared: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert!(prepared["plan"].is_null(), "{prepared}");

    // Suppression : la playlist et ses fichiers disparaissent.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            &format!("/music/playlists/{playlist_id}/delete"),
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
    // Le libelle du formulaire contient aussi « Ma course 10 km » (placeholder) :
    // on cherche donc le lien de la playlist, pas le simple texte.
    assert!(!body.contains(">Ma course 10 km</a>"), "{body}");
    assert!(mpacer_api::db::list_music_playlists(&state.pool, &_user.id)
        .await
        .unwrap()
        .is_empty());
    assert!(
        mpacer_api::db::list_music_tracks(&state.pool, &_user.id, &playlist_id)
            .await
            .unwrap()
            .is_empty()
    );
    let stored: Vec<_> = walkdir(&state.config.media_dir);
    assert!(stored.is_empty(), "fichiers restants : {stored:?}");
}

/// Tous les fichiers d'un dossier, recursivement (verification de nettoyage).
fn walkdir(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walkdir(&path));
        } else {
            files.push(path);
        }
    }
    files
}

#[tokio::test]
async fn spotify_stays_optional_without_credentials() {
    let (app, state) = app_or_skip!(test_app(true).await);
    let (_user, session) = dev_user_session(&state).await;

    // Sans identifiants, /auth/spotify renvoie vers la page avec un message clair
    // (aucun appel reseau).
    let response = app
        .clone()
        .oneshot(get_with_cookie("/auth/spotify", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/music?erreur=spotify_non_configure"
    );

    // La recherche affiche le meme message sans contacter Spotify.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/music/search?q=running", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Spotify n'est pas configure"), "{body}");

    // L'import est refuse proprement.
    let response = app
        .clone()
        .oneshot(form_request(
            "POST",
            "/music/import",
            "spotify_ref=https%3A%2F%2Fopen.spotify.com%2Fplaylist%2F37i9dQZF1DXcBWIGoYBM5M&target_bpm=",
            &session,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[header::LOCATION],
        "/music?erreur=spotify_non_configure"
    );

    // Le compte n'existe pas : rien n'a ete ecrit.
    assert!(mpacer_api::db::get_spotify_account(&state.pool, &_user.id)
        .await
        .unwrap()
        .is_none());
}

/// Requete multipart portant (ou non) le jeton d'appareil.
fn multipart_request(
    uri: &str,
    boundary: &str,
    body: Vec<u8>,
    token: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder().method("POST").uri(uri).header(
        header::CONTENT_TYPE,
        format!("multipart/form-data; boundary={boundary}"),
    );
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    builder.body(Body::from(body)).unwrap()
}

#[tokio::test]
async fn music_upload_endpoint_accepts_a_device_token() {
    let (app, state) = app_or_skip!(test_app(false).await);
    let (user, _session) = dev_user_session(&state).await;
    let token = device_token(&state, &user.id).await;

    let boundary = "----mpacer-device-boundary";
    let files = vec![
        ("Wake_me-up.mp3", tagged_mp3("172")),
        ("Titre_sans_bpm.ogg", b"OggS.....".to_vec()),
    ];
    let body = multipart_body_named(boundary, "name", "Run du soir", &files);

    // Sans jeton d'appareil, l'endpoint est ferme (meme garde que /api/v1/workouts).
    let response = app
        .clone()
        .oneshot(multipart_request(
            "/api/v1/music/playlists",
            boundary,
            body.clone(),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Avec le jeton : la playlist est creee et ses octets sont servables.
    let response = app
        .clone()
        .oneshot(multipart_request(
            "/api/v1/music/playlists",
            boundary,
            body,
            Some(&token),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let created: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    assert_eq!(created["name"], "Run du soir");
    assert_eq!(created["track_count"], 2);
    assert_eq!(created["ready_track_count"], 2);
    assert!(created["total_bytes"].as_i64().unwrap() > 0, "{created}");
    let playlist_id = created["playlist_id"].as_str().unwrap().to_string();

    let response = app
        .clone()
        .oneshot(bearer(
            &format!("/api/v1/music/playlists/{playlist_id}"),
            &token,
        ))
        .await
        .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&body_text(response).await).unwrap();
    let tracks = detail["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0]["bpm"], 172.0);
    assert_eq!(tracks[0]["bpm_source"], "tag");
    let download = tracks[0]["download_url"]
        .as_str()
        .expect("chemin de telechargement")
        .to_string();
    let response = app
        .clone()
        .oneshot(bearer(&download, &token))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "audio/mpeg");

    // Format inconnu : 415 unsupported_media_type.
    let body = multipart_body_named(
        boundary,
        "name",
        "Notes",
        &[("notes.txt", b"pas de l'audio".to_vec())],
    );
    let response = app
        .clone()
        .oneshot(multipart_request(
            "/api/v1/music/playlists",
            boundary,
            body,
            Some(&token),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(body_text(response).await.contains("unsupported_media_type"));

    // Nom de playlist absent : 400 invalid_multipart.
    let body = multipart_body_named(boundary, "name", "", &files);
    let response = app
        .clone()
        .oneshot(multipart_request(
            "/api/v1/music/playlists",
            boundary,
            body,
            Some(&token),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(body_text(response).await.contains("invalid_multipart"));

    // Aucun fichier orphelin : une seule playlist et deux fichiers.
    assert_eq!(
        mpacer_api::db::list_music_playlists(&state.pool, &user.id)
            .await
            .unwrap()
            .len(),
        1
    );
    let stored = walkdir(&state.config.media_dir);
    assert_eq!(stored.len(), 2, "fichiers restants : {stored:?}");
}

#[tokio::test]
async fn spotify_oauth_starts_when_configured() {
    let (app, state) = app_or_skip!(test_app_spotify(true).await);
    let (_user, session) = dev_user_session(&state).await;

    // La page propose la connexion quand le service est configure.
    let body = body_text(
        app.clone()
            .oneshot(get_with_cookie("/music", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Connecter Spotify"), "{body}");
    assert!(!body.contains("Spotify n'est pas configure"), "{body}");

    // /auth/spotify part vers Spotify avec PKCE, sans aucun appel reseau.
    let response = app
        .clone()
        .oneshot(get_with_cookie("/auth/spotify", &session))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        location.starts_with("https://accounts.spotify.com/authorize"),
        "{location}"
    );
    assert!(location.contains("client_id=client-de-test"), "{location}");
    assert!(
        location.contains("code_challenge_method=S256"),
        "{location}"
    );
    assert!(location.contains("playlist-read-private"), "{location}");

    // La recherche demande d'abord de connecter le compte.
    let body = body_text(
        app.oneshot(get_with_cookie("/music/search?q=running", &session))
            .await
            .unwrap(),
    )
    .await;
    assert!(body.contains("Connectez votre compte Spotify"), "{body}");
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
