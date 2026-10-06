//! Bibliotheque locale de mpacer-music : les fichiers audio pousses depuis
//! l'interface web dans l'application, en attendant leur copie sur la montre.
//!
//! Toute la bibliotheque vit sous une racine unique : un dossier par playlist
//! (<root>/<playlist_id>/) et, dans chaque dossier, un index library.json
//! ({"version":1,"files":[...]}) qui porte le statut de synchronisation de
//! chaque fichier. Aucune E/S n'a lieu hors de cette racine et aucun index
//! illisible ne fait paniquer l'application.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::planner::ToolError;

/// Taille maximale d'un fichier pousse (200 Mo).
pub const MAX_FILE_BYTES: u64 = 200 * 1024 * 1024;
/// Nom de l'index, dans le dossier de chaque playlist.
const INDEX_FILE_NAME: &str = "library.json";
/// Version du format d'index.
const INDEX_VERSION: u32 = 1;
/// Fichier present, pas encore copie sur la montre.
pub const STATUS_PENDING: &str = "a_synchroniser";
/// Fichier copie sur la montre.
pub const STATUS_SYNCED: &str = "synchronise";
/// Derniere copie en echec.
pub const STATUS_ERROR: &str = "erreur";

/// Entree de la bibliotheque : un fichier et son statut.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryFile {
    pub file_name: String,
    pub size_bytes: u64,
    pub imported_at_ms: i64,
    /// "a_synchroniser" | "synchronise" | "erreur"
    pub status: String,
    pub synced_at_ms: Option<i64>,
    pub error: Option<String>,
}

/// Forme exacte de library.json.
#[derive(Debug, Serialize, Deserialize)]
struct LibraryIndex {
    version: u32,
    files: Vec<LibraryFile>,
}

/// Racine de la bibliotheque : les fichiers et leurs index.
pub struct LibraryStore {
    root: PathBuf,
}

impl LibraryStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Dossier de la playlist : le nettoyage interdit toute sortie de la racine.
    pub fn playlist_dir(&self, playlist_id: &str) -> PathBuf {
        self.root.join(clean_playlist_id(playlist_id))
    }

    /// Ajoute (ou remplace) un fichier et remet son statut a a_synchroniser.
    pub fn import_bytes(
        &self,
        playlist_id: &str,
        file_name: &str,
        bytes: &[u8],
        now_ms: i64,
    ) -> Result<LibraryFile, ToolError> {
        if bytes.is_empty() {
            return Err(ToolError::Usage(
                "fichier vide : aucun octet recu".to_string(),
            ));
        }
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(ToolError::Usage(format!(
                "fichier trop volumineux : {MAX_FILE_BYTES} octets au maximum"
            )));
        }
        let file_name = sanitize_file_name(file_name).ok_or_else(|| {
            ToolError::Usage(format!(
                "nom de fichier invalide (extension audio attendue, sans separateur) : {file_name}"
            ))
        })?;
        let directory = self.playlist_dir(playlist_id);
        fs::create_dir_all(&directory).map_err(|error| {
            ToolError::Io(format!(
                "creation de {} impossible : {error}",
                directory.display()
            ))
        })?;
        let path = directory.join(&file_name);
        fs::write(&path, bytes).map_err(|error| {
            ToolError::Io(format!(
                "ecriture de {} impossible : {error}",
                path.display()
            ))
        })?;
        let entry = LibraryFile {
            file_name,
            size_bytes: bytes.len() as u64,
            imported_at_ms: now_ms,
            status: STATUS_PENDING.to_string(),
            synced_at_ms: None,
            error: None,
        };
        let mut files = read_index(&directory);
        files.retain(|existing| existing.file_name != entry.file_name);
        files.push(entry.clone());
        write_index(&directory, &files)?;
        Ok(entry)
    }

    /// Fichiers de la playlist, tries par nom. Un index absent ou corrompu
    /// donne une liste vide, jamais une erreur ni un panic.
    pub fn list(&self, playlist_id: &str) -> Result<Vec<LibraryFile>, ToolError> {
        let mut files = read_index(&self.playlist_dir(playlist_id));
        files.sort_by(|left, right| left.file_name.cmp(&right.file_name));
        Ok(files)
    }

    /// Supprime le fichier et son entree ; false s'il n'existait pas.
    pub fn remove(&self, playlist_id: &str, file_name: &str) -> Result<bool, ToolError> {
        let Some(file_name) = sanitize_file_name(file_name) else {
            return Ok(false);
        };
        let directory = self.playlist_dir(playlist_id);
        let path = directory.join(&file_name);
        let mut files = read_index(&directory);
        let before = files.len();
        files.retain(|entry| entry.file_name != file_name);
        let existed = files.len() != before || path.is_file();
        if path.is_file() {
            fs::remove_file(&path).map_err(|error| {
                ToolError::Io(format!(
                    "suppression de {} impossible : {error}",
                    path.display()
                ))
            })?;
        }
        if files.len() != before {
            write_index(&directory, &files)?;
        }
        Ok(existed)
    }

    /// Marque les entrees existantes comme copiees sur la montre.
    pub fn mark_synced(
        &self,
        playlist_id: &str,
        file_names: &[String],
        now_ms: i64,
    ) -> Result<(), ToolError> {
        if file_names.is_empty() {
            return Ok(());
        }
        let directory = self.playlist_dir(playlist_id);
        let mut files = read_index(&directory);
        let mut changed = false;
        for entry in &mut files {
            if file_names.iter().any(|name| name == &entry.file_name) {
                entry.status = STATUS_SYNCED.to_string();
                entry.synced_at_ms = Some(now_ms);
                entry.error = None;
                changed = true;
            }
        }
        if changed {
            write_index(&directory, &files)?;
        }
        Ok(())
    }

    /// Marque une entree existante en erreur avec le message du transfert.
    ///
    /// L'instant n'est pas conserve : le format ne porte pas d'horodatage
    /// d'erreur, seulement celui de la derniere synchronisation reussie.
    pub fn mark_error(
        &self,
        playlist_id: &str,
        file_name: &str,
        message: &str,
        _now_ms: i64,
    ) -> Result<(), ToolError> {
        let Some(file_name) = sanitize_file_name(file_name) else {
            return Ok(());
        };
        let directory = self.playlist_dir(playlist_id);
        let mut files = read_index(&directory);
        let mut changed = false;
        for entry in &mut files {
            if entry.file_name == file_name {
                entry.status = STATUS_ERROR.to_string();
                entry.synced_at_ms = None;
                entry.error = Some(message.to_string());
                changed = true;
            }
        }
        if changed {
            write_index(&directory, &files)?;
        }
        Ok(())
    }

    /// Vrai si le chemin appartient a la playlist, donc gere par cette racine.
    pub fn is_managed_path(&self, playlist_id: &str, path: &Path) -> bool {
        let directory = self.playlist_dir(playlist_id);
        if let (Ok(directory), Ok(candidate)) = (directory.canonicalize(), path.canonicalize()) {
            return candidate.starts_with(&directory);
        }
        path.starts_with(&directory)
    }
}

