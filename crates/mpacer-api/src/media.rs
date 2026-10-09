//! Stockage temporaire des MP3 televerses depuis la page /music.
//!
//! Le volume est optionnel (MPACER_MEDIA_DIR) : sans lui, aucune fonction de ce
//! module n'est appelee et le service reste « aucun audio sur le serveur ».
//!
//! Un fichier vit sous <racine>/<user_id>/<track_id>/<nom> : l'identifiant de la
//! piste (UUID) est le seul nom de dossier accepte, ce qui interdit toute sortie
//! de la racine. L'appareil supprime le fichier des qu'il l'a acquitte
//! (POST /api/v1/music/playlists/{id}/ack), et le quota par compte borne ce qui
//! peut s'accumuler en attendant.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Plafond du corps de requete accepte par le routeur pour un depot.
///
/// C'est une limite de transport, plus large que le plafond par fichier
/// (MPACER_MEDIA_MAX_FILE_BYTES, 200 Mo par defaut) pour qu'un reglage plus
/// genereux de l'exploitant reste possible. Le gestionnaire applique le vrai
/// plafond.
pub const MAX_UPLOAD_BODY_BYTES: u64 = 512 * 1024 * 1024;

/// Extensions audio acceptees (memes formats que l'outil local mpacer-music et
/// que Media3/ExoPlayer sur la montre).
const AUDIO_EXTENSIONS: [&str; 6] = ["mp3", "m4a", "ogg", "opus", "flac", "wav"];

/// Nettoie un nom de fichier audio, ou None s'il ne designe pas un fichier local
/// sur (vide, point, double point, separateur, caractere de controle).
pub fn sanitize_file_name(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    if name
        .chars()
        .any(|character| character == '/' || character == '\\' || character.is_control())
    {
        return None;
    }
    if !is_audio_name(name) {
        return None;
    }
    Some(name.to_string())
}

/// Verifie un composant de cle (identifiant d'utilisateur ou de piste).
///
/// Seuls les identifiants deja propres sont acceptes : un identifiant qui doit
/// etre nettoye est refuse plutot que transforme, pour qu'aucune valeur fournie
/// par un client ne devienne un nom de dossier inattendu.
fn sanitize_component(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    let clean = value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_');
    clean.then(|| value.to_string())
}

/// Extension en minuscules, sans point ("" si le nom n'en porte pas).
pub fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Vrai si le nom porte une extension audio connue.
pub fn is_audio_name(name: &str) -> bool {
    AUDIO_EXTENSIONS.contains(&extension(name).as_str())
}

/// Type MIME du fichier, pour la reponse de telechargement.
pub fn content_type(name: &str) -> &'static str {
    match extension(name).as_str() {
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "ogg" | "opus" => "audio/ogg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}

/// Cle de stockage d'un fichier : <user_id>/<track_id>/<nom>.
///
/// None si un composant n'est pas exploitable (identifiant vide) ou si le nom de
/// fichier est refuse.
pub fn storage_key(user_id: &str, track_id: &str, file_name: &str) -> Option<String> {
    let user = sanitize_component(user_id)?;
    let track = sanitize_component(track_id)?;
    let name = sanitize_file_name(file_name)?;
    Some(format!("{user}/{track}/{name}"))
}

/// Chemin absolu d'une cle, ou None si la cle sort de la racine.
pub fn resolve(root: &Path, key: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    let mut components = key.split('/').peekable();
    components.peek()?;
    for component in components {
        if sanitize_file_name(component).is_some() {
            path.push(component);
            continue;
        }
        // Les dossiers (identifiant utilisateur, identifiant de piste) ne portent
        // pas d'extension audio : ils suivent la regle des composants.
        let cleaned = sanitize_component(component)?;
        if cleaned != component {
            return None;
        }
        path.push(cleaned);
    }
    Some(path)
}

/// Ecrit les octets du fichier (cree les dossiers manquants).
pub fn write_file(root: &Path, key: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let path = resolve(root, key).ok_or_else(|| format!("cle de stockage invalide : {key}"))?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("cle de stockage invalide : {key}"))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("creation de {} impossible : {error}", parent.display()))?;
    std::fs::write(&path, bytes)
        .map_err(|error| format!("ecriture de {} impossible : {error}", path.display()))?;
    Ok(path)
}

