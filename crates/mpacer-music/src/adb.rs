//! Appels adb : detection du binaire, montres branchees, espace libre, push,
//! liste et suppression a distance.
//!
//! Aucune montre n'est necessaire pour compiler ni pour tester : ces fonctions
//! ne sont appelees qu'en mode adb (jamais en `--dry-run` ni `--target-dir`).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

/// Nom du binaire adb selon la plateforme.
fn adb_file_name() -> &'static str {
    if cfg!(windows) {
        "adb.exe"
    } else {
        "adb"
    }
}

/// Trouve adb : option --adb, puis ANDROID_HOME / ANDROID_SDK_ROOT, puis PATH.
pub fn resolve_adb(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return path.is_file().then(|| path.to_path_buf());
    }
    for variable in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(root) = std::env::var_os(variable) {
            let candidate = PathBuf::from(root)
                .join("platform-tools")
                .join(adb_file_name());
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join(adb_file_name());
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Lance adb et renvoie sa sortie standard, ou un message d'erreur clair.
pub fn run(adb: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(adb)
        .args(args)
        .output()
        .map_err(|error| format!("adb ({}) : {error}", adb.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(format!("adb {} a echoue : {detail}", args.join(" ")));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Montre detectee par adb.
#[derive(Debug, Clone, Serialize)]
pub struct Device {
    pub serial: String,
    pub model: Option<String>,
    pub state: String,
    pub free_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}

/// Montres branchees ; l'espace libre est renseigne pour celles qui sont
/// autorisees (etat `device`).
pub fn devices(adb: &Path, probe_path: &str) -> Result<Vec<Device>, String> {
    let output = run(adb, &["devices", "-l"])?;
    let mut devices = Vec::new();
    for line in output.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (Some(serial), Some(state)) = (parts.next(), parts.next()) else {
            continue;
        };
        let model = line.split_whitespace().find_map(|part| {
            part.strip_prefix("model:")
                .map(|model| model.replace('_', " "))
        });
        let (free_bytes, total_bytes) = if state == "device" {
            free_space(adb, serial, probe_path)
        } else {
            (None, None)
        };
        devices.push(Device {
            serial: serial.to_string(),
            model,
            state: state.to_string(),
            free_bytes,
            total_bytes,
        });
    }
    Ok(devices)
}

/// Espace libre et total (octets) du volume qui porte `remote`.
pub fn free_space(adb: &Path, serial: &str, remote: &str) -> (Option<u64>, Option<u64>) {
    for target in [remote, "/sdcard", "/data"] {
        if let Ok(output) = run(adb, &["-s", serial, "shell", "df", "-k", target]) {
            if let Some((free, total)) = parse_df(&output) {
                return (Some(free), Some(total));
            }
        }
    }
    (None, None)
}

/// Lit une sortie `df -k` et renvoie (libre, total) en octets.
fn parse_df(output: &str) -> Option<(u64, u64)> {
    for line in output.lines().rev() {
        let columns: Vec<&str> = line.split_whitespace().collect();
        if columns.len() >= 4 {
            if let (Ok(total), Ok(free)) = (columns[1].parse::<u64>(), columns[3].parse::<u64>()) {
                return Some((free.saturating_mul(1024), total.saturating_mul(1024)));
            }
        }
    }
    None
}

/// Cree le dossier distant (et ses parents).
pub fn mkdirs(adb: &Path, serial: &str, remote: &str) -> Result<(), String> {
    run(adb, &["-s", serial, "shell", "mkdir", "-p", remote]).map(|_| ())
}

/// Pousse un fichier local vers un fichier distant.
pub fn push_file(adb: &Path, serial: &str, local: &Path, remote: &str) -> Result<(), String> {
    let local = local.to_string_lossy().into_owned();
    run(adb, &["-s", serial, "push", &local, remote]).map(|_| ())
}

/// Liste les entrees d'un dossier distant.
pub fn list_remote(adb: &Path, serial: &str, remote: &str) -> Result<Vec<String>, String> {
    let output = run(adb, &["-s", serial, "shell", "ls", remote])?;
    Ok(output
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect())
}

/// Supprime un fichier distant.
pub fn remove_remote(adb: &Path, serial: &str, remote: &str) -> Result<(), String> {
    run(adb, &["-s", serial, "shell", "rm", "-f", remote]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn df_output_is_parsed_in_kibibytes() {
        let output = "Filesystem     1K-blocks     Used Available Use% Mounted on\n\
                      /dev/fuse       12345678  7654321   4691357  62% /storage/emulated\n";
        assert_eq!(
            parse_df(output),
            Some((4_691_357 * 1024, 12_345_678 * 1024))
        );
        assert_eq!(parse_df(""), None);
        assert_eq!(parse_df("Filesystem 1K-blocks Used Available\n"), None);
    }

    #[test]
    fn an_explicit_adb_path_must_exist() {
        assert!(resolve_adb(Some(Path::new("Z:/outils/adb-qui-n-existe-pas.exe"))).is_none());
    }
}
