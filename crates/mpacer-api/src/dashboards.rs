//! Tableaux de bord : catalogue des widgets et rendu des ecrans composes.
//!
//! Un tableau de bord est une **liste ordonnee de widgets** choisie par
//! l'utilisateur. Ce module porte le catalogue (cles stables, libelles,
//! gabarits prets a l'emploi) et le rendu : les routes web ne font que charger
//! les donnees puis appeler [`render_widgets`].
//!
//! Les donnees sont chargees **selon les widgets presents** : un tableau qui
//! n'affiche que l'historique ne relit pas la trace GPS de la derniere seance.

use crate::db;
use crate::error::AppResult;
use crate::models::{DashboardWidget, Race, User, WorkoutRow};
use crate::routes::web::{format_date, mini_card, polyline_points, weekly_chart};
use crate::state::AppState;
use maud::{html, Markup};
use mpacer_core::history::WorkoutSummary;
use mpacer_core::units::{format_duration, format_pace, UnitSystem};

// ------------------------------------------------------------------ catalogue

/// Un widget disponible dans le catalogue.
///
/// La cle (`key`) est persistee en base : elle ne doit jamais changer, sous
/// peine de faire disparaitre les widgets deja enregistres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
    /// Allure de la derniere seance : dernier tour, tour precedent, moyenne.
    Allure,
    /// Resume : distance, temps, allure, vitesse et trace GPS.
    Resume,
    /// Trace GPS seule.
    Carte,
    /// Temps de passage : graphique des allures et tableau des tours.
    Tours,
    /// Meilleurs temps sur les distances de reference (1 km, 5 km, 10 km...).
    MeilleuresDistances,
    /// Frequence cardiaque : moyenne, max et temps par zone.
    Cardio,
    /// Dernieres seances.
    Historique,
    /// Volume des 30 derniers jours et 12 dernieres semaines.
    Statistiques,
    /// Prochaines courses et dernieres courses courues.
    Courses,
}

impl WidgetKind {
    /// Tous les widgets du catalogue, dans l'ordre de presentation.
    pub const ALL: [WidgetKind; 9] = [
        WidgetKind::Allure,
        WidgetKind::Resume,
        WidgetKind::Carte,
        WidgetKind::Tours,
        WidgetKind::MeilleuresDistances,
        WidgetKind::Cardio,
        WidgetKind::Historique,
        WidgetKind::Statistiques,
        WidgetKind::Courses,
    ];