/// Supprime un fichier ; une cle invalide ou un fichier absent n'est pas une erreur.
pub fn remove_file(root: &Path, key: &str) -> Result<(), String> {
    let Some(path) = resolve(root, key) else {
        return Ok(());
    };
    match std::fs::remove_file(&path) {
        Ok(()) => {
            // Le dossier de la piste est propre apres le depart du fichier.
            if let Some(parent) = path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "suppression de {} impossible : {error}",
            path.display()
        )),
    }
}

/// Taille du fichier en octets, ou None s'il n'existe pas.
pub fn file_size(root: &Path, key: &str) -> Option<u64> {
    resolve(root, key)
        .and_then(|path| std::fs::metadata(path).ok())
        .map(|metadata| metadata.len())
}

/// Collecte le fichier d'un corps multipart (champ `file`, ou premier fichier).
///
/// Le nom porte par le formulaire est celui du fichier choisi par l'utilisateur :
/// c'est lui qui est compare aux noms attendus de la playlist.
pub async fn collect_upload(
    mut multipart: axum::extract::Multipart,
) -> Result<(String, Vec<u8>), crate::error::AppError> {
    let mut file: Option<(String, Vec<u8>)> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| crate::error::AppError::InvalidMultipart(error.to_string()))?
    {
        let filename = field.file_name().map(str::to_string).unwrap_or_default();
        let bytes = field
            .bytes()
            .await
            .map_err(|error| crate::error::AppError::InvalidMultipart(error.to_string()))?;
        // Seul le premier fichier non vide compte : un formulaire qui renvoie
        // deux fois le meme champ ne doit pas ecrire deux fois le meme MP3.
        if file.is_none() && !bytes.is_empty() {
            file = Some((filename, bytes.to_vec()));
        }
    }
    file.ok_or_else(|| crate::error::AppError::InvalidMultipart("aucun fichier recu".to_string()))
}

/// Empreinte SHA-256 en hexadecimal (journalisation, verification cote appareil).
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/mpacer-api-media-tests")
            .join(format!("{name}-{unique}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn only_audio_names_are_accepted() {
        assert_eq!(
            sanitize_file_name("01 - Avicii - Wake me up.mp3").as_deref(),
            Some("01 - Avicii - Wake me up.mp3")
        );
        assert_eq!(
            sanitize_file_name("Titre.FLAC").as_deref(),
            Some("Titre.FLAC")
        );
        assert!(sanitize_file_name("notes.txt").is_none());
        assert!(sanitize_file_name("../evasion.mp3").is_none());
        assert!(sanitize_file_name("dossier/titre.mp3").is_none());
        assert!(sanitize_file_name("").is_none());
        assert!(sanitize_file_name(".").is_none());
        assert_eq!(extension("Titre.MP3"), "mp3");
        assert_eq!(content_type("x.mp3"), "audio/mpeg");
        assert_eq!(content_type("x.opus"), "audio/ogg");
    }

    #[test]
    fn storage_keys_stay_inside_the_root() {
        let key = storage_key("user-1", "track-2", "titre.mp3").unwrap();
        assert_eq!(key, "user-1/track-2/titre.mp3");
        assert!(storage_key("", "track", "titre.mp3").is_none());
        assert!(storage_key("user", "../evasion", "titre.mp3").is_none());
        assert!(storage_key("user", "track", "notes.txt").is_none());

        let root = Path::new("/data/media");
        assert_eq!(
            resolve(root, &key).unwrap(),
            root.join("user-1").join("track-2").join("titre.mp3")
        );
        assert!(resolve(root, "../../etc/passwd").is_none());
        assert!(resolve(root, "user/../..").is_none());
        assert!(resolve(root, "").is_none());
    }

    #[test]
    fn write_read_and_remove_round_trip() {
        let root = test_root("round-trip");
        let key = storage_key("u1", "t1", "titre.mp3").unwrap();
        let path = write_file(&root, &key, &[1, 2, 3, 4]).unwrap();
        assert!(path.is_file());
        assert_eq!(file_size(&root, &key), Some(4));
        assert_eq!(std::fs::read(&path).unwrap(), vec![1, 2, 3, 4]);
        remove_file(&root, &key).unwrap();
        assert_eq!(file_size(&root, &key), None);
        // Un retrait deja fait n'est pas une erreur.
        remove_file(&root, &key).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sha256_matches_the_known_empty_digest() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
