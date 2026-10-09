//! Interface en ligne de commande de `mpacer-music`.
//!
//! Sans sous-commande, le binaire lance l'interface web locale sur
//! 127.0.0.1:8077 : la CLI et l'interface appellent le meme planificateur
//! (`crate::planner`).

use std::path::{Path, PathBuf};

use crate::adb;
use crate::library::{default_library_root, LibraryStore};
use crate::planner::{self, Progress, ProgressSink, TransferRequest, WATCH_MUSIC_DIR};
use crate::server::{self, Bootstrap};

/// Succes.
pub const EXIT_OK: i32 = 0;
/// Usage ou entree invalide.
pub const EXIT_USAGE: i32 = 2;
/// adb introuvable.
pub const EXIT_ADB_MISSING: i32 = 3;
/// Aucune montre detectee.
pub const EXIT_NO_DEVICE: i32 = 4;
/// Espace insuffisant sur la montre.
pub const EXIT_NOT_ENOUGH_SPACE: i32 = 5;
/// Titres manquants avec --strict.
pub const EXIT_MISSING_TRACKS: i32 = 6;

/// Point d'entree du binaire : renvoie le code de sortie du contrat.
pub fn run(args: Vec<String>) -> i32 {
    let mut adb_option: Option<PathBuf> = None;
    let mut rest: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--adb" {
            match args.get(index + 1) {
                Some(value) => {
                    adb_option = Some(PathBuf::from(value));
                    index += 2;
                }
                None => {
                    eprintln!("--adb attend un chemin");
                    return EXIT_USAGE;
                }
            }
        } else if let Some(value) = args[index].strip_prefix("--adb=") {
            adb_option = Some(PathBuf::from(value));
            index += 1;
        } else {
            rest.push(args[index].clone());
            index += 1;
        }
    }

    match rest.first().map(String::as_str) {
        None => serve(&[], adb_option),
        Some("--help") | Some("-h") | Some("help") => {
            print_help();
            EXIT_OK
        }
        Some("devices") => devices_command(adb_option),
        Some("inspect") => inspect_command(&rest[1..]),
        Some("transfer") => transfer_command(&rest[1..], adb_option),
        Some(other) if other.starts_with('-') => serve(&rest, adb_option),
        Some(other) => {
            eprintln!("sous-commande inconnue : {other}");
            print_help();
            EXIT_USAGE
        }
    }
}

fn print_help() {
    println!("mpacer-music : copie les fichiers audio du PC vers la montre M-pacer par USB (adb)");
    println!();
    println!(
        "  mpacer-music [--port N] [--no-browser] [--target-dir DIR] [--library DIR] [--allow-origin URL]..."
    );
    println!("  mpacer-music devices");
    println!("  mpacer-music inspect  --manifest FICHIER --folder DOSSIER [--library DIR]");
    println!("  mpacer-music transfer --manifest FICHIER --folder DOSSIER [--library DIR] [--serial XXX]");
    println!("                        [--dry-run] [--prune] [--strict] [--target-dir DIR]");
    println!();
    println!("Option commune : --adb CHEMIN");
    println!("--library DIR : racine de la bibliotheque locale ; sans option, dossier de donnees de l'utilisateur.");
    println!(
        "--allow-origin URL : origine autorisee a appeler l'agent depuis un navigateur (page /music). Repetable ; par defaut localhost:8080 et mpacer.p.zacharie.org."
    );
    println!(
        "Codes de sortie : 0 ok, 2 usage, 3 adb introuvable, 4 aucune montre, 5 espace insuffisant, 6 titres manquants (--strict)"
    );
}

// ------------------------------------------------------------------ serveur

