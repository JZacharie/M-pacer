//! Tests d'integration de l'interface locale : le routeur axum est interroge
//! directement, sans adb et sans montre branchee.

use std::fs;
use std::path::{Path, PathBuf};
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

/// Application avec une bibliotheque locale configuree.
fn app_with_library(root: &Path) -> axum::Router {
    router(AppState::new(Bootstrap {
        library: Some(root.to_path_buf()),
        ..Bootstrap::default()
    }))
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

#[tokio::test]
async fn library_import_list_and_delete_round_trip() {
    let root = temp_dir("library");
    let app = app_with_library(&root);
    let payload = b"ID3fake-mp3".to_vec();
    let uri = format!(
        "/api/library/run-170?name={}",
        encode("01 - Avicii - Wake me up.mp3")
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .body(Body::from(payload.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert_eq!(value["file_name"], "01 - Avicii - Wake me up.mp3");
    assert_eq!(value["status"], "a_synchroniser");
    assert_eq!(value["size_bytes"], payload.len() as u64);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/library/run-170")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    let files = value["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["file_name"], "01 - Avicii - Wake me up.mp3");

    let uri = format!(
        "/api/library/run-170/{}",
        encode("01 - Avicii - Wake me up.mp3")
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert_eq!(value["removed"], true);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/library/run-170")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let value = body_json(response).await;
    assert_eq!(value["files"].as_array().unwrap().len(), 0);
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn library_import_rejects_invalid_names_and_empty_bodies() {
    let root = temp_dir("library-invalid");
    let app = app_with_library(&root);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/library/run-170?name={}",
                    encode("../evil.mp3")
                ))
                .body(Body::from(vec![1_u8, 2, 3]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/library/run-170?name={}", encode("notes.txt")))
                .body(Body::from(vec![1_u8, 2, 3]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/library/run-170?name={}", encode("titre.mp3")))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let _ = fs::remove_dir_all(&root);
}

#[tokio::test]
async fn inspect_without_a_folder_uses_the_library() {
    let root = temp_dir("library-inspect");
    let app = app_with_library(&root);
    let uri = format!(
        "/api/library/run-170?name={}",
        encode("01 - Avicii - Wake me up.mp3")
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .body(Body::from(b"ID3fake".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let payload = serde_json::json!({ "manifest_json": MANIFEST });
    let response = app
        .clone()
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
    assert_eq!(value["matches"][0]["file"], "01 - Avicii - Wake me up.mp3");
    assert_eq!(value["missing"].as_array().unwrap().len(), 3);
    let _ = fs::remove_dir_all(&root);
}

/// Le statut de synchro d'un fichier pousse suit le transfert : "a
/// synchroniser" apres l'import, inchange apres une simulation, "synchronise"
/// apres une copie reelle.
#[tokio::test]
async fn a_pushed_file_becomes_synced_only_after_a_real_transfer() {
    let library = temp_dir("library-status");
    let output = temp_dir("library-status-out");
    let app = app_with_library(&library);

    // 1. Push du premier titre du manifeste : statut "a synchroniser".
    let uri = format!(
        "/api/library/run-170?name={}",
        encode("01 - Avicii - Wake me up.mp3")
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .body(Body::from(b"ID3fake-mp3".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert_eq!(value["status"], "a_synchroniser");

    // 2. Simulation : rien n'est copie, le statut ne bouge pas.
    let job = start_transfer(&app, &output, true).await;
    let state = wait_for_transfer(&app, &job).await;
    assert_eq!(state["state"], "done", "{state}");
    assert_eq!(state["step"], "termine");
    assert_eq!(library_entry(&app).await["status"], "a_synchroniser");

    // 3. Copie reelle dans un dossier cible (mode sans montre) : le fichier
    //    pousse passe en "synchronise" et porte son horodatage.
    let job = start_transfer(&app, &output, false).await;
    let state = wait_for_transfer(&app, &job).await;
    assert_eq!(state["state"], "done", "{state}");
    let entry = library_entry(&app).await;
    assert_eq!(entry["status"], "synchronise");
    assert!(
        entry["synced_at_ms"].as_i64().unwrap_or(0) > 0,
        "horodatage de synchro attendu : {entry}"
    );
    let _ = fs::remove_dir_all(&library);
    let _ = fs::remove_dir_all(&output);
}

/// Lance un transfert et renvoie l'identifiant du job.
async fn start_transfer(app: &axum::Router, target: &Path, dry_run: bool) -> String {
    let payload = serde_json::json!({
        "manifest_json": MANIFEST,
        "dry_run": dry_run,
        "target_dir": target.to_str().unwrap(),
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transfer")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await["job_id"]
        .as_str()
        .expect("identifiant de job")
        .to_string()
}

/// Attend la fin du transfert et renvoie son etat final.
async fn wait_for_transfer(app: &axum::Router, job_id: &str) -> Value {
    for _ in 0..200 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/transfer/{job_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let value = body_json(response).await;
        if value["state"] != "running" {
            return value;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("le transfert {job_id} ne s'est jamais termine");
}

/// Premiere entree de la bibliotheque de la playlist run-170.
async fn library_entry(app: &axum::Router) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/library/run-170")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    let files = value["files"].as_array().expect("liste de fichiers");
    assert_eq!(files.len(), 1, "un seul fichier pousse : {value}");
    files[0].clone()
}

#[tokio::test]
async fn inspect_without_a_folder_or_a_library_is_a_bad_request() {
    let payload = serde_json::json!({ "manifest_json": MANIFEST });
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
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn targets_answers_without_a_watch() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/targets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value = body_json(response).await;
    assert!(value["adb"].is_string());
    assert!(value["targets"].is_array());
}

/// La page /music du service distant n'a le droit de parler a l'agent que si
/// son origine est declaree : le preflight et la requete reelle portent les
/// memes en-tetes, et tout le reste est refuse.
#[tokio::test]
async fn the_cors_preflight_accepts_only_declared_origins() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/library/run-170")
                .header("origin", "https://mpacer.p.zacharie.org")
                .header("access-control-request-method", "POST")
                .header("access-control-request-private-network", "true")
                .header("host", "127.0.0.1:8077")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let headers = response.headers();
    assert_eq!(
        headers.get("access-control-allow-origin").unwrap(),
        "https://mpacer.p.zacharie.org"
    );
    assert_eq!(
        headers.get("access-control-allow-private-network").unwrap(),
        "true"
    );
    assert!(headers.get("access-control-allow-methods").is_some());

    // La page de l'agent lui-meme (meme hote que la requete) passe sans reglage.
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/devices")
                .header("origin", "http://127.0.0.1:8077")
                .header("host", "127.0.0.1:8077")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        "http://127.0.0.1:8077"
    );

    // Une origine inconnue est refusee avant d'atteindre le gestionnaire.
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/devices")
                .header("origin", "https://exemple.inconnu")
                .header("host", "127.0.0.1:8077")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Sans en-tete Origin (curl, tests) : comportement inchange.
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
}
