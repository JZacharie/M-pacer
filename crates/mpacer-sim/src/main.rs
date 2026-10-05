//! # mpacer-sim
//!
//! Simulateur en ligne de commande : genere une trace GPS synthetique
//! (1 point par seconde) et la fait passer dans `PacerEngine`, exactement
//! comme le fera le service Android sur la montre.
//!
//! ```text
//! cargo run -p mpacer-sim -- --mode plan --minutes 45 --gpx trace.gpx
//! ```
//!
//! Utile pour regler les seuils (bruit GPS, detection de changement d'allure,
//! negative split) sans sortir courir, et pour verifier un scenario de course.

use mpacer_core::assistant::{AssistantConfig, AssistantMode};
use mpacer_core::engine::{EngineConfig, PacerEngine};
use mpacer_core::geo::Position;
use mpacer_core::gps::GpsSample;
use mpacer_core::pace::PaceConfig;
use mpacer_core::race_plan::NegativeSplit;
use mpacer_core::units::{format_duration, format_pace, UnitSystem};
use mpacer_core::voice::{Language, VoiceConfig, VoiceFrequency};
use mpacer_core::workout::WorkoutConfig;

/// Parametres de simulation.
#[derive(Debug, Clone)]
struct Options {
    minutes: f64,
    mode: AssistantMode,
    distance_m: f64,
    target_s: f64,
    split_ratio: f64,
    units: UnitSystem,
    noise_m: f64,
    display_every_s: u64,
    seed: u64,
    /// Backend M-pacer ou envoyer la seance (optionnel).
    api_url: Option<String>,
    /// Jeton d'appareil deja obtenu (sinon appairage interactif).
    api_token: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            minutes: 45.0,
            mode: AssistantMode::AchievePlannedTime,
            distance_m: 10_000.0,
            target_s: 3000.0,
            split_ratio: 0.03,
            units: UnitSystem::Metric,
            noise_m: 3.0,
            display_every_s: 60,
            seed: 0x5EED_1234,
            api_url: None,
            api_token: None,
        }
    }
}

/// Generateur pseudo-aleatoire xorshift (aucune dependance externe).
struct Rng(u64);

impl Rng {
    fn next_f64(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Bruit centre entre -amplitude et +amplitude.
    fn noise(&mut self, amplitude: f64) -> f64 {
        (self.next_f64() * 2.0 - 1.0) * amplitude
    }
}

fn print_usage() {
    println!(
        "mpacer-sim : simulateur de seance M-pacer\n\
\n\
USAGE:\n  mpacer-sim [OPTIONS]\n\
\n\
OPTIONS:\n\
  --minutes N       duree de la simulation (defaut 45)\n\
  --mode MODE       track | predict | plan | remote (defaut plan)\n\
  --distance M      distance de course en metres (defaut 10000)\n\
  --time S          temps cible en secondes (defaut 3000)\n\
  --split R         ratio de negative split, 0 pour allure reguliere (defaut 0.03)\n\
  --speed MPS       ignore : l'allure suit le plan (compatibilite)\n\
  --units U         metric | imperial (defaut metric)\n\
  --noise M         bruit GPS gaussien, en metres (defaut 3)\n\
  --every S         periode d'affichage en secondes (defaut 60)\n\
  --gpx PATH        ecrit la trace GPX en fin de simulation\n\
  --seed N          graine du generateur de bruit\n\
\n\
SYNCHRONISATION (optionnelle):\n\
  --api-url URL     envoie la seance au backend M-pacer (ex. https://mpacer.p.zacharie.org)\n\
  --api-token JETON jeton d'appareil ; sans lui, un appairage par code est propose\n\
\n\
  --help            affiche cette aide"
    );
}

fn parse_args() -> Result<(Options, Option<String>), String> {
    let mut options = Options::default();
    let mut gpx_path = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| format!("valeur manquante pour {arg}"))
        };
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--minutes" => options.minutes = value()?.parse().map_err(|_| "minutes invalides")?,
            "--distance" => {
                options.distance_m = value()?.parse().map_err(|_| "distance invalide")?
            }
            "--time" => options.target_s = value()?.parse().map_err(|_| "temps invalide")?,
            "--split" => options.split_ratio = value()?.parse().map_err(|_| "ratio invalide")?,
            "--noise" => options.noise_m = value()?.parse().map_err(|_| "bruit invalide")?,
            "--every" => {
                options.display_every_s = value()?.parse().map_err(|_| "periode invalide")?
            }
            "--seed" => options.seed = value()?.parse().map_err(|_| "graine invalide")?,
            "--api-url" => options.api_url = Some(value()?),
            "--api-token" => options.api_token = Some(value()?),
            "--speed" => {
                let _: f64 = value()?.parse().map_err(|_| "vitesse invalide")?;
            }
            "--units" => {
                options.units = match value()?.as_str() {
                    "metric" | "metrique" => UnitSystem::Metric,
                    "imperial" => UnitSystem::Imperial,
                    other => return Err(format!("unites inconnues : {other}")),
                }
            }
            "--mode" => {
                options.mode = match value()?.as_str() {
                    "track" | "pace" => AssistantMode::TrackPace,
                    "predict" => AssistantMode::PredictFinishTime,
                    "plan" | "shadow" => AssistantMode::AchievePlannedTime,
                    "remote" => AssistantMode::RemoteRace,
                    other => return Err(format!("mode inconnu : {other}")),
                }
            }
            "--gpx" => gpx_path = Some(value()?),
            other => return Err(format!("option inconnue : {other}")),
        }
    }
    Ok((options, gpx_path))
}