fn serve(args: &[String], adb: Option<PathBuf>) -> i32 {
    let mut port: u16 = 8077;
    let mut open_browser = true;
    let mut target_dir: Option<PathBuf> = None;
    let mut library: Option<PathBuf> = Some(default_library_root());
    let mut allow_origins: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--port" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("--port attend un numero");
                    return EXIT_USAGE;
                };
                match value.parse::<u16>() {
                    Ok(parsed) => {
                        port = parsed;
                        index += 2;
                    }
                    Err(_) => {
                        eprintln!("port invalide : {value}");
                        return EXIT_USAGE;
                    }
                }
            }
            "--no-browser" => {
                open_browser = false;
                index += 1;
            }
            "--target-dir" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("--target-dir attend un chemin");
                    return EXIT_USAGE;
                };
                target_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--library" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("--library attend un chemin");
                    return EXIT_USAGE;
                };
                library = Some(PathBuf::from(value));
                index += 2;
            }
            "--allow-origin" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!(
                        "--allow-origin attend une origine (ex. https://mpacer.p.zacharie.org)"
                    );
                    return EXIT_USAGE;
                };
                allow_origins.push(value.clone());
                index += 2;
            }
            other if other.starts_with("--allow-origin=") => {
                let value = other.trim_start_matches("--allow-origin=");
                if value.is_empty() {
                    eprintln!("--allow-origin attend une origine");
                    return EXIT_USAGE;
                }
                allow_origins.push(value.to_string());
                index += 1;
            }
            other => {
                eprintln!("option inconnue : {other}");
                return EXIT_USAGE;
            }
        }
    }

    let url = format!("http://127.0.0.1:{port}");
    println!("mpacer-music : interface locale sur {url}");
    if open_browser {
        server::open_browser(&url);
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("demarrage du serveur impossible : {error}");
            return EXIT_USAGE;
        }
    };
    let bootstrap = Bootstrap {
        adb,
        target_dir,
        library,
        allow_origins,
    };
    match runtime.block_on(server::serve(port, bootstrap)) {
        Ok(()) => EXIT_OK,
        Err(error) => {
            eprintln!("{error}");
            EXIT_USAGE
        }
    }
}

// ----------------------------------------------------------------- montres

fn devices_command(adb_option: Option<PathBuf>) -> i32 {
    let Some(adb) = resolve_adb_or_report(adb_option) else {
        return EXIT_ADB_MISSING;
    };
    match adb::devices(&adb, WATCH_MUSIC_DIR) {
        Ok(devices) if devices.is_empty() => {
            println!("aucune montre detectee par adb");
            EXIT_NO_DEVICE
        }
        Ok(devices) => {
            println!("adb : {}", adb.display());
            for device in devices {
                let model = device.model.unwrap_or_else(|| "?".to_string());
                match (device.free_bytes, device.total_bytes) {
                    (Some(free), Some(total)) => println!(
                        "{}  {}  {}  libre {} / {}",
                        device.serial,
                        model,
                        device.state,
                        human_bytes(free),
                        human_bytes(total)
                    ),
                    _ => println!(
                        "{}  {}  {}  espace libre inconnu",
                        device.serial, model, device.state
                    ),
                }
            }
            EXIT_OK
        }
        Err(error) => {
            eprintln!("{error}");
            EXIT_USAGE
        }
    }
}

fn resolve_adb_or_report(adb_option: Option<PathBuf>) -> Option<PathBuf> {
    let resolved = adb::resolve_adb(adb_option.as_deref());
    if resolved.is_none() {
        eprintln!(
            "adb introuvable : installer les platform-tools Android, definir ANDROID_HOME ou ANDROID_SDK_ROOT, ou passer --adb <chemin>"
        );
    }
    resolved
}

// ----------------------------------------------------------------- inspect

struct InspectArgs {
    manifest: String,
    folder: String,
    library: Option<PathBuf>,
}

