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

use mpacer_core::analysis::{acceleration, splits};
use mpacer_core::assistant::{AssistantConfig, AssistantMode};
use mpacer_core::cardio::{self, HeartRateZones, ZONE_NAMES};
use mpacer_core::engine::{EngineConfig, PacerEngine};
use mpacer_core::geo::Position;
use mpacer_core::gps::GpsSample;
use mpacer_core::music::{
    DirectiveReason, MusicConfig, MusicDirective, NowPlaying, Playlist, Track,
};
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
    /// Parcours suivi par le coureur simule (None : ligne droite).
    course: Option<&'static [(f64, f64)]>,
    /// Backend M-pacer ou envoyer la seance (optionnel).
    api_url: Option<String>,
    /// Jeton d'appareil deja obtenu (sinon appairage interactif).
    api_token: Option<String>,
    /// Simuler une frequence cardiaque (montre ou ceinture).
    heart_rate: bool,
    /// Frequence cardiaque maximale, pour le calcul des zones.
    hr_max: u16,
    /// Nombre de pauses inserees dans la seance.
    pauses: u32,
    /// Simuler une playlist de course : le directeur de musique est pilote et
    /// un lecteur factice joue les pistes demandees.
    music: bool,
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
            course: None,
            api_url: None,
            api_token: None,
            heart_rate: true,
            hr_max: 190,
            pauses: 2,
            music: false,
        }
    }
}

/// Playlist de demonstration : tempos etalonnes de 148 a 190 BPM, comme une
/// playlist de course reelle. Les identifiants sont stables pour que le
/// directeur choisisse toujours la meme piste (demonstration reproductible).
fn demo_playlist() -> Playlist {
    let titres: [(&str, &str, f64); 8] = [
        ("Echauffement", "M-pacer", 148.0),
        ("Mise en jambe", "M-pacer", 156.0),
        ("Allure course", "M-pacer", 164.0),
        ("Rythme 170", "M-pacer", 170.0),
        ("Tempo 176", "M-pacer", 176.0),
        ("Acceleration", "M-pacer", 182.0),
        ("Sprint final", "M-pacer", 190.0),
        ("Recuperation", "M-pacer", 120.0),
    ];
    Playlist {
        id: "demo-run-170".to_string(),
        name: "Demo course 170 BPM".to_string(),
        target_bpm: None,
        tracks: titres
            .iter()
            .enumerate()
            .map(|(index, (title, artist, bpm))| Track {
                id: format!("demo-{}", index + 1),
                title: (*title).to_string(),
                artist: Some((*artist).to_string()),
                duration_s: 210.0,
                bpm: Some(*bpm),
                position: index as u32,
            })
            .collect(),
    }
}

/// Simule une frequence cardiaque realiste : montee en regime, derive
/// progressive avec la distance, recuperation pendant les pauses.
fn simulate_heart_rate(current: &mut f64, distance_km: f64, paused: bool, rng: &mut Rng) -> u16 {
    let target = if paused {
        108.0
    } else {
        // La FC cible monte doucement avec la distance : c'est la derive cardiaque.
        148.0 + distance_km * 0.45
    };
    // Constante de temps ~20 s : ni saut instantane, ni lenteur irrealiste.
    *current += (target - *current) * 0.05 + rng.noise(0.9);
    *current = current.clamp(60.0, 200.0);
    current.round() as u16
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
  --course NOM      parcours suivi : line (defaut) ou bordeaux\n\
  --every S         periode d'affichage en secondes (defaut 60)\n\
  --gpx PATH        ecrit la trace GPX en fin de simulation\n\
  --seed N          graine du generateur de bruit\n\
  --no-hr           ne pas simuler de frequence cardiaque\n\
  --hr-max N        frequence cardiaque maximale pour les zones (defaut 190)\n\
  --pauses N        nombre de pauses inserees, 0 pour aucune (defaut 2)\n\
  --music           simule une playlist de course : tempo cible, Boost/Relax,\n\
                    changement de morceau et lecteur factice\n\
\n\
SYNCHRONISATION (optionnelle):\n\
  --api-url URL     envoie la seance au backend M-pacer (ex. https://mpacer.p.zacharie.org)\n\
  --api-token JETON jeton d'appareil ; sans lui, un appairage par code est propose\n\
\n\
  --help            affiche cette aide"
    );
}