    /// Cle persistee en base.
    pub fn key(self) -> &'static str {
        match self {
            WidgetKind::Allure => "allure",
            WidgetKind::Resume => "resume",
            WidgetKind::Carte => "carte",
            WidgetKind::Tours => "tours",
            WidgetKind::MeilleuresDistances => "meilleures-distances",
            WidgetKind::Cardio => "cardio",
            WidgetKind::Historique => "historique",
            WidgetKind::Statistiques => "statistiques",
            WidgetKind::Courses => "courses",
        }
    }

    /// Widget correspondant a une cle enregistree, si elle est connue.
    pub fn from_key(key: &str) -> Option<WidgetKind> {
        WidgetKind::ALL.into_iter().find(|kind| kind.key() == key)
    }

    pub fn label(self) -> &'static str {
        match self {
            WidgetKind::Allure => "Allure",
            WidgetKind::Resume => "Resume de seance",
            WidgetKind::Carte => "Carte GPS",
            WidgetKind::Tours => "Tours",
            WidgetKind::MeilleuresDistances => "Meilleures distances",
            WidgetKind::Cardio => "Cardio",
            WidgetKind::Historique => "Historique",
            WidgetKind::Statistiques => "Statistiques",
            WidgetKind::Courses => "Courses",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            WidgetKind::Allure => "Allure du dernier tour, du precedent et allure moyenne.",
            WidgetKind::Resume => "Distance, temps, allure, vitesse et trace GPS.",
            WidgetKind::Carte => "Le trace GPS de la derniere seance, sans les chiffres.",
            WidgetKind::Tours => "Graphique des allures et tableau des temps de passage.",
            WidgetKind::MeilleuresDistances => "Meilleurs temps sur 1 km, 5 km, 10 km...",
            WidgetKind::Cardio => "FC moyenne et max, temps passe dans chaque zone.",
            WidgetKind::Historique => "Vos dernieres seances, pretes a ouvrir.",
            WidgetKind::Statistiques => "Volume des 30 derniers jours et 12 semaines.",
            WidgetKind::Courses => "Prochaines courses et dernieres courses courues.",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            WidgetKind::Allure => "icon-activity",
            WidgetKind::Resume => "icon-watch",
            WidgetKind::Carte => "icon-route",
            WidgetKind::Tours => "icon-clock",
            WidgetKind::MeilleuresDistances => "icon-check",
            WidgetKind::Cardio => "icon-heart",
            WidgetKind::Historique => "icon-list",
            WidgetKind::Statistiques => "icon-stats",
            WidgetKind::Courses => "icon-flag",
        }
    }

    /// Le widget s'appuie sur la derniere seance enregistree.
    fn uses_workout(self) -> bool {
        matches!(
            self,
            WidgetKind::Allure
                | WidgetKind::Resume
                | WidgetKind::Carte
                | WidgetKind::Tours
                | WidgetKind::MeilleuresDistances
                | WidgetKind::Cardio
        )
    }

    fn uses_history(self) -> bool {
        self == WidgetKind::Historique
    }

    fn uses_stats(self) -> bool {
        self == WidgetKind::Statistiques
    }

    fn uses_races(self) -> bool {
        self == WidgetKind::Courses
    }
}

/// Modele de tableau propose a la creation.
pub struct DashboardTemplate {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub widgets: &'static [WidgetKind],
}

/// Gabarits prets a l'emploi : ils reprennent les ecrans de reference du
/// controle d'allure (course, analyse, historique).
pub const TEMPLATES: [DashboardTemplate; 3] = [
    DashboardTemplate {
        key: "pace-control",
        name: "Pace Control",
        description: "Allure, tours et meilleures distances : l'ecran de course.",
        widgets: &[
            WidgetKind::Allure,
            WidgetKind::Tours,
            WidgetKind::MeilleuresDistances,
        ],
    },
    DashboardTemplate {
        key: "analyse",
        name: "Analyse de seance",
        description: "Resume, carte, tours, cardio et meilleures distances.",
        widgets: &[
            WidgetKind::Resume,
            WidgetKind::Carte,
            WidgetKind::Tours,
            WidgetKind::Cardio,
            WidgetKind::MeilleuresDistances,
        ],
    },
    DashboardTemplate {
        key: "historique",
        name: "Historique",
        description: "Volume, seances recentes et prochaines courses.",
        widgets: &[
            WidgetKind::Statistiques,
            WidgetKind::Historique,
            WidgetKind::Courses,
        ],
    },
];

/// Gabarit correspondant a une cle, si elle est connue.
pub fn template(key: &str) -> Option<&'static DashboardTemplate> {
    TEMPLATES.iter().find(|template| template.key == key)
}