#[tokio::main]
async fn main() {
    let (options, gpx_path) = match parse_args() {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("erreur : {message}\n");
            print_usage();
            std::process::exit(2);
        }
    };

    let negative_split = if options.split_ratio > 0.0 {
        NegativeSplit::with_ratio(options.split_ratio)
    } else {
        NegativeSplit::even_pace()
    };

    let config = EngineConfig {
        units: options.units,
        pace: PaceConfig::default(),
        detect_pace_change: true,
        workout: WorkoutConfig {
            auto_pause: false,
            ..Default::default()
        },
        voice: VoiceConfig {
            language: Language::Fr,
            frequency: VoiceFrequency::Every5Minutes,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut engine = PacerEngine::new(config);
    engine.set_assistant_config(AssistantConfig {
        mode: options.mode,
        race_distance_m: Some(options.distance_m),
        planned_time_s: Some(options.target_s),
        negative_split,
    });

    println!(
        "M-pacer {} - simulation : {} min max, mode {:?}, cible {} m en {}, negative split {:.0} %\n",
        mpacer_core::VERSION,
        options.minutes,
        options.mode,
        options.distance_m,
        format_duration(options.target_s),
        options.split_ratio * 100.0
    );

    // Le plan sert a piloter la vitesse du coureur simule.
    let plan = engine.assistant().plan().copied();
    let mut rng = Rng(options.seed);
    let mut lat = 45.0;
    let mut lon = 3.0;
    // Horodatage realiste : la trace GPX produite est directement exploitable.
    let started_at_ms: i64 = 1_700_000_000_000;
    let mut t_ms: i64 = started_at_ms;

    // Phase d'acquisition GPS (3 points precis).
    for _ in 0..3 {
        let output = engine.on_gps(GpsSample::new(t_ms, Position::new(lat, lon), 4.0));
        lat += 2.0 / 111_195.0;
        t_ms += 1000;
        let _ = output;
    }
    engine.start(t_ms);
    print_output(
        &engine.tick(t_ms),
        options.units,
        options.display_every_s,
        0,
    );

    let total_samples = (options.minutes * 60.0) as u64;
    let mut travelled = 0.0_f64;
    for second in 0..total_samples {
        t_ms += 1000;
        // Vitesse : celle du plan (shadow runner) si disponible, sinon 3.33 m/s.
        let speed = match plan {
            Some(plan) => 1000.0 / plan.pace_at_distance_s_per_km(travelled),
            None => 3.3333,
        };
        let step_m = speed + rng.noise(0.25);
        let heading_lat = step_m / 111_195.0;
        let heading_lon = rng.noise(0.0015) / 111_195.0;
        lat += heading_lat;
        lon += heading_lon;
        let position = Position::new(lat, lon);
        let accuracy = 5.0 + rng.noise(1.5).abs();
        let output = engine.on_gps(GpsSample::new(t_ms, position, accuracy).with_speed(step_m));
        travelled += step_m;
        let elapsed = second + 1;
        if elapsed % options.display_every_s == 0 || !output.messages.is_empty() {
            print_output(&output, options.units, options.display_every_s, elapsed);
        }
        if travelled >= options.distance_m {
            break;
        }
    }

    let summary = engine.stop(t_ms + 1000);
    print_output(&summary, options.units, options.display_every_s, 0);
    let summary = engine.summary(started_at_ms);

    println!("\n--- Resume de la seance ---");
    println!(
        "Distance      : {}",
        options.units.format_distance(summary.distance_m)
    );
    println!("Duree         : {}", format_duration(summary.duration_s));
    println!(
        "Allure moyenne: {} {}",
        format_pace(summary.average_pace(options.units)),
        options.units.pace_label()
    );
    println!("Tours         : {}", summary.laps.len());
    for lap in &summary.laps {
        println!(
            "  tour {:>2} : {} en {}",
            lap.index,
            options.units.format_distance(lap.distance_m),
            format_duration(lap.duration_s)
        );
    }
    if summary.best_efforts.is_empty() {
        println!("Meilleurs temps: aucun (distance trop courte)");
    } else {
        println!("Meilleurs temps:");
        for effort in &summary.best_efforts {
            println!(
                "  {:>14} : {}",
                effort.label,
                format_duration(effort.time_s)
            );
        }
    }

    if let Some(path) = gpx_path {
        match std::fs::write(&path, mpacer_core::gpx::export_gpx(&summary)) {
            Ok(()) => println!("\nTrace GPX ecrite : {path}"),
            Err(error) => eprintln!("\nerreur d'ecriture GPX : {error}"),
        }
    }

    if let Some(api_url) = options.api_url.clone() {
        synchronise(&api_url, options.api_token.as_deref(), &summary).await;
    }
}

/// Envoie la seance au backend M-pacer.
///
/// Sans jeton, un appairage est propose : un code s'affiche, l'utilisateur
/// l'approuve dans l'interface web, puis le jeton est utilise pour l'envoi.
async fn synchronise(
    api_url: &str,
    token: Option<&str>,
    summary: &mpacer_core::history::WorkoutSummary,
) {
    use mpacer_client::MpacerClient;

    println!("\n--- Synchronisation vers {api_url} ---");
    let client = match MpacerClient::new(api_url) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("URL invalide : {error}");
            return;
        }
    };

    let token = match token {
        Some(token) => token.to_string(),
        None => {
            let code = match client.request_device_code("mpacer-sim").await {
                Ok(code) => code,
                Err(error) => {
                    eprintln!("appairage impossible : {error}");
                    return;
                }
            };
            println!(
                "Ouvrez {} et saisissez le code {}",
                code.verification_uri, code.user_code
            );
            match client.wait_for_token(&code.device_code).await {
                Ok(token) => {
                    println!("Montre appairee.");
                    token.access_token
                }
                Err(error) => {
                    eprintln!("appairage interrompu : {error}");
                    return;
                }
            }
        }
    };

    let client = client.with_token(token);
    match client.upload_workout(summary).await {
        Ok(receipt) => println!(
            "Seance synchronisee : id {} ({})",
            receipt.id,
            if receipt.replaced {
                "version precedente remplacee"
            } else {
                "nouvelle seance"
            }
        ),
        Err(error) => eprintln!("envoi refuse : {error}"),
    }
}

fn print_output(
    output: &mpacer_core::engine::EngineOutput,
    units: UnitSystem,
    _every: u64,
    second: u64,
) {
    let status = match output.light {
        mpacer_core::gps::StatusLight::Green => "VERT ",
        mpacer_core::gps::StatusLight::Yellow => "JAUNE",
        mpacer_core::gps::StatusLight::Orange => "ORANGE",
        mpacer_core::gps::StatusLight::Red => "ROUGE",
    };
    let panel = output
        .panel
        .shadow
        .map(|shadow| {
            format!(
                " | ecart plan {}{}",
                if shadow.ahead { "+" } else { "-" },
                units.format_distance_short(shadow.distance_delta_m.abs())
            )
        })
        .unwrap_or_default();
    if second > 0 {
        println!(
            "[{}] t={:>6} dist={:>7} allure={:>5} tour={:>5}{}",
            status,
            format_duration(output.elapsed_s),
            units.format_distance(output.distance_m),
            format_pace(output.current_pace),
            format_pace(output.current_lap_pace),
            panel
        );
    }
    for message in &output.messages {
        println!("    🔊 {}", message.text);
    }
}