/// Boucle inspiree des quais de Bordeaux (Quinconces, Chartrons, pont Chaban-Delmas,
/// rive droite, pont de pierre, place de la Bourse). Trace **synthetique** : ce n'est
/// pas le parcours officiel du Marathon de Bordeaux, mais il en suit la geographie.
const PARCOURS_BORDEAUX: &[(f64, f64)] = &[
    (44.8437, -0.5747), // place des Quinconces
    (44.8489, -0.5722), // quais des Chartrons (sud)
    (44.8555, -0.5680), // Chartrons
    (44.8590, -0.5620), // approche du pont Chaban-Delmas
    (44.8615, -0.5560), // pont Chaban-Delmas, rive droite
    (44.8645, -0.5540), // parc aux Angeliques
    (44.8580, -0.5480), // quais de la rive droite (nord)
    (44.8482, -0.5482), // quais de la rive droite (sud)
    (44.8410, -0.5560), // approche du pont de pierre
    (44.8375, -0.5655), // pont de pierre, rive gauche
    (44.8385, -0.5705), // quais sud
    (44.8412, -0.5695), // place de la Bourse
    (44.8437, -0.5747), // retour aux Quinconces
];

/// Distance approximative entre deux points (equirectangulaire, suffisant a l'echelle
/// d'une ville).
fn distance_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let lat_m = (b.0 - a.0) * 111_195.0;
    let lon_m = (b.1 - a.1) * 111_195.0 * a.0.to_radians().cos();
    (lat_m * lat_m + lon_m * lon_m).sqrt()
}

/// Parcours ferme : conversion d'une distance parcourue en position, avec bouclage.
struct Trace {
    points: &'static [(f64, f64)],
    cumul: Vec<f64>,
    total: f64,
}

impl Trace {
    fn new(points: &'static [(f64, f64)]) -> Self {
        let mut cumul = Vec::with_capacity(points.len());
        cumul.push(0.0);
        for index in 1..points.len() {
            let pas = distance_m(points[index - 1], points[index]);
            let precedent = cumul[index - 1];
            cumul.push(precedent + pas);
        }
        let total = *cumul.last().unwrap_or(&0.0);
        Self {
            points,
            cumul,
            total,
        }
    }

    /// Position a `distance_m` du depart ; le parcours recommence si on le depasse.
    fn position(&self, distance_m: f64) -> (f64, f64) {
        if self.total <= 0.0 || self.points.len() < 2 {
            return self.points.first().copied().unwrap_or((45.0, 0.0));
        }
        let d = distance_m.rem_euclid(self.total);
        let mut index = self.cumul.partition_point(|v| *v <= d).max(1) - 1;
        if index + 1 >= self.points.len() {
            index = self.points.len() - 2;
        }
        let debut = self.cumul[index];
        let longueur = self.cumul[index + 1] - debut;
        let ratio = if longueur > 0.0 {
            (d - debut) / longueur
        } else {
            0.0
        };
        let (lat1, lon1) = self.points[index];
        let (lat2, lon2) = self.points[index + 1];
        (lat1 + (lat2 - lat1) * ratio, lon1 + (lon2 - lon1) * ratio)
    }

