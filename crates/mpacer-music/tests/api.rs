//! Tests d'integration de l'interface locale : le routeur axum est interroge
//! directement, sans adb et sans montre branchee.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use mpacer_music::server::{router, AppState, Bootstrap};
use serde_json::Value;
use tower::ServiceExt;

/// Manifeste de reference, aussi utilise pour les preuves en ligne de commande.
const MANIFEST: &str = include_str!("fixtures/run-170.json");

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Racine des dossiers de test : sous `target/`, toujours inscriptible par
/// cargo (le dossier temporaire du systeme peut y etre restreint).
fn test_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/mpacer-music-tests")
}

fn temp_dir(name: &str) -> PathBuf {
    let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
    let directory = test_root().join(format!("api-{name}-{unique}"));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("dossier de test");
    directory
}

/// Fichier MP3 minimal avec un tag ID3v2.3 TBPM.
fn write_tagged_mp3(path: &PathBuf, bpm: &str) {
    let mut body = vec![0_u8];
    body.extend_from_slice(bpm.as_bytes());
    let frame_size = body.len() as u32;
    let total = 10 + frame_size;
    let mut tag = Vec::new();
    tag.extend_from_slice(b"ID3");
    tag.extend_from_slice(&[3, 0, 0]);
    tag.push((total >> 21) as u8 & 0x7F);
    tag.push((total >> 14) as u8 & 0x7F);
    tag.push((total >> 7) as u8 & 0x7F);
    tag.push(total as u8 & 0x7F);
    tag.extend_from_slice(b"TBPM");
    tag.extend_from_slice(&frame_size.to_be_bytes());
    tag.extend_from_slice(&[0, 0]);
    tag.extend_from_slice(&body);
    fs::write(path, tag).expect("ecriture du mp3");
}

fn app() -> axum::Router {
    router(AppState::new(Bootstrap::default()))
}

fn encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/' | b':') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

async fn body_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("corps de la reponse");
    serde_json::from_slice(&bytes).expect("reponse JSON")
}

#[tokio::test]
async fn the_page_holds_the_expected_controls() {
    let response = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(bytes.to_vec()).unwrap();
    for label in [
        "Parcourir",
        "Analyser",
        "Transferer",
        "Simulation",
        "/api/inspect",
        "/api/transfer",
    ] {
        assert!(html.contains(label), "la page doit contenir {label}");
    }
}

#[tokio::test]
async fn browse_lists_directories_and_audio_files() {
    let root = temp_dir("browse");
    fs::create_dir_all(root.join("sous-dossier")).unwrap();
    write_tagged_mp3(&root.join("a.mp3"), "124");
    fs::write(root.join("notes.txt"), b"x").unwrap();

    let uri = format!("/api/browse?path={}", encode(root.to_str().unwrap()));
    let response = app()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert_eq!(value["path"], root.to_str().unwrap());
    assert_eq!(value["audio_count"], 1);
    let dirs = value["dirs"].as_array().unwrap();
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0]["name"], "sous-dossier");
    assert!(value["parent"].is_string());
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn browse_without_a_path_starts_from_the_personal_folder() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/browse")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert!(value["path"].is_string());
    assert!(value["dirs"].is_array());
}

#[tokio::test]
async fn inspect_matches_the_fixture_folder() {
    let root = temp_dir("inspect");
    write_tagged_mp3(&root.join("01 - Avicii - Wake me up.mp3"), "124");
    write_tagged_mp3(&root.join("02 - Avicii - Levels.mp3"), "128");
    write_tagged_mp3(&root.join("inutile.mp3"), "100");
    let manifest_path = root.join("run-170.json");
    fs::write(&manifest_path, MANIFEST).unwrap();

    let payload = serde_json::json!({
        "manifest_path": manifest_path.to_str().unwrap(),
        "folder": root.to_str().unwrap(),
    });
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/inspect")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert_eq!(value["playlist"]["id"], "run-170");
    assert_eq!(value["playlist"]["track_count"], 4);
    assert_eq!(value["matches"][0]["file"], "01 - Avicii - Wake me up.mp3");
    assert_eq!(value["matches"][0]["bpm"], 124.0);
    assert!(value["matches"][2]["file"].is_null());
    assert_eq!(value["missing"].as_array().unwrap().len(), 2);
    assert_eq!(value["unused_files"].as_array().unwrap().len(), 1);
    assert!(value["total_bytes"].as_u64().unwrap() > 0);
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn inspect_accepts_inline_manifest_json_and_rejects_broken_input() {
    let root = temp_dir("inspect-json");
    write_tagged_mp3(&root.join("01 - Avicii - Wake me up.mp3"), "124");

    let payload =
        serde_json::json!({ "manifest_json": MANIFEST, "folder": root.to_str().unwrap() });
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/inspect")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert_eq!(value["matches"].as_array().unwrap().len(), 4);

    let broken =
        serde_json::json!({ "manifest_json": "pas du json", "folder": root.to_str().unwrap() });
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/inspect")
                .header("content-type", "application/json")
                .body(Body::from(broken.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn devices_answers_without_a_watch() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/devices")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert!(value["adb"].is_string());
    assert!(value["devices"].is_array());
}

#[tokio::test]
async fn an_unknown_transfer_is_a_not_found() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/transfer/job-inconnu")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transfer/job-inconnu/cancel")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