/// Nettoie une liste de cles : cles inconnues ignorees, doublons retires en
/// conservant la premiere place. L'ordre du formulaire est l'ordre d'affichage.
pub fn normalize(keys: &[String]) -> Vec<WidgetKind> {
    let mut kinds: Vec<WidgetKind> = Vec::new();
    for key in keys {
        let Some(kind) = WidgetKind::from_key(key.trim()) else {
            continue;
        };
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    kinds
}

/// Cles persistables d'une liste de widgets.
pub fn keys_of(kinds: &[WidgetKind]) -> Vec<String> {
    kinds.iter().map(|kind| kind.key().to_string()).collect()
}

// -------------------------------------------------------------------- donnees

/// Donnees partagees par tous les widgets d'un tableau, chargees une seule fois.
#[derive(Default)]
pub struct DashboardData {
    /// Derniere seance enregistree.
    pub workout: Option<WorkoutRow>,
    /// Son resume complet (trace, tours, cardio), si le payload est lisible.
    pub summary: Option<WorkoutSummary>,
    /// Seances recentes, pour le widget Historique.
    pub history: Vec<WorkoutRow>,
    /// Statistiques 30 jours, pour le widget Statistiques.
    pub stats: Option<db::Stats>,
    /// Volume hebdomadaire, pour le widget Statistiques.
    pub weekly: Vec<db::WeekTotal>,
    /// Prochaines courses.
    pub upcoming: Vec<Race>,
    /// Dernieres courses courues.
    pub past: Vec<Race>,
}

impl DashboardData {
    /// Systeme d'unites de la seance affichee (metrique par defaut).
    pub fn units(&self) -> UnitSystem {
        self.summary
            .as_ref()
            .map(|summary| summary.unit_system)
            .unwrap_or(UnitSystem::Metric)
    }
}

/// Charge les donnees utiles aux widgets demandes.
pub async fn load(state: &AppState, user: &User, kinds: &[WidgetKind]) -> AppResult<DashboardData> {
    let mut data = DashboardData::default();
    let now_ms = state.now_ms();

    if kinds.iter().any(|kind| kind.uses_workout()) {
        let filter = db::WorkoutFilter {
            limit: 1,
            ..Default::default()
        };
        if let Some(row) = db::list_workouts(&state.pool, &user.id, &filter)
            .await?
            .into_iter()
            .next()
        {
            if let Some((workout, payload)) =
                db::get_workout(&state.pool, &user.id, &row.id).await?
            {
                data.summary = serde_json::from_str(&payload).ok();
                data.workout = Some(workout);
            }
        }
    }

    if kinds.iter().any(|kind| kind.uses_history()) {
        let filter = db::WorkoutFilter {
            limit: 12,
            ..Default::default()
        };
        data.history = db::list_workouts(&state.pool, &user.id, &filter).await?;
    }

    if kinds.iter().any(|kind| kind.uses_stats()) {
        data.stats = Some(db::stats(&state.pool, &user.id, 30, now_ms).await?);
        data.weekly =
            db::weekly_totals(&state.pool, &user.id, now_ms - 12 * 7 * 24 * 3600 * 1000).await?;
    }

    if kinds.iter().any(|kind| kind.uses_races()) {
        data.upcoming = db::list_upcoming_races(&state.pool, &user.id, now_ms).await?;
        data.past = db::list_past_races(&state.pool, &user.id, now_ms).await?;
    }

    Ok(data)
}

// --------------------------------------------------------------------- rendu

/// Rendu complet d'un tableau : une section par widget, dans l'ordre enregistre.
pub fn render_widgets(widgets: &[DashboardWidget], data: &DashboardData) -> Markup {
    let known: Vec<(&DashboardWidget, WidgetKind)> = widgets
        .iter()
        .filter_map(|widget| WidgetKind::from_key(&widget.kind).map(|kind| (widget, kind)))
        .collect();

    html! {
        @if known.is_empty() {
            div class="empty" {
                span class="icon icon-grid" {}
                p { "Ce tableau de bord est vide." }
                p class="tiny" { "Ajoutez un widget : allure, carte, tours, cardio, historique..." }
            }
        } @else {
            div class="dash-grid" {
                @for (widget, kind) in &known {
                    section class="widget" id=(format!("widget-{}", widget.id)) {
                        div class="widget-head" {
                            span class={ "icon " (kind.icon()) } {}
                            h2 { (kind.label()) }
                        }
                        (render_widget(*kind, data))
                    }
                }
            }
        }
    }
}

/// Contenu d'un widget. Les donnees absentes donnent un message, jamais un
/// chiffre invente.
pub fn render_widget(kind: WidgetKind, data: &DashboardData) -> Markup {
    match kind {
        WidgetKind::Allure => widget_allure(data),
        WidgetKind::Resume => widget_resume(data),
        WidgetKind::Carte => widget_map(data),
        WidgetKind::Tours => widget_laps(data),
        WidgetKind::MeilleuresDistances => widget_best_distances(data),
        WidgetKind::Cardio => widget_cardio(data),
        WidgetKind::Historique => widget_history(data),
        WidgetKind::Statistiques => widget_stats(data),
        WidgetKind::Courses => widget_races(data),
    }
}

/// Message commun quand aucune seance n'est encore synchronisee.
fn no_workout() -> Markup {
    html! {
        p class="muted" { "Aucune seance enregistree pour l'instant." }
        p class="tiny" { "Synchronisez la montre : les widgets se remplissent aussitot." }
    }
}

fn widget_allure(data: &DashboardData) -> Markup {
    let Some(workout) = &data.workout else {
        return no_workout();
    };
    let units = data.units();
    let splits = data
        .summary
        .as_ref()
        .map(mpacer_core::analysis::splits)
        .unwrap_or_default();
    let last = splits.last();
    let previous = splits
        .len()
        .checked_sub(2)
        .and_then(|index| splits.get(index));

    let current_pace = last.map(|split| split.pace_s_per_km);
    let current_label = match last {
        Some(split) if split.distance_m >= 1500.0 => "Dernier tour",
        Some(_) => "Allure",
        None => "Allure moyenne",
    };

    html! {
        div class="pace-hero" {
            span class="card-label" { (current_label) }
            strong class="pace-value" {
                (format_pace(current_pace.or(Some(workout.average_pace_s_per_km))))
            }
            span class="pace-unit" { " /" (units.label()) }
        }
        div class="mini-cards" {
            (mini_card(
                "Tour precedent",
                &previous
                    .map(|split| format!("{} /{}", format_pace(Some(split.pace_s_per_km)), units.label()))
                    .unwrap_or_else(|| "-".to_string()),
            ))
            (mini_card(
                "Allure moyenne",
                &format!("{} /{}", format_pace(Some(workout.average_pace_s_per_km)), units.label()),
            ))
            (mini_card("Distance", &crate::routes::web::format_distance(workout.distance_m, units)))
            (mini_card("Duree", &format_duration(workout.duration_s)))
        }
    }
}

fn widget_resume(data: &DashboardData) -> Markup {
    let Some(workout) = &data.workout else {
        return no_workout();
    };
    let units = data.units();
    let elapsed = data
        .summary
        .as_ref()
        .map(|summary| summary.total_elapsed_s())
        .unwrap_or(workout.duration_s);
    let speed = data
        .summary
        .as_ref()
        .and_then(|summary| summary.average_speed_mps())
        .or_else(|| (workout.duration_s > 0.0).then(|| workout.distance_m / workout.duration_s));

    html! {
        div class="mini-cards" {
            (mini_card("Distance", &crate::routes::web::format_distance(workout.distance_m, units)))
            (mini_card("Temps ecoule", &format_duration(elapsed)))
            (mini_card("Allure moyenne", &format!("{} /{}", format_pace(Some(workout.average_pace_s_per_km)), units.label())))
            @if let Some(speed) = speed {
                (mini_card("Vitesse moyenne", &format!("{:.2} km/h", speed * 3.6)))
            }
        }
        p class="tiny" { "Seance du " (format_date(workout.started_at_ms)) }
    }
}

fn widget_map(data: &DashboardData) -> Markup {
    let Some(summary) = &data.summary else {
        return no_workout();
    };
    map_svg(&summary.track)
}

/// Trace GPS projetee dans un cadre SVG (equirectangulaire locale, suffisante
/// a l'echelle d'une seance).
fn map_svg(track: &[mpacer_core::best_distances::TrackPoint]) -> Markup {
    let points: Vec<(f64, f64)> = track
        .iter()
        .filter(|point| {
            point.lat.is_finite()
                && point.lon.is_finite()
                && !(point.lat == 0.0 && point.lon == 0.0)
        })
        .map(|point| (point.lon, point.lat))
        .collect();
    if points.len() < 2 {
        return html! { p class="muted" { "Cette seance n'a pas de trace GPS." } };
    }

    let min_lon = points
        .iter()
        .map(|(lon, _)| *lon)
        .fold(f64::INFINITY, f64::min);
    let max_lon = points
        .iter()
        .map(|(lon, _)| *lon)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_lat = points
        .iter()
        .map(|(_, lat)| *lat)
        .fold(f64::INFINITY, f64::min);
    let max_lat = points
        .iter()
        .map(|(_, lat)| *lat)
        .fold(f64::NEG_INFINITY, f64::max);

    let width = 340.0_f64;
    let height = 220.0_f64;
    let pad = 10.0_f64;
    let span_lon = (max_lon - min_lon).max(1e-6);
    let span_lat = (max_lat - min_lat).max(1e-6);
    let scale = ((width - 2.0 * pad) / span_lon).min((height - 2.0 * pad) / span_lat);
    let offset_x = pad + ((width - 2.0 * pad) - span_lon * scale) / 2.0;
    let offset_y = pad + ((height - 2.0 * pad) - span_lat * scale) / 2.0;

    let step = (points.len() / 400).max(1);
    let projected: Vec<(f64, f64)> = points
        .iter()
        .step_by(step)
        .map(|(lon, lat)| {
            (
                offset_x + (lon - min_lon) * scale,
                height - offset_y - (lat - min_lat) * scale,
            )
        })
        .collect();

    html! {
        svg class="chart map" viewBox=(format!("0 0 {width} {height}")) preserveAspectRatio="xMidYMid meet" {
            rect class="band" x="0" y="0" width=(format!("{width}")) height=(format!("{height}")) rx="12" {}
            polyline class="trace pace" points=(polyline_points(&projected)) {}
        }
    }
}

fn widget_laps(data: &DashboardData) -> Markup {
    let Some(summary) = &data.summary else {
        return no_workout();
    };
    let units = data.units();
    let splits = mpacer_core::analysis::splits(summary);
    if splits.is_empty() {
        return html! { p class="muted" { "Aucun tour enregistre." } };
    }
    html! {
        (laps_chart(&splits, units))
        div class="table-wrap" {
            table {
                thead {
                    tr {
                        th { "Tour" }
                        th { "Distance" }
                        th { "Temps" }
                        th { "Allure" }
                    }
                }
                tbody {
                    @for split in &splits {
                        tr {
                            td { (split.index) }
                            td { (crate::routes::web::format_distance(split.distance_m, units)) }
                            td { (format_duration(split.duration_s)) }
                            td { (format_pace(Some(split.pace_s_per_km))) " /" (units.label()) }
                        }
                    }
                }
            }
        }
    }
}

/// Histogramme des allures par tour : une barre plus haute = un tour plus rapide.
fn laps_chart(splits: &[mpacer_core::analysis::Split], units: UnitSystem) -> Markup {
    let paces: Vec<f64> = splits
        .iter()
        .map(|split| split.pace_s_per_km)
        .filter(|pace| pace.is_finite() && *pace > 0.0)
        .collect();
    if paces.is_empty() {
        return html! {};
    }
    let slowest = paces.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let fastest = paces.iter().cloned().fold(f64::INFINITY, f64::min);
    let width = (splits.len() as f64 * 46.0).max(240.0);
    let height = 150.0_f64;
    let slot = width / splits.len() as f64;

    html! {
        svg class="chart bars laps" viewBox=(format!("0 0 {width} {height}")) preserveAspectRatio="none" {
            @for (index, split) in splits.iter().enumerate() {
                @let pace = split.pace_s_per_km;
                @let ratio = if slowest > fastest {
                    ((slowest - pace) / (slowest - fastest)).clamp(0.0, 1.0)
                } else {
                    0.5
                };
                @let bar = 14.0 + ratio * (height - 46.0);
                rect class="bar-pace"
                    x=(format!("{:.1}", index as f64 * slot + slot * 0.2))
                    y=(format!("{:.1}", height - bar - 18.0))
                    width=(format!("{:.1}", slot * 0.6))
                    height=(format!("{bar:.1}"))
                    rx="3" {
                    title { "Tour " (split.index) " : " (format_pace(Some(pace))) " /" (units.label()) }
                }
                text class="chart-label"
                    x=(format!("{:.1}", index as f64 * slot + slot / 2.0))
                    y=(format!("{:.1}", height - 4.0))
                    text-anchor="middle" {
                    (split.index)
                }
            }
            text class="chart-label" x="4" y="13" { (format_pace(Some(fastest))) " /" (units.label()) }
            text class="chart-label" x="4" y=(format!("{:.0}", height - 22.0)) { (format_pace(Some(slowest))) }
        }
    }
}

fn widget_best_distances(data: &DashboardData) -> Markup {
    let Some(summary) = &data.summary else {
        return no_workout();
    };
    let units = data.units();
    if summary.best_efforts.is_empty() {
        return html! { p class="muted" { "Pas assez de distance pour un meilleur temps." } };
    }
    html! {
        div class="table-wrap" {
            table {
                thead {
                    tr {
                        th { "Distance" }
                        th { "De - a" }
                        th { "Temps" }
                        th { "Allure" }
                    }
                }
                tbody {
                    @for effort in &summary.best_efforts {
                        @let pace = if effort.time_s > 0.0 {
                            Some(effort.time_s / (effort.distance_m / 1000.0))
                        } else {
                            None
                        };
                        tr {
                            td { (effort.label) }
                            td class="tiny" {
                                (crate::routes::web::format_distance(effort.start_dist_m, units))
                                " - "
                                (crate::routes::web::format_distance(effort.start_dist_m + effort.distance_m, units))
                            }
                            td { (format_duration(effort.time_s)) }
                            td { (format_pace(pace)) " /" (units.label()) }
                        }
                    }
                }
            }
        }
    }
}

fn widget_cardio(data: &DashboardData) -> Markup {
    let Some(summary) = &data.summary else {
        return no_workout();
    };
    let Some(heart) = mpacer_core::cardio::summarize(
        &summary.heart_rate,
        mpacer_core::cardio::HeartRateZones::default(),
    ) else {
        return html! { p class="muted" { "Cette seance n'a pas de frequence cardiaque." } };
    };
    html! {
        div class="mini-cards" {
            (mini_card("FC moyenne", &format!("{:.0} bpm", heart.average_bpm)))
            (mini_card("FC max", &format!("{} bpm", heart.max_bpm)))
            (mini_card("FC min", &format!("{} bpm", heart.min_bpm)))
            (mini_card("Mesures", &heart.sample_count.to_string()))
        }
        div class="zone-bars" {
            @for (index, name) in mpacer_core::cardio::ZONE_NAMES.iter().enumerate() {
                @let percent = heart.zone_percent(index);
                div class="zone-row" {
                    span class="zone-name" { (name) }
                    div class="zone-track" {
                        div class="zone-fill" style=(format!("width:{:.1}%", percent.clamp(0.0, 100.0))) {}
                    }
                    span class="zone-value" { (format!("{percent:.0} %")) }
                }
            }
        }
    }
}

fn widget_history(data: &DashboardData) -> Markup {
    if data.history.is_empty() {
        return html! {
            p class="muted" { "Aucune seance enregistree pour l'instant." }
        };
    }
    html! {
        div class="list" {
            @for workout in &data.history {
                a class="row" href={ "/workouts/" (workout.id) } {
                    div class="row-main" {
                        div class="row-title" { (format_date(workout.started_at_ms)) }
                        div class="row-sub" {
                            (format_duration(workout.duration_s)) " - "
                            (format_pace(Some(workout.average_pace_s_per_km))) " /"
                            (crate::routes::web::units_of(&workout.unit_system).label())
                        }
                    }
                    span class="row-value" {
                        (crate::routes::web::format_distance(
                            workout.distance_m,
                            crate::routes::web::units_of(&workout.unit_system),
                        ))
                    }
                    span class="icon icon-chevron chev" {}
                }
            }
        }
    }
}

fn widget_stats(data: &DashboardData) -> Markup {
    let Some(stats) = &data.stats else {
        return html! { p class="muted" { "Pas encore de statistiques." } };
    };
    html! {
        div class="mini-cards" {
            (mini_card("30 jours", &format!("{} seances", stats.workout_count)))
            (mini_card("Distance", &crate::routes::web::format_distance(stats.total_distance_m, UnitSystem::Metric)))
            (mini_card("Duree", &format_duration(stats.total_duration_s)))
            (mini_card("Allure moyenne", &format!("{} /km", format_pace(stats.average_pace_s_per_km))))
        }
        @if data.weekly.is_empty() {
            p class="muted" { "Pas encore de volume sur 12 semaines." }
        } @else {
            (weekly_chart(&data.weekly))
        }
    }
}

fn widget_races(data: &DashboardData) -> Markup {
    if data.upcoming.is_empty() && data.past.is_empty() {
        return html! {
            p class="muted" { "Aucune course enregistree." }
            p class="tiny" { "Ajoutez une course depuis la page Courses." }
        };
    }
    html! {
        @if !data.upcoming.is_empty() {
            h3 { "A venir" }
            div class="list" {
                @for race in data.upcoming.iter().take(5) {
                    a class="row" href={ "/courses/" (race.id) } {
                        div class="row-main" {
                            div class="row-title" { (race.name) }
                            div class="row-sub" {
                                (race.start_at_ms.map(format_date).unwrap_or_else(|| "date inconnue".to_string()))
                                @if let Some(distance) = race.distance_m {
                                    " - " (crate::routes::web::format_distance(distance, UnitSystem::Metric))
                                }
                            }
                        }
                        span class="icon icon-chevron chev" {}
                    }
                }
            }
        }
        @if !data.past.is_empty() {
            h3 { "Courues" }
            div class="list" {
                @for race in data.past.iter().take(5) {
                    a class="row" href={ "/courses/" (race.id) } {
                        div class="row-main" {
                            div class="row-title" { (race.name) }
                            div class="row-sub" {
                                (race.start_at_ms.map(format_date).unwrap_or_else(|| "date inconnue".to_string()))
                            }
                        }
                        span class="icon icon-chevron chev" {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_keys_round_trip() {
        for kind in WidgetKind::ALL {
            assert_eq!(WidgetKind::from_key(kind.key()), Some(kind));
        }
        assert_eq!(WidgetKind::from_key("inconnu"), None);
    }

    #[test]
    fn normalize_drops_unknown_and_duplicate_keys() {
        let keys = vec![
            "tours".to_string(),
            "inconnu".to_string(),
            "allure".to_string(),
            "tours".to_string(),
            " allure ".to_string(),
        ];
        assert_eq!(
            normalize(&keys),
            vec![WidgetKind::Tours, WidgetKind::Allure]
        );
        assert!(normalize(&[]).is_empty());
    }

    #[test]
    fn templates_only_use_known_widgets() {
        for template in TEMPLATES {
            assert!(!template.widgets.is_empty(), "{}", template.key);
            for kind in template.widgets {
                assert!(WidgetKind::ALL.contains(kind), "{}", template.key);
            }
        }
    }
}