fn parse_inspect_args(args: &[String]) -> Result<InspectArgs, String> {
    let mut manifest = None;
    let mut folder = None;
    let mut library = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                manifest = args.get(index + 1).cloned();
                index += 2;
            }
            "--folder" => {
                folder = args.get(index + 1).cloned();
                index += 2;
            }
            "--library" => {
                library = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            other => return Err(format!("option inconnue : {other}")),
        }
    }
    Ok(InspectArgs {
        manifest: manifest.ok_or("--manifest est obligatoire")?,
        folder: folder.ok_or("--folder est obligatoire")?,
        library,
    })
}

fn inspect_command(args: &[String]) -> i32 {
    let parsed = match parse_inspect_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return EXIT_USAGE;
        }
    };
    let manifest = match planner::load_manifest(Some(&parsed.manifest), None) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("{error}");
            return error.exit_code();
        }
    };
    let library = parsed
        .library
        .map(|root| LibraryStore::new(root).playlist_dir(&manifest.playlist_id));
    match planner::inspect_with_library(
        &manifest,
        Some(Path::new(&parsed.folder)),
        library.as_deref(),
    ) {
        Ok(inspection) => {
            print_inspection(&inspection);
            EXIT_OK
        }
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

// ---------------------------------------------------------------- transfert

struct TransferArgs {
    manifest: String,
    folder: String,
    library: Option<PathBuf>,
    serial: Option<String>,
    dry_run: bool,
    prune: bool,
    strict: bool,
    target_dir: Option<PathBuf>,
}

fn parse_transfer_args(args: &[String]) -> Result<TransferArgs, String> {
    let mut parsed = TransferArgs {
        manifest: String::new(),
        folder: String::new(),
        library: None,
        serial: None,
        dry_run: false,
        prune: false,
        strict: false,
        target_dir: None,
    };
    let mut manifest = None;
    let mut folder = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                manifest = args.get(index + 1).cloned();
                index += 2;
            }
            "--folder" => {
                folder = args.get(index + 1).cloned();
                index += 2;
            }
            "--serial" => {
                parsed.serial = args.get(index + 1).cloned();
                index += 2;
            }
            "--target-dir" => {
                parsed.target_dir = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            "--library" => {
                parsed.library = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            "--dry-run" => {
                parsed.dry_run = true;
                index += 1;
            }
            "--prune" => {
                parsed.prune = true;
                index += 1;
            }
            "--strict" => {
                parsed.strict = true;
                index += 1;
            }
            other => return Err(format!("option inconnue : {other}")),
        }
    }
    parsed.manifest = manifest.ok_or("--manifest est obligatoire")?;
    parsed.folder = folder.ok_or("--folder est obligatoire")?;
    Ok(parsed)
}