    fn longueur_m(&self) -> f64 {
        self.total
    }
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
            "--course" => {
                options.course = match value()?.as_str() {
                    "bordeaux" => Some(PARCOURS_BORDEAUX),
                    "line" | "ligne" | "aucun" => None,
                    autre => return Err(format!("parcours inconnu : {autre} (bordeaux, line)")),
                }
            }
            "--every" => {
                options.display_every_s = value()?.parse().map_err(|_| "periode invalide")?
            }
            "--seed" => options.seed = value()?.parse().map_err(|_| "graine invalide")?,
            "--no-hr" => options.heart_rate = false,
            "--music" => options.music = true,
            "--hr-max" => options.hr_max = value()?.parse().map_err(|_| "FC max invalide")?,
            "--pauses" => {
                options.pauses = value()?.parse().map_err(|_| "nombre de pauses invalide")?
            }
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
        music: MusicConfig {
            enabled: options.music,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut engine = PacerEngine::new(config);
    // Playlist de demonstration : conservee ici pour que le lecteur factice
    // retrouve la piste demandee par le directeur de musique.
    let music_playlist = if options.music {
        Some(demo_playlist())
    } else {
        None
    };
    if let Some(playlist) = music_playlist.as_ref() {
        engine.set_music_playlist(Some(playlist.clone()));
    }
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
    let trace = options.course.map(Trace::new);
    if let Some(trace) = trace.as_ref() {
        println!(
            "Parcours : boucle de {:.2} km (trace synthetique inspiree de Bordeaux)",
            trace.longueur_m() / 1000.0
        );
    }
    let mut rng = Rng(options.seed);
    // Le point de depart doit etre celui du parcours : sinon le premier point GPS
    // saute de plusieurs centaines de kilometres et le moteur rejette la trace.
    let (mut lat, mut lon) = trace
        .as_ref()
        .map_or((45.0, 3.0), |trace| trace.position(0.0));
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

    // Pauses prevues : une a 25 %, 60 % puis 95 % de la distance.
    let pause_plan: Vec<(f64, f64)> = (0..options.pauses)
        .map(|index| {
            let fraction = 0.25 + 0.35 * index as f64;
            (options.distance_m * fraction, 40.0 - 10.0 * index as f64)
        })
        .collect();
    let mut next_pause = 0_usize;
    let mut remaining_pause_s = 0.0_f64;
    let mut heart_rate = 88.0_f64;
    // Lecteur factice : la montre ferait exactement cela (Play / SkipTo -> charger
    // la piste, sinon avancer la position, puis remonter l'instantane au moteur).
    let mut music_now: Option<NowPlaying> = None;

    for second in 0..total_samples {
        t_ms += 1000;

        // Entree en pause : le chrono de course s'arrete, le temps ecoule continue.
        if remaining_pause_s <= 0.0
            && next_pause < pause_plan.len()
            && travelled >= pause_plan[next_pause].0
        {
            remaining_pause_s = pause_plan[next_pause].1;
            next_pause += 1;
            let output = engine.pause(t_ms);
            if options.heart_rate {
                let bpm = simulate_heart_rate(&mut heart_rate, travelled / 1000.0, true, &mut rng);
                engine.on_heart_rate(t_ms, bpm);
            }
            print_output(&output, options.units, options.display_every_s, second + 1);
            continue;
        }

        let paused = remaining_pause_s > 0.0;
        if paused {
            remaining_pause_s -= 1.0;
            if remaining_pause_s <= 0.0 {
                engine.resume(t_ms);
            }
        }

        // Vitesse : celle du plan (shadow runner) si disponible, sinon 3.33 m/s.
        let target_speed = match plan {
            Some(plan) => 1000.0 / plan.pace_at_distance_s_per_km(travelled),
            None => 3.3333,
        };
        let step_m = if paused {
            0.0
        } else {
            target_speed + rng.noise(0.25)
        };
        // Sur un parcours declare, la position suit la trace ; sinon on avance vers le nord.
        if let Some(trace) = trace.as_ref() {
            let (p_lat, p_lon) = trace.position(travelled + step_m);
            lat = p_lat;
            lon = p_lon;
        } else if step_m > 0.0 {
            lat += step_m / 111_195.0;
        }
        // Bruit GPS : l'option --noise etait documentee mais inutilisee.
        let bruit_lat = rng.noise(options.noise_m) / 111_195.0;
        let bruit_lon =
            rng.noise(options.noise_m) / (111_195.0 * lat.to_radians().cos().abs().max(0.2));
        let position = Position::new(lat + bruit_lat, lon + bruit_lon);
        let accuracy = 5.0 + rng.noise(1.5).abs();
        if options.heart_rate {
            let bpm = simulate_heart_rate(&mut heart_rate, travelled / 1000.0, paused, &mut rng);
            engine.on_heart_rate(t_ms, bpm);
        }
        let output = engine.on_gps(GpsSample::new(t_ms, position, accuracy).with_speed(step_m));
        travelled += step_m;
        if let Some(playlist) = music_playlist.as_ref() {
            jouer_directive(playlist, &output, &mut music_now);
            engine.on_now_playing(music_now.clone());
        }
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
    println!(
        "Allure moyenne: {} {}",
        format_pace(summary.average_pace(options.units)),
        options.units.pace_label()
    );

    // Chrono : temps de course, temps ecoule et pauses.
    println!("\n--- Chrono ---");
    println!("Temps de course: {}", format_duration(summary.duration_s));
    println!(
        "Temps ecoule   : {}",
        format_duration(summary.total_elapsed_s())
    );
    println!(
        "Pauses         : {} pour {}",
        summary.pauses.len(),
        format_duration(summary.paused_s())
    );
    for pause in &summary.pauses {
        println!(
            "  pause a {} ({}), duree {}",
            format_duration(pause.at_s),
            options.units.format_distance(pause.at_distance_m),
            format_duration(pause.duration_s)
        );
    }

    // Temps de passage, avec le plan et le cardio quand ils existent.
    println!("\n--- Temps de passage ---");
    for split in splits(&summary) {
        let cardio = match (split.heart_rate_avg, split.heart_rate_max) {
            (Some(average), Some(max)) => format!(" fc {average:.0}/{max}"),
            _ => String::new(),
        };
        let plan = match split.plan_delta_s {
            Some(delta) => format!(" plan {delta:+.0} s"),
            None => String::new(),
        };
        println!(
            "  {:>2} : {:>7} en {:>6} a {:>5} {}{}{}",
            split.index,
            options.units.format_distance(split.distance_m),
            format_duration(split.duration_s),
            format_pace(Some(split.pace_s_per_km)),
            options.units.pace_label(),
            cardio,
            plan
        );
    }

    // Cardio : moyennes, zones et derive.
    if let Some(heart) = cardio::summarize(&summary.heart_rate, HeartRateZones::new(options.hr_max))
    {
        println!("\n--- Cardio ---");
        println!("FC moyenne     : {:.0} bpm", heart.average_bpm);
        println!("FC max / min   : {} / {} bpm", heart.max_bpm, heart.min_bpm);
        for (index, seconds) in heart.zone_seconds.iter().enumerate() {
            println!(
                "  {:>16} : {:>6} ({:>4.0} %)",
                ZONE_NAMES[index],
                format_duration(*seconds),
                heart.zone_percent(index)
            );
        }
        if let Some(drift) = cardio::cardiac_drift(&summary.track, &summary.heart_rate) {
            println!("Derive cardiaque: {:+.1} %", drift.decoupling_percent);
        }
    }

    // Plan de course : cible contre realise.
    if let Some(plan) = &summary.plan {
        println!("\n--- Plan de course ---");
        println!(
            "Cible          : {} sur {}",
            format_duration(plan.target_time_s),
            options.units.format_distance(plan.distance_m)
        );
        println!(
            "Realise        : {} (ecart {:+.0} s)",
            format_duration(summary.duration_s),
            summary.duration_s - plan.target_time_s
        );
    }

    // Acceleration : depart, reprises, repartition du temps.
    if let Some(acceleration) = acceleration(&summary) {
        println!("\n--- Acceleration ---");
        for phase in &acceleration.phases {
            let label = if phase.after_pause {
                "reprise"
            } else {
                "depart "
            };
            match phase.seconds {
                Some(seconds) => println!(
                    "  {} a {} : {} pour atteindre l'allure de croisiere",
                    label,
                    options.units.format_distance(phase.at_distance_m),
                    format_duration(seconds)
                ),
                None => println!(
                    "  {label} a {} : allure de croisiere non atteinte",
                    options.units.format_distance(phase.at_distance_m)
                ),
            }
        }
        println!(
            "  temps a accelerer {} / allure stable {} / ralentissement {}",
            format_duration(acceleration.accelerating_s),
            format_duration(acceleration.steady_s),
            format_duration(acceleration.decelerating_s)
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

/// Etiquette francaise d'une directive de musique.
fn directive_label(directive: MusicDirective) -> &'static str {
    match directive {
        MusicDirective::None => "aucune",
        MusicDirective::Play => "demarrage",
        MusicDirective::Keep => "stable",
        MusicDirective::Boost => "acceleration",
        MusicDirective::Relax => "ralentissement",
        MusicDirective::SkipTo => "changement de morceau",
        MusicDirective::Pause => "pause",
        MusicDirective::Resume => "reprise",
    }
}

/// Etiquette francaise de la raison d'une directive.
fn reason_label(reason: DirectiveReason) -> &'static str {
    match reason {
        DirectiveReason::Disabled => "musique coupee",
        DirectiveReason::NoPlaylist => "aucune playlist",
        DirectiveReason::Steady => "allure libre",
        DirectiveReason::OnPlan => "sur le plan",
        DirectiveReason::BehindPlan => "en retard sur le plan",
        DirectiveReason::AheadOfPlan => "en avance sur le plan",
        DirectiveReason::PaceSlow => "allure trop lente",
        DirectiveReason::PaceFast => "allure trop rapide",
        DirectiveReason::HeartRateHigh => "cardio haut",
        DirectiveReason::Paused => "seance en pause",
        DirectiveReason::Resumed => "reprise",
        DirectiveReason::TrackBpmMismatch => "tempo de la piste inadapte",
    }
}

/// Lecteur factice : applique la directive du moteur a la playlist.
///
/// Le moteur ne connait pas le lecteur audio ; c'est le shell (ici la
/// simulation) qui charge la piste demandee et lui renvoie la position.
fn jouer_directive(
    playlist: &Playlist,
    output: &mpacer_core::engine::EngineOutput,
    now: &mut Option<NowPlaying>,
) {
    let trouver = |id: &str| playlist.tracks.iter().find(|track| track.id == id).cloned();
    match output.music.directive {
        MusicDirective::Play | MusicDirective::SkipTo => {
            if let Some(track) = output.music.next_track_id.as_deref().and_then(trouver) {
                println!(
                    "    >> lecture : {} ({} bpm)",
                    track.title,
                    track.bpm.map_or("?".to_string(), |bpm| format!("{bpm:.0}"))
                );
                *now = Some(NowPlaying {
                    track_id: track.id.clone(),
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    bpm: track.bpm,
                    position_s: 0.0,
                });
            }
        }
        MusicDirective::Pause => {}
        MusicDirective::None => *now = None,
        _ => {
            if let Some(playing) = now.as_mut() {
                playing.position_s += 1.0;
            }
        }
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
    let cardio = match output.heart_rate_bpm {
        Some(bpm) => match output.heart_rate_zone {
            Some(zone) => format!(" | fc={bpm} Z{zone}"),
            None => format!(" | fc={bpm}"),
        },
        None => String::new(),
    };
    let musique = match output.music.directive {
        MusicDirective::None | MusicDirective::Pause => String::new(),
        directive => {
            let cible = output
                .music
                .target_bpm
                .map_or("?".to_string(), |bpm| format!("{bpm:.0}"));
            let cadence = output
                .music
                .cadence_spm
                .map_or(String::new(), |spm| format!(" cad={spm:.0}"));
            let titre = output
                .music
                .current
                .as_ref()
                .map_or(String::new(), |playing| format!(" - {}", playing.title));
            format!(
                " | musique {} bpm{} {} ({}){}",
                cible,
                cadence,
                directive_label(directive),
                reason_label(output.music.reason),
                titre
            )
        }
    };
    if second > 0 {
        println!(
            "[{}] t={:>6} ecoule={:>6} dist={:>7} allure={:>5} tour={:>5}{}{}{}",
            status,
            format_duration(output.elapsed_s),
            second,
            units.format_distance(output.distance_m),
            format_pace(output.current_pace),
            format_pace(output.current_lap_pace),
            cardio,
            panel,
            musique
        );
    }
    for message in &output.messages {
        println!("    🔊 {}", message.text);
    }
}