/// Nettoie un nom de fichier, ou None s'il ne designe pas un fichier audio
/// local sur (vide, point, double point, separateur, caractere de controle).
pub fn sanitize_file_name(name: &str) -> Option<String> {
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    if name
        .chars()
        .any(|character| character == '/' || character == '\\' || character.is_control())
    {
        return None;
    }
    if !crate::planner::is_audio_path(Path::new(name)) {
        return None;
    }
    Some(name.to_string())
}

/// Nettoyage du playlist_id : alphanumerique ASCII, tiret et souligne seulement.
fn clean_playlist_id(playlist_id: &str) -> String {
    let cleaned: String = playlist_id
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || *character == '-' || *character == '_'
        })
        .collect();
    if cleaned.is_empty() {
        "playlist".to_string()
    } else {
        cleaned
    }
}

/// Racine par defaut de la bibliotheque, par plateforme.
pub fn default_library_root() -> PathBuf {
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local).join("mpacer-music").join("library");
        }
        return PathBuf::from("mpacer-music-library");
    }
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(data).join("mpacer-music").join("library");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("mpacer-music")
            .join("library");
    }
    PathBuf::from("mpacer-music-library")
}

/// Lit l'index ; tout probleme donne une liste vide (jamais de panic).
fn read_index(directory: &Path) -> Vec<LibraryFile> {
    let path = directory.join(INDEX_FILE_NAME);
    let Ok(json) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    match serde_json::from_str::<LibraryIndex>(&json) {
        Ok(index) => index.files,
        Err(_) => Vec::new(),
    }
}