fn transfer_command(args: &[String], adb_option: Option<PathBuf>) -> i32 {
    let parsed = match parse_transfer_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return EXIT_USAGE;
        }
    };
    let manifest = match planner::load_manifest(Some(&parsed.manifest), None) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("{error}");
            return error.exit_code();
        }
    };

    let needs_adb = !parsed.dry_run && parsed.target_dir.is_none();
    let adb = if needs_adb {
        resolve_adb_or_report(adb_option)
    } else {
        None
    };
    if needs_adb && adb.is_none() {
        return EXIT_ADB_MISSING;
    }

    let library = parsed
        .library
        .map(|root| LibraryStore::new(root).playlist_dir(&manifest.playlist_id));
    let mut request = TransferRequest::new(manifest, PathBuf::from(&parsed.folder));
    request.library = library;
    request.serial = parsed.serial;
    request.prune = parsed.prune;
    request.dry_run = parsed.dry_run;
    request.strict = parsed.strict;
    request.target_dir = parsed.target_dir;
    request.adb = adb;

    match planner::transfer(&request, &ConsoleSink) {
        Ok(report) => {
            print_report(&report);
            EXIT_OK
        }
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

// -------------------------------------------------------------- affichage

/// Sink d'affichage de la CLI.
struct ConsoleSink;

impl ProgressSink for ConsoleSink {
    fn progress(&self, progress: &Progress) {
        if progress.total > 0 {
            println!(
                "[{}] {}/{} - {} envoyes",
                progress.step,
                progress.current,
                progress.total,
                human_bytes(progress.bytes_sent)
            );
        }
    }

    fn log(&self, message: &str) {
        println!("{message}");
    }
}

fn print_inspection(inspection: &planner::Inspection) {
    let playlist = &inspection.playlist;
    let target = playlist
        .target_bpm
        .map(|bpm| format!(" - BPM cible {bpm:.0}"))
        .unwrap_or_default();
    println!(
        "Playlist   : {} ({}) - source {} - {} piste(s){target}",
        playlist.name, playlist.id, playlist.source, playlist.track_count
    );
    println!(
        "Trouvees   : {}/{} - manquantes {} - fichiers ignores {}",
        inspection.matched_count(),
        playlist.track_count,
        inspection.missing_count(),
        inspection.unused_files.len()
    );
    println!("Octets     : {}", human_bytes(inspection.total_bytes));
    for entry in &inspection.matches {
        let file = entry
            .file
            .clone()
            .unwrap_or_else(|| "-- manquant --".to_string());
        println!(
            "{:>3}  {:<28}  {:<42}  score {:.2}{}",
            entry.position,
            truncate(&entry.title, 28),
            truncate(&file, 42),
            entry.score,
            bpm_suffix(entry.bpm)
        );
    }
    for unused in &inspection.unused_files {
        println!("ignore     : {}", unused.path);
    }
}

fn print_report(report: &planner::TransferReport) {
    println!("Playlist   : {} ({})", report.name, report.playlist_id);
    println!("Mode       : {}", report.mode);
    println!("Cible      : {}", report.destination);
    println!(
        "Titres     : {} apparies, {} manquants, {} fichiers ignores",
        report.matched, report.missing, report.unused_files
    );
    println!(
        "Copie      : {} fichier(s), {}",
        report.copied_files,
        human_bytes(report.bytes)
    );
    if report.pruned > 0 {
        println!(
            "Nettoyage  : {} fichier(s) obsoletes supprimes",
            report.pruned
        );
    }
    if report.dry_run {
        println!("Etat       : simulation (aucune ecriture)");
    }
}

fn bpm_suffix(bpm: Option<f64>) -> String {
    bpm.map(|value| format!("  {value:.0} bpm"))
        .unwrap_or_default()
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut shortened: String = text.chars().take(width.saturating_sub(1)).collect();
    shortened.push('.');
    shortened
}

/// Taille lisible (sans accent).
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["o", "Ko", "Mo", "Go", "To"];
    if bytes < 1024 {
        return format!("{bytes} o");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_and_bad_options_return_usage_codes() {
        assert_eq!(run(vec!["--help".to_string()]), EXIT_OK);
        assert_eq!(run(vec!["inconnue".to_string()]), EXIT_USAGE);
        assert_eq!(run(vec!["--port".to_string()]), EXIT_USAGE);
        assert_eq!(run(vec!["transfer".to_string()]), EXIT_USAGE);
        assert_eq!(run(vec!["--adb".to_string()]), EXIT_USAGE);
        assert_eq!(
            run(vec!["transfer".to_string(), "--bidon".to_string()]),
            EXIT_USAGE
        );
    }

    #[test]
    fn devices_with_a_missing_adb_returns_code_3() {
        assert_eq!(
            run(vec![
                "--adb".to_string(),
                "Z:/absent/adb.exe".to_string(),
                "devices".to_string()
            ]),
            EXIT_ADB_MISSING
        );
    }

    #[test]
    fn human_bytes_switches_units() {
        assert_eq!(human_bytes(512), "512 o");
        assert_eq!(human_bytes(2048), "2.0 Ko");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 Mo");
    }

    #[test]
    fn truncate_keeps_short_labels() {
        assert_eq!(truncate("court", 10), "court");
        assert_eq!(truncate("beaucoup trop long", 6).chars().count(), 6);
    }
}