/// Ecrit l'index, entrees triees par nom pour rester stable.
fn write_index(directory: &Path, files: &[LibraryFile]) -> Result<(), ToolError> {
    let mut files: Vec<LibraryFile> = files.to_vec();
    files.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    let index = LibraryIndex {
        version: INDEX_VERSION,
        files,
    };
    let json = serde_json::to_string_pretty(&index)
        .map_err(|error| ToolError::Io(format!("index non serialisable : {error}")))?;
    fs::create_dir_all(directory).map_err(|error| {
        ToolError::Io(format!(
            "creation de {} impossible : {error}",
            directory.display()
        ))
    })?;
    let path = directory.join(INDEX_FILE_NAME);
    fs::write(&path, format!("{json}\n")).map_err(|error| {
        ToolError::Io(format!(
            "ecriture de {} impossible : {error}",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Racine des dossiers de test : sous target/, toujours inscriptible par
    /// cargo (le dossier temporaire du systeme peut y etre restreint).
    fn test_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/mpacer-music-tests")
    }

    fn temp_root(name: &str) -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let directory = test_root().join(format!("library-{name}-{unique}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("dossier de test");
        directory
    }

    #[test]
    fn import_list_mark_and_remove_round_trip() {
        let root = temp_root("round-trip");
        let store = LibraryStore::new(&root);
        let entry = store
            .import_bytes("run-170", "titre.mp3", &[1, 2, 3, 4], 1000)
            .unwrap();
        assert_eq!(entry.status, STATUS_PENDING);
        assert_eq!(entry.size_bytes, 4);
        assert_eq!(entry.imported_at_ms, 1000);
        assert!(store.playlist_dir("run-170").join("titre.mp3").is_file());

        let files = store.list("run-170").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name, "titre.mp3");

        store
            .mark_synced("run-170", &["titre.mp3".to_string()], 2000)
            .unwrap();
        let files = store.list("run-170").unwrap();
        assert_eq!(files[0].status, STATUS_SYNCED);
        assert_eq!(files[0].synced_at_ms, Some(2000));

        assert!(store.remove("run-170", "titre.mp3").unwrap());
        assert!(store.list("run-170").unwrap().is_empty());
        assert!(!store.playlist_dir("run-170").join("titre.mp3").exists());
        assert!(!store.remove("run-170", "titre.mp3").unwrap());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn import_rejects_a_traversal_name() {
        let root = temp_root("traversal");
        let store = LibraryStore::new(&root);
        assert!(sanitize_file_name("../evil.mp3").is_none());
        assert!(sanitize_file_name("dossier/evil.mp3").is_none());
        assert!(sanitize_file_name("dossier\\evil.mp3").is_none());
        assert!(sanitize_file_name("..").is_none());
        assert!(sanitize_file_name("").is_none());
        assert!(store
            .import_bytes("run-170", "../evil.mp3", &[1], 0)
            .is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn import_rejects_a_non_audio_extension() {
        let root = temp_root("extension");
        let store = LibraryStore::new(&root);
        assert!(sanitize_file_name("notes.txt").is_none());
        assert!(sanitize_file_name("titre.mp3").is_some());
        assert!(store.import_bytes("run-170", "notes.txt", &[1], 0).is_err());
        assert!(store.import_bytes("run-170", "titre.mp3", &[], 0).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_corrupt_index_lists_nothing() {
        let root = temp_root("corrupt");
        let store = LibraryStore::new(&root);
        let directory = store.playlist_dir("run-170");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("library.json"), "pas du json").unwrap();
        assert!(store.list("run-170").unwrap().is_empty());
        // Un import reecrit un index valide.
        store
            .import_bytes("run-170", "titre.mp3", &[1, 2], 10)
            .unwrap();
        assert_eq!(store.list("run-170").unwrap().len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn importing_twice_replaces_the_entry() {
        let root = temp_root("twice");
        let store = LibraryStore::new(&root);
        store
            .import_bytes("run-170", "titre.mp3", &[1, 2, 3], 10)
            .unwrap();
        store
            .mark_synced("run-170", &["titre.mp3".to_string()], 20)
            .unwrap();
        let entry = store
            .import_bytes("run-170", "titre.mp3", &[4, 5], 30)
            .unwrap();
        assert_eq!(entry.status, STATUS_PENDING);
        assert_eq!(entry.synced_at_ms, None);
        assert_eq!(entry.error, None);
        let files = store.list("run-170").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].size_bytes, 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn only_existing_entries_are_marked() {
        let root = temp_root("marks");
        let store = LibraryStore::new(&root);
        store
            .import_bytes("run-170", "titre.mp3", &[1, 2], 10)
            .unwrap();
        store
            .mark_synced("run-170", &["inconnu.mp3".to_string()], 20)
            .unwrap();
        store
            .mark_error("run-170", "titre.mp3", "adb a echoue", 30)
            .unwrap();
        let files = store.list("run-170").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, STATUS_ERROR);
        assert_eq!(files[0].error.as_deref(), Some("adb a echoue"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn managed_paths_stay_inside_the_playlist_folder() {
        let root = temp_root("managed");
        let store = LibraryStore::new(&root);
        let directory = store.playlist_dir("run-170");
        fs::create_dir_all(&directory).unwrap();
        assert!(store.is_managed_path("run-170", &directory.join("titre.mp3")));
        assert!(!store.is_managed_path("run-170", &root.join("ailleurs.mp3")));
        assert_eq!(store.playlist_dir("").file_name().unwrap(), "playlist");
        assert_eq!(
            store.playlist_dir("../evasion").file_name().unwrap(),
            "evasion"
        );
        let _ = fs::remove_dir_all(&root);
    }
}
