//! Interface web : tableau de bord, detail des seances, appairage de la montre.
//!
//! Rendu cote serveur en Rust (maud) : aucune chaine d'outils JavaScript, aucune
//! donnee sensible exposee au navigateur (session en cookie HttpOnly).

use crate::auth::device;
use crate::auth::{AuthUser, OptionalUser, SESSION_COOKIE};
use crate::error::{AppError, AppResult};
use crate::live::{LivePoint, LiveSessionView};
use crate::models::{
    DeezerAccount, MusicPlaylistInput, MusicTrack, MusicTrackInput, Race, RaceInput, RaceTask,
    SourcePlaylist, SpotifyAccount, User,
};
use crate::mqtt::MqttStatus;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use cookie::{Cookie, SameSite};
use maud::{html, Markup, PreEscaped, DOCTYPE};
use mpacer_core::cardio::HeartRateZones;
use mpacer_core::music::{
    music_coverage, race_duration_s, suggested_file_name, MusicConfig, Track as MusicTrackCore,
    WantedTrack, MUSIC_MARGIN_RATIO,
};
use mpacer_core::units::{format_duration, format_pace, UnitSystem};
use serde::Deserialize;
use std::fmt::Write as _;

/// Routes web.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(dashboard))
        .route("/stats", get(stats_page))
        // Suivi en direct : positions publiees par la montre en MQTT pendant la
        // seance (page HTML pour les proches, JSON pour un outil externe).
        .route("/live", get(live_page))
        .route("/live.json", get(live_json))
        // Tableaux de bord : ecrans composes par l'utilisateur (docs/08).
        .route("/dashboards", get(dashboards_page))
        .route(
            "/dashboards/nouveau",
            get(dashboard_new_page).post(dashboard_create),
        )
        .route("/dashboards/{id}", get(dashboard_page))
        .route(
            "/dashboards/{id}/modifier",
            get(dashboard_edit_page).post(dashboard_rename),
        )
        .route("/dashboards/{id}/supprimer", post(dashboard_delete))
        .route(
            "/dashboards/{id}/widgets/ajouter",
            post(dashboard_widget_add),
        )
        .route(
            "/dashboards/{id}/widgets/{widget}/retirer",
            post(dashboard_widget_remove),
        )
        .route(
            "/dashboards/{id}/widgets/{widget}/monter",
            post(dashboard_widget_move_up),
        )
        .route(
            "/dashboards/{id}/widgets/{widget}/descendre",
            post(dashboard_widget_move_down),
        )
        .route("/login", get(login_page))
        .route("/auth/google/start", get(google_start))
        .route("/auth/google/callback", get(google_callback))
        .route("/auth/dev-login", post(dev_login))
        .route("/logout", post(logout))
        .route("/link", get(link_page).post(link_submit))
        .route("/avatar", get(avatar))
        .route("/settings", get(settings_page))
        .route("/settings/tokens/{id}/revoke", post(revoke_token))
        .route("/workouts/{id}", get(workout_page))
        .route("/workouts/{id}/gpx", get(workout_gpx))
        .route("/workouts/{id}/delete", post(delete_workout))
        .route("/workouts/{id}/comment", post(workout_comment))
        .route("/courses", get(races_page))
        .route("/courses/planning", get(planning_page))
        .route("/courses/nouvelle", get(race_new_page).post(race_create))
        // Import d'une ancienne course (export Strava/Garmin) : la route fixe
        // elle-meme le plafond du corps, sans elargir le reste du service.
        .route(
            "/courses/importer",
            get(race_import_page)
                .post(race_import_submit)
                .layer(DefaultBodyLimit::max(
                    mpacer_core::race_import::MAX_IMPORT_BYTES,
                )),
        )
        .route("/courses/{id}", get(race_page))
        .route("/courses/{id}/trace.gpx", get(race_track_gpx))
        .route(
            "/courses/{id}/modifier",
            get(race_edit_page).post(race_update),
        )
        .route("/courses/{id}/supprimer", post(race_delete))
        .route("/courses/{id}/suivi", post(race_task_create))
        .route("/courses/{id}/suivi/{task}", post(race_task_toggle))
        .route(
            "/courses/{id}/suivi/{task}/supprimer",
            post(race_task_delete),
        )
        // Musique : metadonnees seulement, le transfert audio passe par USB
        // (outil local mpacer-music et manifeste de transfert).
        .route("/music", get(music_page))
        .route("/music/search", get(music_search))
        .route("/auth/spotify", get(spotify_start))
        .route("/auth/spotify/callback", get(spotify_callback))
        .route("/auth/deezer", get(deezer_start))
        .route("/auth/deezer/callback", get(deezer_callback))
        .route("/music/spotify/disconnect", post(spotify_disconnect))
        .route("/music/deezer/disconnect", post(deezer_disconnect))
        .route("/music/import", post(music_import))
        .route("/music/playlists/{id}/manifest", get(music_manifest))
        .route("/music/playlists/{id}/files", get(music_files))
        .route("/music/playlists/{id}/track-bpm", post(music_track_bpm))
        .route("/music/playlists/{id}/target", post(music_target))
        .route("/music/playlists/{id}/rename", post(music_rename))
        .route("/music/playlists/{id}/delete", post(music_delete))
        .route("/static/app.css", get(stylesheet))
        .route("/static/app.js", get(script))
        // Identite visuelle (docs/07 section 10.4) : SVG embarques.
        .route("/static/logo.svg", get(logo))
        .route("/static/logo-mark.svg", get(logo_mark))
        .route("/static/illustration-usb.svg", get(illustration_usb))
}

// ------------------------------------------------------------------ ressources

async fn stylesheet() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        crate::assets::APP_CSS,
    )
        .into_response()
}

async fn script() -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/javascript; charset=utf-8"),
        )],
        crate::assets::APP_JS,
    )
        .into_response()
}

/// Logo complet (marque + mot-cle "M-pacer").
async fn logo() -> Response {
    svg_response(crate::assets::LOGO_SVG)
}

/// Marque seule, utilisee en favicon et en petit format.
async fn logo_mark() -> Response {
    svg_response(crate::assets::LOGO_MARK_SVG)
}

/// Schema "playlist -> cable USB -> montre" du bloc 4.
async fn illustration_usb() -> Response {
    svg_response(crate::assets::ILLUSTRATION_USB_SVG)
}

/// Reponse SVG inline : aucun fichier n'est lu sur le disque.
fn svg_response(body: &'static str) -> Response {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/svg+xml; charset=utf-8"),
        )],
        body,
    )
        .into_response()
}

// ------------------------------------------------------------------ tableau de bord

/// Parametres de pagination de l'historique.
#[derive(Debug, Deserialize)]
struct PageQuery {
    #[serde(default)]
    page: Option<u32>,
}

/// Seances affichees par page.
const PAGE_SIZE: i64 = 20;

async fn dashboard(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(query): Query<PageQuery>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(page(landing()));
    };

    let page_number = query.page.unwrap_or(1).max(1);
    let filter = crate::db::WorkoutFilter {
        limit: PAGE_SIZE,
        offset: (page_number as i64 - 1) * PAGE_SIZE,
        ..Default::default()
    };
    let workouts = crate::db::list_workouts(&state.pool, &user.id, &filter).await?;
    let total_workouts =
        crate::db::count_workouts(&state.pool, &user.id, &crate::db::WorkoutFilter::default())
            .await?;
    let last_page = ((total_workouts + PAGE_SIZE - 1) / PAGE_SIZE).max(1) as u32;
    let stats = crate::db::stats(&state.pool, &user.id, 30, state.now_ms()).await?;
    let user_dashboards = crate::db::list_dashboards(&state.pool, &user.id).await?;
    let tokens = crate::db::list_tokens(&state.pool, &user.id).await?;
    let active_tokens = tokens
        .iter()
        .filter(|token| token.revoked_at_ms.is_none())
        .count();

    let content = html! {
        section class="hero" {
            h1 { "Vos seances" }
            p class="muted" { "Synchronisees depuis la montre, stockees chez vous." }
            div class="actions" {
                @for dashboard in &user_dashboards {
                    a class="button ghost small" href={ "/dashboards/" (dashboard.id) } {
                        span class="icon icon-grid" {} (dashboard.name)
                    }
                }
                a class="button ghost small" href="/dashboards/nouveau" {
                    span class="icon icon-plus" {} "Nouveau tableau de bord"
                }
            }
        }
        section class="cards" {
            div class="card" {
                span class="card-label" { "30 derniers jours" }
                strong { (stats.workout_count) " seances" }
            }
            div class="card" {
                span class="card-label" { "Distance" }
                strong { (format_distance(stats.total_distance_m, UnitSystem::Metric)) }
            }
            div class="card" {
                span class="card-label" { "Duree" }
                strong { (format_duration(stats.total_duration_s)) }
            }
            div class="card" {
                span class="card-label" { "Allure moyenne" }
                strong { (format_pace(stats.average_pace_s_per_km)) " /km" }
            }
            div class="card" {
                span class="card-label" { "Montres appairees" }
                strong { (active_tokens) }
            }
        }
        section {
            div class="section-head" {
                h2 { "Historique" }
                div class="actions" {
                    a class="button ghost small" href="/api/v1/export" {
                        span class="icon icon-export" {} "Exporter"
                    }
                    a class="button ghost small" href="/link" {
                        span class="icon icon-watch" {} "Appairer"
                    }
                }
            }
            @if workouts.is_empty() {
                div class="empty" {
                    span class="icon icon-activity" {}
                    p { "Aucune seance pour l'instant." }
                    p class="tiny" { "Appairez votre montre pour lancer la synchronisation." }
                }
            } @else {
                // Liste de cartes plutot qu'un tableau : lisible au pouce sur telephone.
                div class="list" {
                    @for workout in &workouts {
                        a class="row" href={ "/workouts/" (workout.id) } {
                            div class="row-main" {
                                div class="row-title" { (format_date(workout.started_at_ms)) }
                                div class="row-sub" {
                                    (format_duration(workout.duration_s)) " - "
                                    (format_pace(Some(workout.average_pace_s_per_km))) " /km"
                                }
                                @if let Some(comment) = &workout.comment {
                                    div class="row-sub row-comment" { (comment) }
                                }
                            }
                            span class="row-value" { (format_distance(workout.distance_m, units_of(&workout.unit_system))) }
                            span class="icon icon-chevron chev" {}
                        }
                    }
                }
                @if last_page > 1 {
                    div class="actions pager" {
                        @if page_number > 1 {
                            a class="button ghost" href={ "/?page=" (page_number - 1) } { "Precedent" }
                        }
                        span class="muted" { "Page " (page_number) " / " (last_page) " - " (total_workouts) " seances" }
                        @if page_number < last_page {
                            a class="button ghost" href={ "/?page=" (page_number + 1) } { "Suivant" }
                        }
                    }
                }
            }
        }
    };

    Ok(page(layout(
        "Tableau de bord",
        "seances",
        Some(&user),
        content,
    )))
}

fn landing() -> Markup {
    layout(
        "Accueil",
        "",
        None,
        html! {
            section class="hero" {
                span class="badge-live" { span class="dot" {} "Auto-heberge" }
                h1 { "M-pacer" }
                p class="muted" {
                    "Controlez votre allure en course, suivez un plan et retrouvez toutes vos seances."
                }
                div class="hero-actions" {
                    a class="button" href="/login" {
                        span class="icon icon-google" {} "Se connecter"
                    }
                }
            }
            section class="cards" {
                div class="card reveal" {
                    span class="card-label" { span class="icon icon-watch" {} " Montre" }
                    p { "L'application Wear OS enregistre la seance et l'envoie seule, meme si le telephone reste a la maison." }
                }
                div class="card reveal" {
                    span class="card-label" { span class="icon icon-key" {} " Backend" }
                    p { "Rust, PostgreSQL et votre hebergement : aucune donnee revendue." }
                }
                div class="card reveal" {
                    span class="card-label" { span class="icon icon-stats" {} " Analyse" }
                    p { "Tours, meilleures distances, statistiques et export GPX vers Strava ou Garmin." }
                }
            }
        },
    )
}

// ------------------------------------------------------------------ statistiques

async fn stats_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };

    let now = state.now_ms();
    let weekly =
        crate::db::weekly_totals(&state.pool, &user.id, now - 12 * 7 * 24 * 3600 * 1000).await?;
    let month = crate::db::stats(&state.pool, &user.id, 30, now).await?;
    let year = crate::db::stats(&state.pool, &user.id, 365, now).await?;

    let content = html! {
        section class="hero" {
            h1 { "Statistiques" }
            p class="muted" { "12 dernieres semaines" }
        }
        section class="cards" {
            div class="card" {
                span class="card-label" { "30 jours" }
                strong { (month.workout_count) " seances" }
            }
            div class="card" {
                span class="card-label" { "Distance 30 j" }
                strong { (format_distance(month.total_distance_m, UnitSystem::Metric)) }
            }
            div class="card" {
                span class="card-label" { "Allure moyenne 30 j" }
                strong { (format_pace(month.average_pace_s_per_km)) " /km" }
            }
            div class="card" {
                span class="card-label" { "Distance 12 mois" }
                strong { (format_distance(year.total_distance_m, UnitSystem::Metric)) }
            }
        }
        section {
            h2 { "Volume hebdomadaire" }
            @if weekly.is_empty() {
                p class="muted" { "Pas encore de donnees sur la periode." }
            } @else {
                (weekly_chart(&weekly))
                div class="table-wrap" {
                table {
                    thead { tr { th { "Semaine" } th { "Seances" } th { "Distance" } th { "Duree" } } }
                    tbody {
                        @for week in &weekly {
                            tr {
                                td { (week.label) }
                                td { (week.workout_count) }
                                td { (format_distance(week.distance_m, UnitSystem::Metric)) }
                                td { (format_duration(week.duration_s)) }
                            }
                        }
                    }
                }
                }
            }
        }
    };
    Ok(page(layout("Statistiques", "stats", Some(&user), content)))
}

/// Histogramme du volume hebdomadaire.
///
/// Rendu en HTML/CSS plutot qu'en SVG : les barres s'adaptent a la largeur de
/// l'ecran (une seule semaine ne s'etire plus en un pave geant, l'ancien
/// `preserveAspectRatio="none"` deformait barres et etiquettes) et le texte
/// garde une taille de police normale.
/// Hauteur maximale d'une barre, en pixels (le graphique est responsive en largeur).
const CR_HAUTEUR_BARRES: f64 = 150.0;

pub(crate) fn weekly_chart(weeks: &[crate::db::WeekTotal]) -> Markup {
    let max_distance = weeks
        .iter()
        .map(|week| week.distance_m)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let total: f64 = weeks.iter().map(|week| week.distance_m).sum();

    html! {
        div class="weeks" role="img"
            aria-label=(format!(
                "Volume hebdomadaire sur {} semaine(s), {} au total",
                weeks.len(),
                format_distance(total, UnitSystem::Metric)
            )) {
            @for week in weeks {
                @let ratio = (week.distance_m / max_distance).clamp(0.0, 1.0);
                // Hauteur en pixels : le graphique est responsive en largeur, et
                // une semaine sans course garde une barre visible de 4 px.
                @let hauteur_px = 4.0 + ratio * (CR_HAUTEUR_BARRES - 4.0);
                div class="week" {
                    span class="week-value" {
                        @if week.distance_m > 0.0 {
                            (format!("{:.1}", week.distance_m / 1000.0)) " km"
                        }
                    }
                    div class="week-track" {
                        div class="week-bar" style=(format!("height: {hauteur_px:.0}px"))
                            title=(format!("{} : {:.1} km", week.label, week.distance_m / 1000.0)) {}
                    }
                    span class="week-label" { (week.label) }
                }
            }
        }
    }
}

// ------------------------------------------------------------ tableaux de bord
//
// Un tableau de bord est une liste ordonnee de widgets (docs/08). Le catalogue
// des widgets et leur rendu vivent dans `crate::dashboards` : ici, on lit le
// formulaire, on persiste et on redirige.

use crate::dashboards::WidgetKind;

/// Formulaire de creation : nom, gabarit eventuel et widgets coches.
///
/// Les cases a cocher portent toutes le meme nom : le corps est donc relu comme
/// une suite de paires (cle, valeur), seul moyen de conserver les repetitions,
/// et donc l'ordre dans lequel le coureur a coche ses widgets.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct DashboardForm(Vec<(String, String)>);

impl DashboardForm {
    /// Champs utiles, dans l'ordre du formulaire.
    fn fields(&self) -> (String, Option<String>, Vec<String>) {
        let mut name = String::new();
        let mut template = None;
        let mut widgets = Vec::new();
        for (key, value) in &self.0 {
            match key.as_str() {
                "name" => name = value.clone(),
                "template" => {
                    if !value.trim().is_empty() {
                        template = Some(value.clone());
                    }
                }
                "widgets" => widgets.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|item| !item.is_empty())
                        .map(str::to_string),
                ),
                _ => {}
            }
        }
        (name, template, widgets)
    }
}

/// Formulaire de renommage.
#[derive(Debug, Deserialize)]
struct DashboardRenameForm {
    #[serde(default)]
    name: String,
}

/// Formulaire d'ajout d'un widget.
#[derive(Debug, Deserialize)]
struct WidgetAddForm {
    #[serde(default)]
    kind: String,
}

/// Cases a cocher des widgets disponibles.
fn widget_picker(selected: &[WidgetKind]) -> Markup {
    html! {
        fieldset class="widget-picker" {
            legend { "Widgets a afficher" }
            @for kind in WidgetKind::ALL {
                label class="check" {
                    input type="checkbox" name="widgets" value=(kind.key())
                          checked[selected.contains(&kind)] {}
                    span class="check-label" {
                        span class={ "icon " (kind.icon()) } {}
                        span { (kind.label()) }
                    }
                    small { (kind.description()) }
                }
            }
        }
    }
}

/// Choix d'un gabarit pret a l'emploi. Le gabarit coche les cases correspondantes
/// cote navigateur ; sans JavaScript, le serveur l'applique si rien n'est coche.
fn template_picker() -> Markup {
    html! {
        div class="template-picker" {
            label for="template" { "Partir d'un modele" }
            select id="template" name="template" data-widget-target="widgets" {
                option value="" { "Choisir moi-meme" }
                @for template in crate::dashboards::TEMPLATES {
                    @let keys = crate::dashboards::keys_of(template.widgets).join(",");
                    option value=(template.key) data-widgets=(keys) { (template.name) }
                }
            }
            ul class="template-notes" {
                @for template in crate::dashboards::TEMPLATES {
                    li { strong { (template.name) } " - " (template.description) }
                }
            }
        }
    }
}

async fn dashboards_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let dashboards = crate::db::list_dashboards(&state.pool, &user.id).await?;
    let counts = crate::db::dashboard_widget_counts(&state.pool, &user.id).await?;

    let content = html! {
        section class="hero" {
            h1 { "Tableaux de bord" }
            p class="muted" {
                "Composez vos ecrans : allure, carte, tours, meilleures distances, cardio, historique."
            }
            div class="actions" {
                a class="button" href="/dashboards/nouveau" {
                    span class="icon icon-plus" {} "Nouveau tableau de bord"
                }
            }
        }
        @if dashboards.is_empty() {
            div class="empty" {
                span class="icon icon-grid" {}
                p { "Aucun tableau de bord pour l'instant." }
                p class="tiny" {
                    "Creez-en un depuis un modele : Pace Control, analyse de seance ou historique."
                }
            }
        } @else {
            div class="list" {
                @for dashboard in &dashboards {
                    @let widget_count = counts
                        .iter()
                        .find(|(id, _)| id == &dashboard.id)
                        .map(|(_, count)| *count)
                        .unwrap_or(0);
                    a class="row" href={ "/dashboards/" (dashboard.id) } {
                        div class="row-main" {
                            div class="row-title" { (dashboard.name) }
                            div class="row-sub" {
                                (widget_count) " widget" @if widget_count > 1 { "s" }
                                " - modifie le " (format_date(dashboard.updated_at_ms))
                            }
                        }
                        span class="icon icon-chevron chev" {}
                    }
                }
            }
        }
    };
    Ok(page(layout(
        "Tableaux de bord",
        "dashboards",
        Some(&user),
        content,
    )))
}

async fn dashboard_new_page(OptionalUser(user): OptionalUser) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let content = html! {
        section class="hero" {
            h1 { "Nouveau tableau de bord" }
            p class="muted" { "Un nom, des widgets, et l'ordre que vous voulez." }
        }
        section class="panel" {
            form method="post" action="/dashboards/nouveau" {
                (template_picker())
                label for="name" { "Nom du tableau de bord" }
                input id="name" name="name" maxlength="80" required placeholder="Pace Control" {}
                (widget_picker(&[]))
                div class="actions" {
                    button type="submit" { "Creer" }
                    a class="button ghost" href="/dashboards" { "Annuler" }
                }
            }
        }
    };
    Ok(page(layout(
        "Nouveau tableau de bord",
        "dashboards",
        Some(&user),
        content,
    )))
}

async fn dashboard_create(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Form(form): Form<DashboardForm>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let (raw_name, raw_template, widget_keys) = form.fields();
    let name = crate::models::clean_dashboard_name(&raw_name)
        .ok_or_else(|| AppError::bad_request("donnez un nom au tableau de bord"))?;
    let mut kinds = crate::dashboards::normalize(&widget_keys);
    if kinds.is_empty() {
        if let Some(template) = raw_template
            .as_deref()
            .and_then(crate::dashboards::template)
        {
            kinds = template.widgets.to_vec();
        }
    }
    let dashboard = crate::db::insert_dashboard(
        &state.pool,
        &user.id,
        &name,
        &crate::dashboards::keys_of(&kinds),
        state.now_ms(),
    )
    .await?;
    // Un tableau vide s'ouvre directement sur l'edition : il n'y a rien a voir.
    let target = if kinds.is_empty() {
        format!("/dashboards/{}/modifier", dashboard.id)
    } else {
        format!("/dashboards/{}", dashboard.id)
    };
    Ok(Redirect::to(&target).into_response())
}

async fn dashboard_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let dashboard = crate::db::get_dashboard(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let widgets = crate::db::list_dashboard_widgets(&state.pool, &user.id, &dashboard.id).await?;
    let kinds: Vec<WidgetKind> = widgets
        .iter()
        .filter_map(|widget| WidgetKind::from_key(&widget.kind))
        .collect();
    let data = crate::dashboards::load(&state, &user, &kinds).await?;

    let content = html! {
        section class="hero" {
            h1 { (dashboard.name) }
            p class="muted" {
                @if let Some(workout) = &data.workout {
                    "Derniere seance : " (format_date(workout.started_at_ms))
                } @else {
                    "Aucune seance synchronisee pour l'instant."
                }
            }
            div class="actions" {
                a class="button ghost" href={ "/dashboards/" (dashboard.id) "/modifier" } {
                    span class="icon icon-settings" {} "Modifier"
                }
                form method="post" action={ "/dashboards/" (dashboard.id) "/supprimer" }
                     data-confirm="Supprimer ce tableau de bord ?" {
                    button class="ghost danger" type="submit" { "Supprimer" }
                }
            }
        }
        (crate::dashboards::render_widgets(&widgets, &data))
    };
    Ok(page(layout(
        &dashboard.name,
        "dashboards",
        Some(&user),
        content,
    )))
}

async fn dashboard_edit_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let dashboard = crate::db::get_dashboard(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let widgets = crate::db::list_dashboard_widgets(&state.pool, &user.id, &dashboard.id).await?;

    let content = html! {
        section class="hero" {
            h1 { "Modifier " (dashboard.name) }
            p class="muted" { "Renommez, ajoutez, retirez et ordonnez les widgets." }
            div class="actions" {
                a class="button ghost" href={ "/dashboards/" (dashboard.id) } { "Voir le tableau" }
            }
        }
        section class="panel" {
            form method="post" action={ "/dashboards/" (dashboard.id) "/modifier" } {
                label for="name" { "Nom" }
                input id="name" name="name" maxlength="80" required value=(dashboard.name) {}
                div class="actions" { button type="submit" { "Renommer" } }
            }
        }
        section class="panel" {
            div class="section-head" {
                h2 { "Widgets" }
                span class="muted" { (widgets.len()) " en place" }
            }
            @if widgets.is_empty() {
                p class="muted" { "Aucun widget : choisissez-en un ci-dessous." }
            } @else {
                ol class="widget-list" {
                    @for (index, widget) in widgets.iter().enumerate() {
                        @let kind = WidgetKind::from_key(&widget.kind);
                        @let icon = kind.map(|kind| kind.icon()).unwrap_or("icon-grid");
                        @let label = kind.map(|kind| kind.label()).unwrap_or("Widget inconnu");
                        li class="widget-row" {
                            span class="widget-name" {
                                span class={ "icon " (icon) } {}
                                (label)
                            }
                            div class="widget-actions" {
                                @if index > 0 {
                                    form method="post"
                                         action={ "/dashboards/" (dashboard.id) "/widgets/" (widget.id) "/monter" } {
                                        button class="ghost small" type="submit" { "Monter" }
                                    }
                                }
                                @if index + 1 < widgets.len() {
                                    form method="post"
                                         action={ "/dashboards/" (dashboard.id) "/widgets/" (widget.id) "/descendre" } {
                                        button class="ghost small" type="submit" { "Descendre" }
                                    }
                                }
                                form method="post"
                                     action={ "/dashboards/" (dashboard.id) "/widgets/" (widget.id) "/retirer" } {
                                    button class="ghost small danger" type="submit" { "Retirer" }
                                }
                            }
                        }
                    }
                }
            }
            form class="widget-add" method="post"
                 action={ "/dashboards/" (dashboard.id) "/widgets/ajouter" } {
                label for="kind" { "Ajouter un widget" }
                select id="kind" name="kind" {
                    @for kind in WidgetKind::ALL {
                        option value=(kind.key()) { (kind.label()) " - " (kind.description()) }
                    }
                }
                button type="submit" { "Ajouter" }
            }
        }
    };
    Ok(page(layout(
        "Modifier un tableau de bord",
        "dashboards",
        Some(&user),
        content,
    )))
}

async fn dashboard_rename(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<String>,
    Form(form): Form<DashboardRenameForm>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let name = crate::models::clean_dashboard_name(&form.name)
        .ok_or_else(|| AppError::bad_request("donnez un nom au tableau de bord"))?;
    if !crate::db::rename_dashboard(&state.pool, &user.id, &id, &name, state.now_ms()).await? {
        return Err(AppError::NotFound);
    }
    Ok(Redirect::to(&format!("/dashboards/{id}/modifier")).into_response())
}

async fn dashboard_delete(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    if !crate::db::delete_dashboard(&state.pool, &user.id, &id).await? {
        return Err(AppError::NotFound);
    }
    Ok(Redirect::to("/dashboards").into_response())
}

async fn dashboard_widget_add(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<String>,
    Form(form): Form<WidgetAddForm>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let Some(kind) = WidgetKind::from_key(form.kind.trim()) else {
        return Err(AppError::bad_request("widget inconnu"));
    };
    crate::db::add_dashboard_widget(&state.pool, &user.id, &id, kind.key(), state.now_ms())
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Redirect::to(&format!("/dashboards/{id}/modifier")).into_response())
}

async fn dashboard_widget_remove(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path((id, widget)): Path<(String, String)>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    if !crate::db::remove_dashboard_widget(&state.pool, &user.id, &id, &widget, state.now_ms())
        .await?
    {
        return Err(AppError::NotFound);
    }
    Ok(Redirect::to(&format!("/dashboards/{id}/modifier")).into_response())
}

async fn dashboard_widget_move_up(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path((id, widget)): Path<(String, String)>,
) -> AppResult<Response> {
    dashboard_widget_move(state, user, id, widget, true).await
}

async fn dashboard_widget_move_down(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path((id, widget)): Path<(String, String)>,
) -> AppResult<Response> {
    dashboard_widget_move(state, user, id, widget, false).await
}

/// Deplace un widget d'un cran. Un deplacement impossible (deja en tete ou en
/// queue) ramene simplement a l'edition : ce n'est pas une erreur.
async fn dashboard_widget_move(
    state: AppState,
    user: Option<User>,
    id: String,
    widget: String,
    up: bool,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    crate::db::move_dashboard_widget(&state.pool, &user.id, &id, &widget, up, state.now_ms())
        .await?;
    Ok(Redirect::to(&format!("/dashboards/{id}/modifier")).into_response())
}

// ------------------------------------------------------------------ connexion

async fn login_page(Query(query): Query<LoginQuery>) -> Html<String> {
    let content = html! {
        section class="hero" {
            h1 { "Connexion" }
            @if let Some(erreur) = &query.erreur {
                p class="alert" {
                    span class="icon icon-alert" {}
                    span { (message_erreur(erreur)) }
                }
            }
            p class="muted" { "L'acces se fait avec votre compte Google. Aucun mot de passe n'est stocke par M-pacer." }
            div class="hero-actions" {
                a class="button" href="/auth/google/start" {
                    span class="icon icon-google" {} "Continuer avec Google"
                }
            }
        }
    };
    Html(layout("Connexion", "login", None, content).into_string())
}

#[derive(Debug, Deserialize)]
struct LoginQuery {
    erreur: Option<String>,
}

async fn google_start(State(state): State<AppState>) -> AppResult<Response> {
    if !state.config.google_configured() {
        return Ok(Redirect::to("/login?erreur=google_non_configure").into_response());
    }
    let verifier = crate::auth::random_urlsafe(32);
    let challenge = crate::auth::pkce_challenge(&verifier);
    let oauth_state = crate::auth::random_urlsafe(24);
    let now = state.now_ms();
    sqlx::query(
        "INSERT INTO oauth_states (state, pkce_verifier, redirect_to, created_at_ms, expires_at_ms) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&oauth_state)
    .bind(&verifier)
    .bind("/")
    .bind(now)
    .bind(now + 600_000)
    .execute(&state.pool)
    .await?;

    let url = state.oidc.authorize_url(
        &state.config.google_redirect_uri(),
        &oauth_state,
        &challenge,
    );
    Ok(Redirect::to(&url).into_response())
}

#[derive(Debug, Deserialize)]
pub(crate) struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub(crate) async fn google_callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
    jar: axum_extra::extract::CookieJar,
) -> AppResult<Response> {
    if let Some(error) = query.error {
        return Ok(Redirect::to(&format!("/login?erreur={error}")).into_response());
    }
    let (Some(code), Some(oauth_state)) = (query.code, query.state) else {
        return Err(AppError::bad_request("parametres OAuth manquants"));
    };

    let now = state.now_ms();
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT pkce_verifier, redirect_to FROM oauth_states WHERE state = $1 AND expires_at_ms > $2",
    )
    .bind(&oauth_state)
    .bind(now)
    .fetch_optional(&state.pool)
    .await?;
    let Some((verifier, redirect_to)) = row else {
        return Err(AppError::bad_request("etat OAuth inconnu ou expire"));
    };
    sqlx::query("DELETE FROM oauth_states WHERE state = $1")
        .bind(&oauth_state)
        .execute(&state.pool)
        .await?;

    let google_user = match state
        .oidc
        .exchange(&code, &verifier, &state.config.google_redirect_uri())
        .await
    {
        Ok(user) => user,
        Err(error) => {
            tracing::warn!(error = %error, "echange de jeton Google refuse");
            return Ok(Redirect::to("/login?erreur=google_refuse").into_response());
        }
    };

    let user = crate::db::upsert_user(
        &state.pool,
        Some(&google_user.sub),
        &google_user.email,
        google_user.name.as_deref(),
        google_user.picture.as_deref(),
        now,
    )
    .await?;

    let session = crate::auth::issue_session(&state.config, &user, now)?;
    let jar = jar.add(session_cookie(&state.config, session));
    tracing::info!(user = %user.email, "connexion Google reussie");
    Ok((jar, Redirect::to(redirect_to.as_deref().unwrap_or("/"))).into_response())
}

/// Connexion de test, activee uniquement par `MPACER_DEV_AUTH=1`.
async fn dev_login(
    State(state): State<AppState>,
    jar: axum_extra::extract::CookieJar,
) -> AppResult<Response> {
    if !state.config.dev_auth {
        return Err(AppError::NotFound);
    }
    let now = state.now_ms();
    let user = crate::db::upsert_user(
        &state.pool,
        None,
        "dev@localhost",
        Some("Developpeur"),
        None,
        now,
    )
    .await?;
    let session = crate::auth::issue_session(&state.config, &user, now)?;
    let jar = jar.add(session_cookie(&state.config, session));
    Ok((jar, Redirect::to("/")).into_response())
}

async fn logout(jar: axum_extra::extract::CookieJar) -> Response {
    let jar = jar.remove(Cookie::from(SESSION_COOKIE));
    (jar, Redirect::to("/")).into_response()
}

fn session_cookie(config: &crate::config::Config, token: String) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(config.cookie_secure)
        .max_age(cookie::time::Duration::days(30))
        .build()
}

// ------------------------------------------------------------------ appairage

#[derive(Debug, Deserialize)]
struct LinkQuery {
    code: Option<String>,
    ok: Option<String>,
}

async fn link_page(
    OptionalUser(user): OptionalUser,
    Query(query): Query<LinkQuery>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let content = html! {
        section class="hero" {
            h1 { "Appairer une montre" }
            p class="muted" { "Sur la montre, ouvrez M-pacer et lancez la synchronisation : un code du type " code { "BCDF-GHJK" } " s'affiche. Saisissez-le ici." }
            @if query.ok.is_some() {
                p class="alert ok" { "Montre appairee. Elle peut maintenant envoyer ses seances." }
            }
        }
        section class="card form" {
            form method="post" action="/link" {
                label for="user_code" { "Code affiche sur la montre" }
                input id="user_code" name="user_code" required autocomplete="off" autocapitalize="characters"
                      placeholder="BCDF-GHJK" value=(query.code.clone().unwrap_or_default());
                button type="submit" { "Appairer" }
            }
        }
    };
    Ok(page(layout(
        "Appairer une montre",
        "link",
        Some(&user),
        content,
    )))
}

#[derive(Debug, Deserialize)]
struct LinkForm {
    user_code: String,
}

async fn link_submit(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Form(form): Form<LinkForm>,
) -> AppResult<Response> {
    match device::approve(&state, &form.user_code, &user.id).await {
        Ok(row) => {
            tracing::info!(user = %user.email, device = %row.label, "montre appairee");
            Ok(Redirect::to("/link?ok=1").into_response())
        }
        Err(error) => {
            let content = html! {
                section class="hero" {
                    h1 { "Appairage impossible" }
                    p class="alert" { (error.to_string()) }
                    a class="button ghost" href="/link" { "Reessayer" }
                }
            };
            Ok((
                StatusCode::BAD_REQUEST,
                page(layout("Appairer une montre", "link", Some(&user), content)),
            )
                .into_response())
        }
    }
}

// ------------------------------------------------------------------ jetons

async fn settings_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let tokens = crate::db::list_tokens(&state.pool, &user.id).await?;
    let active_tokens = tokens
        .iter()
        .filter(|token| token.revoked_at_ms.is_none())
        .count();

    let content = html! {
        section class="hero" {
            h1 { "Reglages" }
            p class="muted" { "Votre compte, vos montres appairees et l'appairage d'un nouvel appareil." }
        }

        // Menu unique des reglages : appairer et les jetons ne sont plus des
        // onglets de la navigation, ils vivent ici.
        nav class="settings-menu" aria-label="Sections des reglages" {
            a href="#profil" { span class="icon icon-user" {} "Profil" }
            a href="#appareils" { span class="icon icon-watch" {} "Appareils appaires" }
            a href="#appairer" { span class="icon icon-key" {} "Appairer une montre" }
            a href="#deconnexion" { span class="icon icon-logout" {} "Deconnexion" }
        }

        // ------------------------------------------------ profil
        div class="section-head" id="profil" { h2 { "Profil" } }
        section class="card profile" {
            img class="avatar avatar-large" src="/avatar" alt="" width="64" height="64" decoding="async";
            div class="profile-identite" {
                strong { (user.name.clone().unwrap_or_else(|| user.email.clone())) }
                span class="muted" { (user.email.clone()) }
                span class="tiny muted" {
                    @if user.picture_url.is_some() {
                        "Photo fournie par votre compte Google."
                    } @else {
                        "Aucune photo Google : pastille d'initiales."
                    }
                }
            }
        }

        // ------------------------------------------------ appareils appaires
        div class="section-head" id="appareils" {
            h2 { "Appareils appaires" }
            span class="muted" { (active_tokens) " actif(s) sur " (tokens.len()) }
        }
        @if tokens.is_empty() {
            p class="muted" { "Aucune montre appairee pour l'instant." }
        } @else {
            div class="table-wrap" {
            table {
                thead { tr { th { "Appareil" } th { "Cree le" } th { "Dernier envoi" } th { "Etat" } th {} } }
                tbody {
                    @for token in &tokens {
                        tr {
                            td { (token.label) }
                            td { (format_date(token.created_at_ms)) }
                            td { (token.last_used_ms.map(format_date).unwrap_or_else(|| "-".into())) }
                            td {
                                @if token.revoked_at_ms.is_some() { span class="pill off" { "revoque" } }
                                @else { span class="pill on" { "actif" } }
                            }
                            td {
                                @if token.revoked_at_ms.is_none() {
                                    form method="post" action={ "/settings/tokens/" (token.id) "/revoke" } {
                                        button class="ghost" type="submit" { "Revoquer" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            }
        }

        // ------------------------------------------------ appairer une montre
        div class="section-head" id="appairer" { h2 { "Appairer une montre" } }
        section class="panel" {
            p class="muted" {
                "Sur la montre, ouvrez M-pacer et lancez la synchronisation : un code du type "
                code { "BCDF-GHJK" }
                " s'affiche. Saisissez-le ici : chaque montre appairee recoit son propre jeton."
            }
            form class="split" method="post" action="/link" {
                input type="text" name="user_code" required autocomplete="off" autocapitalize="characters"
                      maxlength="9" placeholder="BCDF-GHJK";
                button type="submit" { "Appairer" }
            }
            p class="tiny muted" {
                "La page " a href="/link" { "Appairer une montre" } " detaille la procedure."
            }
        }

        // ------------------------------------------------ deconnexion
        div class="section-head" id="deconnexion" { h2 { "Deconnexion" } }
        section class="panel" {
            p class="muted" {
                "Fermer la session sur ce navigateur. Vos seances et vos playlists restent sur le serveur."
            }
            form method="post" action="/logout" {
                button class="ghost danger" type="submit" { "Se deconnecter" }
            }
        }
    };
    Ok(page(layout("Reglages", "reglages", Some(&user), content)))
}

async fn revoke_token(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    crate::db::revoke_token(&state.pool, &user.id, &id, state.now_ms()).await?;
    Ok(Redirect::to("/settings").into_response())
}

// ------------------------------------------------------------------ detail seance

async fn workout_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let (workout, payload) = crate::db::get_workout(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;

    let summary: Option<mpacer_core::history::WorkoutSummary> = serde_json::from_str(&payload).ok();
    let units = summary
        .as_ref()
        .map(|s| s.unit_system)
        .unwrap_or(UnitSystem::Metric);

    // Tout est calcule avant le rendu : une seule passe sur la trace.
    let moving_s = summary
        .as_ref()
        .map(|summary| summary.duration_s)
        .unwrap_or(workout.duration_s);
    let elapsed_total_s = summary
        .as_ref()
        .map(|summary| summary.total_elapsed_s())
        .unwrap_or(workout.duration_s);
    let paused_s = summary
        .as_ref()
        .map(|summary| summary.paused_s())
        .unwrap_or(0.0);
    let split_list = summary
        .as_ref()
        .map(mpacer_core::analysis::splits)
        .unwrap_or_default();
    let heart = summary.as_ref().and_then(|summary| {
        mpacer_core::cardio::summarize(&summary.heart_rate, HeartRateZones::default())
    });
    let drift = summary.as_ref().and_then(|summary| {
        mpacer_core::cardio::cardiac_drift(&summary.track, &summary.heart_rate)
    });
    let acceleration = summary
        .as_ref()
        .and_then(mpacer_core::analysis::acceleration);
    let elevation_gain = summary
        .as_ref()
        .map(|summary| mpacer_core::analysis::elevation_gain_m(&summary.track))
        .unwrap_or(0.0);
    // Allure ajustee a la pente : seulement si la montre a enregistre l'altitude.
    let gap = summary
        .as_ref()
        .and_then(mpacer_core::analysis::grade_adjusted);
    let has_gap = split_list
        .iter()
        .any(|split| split.gap_pace_s_per_km.is_some());
    let plan = summary.as_ref().and_then(|summary| summary.plan);

    let content = html! {
        section class="hero" {
            h1 { (format_date(workout.started_at_ms)) }
            p class="muted" {
                (format_distance(workout.distance_m, units)) " - "
                (format_duration(moving_s)) " en mouvement - "
                (format_pace(Some(workout.average_pace_s_per_km))) " /" (units.label())
            }
            div class="actions" {
                a class="button" href={ "/workouts/" (workout.id) "/gpx" } { "Telecharger le GPX" }
                form method="post" action={ "/workouts/" (workout.id) "/delete" }
                     data-confirm="Supprimer definitivement cette seance ?" {
                    button class="ghost danger" type="submit" { "Supprimer" }
                }
            }
        }
        section class="panel" {
            div class="section-head" {
                h2 { "Commentaire de course" }
                @if workout.comment.is_some() {
                    span class="pill on" { "enregistre" }
                }
            }
            // Le commentaire est un champ a part du resume envoye par la montre :
            // une nouvelle synchronisation de la seance ne l'efface pas.
            form method="post" action={ "/workouts/" (workout.id) "/comment" } {
                label for="comment" { "Sensations, meteo, parcours, materiel" }
                textarea id="comment" name="comment" rows="4" maxlength="2000"
                    placeholder="Ce qu'il faut retenir de cette seance." {
                    (workout.comment.clone().unwrap_or_default())
                }
                div class="actions" {
                    button type="submit" { "Enregistrer le commentaire" }
                }
            }
        }
        @if let Some(summary) = &summary {
            // -------------------------------------------- en-tete de resume
            section class="mini-cards" {
                (mini_card("Distance", &format_distance(summary.distance_m, units)))
                (mini_card("Temps en mouvement", &format_duration(summary.duration_s)))
                (mini_card("Temps ecoule", &format_duration(elapsed_total_s)))
                (mini_card("Allure moyenne", &format!("{} /{}", format_pace(summary.average_pace(units)), units.label())))
                @if let Some(gap) = &gap {
                    (mini_card("Allure ajustee (GAP)", &format!("{} /{}", format_pace(Some(gap.pace_s_per_km)), units.label())))
                }
                @if let Some(heart) = &heart {
                    (mini_card("FC moyenne", &format!("{:.0} bpm", heart.average_bpm)))
                    (mini_card("FC max", &format!("{} bpm", heart.max_bpm)))
                }
                @if elevation_gain > 0.5 {
                    (mini_card("Denivele +", &format!("{elevation_gain:.0} m")))
                }
                @if paused_s > 0.5 {
                    (mini_card("Pauses", &format!("{} en {}", format_duration(paused_s), summary.pauses.len())))
                }
            }

            // -------------------------------------------- graphique multi-courbes
            @if summary.track.len() > 10 {
                section class="panel" {
                    div class="section-head" {
                        h2 { "Graphique" }
                        span class="muted" { "allure, frequence cardiaque et altitude sur la distance" }
                    }
                    (performance_chart(summary, units))
                }
            }

            // -------------------------------------------- plan de course
            @if let Some(plan) = plan {
                section class="panel" {
                    h2 { "Plan de course" }
                    div class="mini-cards" {
                        (mini_card("Cible", &format!("{} sur {}", format_duration(plan.target_time_s), format_distance(plan.distance_m, units))))
                        (mini_card("Realise", &format_duration(summary.duration_s)))
                        (mini_card("Ecart au finish", &signed_seconds(summary.duration_s - plan.target_time_s)))
                        (mini_card("Allure cible", &format!("{} /{}", format_pace(Some(plan.average_pace(units))), units.label())))
                        (mini_card("Allure realisee", &format!("{} /{}", format_pace(summary.average_pace(units)), units.label())))
                        @if plan.negative_split.enabled {
                            (mini_card("Negative split", &format!("{:.1} %", plan.negative_split.ratio * 100.0)))
                        }
                    }
                    @if !split_list.is_empty() {
                        (plan_delta_chart(&split_list))
                        p class="muted" { "Ecart cumule au plan (barres vers le haut : en avance ; vers le bas : en retard)." }
                    }
                }
            }

            // -------------------------------------------- cardio
            @if let Some(heart) = &heart {
                section class="panel" {
                    div class="section-head" {
                        h2 { "Frequence cardiaque" }
                        span class="muted" { (heart.sample_count) " mesures - zones en % de la FC max (" (heart.zones.max_bpm) " bpm)" }
                    }
                    (heart_rate_zones(heart))
                    @if let Some(drift) = drift {
                        p class="muted" {
                            "Derive cardiaque (decouplage aerobie) : "
                            strong { (format!("{:+.1} %", drift.decoupling_percent)) }
                            " entre la premiere et la seconde moitie."
                        }
                    }
                }
            }

            // -------------------------------------------- temps de passage
            section class="panel" {
                h2 { "Temps de passage" }
                @if split_list.is_empty() {
                    p class="muted" { "Aucun tour enregistre." }
                } @else {
                    table {
                        thead {
                            tr {
                                th { "#" }
                                th { "Distance" }
                                th { "Temps" }
                                th { "Allure" }
                                th { "Ecart" }
                                @if heart.is_some() { th { "FC" } }
                                @if elevation_gain > 0.5 { th { "D+" } }
                                @if has_gap { th { "GAP" } }
                                @if has_gap { th { "Pente" } }
                                @if plan.is_some() { th { "Plan" } }
                            }
                        }
                        tbody {
                            @for split in &split_list {
                                tr {
                                    td { (split.index) }
                                    td { (format_distance(split.distance_m, units)) }
                                    td { (format_duration(split.duration_s)) }
                                    td { (format_pace(Some(split.pace_s_per_km))) " /" (units.label()) }
                                    td {
                                        @match split.pace_delta_s {
                                            Some(delta) => (signed_seconds(delta)),
                                            None => "-",
                                        }
                                    }
                                    @if heart.is_some() {
                                        td {
                                            @match split.heart_rate_avg {
                                                Some(average) => (format!("{average:.0}")),
                                                None => "-",
                                            }
                                        }
                                    }
                                    @if elevation_gain > 0.5 {
                                        td { (format!("{:.0} m", split.elevation_gain_m)) }
                                    }
                                    @if has_gap {
                                        td {
                                            @match split.gap_pace_s_per_km {
                                                Some(pace) => (format_pace(Some(pace))),
                                                None => "-",
                                            }
                                        }
                                    }
                                    @if has_gap {
                                        td {
                                            @match split.grade_percent {
                                                Some(grade) => (format!("{:+.1} %", grade * 100.0)),
                                                None => "-",
                                            }
                                        }
                                    }
                                    @if plan.is_some() {
                                        td {
                                            @match split.plan_delta_s {
                                                Some(delta) => (signed_seconds(delta)),
                                                None => "-",
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // -------------------------------------------- chronologie
            section class="panel" {
                h2 { "Chronologie" }
                div class="mini-cards" {
                    (mini_card("Temps de course", &format_duration(summary.duration_s)))
                    (mini_card("Temps ecoule", &format_duration(elapsed_total_s)))
                    (mini_card("Temps de pause", &format!("{} en {}", format_duration(paused_s), summary.pauses.len())))
                }
                @if summary.pauses.is_empty() {
                    p class="muted" { "Aucune pause pendant cette seance." }
                } @else {
                    ul class="pauses" {
                        @for (index, pause) in summary.pauses.iter().enumerate() {
                            li {
                                strong { "Pause " (index + 1) }
                                " a " (format_duration(pause.at_s))
                                " (" (format_distance(pause.at_distance_m, units)) ") pendant "
                                (format_duration(pause.duration_s))
                                " - " (if pause.automatic { "automatique" } else { "manuelle" })
                            }
                        }
                    }
                }
                @if let Some(acceleration) = &acceleration {
                    h3 { "Acceleration" }
                    p class="muted" {
                        "Allure de croisiere de reference : "
                        (format_pace(Some(units.pace_from_speed(acceleration.cruise_speed_mps).unwrap_or(0.0))))
                        " /" (units.label())
                    }
                    table {
                        thead { tr { th { "Phase" } th { "A la distance" } th { "Temps pour atteindre l'allure" } th { "Distance de la phase" } } }
                        tbody {
                            @for phase in &acceleration.phases {
                                tr {
                                    td { @if phase.after_pause { "Reprise" } @else { "Depart" } }
                                    td { (format_distance(phase.at_distance_m, units)) }
                                    td {
                                        @match phase.seconds {
                                            Some(seconds) => (format_duration(seconds)),
                                            None => "non atteinte",
                                        }
                                    }
                                    td { (format_distance(phase.distance_m, units)) }
                                }
                            }
                        }
                    }
                    (acceleration_bar(acceleration))
                    p class="muted" {
                        "Temps passe a accelerer : " (format_duration(acceleration.accelerating_s))
                        " - allure stable : " (format_duration(acceleration.steady_s))
                        " - ralentissement : " (format_duration(acceleration.decelerating_s))
                    }
                }
            }

            // -------------------------------------------- meilleures distances
            section class="panel" {
                h2 { "Meilleures distances" }
                @if summary.best_efforts.is_empty() {
                    p class="muted" { "Distance trop courte pour un temps de reference." }
                } @else {
                    table {
                        thead { tr { th { "Distance" } th { "Temps" } th { "Allure" } } }
                        tbody {
                            @for effort in &summary.best_efforts {
                                tr {
                                    td { (effort.label) }
                                    td { (format_duration(effort.time_s)) }
                                    td { (format_pace(pace_of(effort.time_s, effort.distance_m, units))) }
                                }
                            }
                        }
                    }
                }
            }
        } @else {
            p class="alert" { "Le detail de cette seance n'a pas pu etre relu." }
        }
    };
    Ok(page(layout("Seance", "seances", Some(&user), content)))
}

async fn delete_workout(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    crate::db::delete_workout(&state.pool, &user.id, &id).await?;
    Ok(Redirect::to("/").into_response())
}

#[derive(Debug, Deserialize)]
struct WorkoutCommentForm {
    #[serde(default)]
    comment: String,
}

/// Enregistre le commentaire d'une seance (texte vide pour l'effacer).
async fn workout_comment(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Form(form): Form<WorkoutCommentForm>,
) -> AppResult<Response> {
    let comment = crate::models::clean_comment(&form.comment);
    if !crate::db::set_workout_comment(&state.pool, &user.id, &id, comment.as_deref()).await? {
        return Err(AppError::NotFound);
    }
    tracing::info!(user = %user.email, seance = %id, "commentaire de seance enregistre");
    Ok(Redirect::to(&format!("/workouts/{id}")).into_response())
}

/// Telechargement GPX depuis le navigateur (session, pas de jeton a copier).
async fn workout_gpx(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    super::gpx_response(&state, &user.id, &id).await
}

// ------------------------------------------------------------------ avatar

/// Photo de profil du compte connecte.
///
/// Le service telecharge la photo Google (et la garde en memoire) avant de la
/// servir lui-meme : le navigateur ne contacte jamais Google. Sans photo, ou si
/// Google ne repond pas, une pastille aux initiales du compte prend le relais.
async fn avatar(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };

    if let Some(url) = user.picture_url.as_deref() {
        if let Some(image) = state.avatars.picture(&state.http, url).await {
            return Ok(avatar_response(&image.bytes, &image.content_type));
        }
    }

    Ok(avatar_response(
        crate::avatar::monogram_svg(&user).as_bytes(),
        "image/svg+xml; charset=utf-8",
    ))
}

/// Reponse image d'un avatar, jamais conservee par un cache partage.
fn avatar_response(bytes: &[u8], content_type: &str) -> Response {
    let content_type = HeaderValue::from_str(content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("image/png"));
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(axum::body::Body::from(bytes.to_vec()))
        .unwrap_or_else(|error| {
            AppError::internal(format!("reponse avatar : {error}")).into_response()
        })
}

// ------------------------------------------------------------------ rendu

fn page(markup: Markup) -> Response {
    Html(markup.into_string()).into_response()
}

/// Sections de navigation : cle interne, libelle, icone, chemin.
///
/// Reglages est un onglet comme les autres (docs/07 section 10.3) : l'appairage
/// d'une montre, les jetons et la deconnexion vivent sur `/settings`, et le menu
/// deroulant de l'en-tete a disparu.
const NAV: [(&str, &str, &str, &str); 8] = [
    ("seances", "Seances", "icon-activity", "/"),
    ("live", "Direct", "icon-watch", "/live"),
    ("dashboards", "Tableaux", "icon-grid", "/dashboards"),
    ("courses", "Courses", "icon-route", "/courses"),
    ("planning", "Planning", "icon-clock", "/courses/planning"),
    ("stats", "Statistiques", "icon-stats", "/stats"),
    ("music", "Musique", "icon-music", "/music"),
    ("reglages", "Reglages", "icon-settings", "/settings"),
];

fn layout(title: &str, active: &str, user: Option<&User>, content: Markup) -> Markup {
    layout_refresh(title, active, user, None, content)
}

/// Mise en page avec rafraichissement automatique optionnel (suivi en direct).
///
/// Le rafraichissement est un simple `<meta http-equiv="refresh">` : aucune
/// ligne de JavaScript, donc rien a charger ni a faire tourner sur le telephone
/// des proches.
fn layout_refresh(
    title: &str,
    active: &str,
    user: Option<&User>,
    refresh_s: Option<u64>,
    content: Markup,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="fr" {
            head {
                meta charset="utf-8";
                // viewport-fit=cover : l'encoche et la barre d'accueil iOS sont prises en
                // compte via les variables safe-area du CSS.
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                meta name="color-scheme" content="dark light";
                meta name="theme-color" content="#0b0d10" media="(prefers-color-scheme: dark)";
                meta name="theme-color" content="#f4f5f7" media="(prefers-color-scheme: light)";
                // mobile-web-app-capable remplace apple-mobile-web-app-capable
                // (depreciee) ; on garde les deux pour couvrir iOS et Android.
                meta name="mobile-web-app-capable" content="yes";
                meta name="apple-mobile-web-app-capable" content="yes";
                meta name="apple-mobile-web-app-title" content="M-pacer";
                meta name="description" content="Suivi de course auto-heberge : seances, analyse, export GPX.";
                @if let Some(secondes) = refresh_s {
                    meta http-equiv="refresh" content=(secondes);
                }
                title { (title) " - M-pacer" }
                link rel="stylesheet" href="/static/app.css";
                // Favicon : la marque seule, servie par le service (aucune image externe).
                link rel="icon" type="image/svg+xml" href="/static/logo-mark.svg";
            }
            body {
                header class="site" {
                    // Logo complet (marque + mot-cle) fourni par le chantier design.
                    a class="brand logo" href="/" { (PreEscaped(crate::assets::LOGO_SVG)) }
                    nav {
                        @if user.is_some() {
                            @for (cle, libelle, icone, chemin) in NAV {
                                a href=(chemin) class=(if cle == active { "active" } else { "" }) {
                                    span class={ "icon " (icone) } {}
                                    (libelle)
                                }
                            }
                        } @else {
                            a href="/login" { "Connexion" }
                        }
                    }
                    @if let Some(user) = user {
                        // Photo de profil Google (pastille d'initiales a defaut) :
                        // le lien reste visible sur mobile, ou la navigation est masquee.
                        a class="user-chip" href="/settings" title="Votre compte Google" {
                            img class="avatar" src="/avatar" alt="" width="32" height="32" decoding="async";
                            span class="who" { (user.name.clone().unwrap_or_else(|| user.email.clone())) }
                        }
                    }
                }
                main { (content) }
                footer { "M-pacer - " (mpacer_core::VERSION) " - vos donnees restent chez vous" }
                @if user.is_some() {
                    nav class="tabbar" aria-label="Navigation principale" {
                        @for (cle, libelle, icone, chemin) in NAV {
                            a href=(chemin) class=(if cle == active { "active" } else { "" }) {
                                span class={ "icon " (icone) } {}
                                span { (libelle) }
                            }
                        }
                    }
                }
                script src="/static/app.js" {}
            }
        }
    }
}

// ------------------------------------------------------- graphiques de seance
//
// Aucune librairie de graphiques : quelques polylignes SVG suffisent, et le
// rendu reste identique partout (y compris sans JavaScript).

/// Largeur du repere SVG (le CSS l'etire sur toute la page).
const CHART_WIDTH: f64 = 1000.0;
/// Fenetre de lissage de l'allure affichee (s) : assez large pour que la
/// courbe reste lisible une fois sous-echantillonnee.
const CHART_SMOOTHING_S: f64 = 30.0;
/// Marge gauche, pour les libelles de valeurs.
const CHART_LEFT: f64 = 64.0;
/// Marge droite.
const CHART_RIGHT: f64 = 14.0;

/// Petite carte de statistique (en-tete de resume).
pub(crate) fn mini_card(label: &str, value: &str) -> Markup {
    html! {
        div class="mini-card" {
            span class="card-label" { (label) }
            strong { (value) }
        }
    }
}

/// Duree signee ("+1:23", "-0:45"), pour les ecarts au plan.
fn signed_seconds(seconds: f64) -> String {
    if seconds.abs() < 0.5 {
        return "0 s".to_string();
    }
    format!(
        "{}{}",
        if seconds >= 0.0 { "+" } else { "-" },
        format_duration(seconds.abs())
    )
}

/// Bornes d'une serie, elargies quand elle est parfaitement plate.
fn series_range(values: &[f64]) -> (f64, f64) {
    let low = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let high = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if !low.is_finite() || !high.is_finite() {
        return (0.0, 1.0);
    }
    if (high - low).abs() < 1e-6 {
        return (low - 0.5, high + 0.5);
    }
    (low, high)
}

/// Projette une serie (distance en m, valeur) dans une bande verticale du SVG.
fn band_points(
    series: &[(f64, f64)],
    total_m: f64,
    plot_width: f64,
    band: (f64, f64),
    range: (f64, f64),
    invert: bool,
) -> Vec<(f64, f64)> {
    let (top, bottom) = band;
    let (low, high) = range;
    let span = (high - low).abs().max(1e-9);
    series
        .iter()
        .map(|(distance, value)| {
            let ratio = ((value - low) / span).clamp(0.0, 1.0);
            let ratio = if invert { 1.0 - ratio } else { ratio };
            let x = CHART_LEFT + (distance / total_m).clamp(0.0, 1.0) * plot_width;
            let y = bottom - ratio * (bottom - top);
            (x, y)
        })
        .collect()
}

/// Attribut "points" d'une polyligne SVG.
pub(crate) fn polyline_points(points: &[(f64, f64)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{x:.1},{y:.1}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Graphique multi-courbes : allure, frequence cardiaque et altitude, sur la
/// meme axe de distance, chacun dans sa bande et sur sa propre echelle.
fn performance_chart(summary: &mpacer_core::history::WorkoutSummary, units: UnitSystem) -> Markup {
    let plot_width = CHART_WIDTH - CHART_LEFT - CHART_RIGHT;
    let total_m = summary.distance_m.max(1.0);
    let pace_band = (16.0, 120.0);

    // Allure : courbe de vitesse lissee, replacee sur l'axe des distances.
    let curve = mpacer_core::analysis::speed_curve(&summary.track, CHART_SMOOTHING_S);
    let step = (curve.len() / 260).max(1);
    let mut pace_series: Vec<(f64, f64)> = Vec::new();
    for sample in curve.iter().step_by(step) {
        if sample.speed_mps <= 0.3 {
            continue;
        }
        if let Some(distance) =
            mpacer_core::analysis::distance_at_elapsed_s(&summary.track, sample.t_s)
        {
            pace_series.push((distance, 1000.0 / sample.speed_mps));
        }
    }

    let cardio_step = (summary.heart_rate.len() / 260).max(1);
    let cardio_series: Vec<(f64, f64)> = summary
        .heart_rate
        .iter()
        .step_by(cardio_step)
        .filter_map(|sample| {
            mpacer_core::best_distances::distance_at_time_m(&summary.track, sample.t_ms)
                .map(|distance| (distance, sample.bpm as f64))
        })
        .collect();

    let elevation_step = (summary.track.len() / 260).max(1);
    let elevation_series: Vec<(f64, f64)> = summary
        .track
        .iter()
        .step_by(elevation_step)
        .filter_map(|point| point.elevation_m.map(|elevation| (point.dist_m, elevation)))
        .collect();

    // L'altitude n'est affichee que si la montre en a enregistre : sinon la
    // bande resterait vide et volerait la place du cardio.
    let has_elevation = !elevation_series.is_empty();
    let height = if has_elevation { 340.0 } else { 302.0 };
    let cardio_band = if has_elevation {
        (152.0, 256.0)
    } else {
        (152.0, 288.0)
    };
    let elevation_band = (278.0, 326.0);

    let pace_range = series_range(
        &pace_series
            .iter()
            .map(|(_, pace)| *pace)
            .collect::<Vec<_>>(),
    );
    let cardio_range = series_range(
        &cardio_series
            .iter()
            .map(|(_, bpm)| *bpm)
            .collect::<Vec<_>>(),
    );
    let elevation_range = series_range(
        &elevation_series
            .iter()
            .map(|(_, elevation)| *elevation)
            .collect::<Vec<_>>(),
    );

    let pace_points = band_points(
        &pace_series,
        total_m,
        plot_width,
        pace_band,
        pace_range,
        true,
    );
    let cardio_points = band_points(
        &cardio_series,
        total_m,
        plot_width,
        cardio_band,
        cardio_range,
        false,
    );
    let elevation_points = band_points(
        &elevation_series,
        total_m,
        plot_width,
        elevation_band,
        elevation_range,
        false,
    );

    html! {
        div class="chart-block" {
            svg class="chart multi" viewBox=(format!("0 0 {CHART_WIDTH} {height}")) preserveAspectRatio="none" {
                rect class="band" x="0" y=(format!("{:.0}", pace_band.0 - 10.0))
                     width=(format!("{CHART_WIDTH:.0}")) height=(format!("{:.0}", pace_band.1 - pace_band.0 + 20.0)) {}
                rect class="band" x="0" y=(format!("{:.0}", cardio_band.0 - 10.0))
                     width=(format!("{CHART_WIDTH:.0}")) height=(format!("{:.0}", cardio_band.1 - cardio_band.0 + 20.0)) {}
                @if has_elevation {
                    rect class="band" x="0" y=(format!("{:.0}", elevation_band.0 - 8.0))
                         width=(format!("{CHART_WIDTH:.0}")) height=(format!("{:.0}", elevation_band.1 - elevation_band.0 + 16.0)) {}
                }
                @for split in mpacer_core::analysis::splits(summary) {
                    @let x = CHART_LEFT + (split.cumulative_distance_m / total_m).clamp(0.0, 1.0) * plot_width;
                    line class="grid" x1=(format!("{x:.1}")) y1="6" x2=(format!("{x:.1}")) y2=(format!("{:.0}", height - 6.0)) {}
                }
                @if !pace_points.is_empty() {
                    polyline class="trace pace" points=(polyline_points(&pace_points)) {}
                }
                @if !cardio_points.is_empty() {
                    polyline class="trace cardio" points=(polyline_points(&cardio_points)) {}
                }
                @if has_elevation {
                    polyline class="trace elevation" points=(polyline_points(&elevation_points)) {}
                }
                text class="chart-label" x="4" y=(format!("{:.0}", pace_band.0 + 10.0)) {
                    (format_pace(Some(pace_range.0))) " /" (units.label())
                }
                text class="chart-label" x="4" y=(format!("{:.0}", pace_band.1)) {
                    (format_pace(Some(pace_range.1)))
                }
                @if !cardio_series.is_empty() {
                    text class="chart-label" x="4" y=(format!("{:.0}", cardio_band.0 + 10.0)) { (format!("{:.0} bpm", cardio_range.1)) }
                    text class="chart-label" x="4" y=(format!("{:.0}", cardio_band.1)) { (format!("{:.0}", cardio_range.0)) }
                }
                @if has_elevation {
                    text class="chart-label" x="4" y=(format!("{:.0}", elevation_band.0 + 10.0)) { (format!("{:.0} m", elevation_range.1)) }
                    text class="chart-label" x="4" y=(format!("{:.0}", elevation_band.1)) { (format!("{:.0}", elevation_range.0)) }
                }
                text class="chart-label" x=(format!("{CHART_LEFT:.0}")) y=(format!("{:.0}", height - 2.0)) { "0" }
                text class="chart-label" x=(format!("{:.0}", CHART_WIDTH - CHART_RIGHT)) y=(format!("{:.0}", height - 2.0)) text-anchor="end" {
                    (format_distance(total_m, units))
                }
            }
            div class="legend" {
                span class="legend-item pace" { "Allure" }
                @if !cardio_series.is_empty() {
                    span class="legend-item cardio" { "Frequence cardiaque" }
                }
                @if has_elevation {
                    span class="legend-item elevation" { "Altitude" }
                }
            }
        }
    }
}

/// Ecart cumule au plan, tronçon par tronçon : au-dessus de l'axe, le coureur
/// est en avance ; en dessous, il est en retard.
fn plan_delta_chart(splits: &[mpacer_core::analysis::Split]) -> Markup {
    const HEIGHT: f64 = 150.0;
    let left = 20.0;
    let plot_width = CHART_WIDTH - left - CHART_RIGHT;
    let total_m = splits
        .last()
        .map(|split| split.cumulative_distance_m)
        .unwrap_or(1.0)
        .max(1.0);
    let max_delta = splits
        .iter()
        .filter_map(|split| split.plan_delta_s)
        .fold(1.0_f64, |largest, delta| largest.max(delta.abs()));
    let zero = HEIGHT / 2.0;
    let slot = plot_width / splits.len().max(1) as f64;
    let bar_width = (slot - 6.0).max(2.0);

    html! {
        svg class="chart bars" viewBox=(format!("0 0 {CHART_WIDTH} {HEIGHT}")) preserveAspectRatio="none" {
            line class="axis" x1="0" y1=(format!("{zero:.0}")) x2=(format!("{CHART_WIDTH:.0}")) y2=(format!("{zero:.0}")) {}
            @for split in splits {
                @if let Some(delta) = split.plan_delta_s {
                    @let ahead = delta < 0.0;
                    @let height = (delta.abs() / max_delta) * (zero - 16.0);
                    @let distance = split.cumulative_distance_m - split.distance_m / 2.0;
                    @let x = left + (distance / total_m).clamp(0.0, 1.0) * plot_width;
                    rect
                        class=(if ahead { "bar positive" } else { "bar negative" })
                        x=(format!("{x:.1}"))
                        y=(format!("{:.1}", if ahead { zero - height } else { zero }))
                        width=(format!("{bar_width:.1}"))
                        height=(format!("{:.1}", height.max(1.5))) {
                        title { (format!("{} : {}", format_distance(split.cumulative_distance_m, UnitSystem::Metric), signed_seconds(delta))) }
                    }
                }
            }
        }
    }
}

/// Repartition du temps par zone de frequence cardiaque.
fn heart_rate_zones(heart: &mpacer_core::cardio::HeartRateSummary) -> Markup {
    let total = heart.total_seconds().max(1.0);
    html! {
        div class="zones" {
            div class="zone-bar" {
                @for (index, seconds) in heart.zone_seconds.iter().enumerate() {
                    @if *seconds > 0.0 {
                        div class={ "zone-segment z" (index + 1) }
                            style=(format!("width: {:.3}%", seconds / total * 100.0)) {
                            title { (mpacer_core::cardio::ZONE_NAMES[index]) " : " (format_duration(*seconds)) }
                        }
                    }
                }
                @if heart.below_zone1_s > 0.0 {
                    div class="zone-segment z0" style=(format!("width: {:.3}%", heart.below_zone1_s / total * 100.0)) {
                        title { "Sous la zone 1 : " (format_duration(heart.below_zone1_s)) }
                    }
                }
            }
            table {
                thead { tr { th { "Zone" } th { "Plage" } th { "Temps" } th { "%" } } }
                tbody {
                    @for (index, seconds) in heart.zone_seconds.iter().enumerate() {
                        @let bounds = heart.zones.boundaries()[index];
                        tr {
                            td { (mpacer_core::cardio::ZONE_NAMES[index]) }
                            td { (format!("{:.0} - {:.0} bpm", bounds.0, bounds.1)) }
                            td { (format_duration(*seconds)) }
                            td { (format!("{:.0} %", heart.zone_percent(index))) }
                        }
                    }
                    @if heart.below_zone1_s > 0.0 {
                        tr {
                            td { "Sous la zone 1" }
                            td { (format!("< {:.0} bpm", heart.zones.boundaries()[0].0)) }
                            td { (format_duration(heart.below_zone1_s)) }
                            td { (format!("{:.0} %", heart.below_zone1_s / total * 100.0)) }
                        }
                    }
                }
            }
        }
    }
}

/// Repartition du temps entre acceleration, allure stable et ralentissement.
fn acceleration_bar(analysis: &mpacer_core::analysis::AccelerationAnalysis) -> Markup {
    let total = (analysis.accelerating_s + analysis.steady_s + analysis.decelerating_s).max(1.0);
    html! {
        div class="zone-bar" {
            div class="zone-segment accel" style=(format!("width: {:.3}%", analysis.accelerating_s / total * 100.0)) {
                title { "Acceleration : " (format_duration(analysis.accelerating_s)) }
            }
            div class="zone-segment steady" style=(format!("width: {:.3}%", analysis.steady_s / total * 100.0)) {
                title { "Allure stable : " (format_duration(analysis.steady_s)) }
            }
            div class="zone-segment decel" style=(format!("width: {:.3}%", analysis.decelerating_s / total * 100.0)) {
                title { "Ralentissement : " (format_duration(analysis.decelerating_s)) }
            }
        }
    }
}

fn pace_of(time_s: f64, distance_m: f64, units: UnitSystem) -> Option<f64> {
    if distance_m <= 0.0 {
        return None;
    }
    let seconds_per_meter = time_s / distance_m;
    Some(seconds_per_meter * units.meters_per_unit())
}

pub(crate) fn format_distance(meters: f64, units: UnitSystem) -> String {
    units.format_distance(meters)
}

/// Systeme d'unites stocke en base ("Metric" / "Imperial").
pub(crate) fn units_of(value: &str) -> UnitSystem {
    match value {
        "Imperial" | "imperial" => UnitSystem::Imperial,
        _ => UnitSystem::Metric,
    }
}

/// Date lisible en francais, a partir d'un horodatage UNIX (ms).
pub(crate) fn format_date(timestamp_ms: i64) -> String {
    match chrono::DateTime::from_timestamp_millis(timestamp_ms) {
        Some(datetime) => datetime
            .with_timezone(&chrono::Local)
            .format("%d/%m/%Y %H:%M")
            .to_string(),
        None => "-".to_string(),
    }
}

/// Traduit un code d'erreur de connexion en message lisible par l'utilisateur.
fn message_erreur(code: &str) -> String {
    match code {
        "google_non_configure" => "La connexion Google n'est pas encore configuree sur ce service :              l'administrateur doit renseigner le client OAuth."
            .to_string(),
        "google_refuse" => {
            "Google a refuse la connexion (client OAuth invalide, redirection non autorisee ou compte non autorise)."
                .to_string()
        }
        "access_denied" => "Vous avez refuse l'acces a votre compte Google.".to_string(),
        other => format!("Connexion impossible ({other})."),
    }
}

// ------------------------------------------------------------------ courses
//
// Trois ecrans, dans l'ordre du parcours reel du coureur :
//   * « Vos courses » : une carte par course (dossard, horaire, suivi) ;
//   * « Planning »    : l'agenda des echeances a venir (depart, dossard, hotel,
//                       elements de suivi) ;
//   * la fiche        : tout ce qui compte pour cette course, modifiable, avec
//                       le suivi des elements importants a cocher.

/// Une echeance du planning, quelle que soit son origine.
struct AgendaEntry {
    at_ms: i64,
    kind: &'static str,
    title: String,
    detail: Option<String>,
    race_id: String,
}

/// Valeur de formulaire nettoyee (chaine vide -> `None`).
fn some(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Formate un nombre pour un champ de formulaire ("42.195", "10").
fn format_number(value: f64) -> String {
    format!("{value}")
}

fn local_dt(ms: i64) -> Option<chrono::DateTime<chrono::Local>> {
    chrono::DateTime::from_timestamp_millis(ms).map(|dt| dt.with_timezone(&chrono::Local))
}

/// Date seule d'un horodatage (ms), au format d'un `input type="date"`.
fn format_date_input(ms: i64) -> String {
    local_dt(ms)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// Heure seule d'un horodatage (ms), au format d'un `input type="time"`.
fn format_time_input(ms: i64) -> String {
    local_dt(ms)
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_default()
}

/// Date et heure lisibles ; l'heure est omise quand elle vaut minuit, cas d'une
/// course dont seul le jour est connu.
fn format_datetime_short(ms: i64) -> String {
    match local_dt(ms) {
        Some(dt) if dt.time() == chrono::NaiveTime::MIN => dt.format("%d/%m/%Y").to_string(),
        Some(dt) => dt.format("%d/%m/%Y a %H:%M").to_string(),
        None => "-".to_string(),
    }
}

/// Heure d'un horodatage, sauf quand elle vaut minuit : une simple date ne
/// doit pas s'afficher comme un rendez-vous a 00:00.
fn format_time_short(ms: i64) -> Option<String> {
    match local_dt(ms) {
        Some(dt) if dt.time() != chrono::NaiveTime::MIN => Some(dt.format("%H:%M").to_string()),
        _ => None,
    }
}

/// Jour court, pour les pastilles d'agenda ("12/10").
fn format_day_month(ms: i64) -> String {
    local_dt(ms)
        .map(|dt| dt.format("%d/%m").to_string())
        .unwrap_or_else(|| "-".to_string())
}

/// Mois en toutes lettres, en francais (chrono formate en anglais).
fn month_label(ms: i64) -> String {
    use chrono::Datelike;
    const MONTHS: [&str; 12] = [
        "janvier",
        "fevrier",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "aout",
        "septembre",
        "octobre",
        "novembre",
        "decembre",
    ];
    match local_dt(ms) {
        Some(dt) => format!("{} {}", MONTHS[(dt.month0() as usize).min(11)], dt.year()),
        None => "sans date".to_string(),
    }
}

/// Compte a rebours en jours calendaires ("dans 12 jours", "demain").
fn countdown_label(start_ms: Option<i64>, now_ms: i64) -> String {
    let Some(start) = start_ms else {
        return "date a definir".to_string();
    };
    let (Some(day), Some(today)) = (
        local_dt(start).map(|dt| dt.date_naive()),
        local_dt(now_ms).map(|dt| dt.date_naive()),
    ) else {
        return "-".to_string();
    };
    match (day - today).num_days() {
        0 => "aujourd'hui".to_string(),
        1 => "demain".to_string(),
        days if days > 1 => format!("dans {days} jours"),
        -1 => "hier".to_string(),
        days => format!("il y a {} jours", -days),
    }
}

/// Encodage pourcent minimal, suffisant pour une recherche OpenStreetMap.
fn percent_encode(value: &str) -> String {
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

/// Lien cartographique : coordonnees si on les a, sinon le nom du lieu.
fn map_url(race: &Race) -> Option<String> {
    if let (Some(latitude), Some(longitude)) = (race.latitude, race.longitude) {
        return Some(format!(
            "https://www.openstreetmap.org/?mlat={latitude}&mlon={longitude}#map=15/{latitude}/{longitude}"
        ));
    }
    race.start_location
        .as_ref()
        .or(race.location.as_ref())
        .map(|place| {
            format!(
                "https://www.openstreetmap.org/search?query={}",
                percent_encode(place)
            )
        })
}

/// Analyse une date (`YYYY-MM-DD`) et une heure (`HH:MM`) locales.
///
/// Une heure seule est refusee (elle serait rattachee a un jour arbitraire) et
/// une date seule est acceptee a minuit : c'est le cas d'une course dont
/// l'horaire n'est pas encore publie.
fn parse_local_datetime(date: &str, time: &str) -> Result<Option<i64>, String> {
    let date = date.trim();
    let time = time.trim();
    if date.is_empty() {
        if time.is_empty() {
            return Ok(None);
        }
        return Err("precisez la date qui va avec l'horaire".to_string());
    }
    let time = if time.is_empty() { "00:00" } else { time };
    let naive = chrono::NaiveDateTime::parse_from_str(&format!("{date} {time}"), "%Y-%m-%d %H:%M")
        .map_err(|_| "date ou horaire illisible".to_string())?;
    let stamp = naive
        .and_local_timezone(chrono::Local)
        .single()
        .ok_or_else(|| "horaire ambigu (changement d'heure)".to_string())?;
    Ok(Some(stamp.timestamp_millis()))
}

/// Analyse un nombre decimal (la virgule francaise est acceptee).
fn parse_number(value: &str, label: &str) -> Result<Option<f64>, String> {
    let text = value.trim().replace(',', ".");
    if text.is_empty() {
        return Ok(None);
    }
    text.parse::<f64>()
        .map(Some)
        .map_err(|_| format!("{label} doit etre un nombre"))
}

/// Analyse un objectif de temps : `1:23:45`, `42:15` ou des minutes seules.
fn parse_duration(value: &str) -> Result<Option<f64>, String> {
    let text = value.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let error = || "objectif de temps illisible (exemples : 1:23:45, 42:15)".to_string();
    let parts: Vec<f64> = text
        .split(':')
        .map(|part| part.trim().parse::<f64>().map_err(|_| error()))
        .collect::<Result<Vec<_>, _>>()?;
    let seconds = match parts.as_slice() {
        [minutes] => minutes * 60.0,
        [minutes, seconds] => minutes * 60.0 + seconds,
        [hours, minutes, seconds] => hours * 3600.0 + minutes * 60.0 + seconds,
        _ => return Err(error()),
    };
    Ok(Some(seconds))
}

/// Valeurs brutes du formulaire de course.
///
/// Des chaines, et non des nombres : le formulaire peut ainsi etre reaffiche
/// tel que l'utilisateur l'a saisi quand une valeur est refusee, sans rien
/// perdre ni rien inventer.
#[derive(Debug, Clone, Default, Deserialize)]
struct RaceForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    time: String,
    #[serde(default)]
    distance_km: String,
    #[serde(default)]
    discipline: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    start_location: String,
    #[serde(default)]
    bib_number: String,
    #[serde(default)]
    bib_pickup_date: String,
    #[serde(default)]
    bib_pickup_time: String,
    #[serde(default)]
    bib_pickup_location: String,
    #[serde(default)]
    live_url: String,
    #[serde(default)]
    registration_url: String,
    #[serde(default)]
    website_url: String,
    #[serde(default)]
    latitude: String,
    #[serde(default)]
    longitude: String,
    #[serde(default)]
    hotel_name: String,
    #[serde(default)]
    hotel_address: String,
    #[serde(default)]
    hotel_phone: String,
    #[serde(default)]
    hotel_url: String,
    #[serde(default)]
    hotel_booked: String,
    #[serde(default)]
    hotel_check_in: String,
    #[serde(default)]
    hotel_check_out: String,
    #[serde(default)]
    lodging_notes: String,
    #[serde(default)]
    nutrition_notes: String,
    #[serde(default)]
    important_info: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    goal_time: String,
}

impl RaceForm {
    /// Pre-remplit le formulaire depuis une fiche existante.
    fn from_race(race: &Race) -> RaceForm {
        RaceForm {
            name: race.name.clone(),
            date: race.start_at_ms.map(format_date_input).unwrap_or_default(),
            time: race.start_at_ms.map(format_time_input).unwrap_or_default(),
            distance_km: race
                .distance_m
                .map(|meters| format_number(meters / 1000.0))
                .unwrap_or_default(),
            discipline: race.discipline.clone().unwrap_or_default(),
            location: race.location.clone().unwrap_or_default(),
            start_location: race.start_location.clone().unwrap_or_default(),
            bib_number: race.bib_number.clone().unwrap_or_default(),
            bib_pickup_date: race
                .bib_pickup_at_ms
                .map(format_date_input)
                .unwrap_or_default(),
            bib_pickup_time: race
                .bib_pickup_at_ms
                .map(format_time_input)
                .unwrap_or_default(),
            bib_pickup_location: race.bib_pickup_location.clone().unwrap_or_default(),
            live_url: race.live_url.clone().unwrap_or_default(),
            registration_url: race.registration_url.clone().unwrap_or_default(),
            website_url: race.website_url.clone().unwrap_or_default(),
            latitude: race.latitude.map(format_number).unwrap_or_default(),
            longitude: race.longitude.map(format_number).unwrap_or_default(),
            hotel_name: race.hotel_name.clone().unwrap_or_default(),
            hotel_address: race.hotel_address.clone().unwrap_or_default(),
            hotel_phone: race.hotel_phone.clone().unwrap_or_default(),
            hotel_url: race.hotel_url.clone().unwrap_or_default(),
            hotel_booked: if race.hotel_booked {
                "1".to_string()
            } else {
                String::new()
            },
            hotel_check_in: race
                .hotel_check_in_ms
                .map(format_date_input)
                .unwrap_or_default(),
            hotel_check_out: race
                .hotel_check_out_ms
                .map(format_date_input)
                .unwrap_or_default(),
            lodging_notes: race.lodging_notes.clone().unwrap_or_default(),
            nutrition_notes: race.nutrition_notes.clone().unwrap_or_default(),
            important_info: race.important_info.clone().unwrap_or_default(),
            notes: race.notes.clone().unwrap_or_default(),
            goal_time: race
                .goal_time_s
                .map(mpacer_core::units::format_duration)
                .unwrap_or_default(),
        }
    }

    /// Convertit la saisie en champs enregistrables.
    fn to_input(&self) -> Result<RaceInput, String> {
        Ok(RaceInput {
            name: self.name.clone(),
            start_at_ms: parse_local_datetime(&self.date, &self.time)?,
            distance_m: parse_number(&self.distance_km, "la distance")?.map(|km| km * 1000.0),
            discipline: some(&self.discipline),
            location: some(&self.location),
            start_location: some(&self.start_location),
            bib_number: some(&self.bib_number),
            bib_pickup_at_ms: parse_local_datetime(&self.bib_pickup_date, &self.bib_pickup_time)?,
            bib_pickup_location: some(&self.bib_pickup_location),
            live_url: some(&self.live_url),
            registration_url: some(&self.registration_url),
            website_url: some(&self.website_url),
            latitude: parse_number(&self.latitude, "la latitude")?,
            longitude: parse_number(&self.longitude, "la longitude")?,
            hotel_name: some(&self.hotel_name),
            hotel_address: some(&self.hotel_address),
            hotel_phone: some(&self.hotel_phone),
            hotel_url: some(&self.hotel_url),
            hotel_booked: !self.hotel_booked.trim().is_empty(),
            hotel_check_in_ms: parse_local_datetime(&self.hotel_check_in, "")?,
            hotel_check_out_ms: parse_local_datetime(&self.hotel_check_out, "")?,
            lodging_notes: some(&self.lodging_notes),
            nutrition_notes: some(&self.nutrition_notes),
            important_info: some(&self.important_info),
            notes: some(&self.notes),
            goal_time_s: parse_duration(&self.goal_time)?,
        })
    }
}

/// Champ texte du formulaire.
fn text_field(name: &str, label: &str, value: &str, input_type: &str, placeholder: &str) -> Markup {
    html! {
        div class="field" {
            label for=(name) { (label) }
            input type=(input_type) id=(name) name=(name) value=(value) placeholder=(placeholder);
        }
    }
}

/// Zone de texte du formulaire.
fn area_field(name: &str, label: &str, value: &str, placeholder: &str) -> Markup {
    html! {
        div class="field" {
            label for=(name) { (label) }
            textarea id=(name) name=(name) rows="3" placeholder=(placeholder) { (value) }
        }
    }
}

/// Ligne d'information : la valeur manquante est montree, pas cachee, pour que
/// le coureur voie d'un coup d'oeil ce qu'il lui reste a remplir.
fn info_row(label: &str, value: Option<&str>) -> Markup {
    html! {
        div class="info-row" {
            span class="info-label" { (label) }
            @match value {
                Some(value) => span class="info-value" { (value) },
                None => span class="info-value empty" { "non renseigne" },
            }
        }
    }
}

/// Ligne d'information contenant un lien externe.
fn link_row(label: &str, url: Option<&str>, text: &str) -> Markup {
    html! {
        div class="info-row" {
            span class="info-label" { (label) }
            @match url {
                Some(url) => span class="info-value" {
                    a href=(url) target="_blank" rel="noopener noreferrer" { (text) }
                },
                None => span class="info-value empty" { "non renseigne" },
            }
        }
    }
}

/// Le formulaire complet d'une course, en sections.
fn race_form(action: &str, values: &RaceForm, error: Option<&str>, submit: &str) -> Markup {
    html! {
        form class="race-form" method="post" action=(action) {
            @if let Some(error) = error {
                p class="alert" { (error) }
            }
            section class="panel" {
                h2 { "La course" }
                div class="grid-2" {
                    (text_field("name", "Nom de la course *", &values.name, "text", "Marathon de Lyon"))
                    (text_field("discipline", "Discipline", &values.discipline, "text", "Route, trail, ultra..."))
                    (text_field("date", "Date", &values.date, "date", ""))
                    (text_field("time", "Horaire de depart", &values.time, "time", ""))
                    (text_field("distance_km", "Distance (km)", &values.distance_km, "text", "42.195"))
                    (text_field("goal_time", "Objectif de temps", &values.goal_time, "text", "3:30:00"))
                    (text_field("location", "Ville / region", &values.location, "text", "Lyon"))
                    (text_field("start_location", "Lieu de depart", &values.start_location, "text", "Place Bellecour"))
                    (text_field("latitude", "Latitude", &values.latitude, "text", "45.7578"))
                    (text_field("longitude", "Longitude", &values.longitude, "text", "4.8320"))
                }
            }
            section class="panel" {
                h2 { "Dossard et inscription" }
                div class="grid-2" {
                    (text_field("bib_number", "Numero de dossard", &values.bib_number, "text", "1234"))
                    (text_field("bib_pickup_date", "Prise de dossard - date", &values.bib_pickup_date, "date", ""))
                    (text_field("bib_pickup_time", "Prise de dossard - heure", &values.bib_pickup_time, "time", ""))
                    (text_field("bib_pickup_location", "Prise de dossard - lieu", &values.bib_pickup_location, "text", "Village depart, stand 12"))
                    (text_field("registration_url", "Lien d'inscription", &values.registration_url, "url", "https://..."))
                    (text_field("website_url", "Site officiel de la course", &values.website_url, "url", "https://..."))
                }
            }
            section class="panel" {
                h2 { "Suivi en direct" }
                (text_field("live_url", "Lien du live (tracking)", &values.live_url, "url", "https://live.exemple.org/coureur/1234"))
                p class="muted" { "Ce lien sera accessible depuis la fiche et partageable avec vos proches." }
            }
            section class="panel" {
                h2 { "Hebergement" }
                div class="field check" {
                    label {
                        input type="checkbox" name="hotel_booked" value="1" checked[!values.hotel_booked.trim().is_empty()];
                        " Hotel reserve"
                    }
                }
                div class="grid-2" {
                    (text_field("hotel_name", "Nom de l'hotel", &values.hotel_name, "text", "Ibis Lyon Centre"))
                    (text_field("hotel_address", "Adresse", &values.hotel_address, "text", "12 rue de la Paix, Lyon"))
                    (text_field("hotel_phone", "Telephone", &values.hotel_phone, "text", "+33 4 00 00 00 00"))
                    (text_field("hotel_url", "Lien de reservation", &values.hotel_url, "url", "https://..."))
                    (text_field("hotel_check_in", "Arrivee", &values.hotel_check_in, "date", ""))
                    (text_field("hotel_check_out", "Depart", &values.hotel_check_out, "date", ""))
                }
                (area_field("lodging_notes", "Autres solutions pour dormir", &values.lodging_notes, "Camping, famille sur place, auberge..."))
            }
            section class="panel" {
                h2 { "Informations importantes" }
                (area_field("nutrition_notes", "Nutrition et ravitaillement", &values.nutrition_notes, "Ravitos tous les 5 km, gel au 25e km, boisson a emporter..."))
                (area_field("important_info", "Informations importantes pour la course", &values.important_info, "Certificat medical, PPS, consignes, barrieres horaires, meteo..."))
                (area_field("notes", "Autres informations", &values.notes, "Transport, accompagnants, dossards des amis..."))
            }
            div class="actions" {
                button type="submit" { (submit) }
                a class="button ghost" href="/courses" { "Annuler" }
            }
        }
    }
}

fn race_form_response(
    user: &User,
    heading: &str,
    action: &str,
    values: &RaceForm,
    error: Option<&str>,
    submit: &str,
    status: StatusCode,
) -> Response {
    let content = html! {
        section class="hero" {
            h1 { (heading) }
            p class="muted" { "Renseignez ce que vous savez : la fiche se complete au fil des semaines." }
        }
        (race_form(action, values, error, submit))
    };
    (status, page(layout(heading, "", Some(user), content))).into_response()
}

/// Ligne de resume d'une course : quand et ou.
fn race_summary_line(race: &Race) -> String {
    let mut parts = Vec::new();
    match race.start_at_ms {
        Some(ms) => parts.push(format_datetime_short(ms)),
        None => parts.push("date a definir".to_string()),
    }
    if let Some(place) = race.start_location.as_ref().or(race.location.as_ref()) {
        parts.push(place.clone());
    }
    parts.join(" - ")
}

/// Carte d'une course : l'essentiel visible sans ouvrir la fiche.
fn race_card(race: &Race, tasks: &[RaceTask], now_ms: i64) -> Markup {
    let done = tasks.iter().filter(|task| task.done).count();
    let total = tasks.len();
    let next_task = tasks.iter().find(|task| !task.done);
    let ratio = if total == 0 {
        0.0
    } else {
        done as f64 / total as f64 * 100.0
    };

    html! {
        article class="race-card" {
            div class="race-card-head" {
                h3 { a href={ "/courses/" (race.id) } { (race.name) } }
                span class="countdown" data-at=(race.start_at_ms.unwrap_or(0)) {
                    span class="dot" {}
                    (countdown_label(race.start_at_ms, now_ms))
                }
            }
            p class="muted" { (race_summary_line(race)) }
            div class="chips" {
                @if let Some(bib) = &race.bib_number {
                    span class="chip" { "Dossard " (bib) }
                }
                @if let Some(distance) = race.distance_m {
                    span class="chip" { (format_distance(distance, UnitSystem::Metric)) }
                }
                @if let Some(discipline) = &race.discipline {
                    span class="chip" { (discipline) }
                }
                @if race.hotel_booked {
                    span class="chip ok" { "Hotel reserve" }
                } @else if race.hotel_name.is_some() {
                    span class="chip warn" { "Hotel a confirmer" }
                }
                @if race.live_url.is_some() {
                    span class="chip" { "Live" }
                }
            }
            @if total > 0 {
                div class="progress" {
                    span class="muted" { "Suivi : " (done) "/" (total) }
                    @if let Some(task) = next_task {
                        span class="muted" { " - a faire : " (task.label) }
                    }
                }
                div class="bar" {
                    div class="bar-fill" style=(format!("width: {ratio:.0}%")) {}
                }
            }
        }
    }
}

/// Etiquette d'une echeance du planning.
fn agenda_label(kind: &str) -> &'static str {
    match kind {
        "depart" => "Depart",
        "dossard" => "Dossard",
        "hotel" => "Hotel",
        _ => "Suivi",
    }
}

async fn races_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let now = state.now_ms();
    let upcoming = crate::db::list_upcoming_races(&state.pool, &user.id, now).await?;
    let past = crate::db::list_past_races(&state.pool, &user.id, now).await?;

    let mut cards = Vec::with_capacity(upcoming.len());
    let mut pending_tasks = 0_usize;
    for race in &upcoming {
        let tasks = crate::db::list_race_tasks(&state.pool, &user.id, &race.id).await?;
        pending_tasks += tasks.iter().filter(|task| !task.done).count();
        cards.push(race_card(race, &tasks, now));
    }

    let next_race = upcoming
        .iter()
        .find(|race| race.start_at_ms.is_some())
        .map(|race| format!("{} - {}", race.name, countdown_label(race.start_at_ms, now)))
        .unwrap_or_else(|| "-".to_string());

    let content = html! {
        section class="hero" {
            h1 { "Vos courses" }
            p class="muted" { "Chaque course a sa fiche : dossard, horaires, lieux, live, hebergement et elements a preparer." }
        }
        section class="cards" {
            div class="card" {
                span class="card-label" { "Courses a venir" }
                strong { (upcoming.len()) }
            }
            div class="card" {
                span class="card-label" { "Prochain depart" }
                strong class="small" { (next_race) }
            }
            div class="card" {
                span class="card-label" { "Elements a preparer" }
                strong { (pending_tasks) }
            }
            div class="card" {
                span class="card-label" { "Deja courues" }
                strong { (past.len()) }
            }
        }
        section {
            div class="section-head" {
                h2 { "Cartes des courses" }
                div class="actions" {
                    a class="button ghost" href="/courses/planning" { "Planning" }
                    a class="button ghost" href="/courses/importer" {
                        span class="icon icon-export" {}
                        "Importer une course"
                    }
                    a class="button" href="/courses/nouvelle" { "Ajouter une course" }
                }
            }
            @if cards.is_empty() {
                p class="muted" {
                    "Aucune course enregistree. Ajoutez votre prochaine course pour suivre son dossard, "
                    "son horaire, son lieu de depart, son live et votre hebergement, ou importez une "
                    "ancienne course depuis un export Strava ou Garmin."
                }
            } @else {
                div class="race-grid" {
                    @for card in &cards {
                        (card)
                    }
                }
            }
        }
        @if !past.is_empty() {
            section {
                h2 { "Deja courues" }
                table {
                    thead { tr { th { "Date" } th { "Course" } th { "Distance" } th { "Dossard" } th {} } }
                    tbody {
                        @for race in &past {
                            tr {
                                td { (race.start_at_ms.map(format_datetime_short).unwrap_or_else(|| "-".into())) }
                                td {
                                    (race.name)
                                    @if race.is_reference {
                                        " "
                                        span class="pill brand" { "Reference" }
                                    }
                                }
                                td {
                                    @match race.distance_m {
                                        Some(distance) => (format_distance(distance, UnitSystem::Metric)),
                                        None => "-",
                                    }
                                }
                                td { (race.bib_number.clone().unwrap_or_else(|| "-".into())) }
                                td { a href={ "/courses/" (race.id) } { "Fiche" } }
                            }
                        }
                    }
                }
            }
        }
    };
    Ok(page(layout("Vos courses", "courses", Some(&user), content)))
}

async fn planning_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let now = state.now_ms();
    let upcoming = crate::db::list_upcoming_races(&state.pool, &user.id, now).await?;

    // Le planning rassemble des echeances venues de plusieurs champs : depart,
    // prise de dossard, hotel et elements de suivi dates.
    let mut entries: Vec<AgendaEntry> = Vec::new();
    for race in &upcoming {
        let place = race
            .start_location
            .clone()
            .or_else(|| race.location.clone());
        let hotel = race.hotel_name.clone().unwrap_or_else(|| race.name.clone());
        let mut push =
            |at_ms: Option<i64>, kind: &'static str, title: String, detail: Option<String>| {
                if let Some(at_ms) = at_ms {
                    entries.push(AgendaEntry {
                        at_ms,
                        kind,
                        title,
                        detail,
                        race_id: race.id.clone(),
                    });
                }
            };
        push(
            race.start_at_ms,
            "depart",
            format!("Depart - {}", race.name),
            place.clone(),
        );
        push(
            race.bib_pickup_at_ms,
            "dossard",
            format!("Prise de dossard - {}", race.name),
            race.bib_pickup_location.clone().or_else(|| place.clone()),
        );
        push(
            race.hotel_check_in_ms,
            "hotel",
            format!("Arrivee a l'hotel - {hotel}"),
            race.hotel_address.clone(),
        );
        push(
            race.hotel_check_out_ms,
            "hotel",
            format!("Depart de l'hotel - {hotel}"),
            race.hotel_address.clone(),
        );
        for task in crate::db::list_race_tasks(&state.pool, &user.id, &race.id).await? {
            if task.done {
                continue;
            }
            push(
                task.due_at_ms,
                "suivi",
                task.label.clone(),
                Some(race.name.clone()),
            );
        }
    }
    entries.sort_by_key(|entry| entry.at_ms);

    // Regroupement par mois, sans dependance a une bibliotheque de calendrier.
    let mut months: Vec<(String, Vec<&AgendaEntry>)> = Vec::new();
    for entry in &entries {
        let label = month_label(entry.at_ms);
        match months.last_mut() {
            Some((current, items)) if *current == label => items.push(entry),
            _ => months.push((label, vec![entry])),
        }
    }

    let content = html! {
        section class="hero" {
            h1 { "Planning" }
            p class="muted" { "Toutes les echeances de vos prochaines courses, dans l'ordre." }
            div class="actions" {
                a class="button ghost" href="/courses" { "Vos courses" }
                a class="button" href="/courses/nouvelle" { "Ajouter une course" }
            }
        }
        @if entries.is_empty() {
            section {
                p class="muted" {
                    "Rien au planning. Renseignez la date de depart, la prise de dossard, "
                    "l'arrivee a l'hotel ou l'echeance d'un element de suivi pour le remplir."
                }
            }
        } @else {
            @for (month, items) in &months {
                section {
                    h2 { (month) }
                    ul class="agenda" {
                        @for entry in items {
                            li class={ "agenda-item " (entry.kind) } {
                                span class="agenda-day" {
                                    (format_day_month(entry.at_ms))
                                    @if let Some(time) = format_time_short(entry.at_ms) {
                                        span class="agenda-time" { (time) }
                                    }
                                }
                                div class="agenda-body" {
                                    span class="agenda-kind" { (agenda_label(entry.kind)) }
                                    a class="agenda-title" href={ "/courses/" (entry.race_id) } { (entry.title) }
                                    @if let Some(detail) = &entry.detail {
                                        span class="muted" { (detail) }
                                    }
                                }
                                span class="agenda-when muted" { (countdown_label(Some(entry.at_ms), now)) }
                            }
                        }
                    }
                }
            }
        }
    };
    Ok(page(layout("Planning", "planning", Some(&user), content)))
}

async fn race_new_page(OptionalUser(user): OptionalUser) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    Ok(race_form_response(
        &user,
        "Nouvelle course",
        "/courses/nouvelle",
        &RaceForm::default(),
        None,
        "Ajouter la course",
        StatusCode::OK,
    ))
}

async fn race_create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Form(form): Form<RaceForm>,
) -> AppResult<Response> {
    let input = match form.to_input().and_then(|input| {
        input.validate()?;
        Ok(input)
    }) {
        Ok(input) => input,
        Err(message) => {
            return Ok(race_form_response(
                &user,
                "Nouvelle course",
                "/courses/nouvelle",
                &form,
                Some(&message),
                "Ajouter la course",
                StatusCode::BAD_REQUEST,
            ))
        }
    };
    let race = crate::db::insert_race(&state.pool, &user.id, &input, state.now_ms()).await?;
    tracing::info!(user = %user.email, race = %race.id, "course ajoutee");
    Ok(Redirect::to(&format!("/courses/{}", race.id)).into_response())
}

async fn race_edit_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let race = crate::db::get_race(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(race_form_response(
        &user,
        "Modifier la course",
        &format!("/courses/{id}/modifier"),
        &RaceForm::from_race(&race),
        None,
        "Enregistrer",
        StatusCode::OK,
    ))
}

async fn race_update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Form(form): Form<RaceForm>,
) -> AppResult<Response> {
    let action = format!("/courses/{id}/modifier");
    let input = match form.to_input().and_then(|input| {
        input.validate()?;
        Ok(input)
    }) {
        Ok(input) => input,
        Err(message) => {
            return Ok(race_form_response(
                &user,
                "Modifier la course",
                &action,
                &form,
                Some(&message),
                "Enregistrer",
                StatusCode::BAD_REQUEST,
            ))
        }
    };
    let updated =
        crate::db::update_race(&state.pool, &user.id, &id, &input, state.now_ms()).await?;
    if updated.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(Redirect::to(&format!("/courses/{id}")).into_response())
}

async fn race_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    if !crate::db::delete_race(&state.pool, &user.id, &id).await? {
        return Err(AppError::NotFound);
    }
    tracing::info!(user = %user.email, race = %id, "course supprimee");
    Ok(Redirect::to("/courses").into_response())
}

/// Page d'import d'une ancienne course (export Strava ou Garmin).
async fn race_import_page(OptionalUser(user): OptionalUser) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    Ok(race_import_response(&user, None, StatusCode::OK))
}

/// Formulaire d'import, avec le message d'erreur eventuel.
fn race_import_response(user: &User, error: Option<&str>, status: StatusCode) -> Response {
    let content = html! {
        section class="hero" {
            h1 { "Importer une ancienne course" }
            p class="muted" {
                "Deposez un export Strava ou Garmin (GPX ou TCX) : la course rejoint "
                "\"deja courues\" avec sa date, sa distance, ses temps et son denivele. "
                "Elle sert ensuite de course de reference."
            }
        }
        section class="panel form" {
            @if let Some(error) = error {
                p class="alert" { (error) }
            }
            form method="post" action="/courses/importer" enctype="multipart/form-data" {
                label for="file" { "Fichier exporte (GPX ou TCX)" }
                input type="file" id="file" name="file" accept=".gpx,.tcx,application/gpx+xml,application/xml" required;
                p class="muted tiny" {
                    "Strava : ouvrez la course, puis \"Exporter le GPX\". "
                    "Garmin Connect : \"Exporter au format GPX\" ou \"TCX\"."
                }
                div class="actions" {
                    button type="submit" { "Importer la course" }
                    a class="button ghost" href="/courses" { "Annuler" }
                }
            }
        }
        section class="panel" {
            h2 { "Ce qui est repris du fichier" }
            ul class="muted tiny" {
                li { "Le nom de la trace, sinon le nom du fichier." }
                li { "La date et l'heure de depart." }
                li { "La distance, le temps en mouvement, le temps ecoule et le denivele positif." }
                li { "La trace, reexportable en GPX depuis la fiche." }
            }
            p class="muted tiny" {
                "Le reste de la fiche reste vide : dossard, notes et objectif se "
                "completent a la main, comme pour toute course."
            }
        }
    };
    (
        status,
        page(layout(
            "Importer une course",
            "courses",
            Some(user),
            content,
        )),
    )
        .into_response()
}

/// Traite le fichier depose et cree la course de reference.
///
/// Une erreur revient sur la page avec son message : c'est un formulaire de
/// navigateur, pas un appel d'API.
async fn race_import_submit(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    multipart: Multipart,
) -> AppResult<Response> {
    let imported = async {
        let (filename, bytes) = crate::race_import::collect_race_file(multipart).await?;
        crate::race_import::import_uploaded_race(&state, &user.id, &filename, &bytes).await
    }
    .await;
    match imported {
        Ok(race) => {
            tracing::info!(user = %user.email, race = %race.id, "course importee depuis le navigateur");
            Ok(Redirect::to(&format!("/courses/{}?ok=course_importee", race.id)).into_response())
        }
        Err(error) => Ok(race_import_response(
            &user,
            Some(&error.to_string()),
            error.status(),
        )),
    }
}

/// Trace GPX conservee d'une course importee.
async fn race_track_gpx(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let Some((gpx, _points)) = crate::db::get_race_track(&state.pool, &user.id, &id).await? else {
        return Err(AppError::NotFound);
    };
    Response::builder()
        .header(header::CONTENT_TYPE, "application/gpx+xml; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{id}.gpx\""),
        )
        .body(Body::from(gpx))
        .map_err(|error| AppError::internal(error.to_string()))
}

/// Parametres de la fiche d'une course.
#[derive(Debug, Deserialize)]
struct RaceQuery {
    /// Code de succes renvoye par un import.
    #[serde(default)]
    ok: Option<String>,
}

async fn race_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<String>,
    Query(query): Query<RaceQuery>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let race = crate::db::get_race(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let tasks = crate::db::list_race_tasks(&state.pool, &user.id, &race.id).await?;
    let now = state.now_ms();
    let done = tasks.iter().filter(|task| task.done).count();

    // Valeurs derivees calculees avant le rendu : aucune reference a un temporaire.
    let start_line = race.start_at_ms.map(format_datetime_short);
    let distance_line = race
        .distance_m
        .map(|meters| format_distance(meters, UnitSystem::Metric));
    let goal_line = race.goal_time_s.map(mpacer_core::units::format_duration);
    let pickup_line = race.bib_pickup_at_ms.map(format_datetime_short);
    let check_in_line = race.hotel_check_in_ms.map(format_datetime_short);
    let check_out_line = race.hotel_check_out_ms.map(format_datetime_short);
    let map = map_url(&race);

    // Course de reference : origine et mesures viennent de l'export importe.
    let source = race
        .source
        .as_deref()
        .and_then(mpacer_core::race_import::ImportSource::from_code);
    let track = if source.is_some() {
        crate::db::get_race_track(&state.pool, &user.id, &race.id).await?
    } else {
        None
    };
    let track_line = track.as_ref().map(|(_, points)| format!("{points} points"));
    let moving_line = race.moving_time_s.map(mpacer_core::units::format_duration);
    let elapsed_line = race.elapsed_time_s.map(mpacer_core::units::format_duration);
    let pace_line = match (race.moving_time_s, race.distance_m) {
        (Some(moving_s), Some(distance_m)) if moving_s > 0.0 && distance_m > 0.0 => Some(format!(
            "{} /km",
            format_pace(Some(moving_s / (distance_m / 1000.0)))
        )),
        _ => None,
    };
    let gain_line = race
        .elevation_gain_m
        .filter(|gain| *gain > 0.5)
        .map(|gain| format!("{gain:.0} m"));
    let imported_ok = query.ok.as_deref() == Some("course_importee");

    let content = html! {
        section class="hero" {
            div class="race-title" {
                h1 { (race.name) }
                @if race.is_reference {
                    span class="pill brand" { "Course de reference" }
                }
                span class="countdown" data-at=(race.start_at_ms.unwrap_or(0)) {
                    span class="dot" {}
                    (countdown_label(race.start_at_ms, now))
                }
            }
            p class="muted" { (race_summary_line(&race)) }
            div class="actions" {
                a class="button" href={ "/courses/" (race.id) "/modifier" } { "Modifier la fiche" }
                @if let Some(url) = &map {
                    a class="button ghost" href=(url) target="_blank" rel="noopener noreferrer" { "Voir sur la carte" }
                }
                form method="post" action={ "/courses/" (race.id) "/supprimer" }
                     data-confirm="Supprimer definitivement cette course et son suivi ?" {
                    button class="ghost danger" type="submit" { "Supprimer" }
                }
            }
        }
        @if imported_ok {
            p class="alert ok" {
                "Course importee : elle est enregistree avec vos courses deja courues, "
                "comme course de reference."
            }
        }
        @if let Some(source) = source {
            section class="panel" {
                div class="section-head" {
                    h2 { "Course de reference" }
                    span class="pill brand" { (source.label()) }
                }
                (info_row("Origine du fichier", Some(source.label())))
                (info_row("Temps en mouvement", moving_line.as_deref()))
                (info_row("Temps ecoule", elapsed_line.as_deref()))
                (info_row("Allure moyenne", pace_line.as_deref()))
                (info_row("Denivele positif", gain_line.as_deref()))
                (info_row("Trace conservee", track_line.as_deref()))
                @if track.is_some() {
                    div class="actions" {
                        a class="button ghost" href={ "/courses/" (race.id) "/trace.gpx" } {
                            span class="icon icon-export" {}
                            "Telecharger le GPX"
                        }
                    }
                }
            }
        }
        section class="panel" {
            h2 { "La course" }
            (info_row("Date et horaire de depart", start_line.as_deref()))
            (info_row("Lieu de depart", race.start_location.as_deref().or(race.location.as_deref())))
            (info_row("Ville / region", race.location.as_deref()))
            (info_row("Distance", distance_line.as_deref()))
            (info_row("Discipline", race.discipline.as_deref()))
            (info_row("Objectif de temps", goal_line.as_deref()))
            (link_row("Inscription", race.registration_url.as_deref(), "Page d'inscription"))
            (link_row("Site officiel", race.website_url.as_deref(), "Site de la course"))
            @match &map {
                Some(url) => (link_row("Carte", Some(url.as_str()), "Ouvrir dans OpenStreetMap")),
                None => (info_row("Carte", None)),
            }
        }
        section class="panel" {
            h2 { "Dossard" }
            (info_row("Numero de dossard", race.bib_number.as_deref()))
            (info_row("Rendez-vous de prise de dossard", pickup_line.as_deref()))
            (info_row("Lieu de retrait", race.bib_pickup_location.as_deref()))
        }
        section class="panel live" {
            h2 { "Suivi en direct" }
            @if let Some(url) = &race.live_url {
                p {
                    a class="button" href=(url) target="_blank" rel="noopener noreferrer" { "Ouvrir le live" }
                    span class="muted" { " A partager avec vos proches le jour de la course." }
                }
            } @else {
                (info_row("Lien du live", None))
            }
        }
        section class="panel" {
            div class="section-head" {
                h2 { "Hebergement" }
                @if race.hotel_booked {
                    span class="pill on" { "reserve" }
                } @else if race.hotel_name.is_some() {
                    span class="pill off" { "a confirmer" }
                }
            }
            (info_row("Hotel", race.hotel_name.as_deref()))
            (info_row("Adresse", race.hotel_address.as_deref()))
            (info_row("Telephone", race.hotel_phone.as_deref()))
            (link_row("Reservation", race.hotel_url.as_deref(), "Ouvrir la reservation"))
            (info_row("Arrivee", check_in_line.as_deref()))
            (info_row("Depart", check_out_line.as_deref()))
        }
        @if let Some(notes) = &race.lodging_notes {
            section class="panel" {
                h2 { "Autres solutions pour dormir" }
                p class="notes" { (notes) }
            }
        }
        @if let Some(notes) = &race.nutrition_notes {
            section class="panel" {
                h2 { "Nutrition et ravitaillement" }
                p class="notes" { (notes) }
            }
        }
        @if let Some(notes) = &race.important_info {
            section class="panel" {
                h2 { "Informations importantes pour la course" }
                p class="notes" { (notes) }
            }
        }
        @if let Some(notes) = &race.notes {
            section class="panel" {
                h2 { "Autres informations" }
                p class="notes" { (notes) }
            }
        }
        section class="panel" {
            div class="section-head" {
                h2 { "Suivi des elements importants" }
                span class="muted" { (done) " / " (tasks.len()) " prets" }
            }
            @if tasks.is_empty() {
                p class="muted" { "Aucun element de suivi. Ajoutez ce qu'il vous reste a preparer." }
            } @else {
                ul class="tasks" {
                    @for task in &tasks {
                        @let next_done = if task.done { "0" } else { "1" };
                        @let css = if task.done { "task done" } else { "task" };
                        @let mark = if task.done { "x" } else { "-" };
                        li class=(css) {
                            form method="post" action={ "/courses/" (race.id) "/suivi/" (task.id) } {
                                input type="hidden" name="done" value=(next_done);
                                button class="tick" type="submit" title="Cocher ou decocher" { (mark) }
                            }
                            span class="task-label" { (task.label) }
                            @if let Some(due) = task.due_at_ms {
                                span class="task-due muted" { "echeance " (format_day_month(due)) }
                            }
                            form method="post" action={ "/courses/" (race.id) "/suivi/" (task.id) "/supprimer" } {
                                button class="ghost danger small" type="submit" { "Retirer" }
                            }
                        }
                    }
                }
            }
            form class="race-form inline" method="post" action={ "/courses/" (race.id) "/suivi" } {
                input type="text" name="label" placeholder="Ajouter un element a preparer" required maxlength="200";
                input type="date" name="due" title="Echeance (facultative)";
                button type="submit" { "Ajouter" }
            }
        }
    };
    Ok(page(layout(&race.name, "courses", Some(&user), content)))
}

#[derive(Debug, Deserialize)]
struct TaskForm {
    #[serde(default)]
    label: String,
    #[serde(default)]
    due: String,
}

async fn race_task_create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Form(form): Form<TaskForm>,
) -> AppResult<Response> {
    let label = form.label.trim();
    if label.is_empty() {
        return Err(AppError::bad_request(
            "l'element de suivi ne peut pas etre vide",
        ));
    }
    if label.chars().count() > 200 {
        return Err(AppError::bad_request(
            "l'element de suivi est trop long (200 caracteres maximum)",
        ));
    }
    let due_at_ms = parse_local_datetime(&form.due, "").map_err(AppError::bad_request)?;
    let created =
        crate::db::insert_race_task(&state.pool, &user.id, &id, label, due_at_ms, state.now_ms())
            .await?;
    if created.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(Redirect::to(&format!("/courses/{id}")).into_response())
}

#[derive(Debug, Deserialize)]
struct TaskToggleForm {
    #[serde(default)]
    done: String,
}

async fn race_task_toggle(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((id, task)): Path<(String, String)>,
    Form(form): Form<TaskToggleForm>,
) -> AppResult<Response> {
    let done = form.done.trim() == "1";
    let updated =
        crate::db::set_race_task_done(&state.pool, &user.id, &id, &task, done, state.now_ms())
            .await?;
    if !updated {
        return Err(AppError::NotFound);
    }
    Ok(Redirect::to(&format!("/courses/{id}")).into_response())
}

async fn race_task_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((id, task)): Path<(String, String)>,
) -> AppResult<Response> {
    if !crate::db::delete_race_task(&state.pool, &user.id, &id, &task).await? {
        return Err(AppError::NotFound);
    }
    Ok(Redirect::to(&format!("/courses/{id}")).into_response())
}

// ------------------------------------------------------------------ musique
//
// La page /music reprend les six blocs de docs/11 :
//   1. source des playlists : Spotify **ou** Deezer (connexion, recherche,
//      playlists du compte, import) ;
//   2. playlists preparees (source, titres, duree, BPM cible, manifeste) ;
//   3. titres de la playlist selectionnee (tap-tempo ou saisie manuelle) ;
//   4. fichiers MP3 a mettre en place (un nom attendu par piste, export .txt) ;
//   5. transfert vers la montre par USB ;
//   6. la playlist couvre-t-elle la course ?
//
// Le serveur ne stocke aucun audio : il publie les fiches, la liste des MP3 a
// rassembler et le manifeste de transfert, que l'outil local `mpacer-music`
// consomme pour apporter les fichiers (dossier du disque ou fichiers pousses
// dans l'application locale) puis les copier sur la montre (adb push).

/// Parametres de la page musique.
#[derive(Debug, Deserialize)]
struct MusicQuery {
    /// Playlist affichee dans le bloc 3.
    #[serde(default)]
    playlist: Option<String>,
    /// Terme de recherche (Spotify ou Deezer).
    #[serde(default)]
    q: Option<String>,
    /// Source interrogee : `spotify` (defaut) ou `deezer`.
    #[serde(default)]
    source: Option<String>,
    /// `mes` : liste les playlists du compte lie au lieu de chercher.
    #[serde(default)]
    vue: Option<String>,
    /// Bloc 6 : course dont on verifie la couverture.
    #[serde(default)]
    race: Option<String>,
    /// Bloc 6 : distance saisie (km), prioritaire sur la fiche de course.
    #[serde(default)]
    distance_km: Option<String>,
    /// Bloc 6 : allure (m:ss au km, ou secondes).
    #[serde(default)]
    allure: Option<String>,
    /// Bloc 6 : temps vise (h:mm:ss).
    #[serde(default)]
    temps: Option<String>,
    #[serde(default)]
    erreur: Option<String>,
    #[serde(default)]
    ok: Option<String>,
}

/// Traduit un code d'erreur de la page musique en message lisible.
fn music_error_message(code: &str) -> String {
    match code {
        "spotify_non_configure" => "Spotify n'est pas configure sur ce service : renseignez MPACER_SPOTIFY_CLIENT_ID et MPACER_SPOTIFY_CLIENT_SECRET.".to_string(),
        "spotify_refuse" => "Spotify a refuse la demande (identifiants invalides, redirection non autorisee ou endpoint restreint).".to_string(),
        "spotify_ref_invalide" => "La reference de playlist Spotify est illisible : collez un lien open.spotify.com ou un identifiant.".to_string(),
        "spotify_non_connecte" => "Connectez votre compte Spotify avant d'importer une playlist.".to_string(),
        "deezer_non_configure" => "Deezer n'est pas configure sur ce service : renseignez MPACER_DEEZER_APP_ID et MPACER_DEEZER_APP_SECRET.".to_string(),
        "deezer_refuse" => "Deezer a refuse la demande (identifiants invalides, redirection non autorisee ou jeton expire).".to_string(),
        "deezer_ref_invalide" => "La reference de playlist Deezer est illisible : collez un lien deezer.com ou un identifiant numerique.".to_string(),
        "deezer_non_connecte" => "Connectez votre compte Deezer avant de recuperer une playlist.".to_string(),
        "source_inconnue" => "Source musicale inconnue : choisissez Spotify ou Deezer.".to_string(),
        "playlist_inconnue" => "Cette playlist n'existe plus.".to_string(),
        "nom_invalide" => "Le nom de la playlist ne peut pas etre vide.".to_string(),
        "bpm_invalide" => "Le BPM doit etre un nombre entre 30 et 300.".to_string(),
        other => format!("Operation impossible ({other})."),
    }
}

/// Traduit un code de succes de la page musique en message lisible.
fn music_ok_message(code: &str) -> String {
    match code {
        "spotify_connecte" => "Compte Spotify connecte.".to_string(),
        "spotify_deconnecte" => "Compte Spotify deconnecte.".to_string(),
        "deezer_connecte" => "Compte Deezer connecte.".to_string(),
        "deezer_deconnecte" => "Compte Deezer deconnecte.".to_string(),
        "playlist_importee" => "Playlist importee.".to_string(),
        "bpm_enregistre" => "BPM enregistre.".to_string(),
        "playlist_renommee" => "Playlist renommee.".to_string(),
        "playlist_supprimee" => "Playlist supprimee.".to_string(),
        other => format!("Operation effectuee ({other})."),
    }
}

/// Source musicale normalisee depuis la chaine de requete.
///
/// Deezer est la seule valeur alternative : tout le reste retombe sur Spotify,
/// pour que `/?q=...` sans `source` continue de fonctionner comme avant.
fn normalize_source(source: Option<&str>) -> &'static str {
    match source
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("deezer") => "deezer",
        _ => "spotify",
    }
}

/// Libelle affiche d'une source.
fn source_label(source: &str) -> &'static str {
    match source {
        "deezer" => "Deezer",
        _ => "Spotify",
    }
}

/// Message affiche quand la source n'est pas configuree sur ce service.
fn source_unconfigured_message(source: &str) -> &'static str {
    match source {
        "deezer" => {
            "Deezer n'est pas configure sur ce service : renseignez MPACER_DEEZER_APP_ID et MPACER_DEEZER_APP_SECRET."
        }
        _ => {
            "Spotify n'est pas configure sur ce service : renseignez MPACER_SPOTIFY_CLIENT_ID et MPACER_SPOTIFY_CLIENT_SECRET."
        }
    }
}

/// Rappel affiche avant la connexion du compte.
fn source_connect_hint(source: &str) -> &'static str {
    match source {
        "deezer" => {
            "Connectez Deezer pour retrouver vos playlists : seules les metadonnees (titres, durees) sont stockees. Deezer ne fournit pas de tempo : le BPM se complete par la balise du MP3, le tap ou la saisie."
        }
        _ => {
            "Connectez Spotify pour retrouver vos playlists : seules les metadonnees (titres, durees, BPM) sont stockees. L'audio est copie sur la montre par USB, depuis les fichiers de votre ordinateur."
        }
    }
}

/// Vue commune d'une playlist Spotify (voir `SourcePlaylist`).
fn spotify_source_playlist(item: crate::spotify::PlaylistRef) -> SourcePlaylist {
    SourcePlaylist {
        source: "spotify".to_string(),
        id: item.id,
        name: item.name,
        track_count: item.track_count,
        cover_url: item.cover_url,
        owner: item.owner,
    }
}

/// Vue commune d'une playlist Deezer (voir `SourcePlaylist`).
fn deezer_source_playlist(item: crate::deezer::PlaylistRef) -> SourcePlaylist {
    SourcePlaylist {
        source: "deezer".to_string(),
        id: item.id,
        name: item.name,
        track_count: item.track_count,
        cover_url: item.cover_url,
        owner: item.owner,
    }
}

/// Panneau d'une source musicale : connexion, recherche, playlists du compte.
///
/// Les resultats affiches appartiennent a la source interrogee (`active`) : le
/// panneau de l'autre service reste visible pour changer de source.
fn source_panel(
    source: &'static str,
    ready: bool,
    connected_name: Option<String>,
    results: &[SourcePlaylist],
    active: bool,
    term: &str,
    error: Option<&str>,
) -> Markup {
    let label = source_label(source);
    html! {
        div class="panel" {
            h3 { (label) }
            @if active {
                @if let Some(error) = error {
                    p class="alert" {
                        span class="icon icon-alert" {}
                        span { (error) }
                    }
                }
            }
            @if !ready {
                p class="muted" { (source_unconfigured_message(source)) }
            } @else if let Some(name) = connected_name {
                p class="split" {
                    span class="pill on" { "Connecte" }
                    span class="muted" { (name) }
                }
                form method="get" action="/music/search" class="music-search" {
                    input type="hidden" name="source" value=(source);
                    input type="text" name="q" value=(term) placeholder="running";
                    button type="submit" { "Chercher" }
                }
                div class="actions" {
                    a class="button small ghost" href={ "/music/search?source=" (source) "&vue=mes" } {
                        "Mes playlists"
                    }
                }
                @if active && !results.is_empty() {
                    ul class="music-results" {
                        @for result in results {
                            li {
                                div class="music-result" {
                                    strong { (result.name) }
                                    span class="muted" { (result.track_count) " titres" }
                                    @if let Some(owner) = &result.owner {
                                        span class="tiny muted" { (owner) }
                                    }
                                }
                                form method="post" action="/music/import" class="music-import" {
                                    input type="hidden" name="source" value=(source);
                                    input type="hidden" name="ref" value=(result.id);
                                    input type="text" name="target_bpm" inputmode="numeric" placeholder="BPM cible (auto)";
                                    button class="small" type="submit" { "Importer" }
                                }
                            }
                        }
                    }
                }
                @if active && results.is_empty() {
                    p class="muted" { "Aucune playlist trouvee." }
                }
                form method="post" action={ "/music/" (source) "/disconnect" } {
                    button class="ghost" type="submit" { "Deconnecter" }
                }
            } @else {
                p class="muted" { (source_connect_hint(source)) }
                div class="actions" {
                    a class="button" href={ "/auth/" (source) } { "Connecter " (label) }
                }
            }
        }
    }
}

/// Un fichier MP3 a mettre en place pour une playlist, avec le nom attendu.
struct PreparedFile {
    position: u32,
    title: String,
    artist: Option<String>,
    file_name: String,
}

/// Liste des fichiers MP3 a preparer pour une playlist selectionnee.
///
/// Le nom est exactement celui que `mpacer-music` ecrit sur la montre
/// (`01 - Artiste - Titre.mp3`) : l'utilisateur sait donc quoi telecharger ou
/// convertir, et l'appariement du dossier local retrouve le fichier.
fn prepared_files(tracks: &[MusicTrack]) -> Vec<PreparedFile> {
    tracks
        .iter()
        .map(|track| {
            let wanted = WantedTrack {
                id: track.id.clone(),
                position: track.position.max(0) as u32,
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                duration_s: track.duration_s,
                bpm: track.bpm,
                file: None,
                size_bytes: None,
            };
            PreparedFile {
                position: wanted.position,
                title: track.title.clone(),
                artist: track.artist.clone(),
                file_name: suggested_file_name(&wanted, "mp3"),
            }
        })
        .collect()
}

/// Contenu texte de la liste des fichiers a preparer (piece jointe `.txt`).
fn prepared_files_text(playlist_name: &str, files: &[PreparedFile]) -> String {
    let mut text = String::new();
    let _ = writeln!(
        text,
        "# {playlist_name} : {} fichier(s) MP3 a mettre en place",
        files.len()
    );
    let _ = writeln!(
        text,
        "# un nom par ligne, tel qu'attendu sur la montre (mpacer-music apparie le dossier)"
    );
    for file in files {
        let _ = writeln!(text, "{}", file.file_name);
    }
    text
}

/// Commande de transfert montree dans le bloc 5 (exemple de dossier).
///
/// Seul le nom du manifeste compte : l'utilisateur remplace le dossier par le
/// sien, celui ou sont ranges ses fichiers audio.
fn transfer_command(manifest_name: &str) -> String {
    format!("mpacer-music transfer --manifest {manifest_name} --folder \"D:\\Musique\\Course\"")
}

/// Analyse une allure saisie : "5:00" (minutes:secondes par km), "5:00/km" ou
/// un nombre de secondes. `None` si la valeur est vide ou illisible.
fn parse_pace_input(text: &str) -> Option<f64> {
    let cleaned = text
        .trim()
        .trim_end_matches("/km")
        .trim_end_matches("km")
        .trim();
    if cleaned.is_empty() {
        return None;
    }
    let seconds = match cleaned.split_once(':') {
        Some((minutes, seconds)) => {
            let minutes: f64 = minutes.trim().replace(',', ".").parse().ok()?;
            let seconds: f64 = seconds.trim().replace(',', ".").parse().ok()?;
            minutes * 60.0 + seconds
        }
        None => cleaned.replace(',', ".").parse().ok()?,
    };
    (seconds.is_finite() && seconds > 0.0).then_some(seconds)
}

/// Duree en mots, pour la phrase du verdict ("1 h 05", "45 min").
fn human_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "0 min".to_string();
    }
    let total = seconds.round() as i64;
    let (hours, minutes) = (total / 3600, (total % 3600) / 60);
    match (hours, minutes) {
        (0, minutes) => format!("{} min", minutes.max(1)),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes:02}"),
    }
}

/// Duree signee en mots ("+7 min", "-3 min"), pour la marge.
fn signed_minutes(seconds: f64) -> String {
    if !seconds.is_finite() || seconds.abs() < 30.0 {
        return "0 min".to_string();
    }
    format!(
        "{}{}",
        if seconds > 0.0 { "+" } else { "-" },
        human_duration(seconds.abs())
    )
}

/// Bloc 5 pret a afficher : verdict, jauges et valeurs du formulaire.
struct CoverageView {
    /// `coverage-ok` | `coverage-warn` | `coverage-bad`.
    verdict_class: &'static str,
    verdict: String,
    playlist_duration: String,
    race_duration: String,
    margin: String,
    tracks_needed: String,
    average_bpm: String,
    target_bpm: String,
    tempo: String,
    gauge_percent: f64,
    bpm_marker_percent: Option<f64>,
    race_id: String,
    distance_km: String,
    allure: String,
    temps: String,
}

/// Construit le bloc 5 : la playlist couvre-t-elle la course visee ?
///
/// Les valeurs saisies dans le formulaire priment sur la fiche de course ; une
/// duree inconnue reste inconnue, le verdict n'est jamais invente.
fn coverage_view(tracks: &[MusicTrack], races: &[Race], query: &MusicQuery) -> CoverageView {
    let race = query
        .race
        .as_deref()
        .and_then(|id| races.iter().find(|race| race.id == id));

    let distance_km = query
        .distance_km
        .clone()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| {
            race.and_then(|race| race.distance_m)
                .map(|meters| format!("{:.2}", meters / 1000.0))
                .unwrap_or_default()
        });
    let distance_m = distance_km
        .trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|km| km.is_finite() && *km > 0.0)
        .map(|km| km * 1000.0)
        .or_else(|| race.and_then(|race| race.distance_m));

    let temps = query
        .temps
        .clone()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| {
            race.and_then(|race| race.goal_time_s)
                .map(format_duration)
                .unwrap_or_default()
        });
    let target_time_s = parse_duration(&temps).ok().flatten();
    let allure = query.allure.clone().unwrap_or_default();
    let pace_s_per_km = parse_pace_input(&allure);

    let estimated_race_s = race_duration_s(distance_m, target_time_s, pace_s_per_km);
    let core_tracks: Vec<MusicTrackCore> = tracks
        .iter()
        .map(|track| MusicTrackCore {
            id: track.id.clone(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            duration_s: track.duration_s.unwrap_or(0.0),
            bpm: track.bpm,
            position: track.position.max(0) as u32,
        })
        .collect();
    let config = MusicConfig::default();
    let coverage = music_coverage(
        &core_tracks,
        estimated_race_s,
        pace_s_per_km,
        &config,
        MUSIC_MARGIN_RATIO,
    );

    let verdict_class = match coverage.sufficient {
        Some(true) => "coverage-ok",
        Some(false) => "coverage-bad",
        None => "coverage-warn",
    };
    let verdict = match (
        coverage.sufficient,
        coverage.race_duration_s,
        coverage.margin_s,
    ) {
        (Some(true), Some(race_s), Some(margin)) => format!(
            "{} de musique pour {} de course : OK, +{} de marge",
            human_duration(coverage.playlist_duration_s),
            human_duration(race_s),
            human_duration(margin)
        ),
        (Some(false), Some(race_s), Some(margin)) => format!(
            "{} de musique pour {} de course : insuffisant, il manque {}",
            human_duration(coverage.playlist_duration_s),
            human_duration(race_s),
            human_duration(margin.abs())
        ),
        _ => format!(
            "{} de musique : duree de course inconnue, impossible de conclure",
            human_duration(coverage.playlist_duration_s)
        ),
    };

    let tempo = match (coverage.average_bpm, coverage.target_bpm) {
        (Some(average), Some(target)) => {
            let delta = coverage.bpm_delta.unwrap_or(average - target);
            if coverage.tempo_ok == Some(true) {
                format!("BPM moyen {average:.0} vs cible {target:.0} : tempo coherent.")
            } else if delta < 0.0 {
                format!(
                    "BPM moyen {average:.0} vs cible {target:.0} : tempo trop lent (il manque {:.0} bpm).",
                    delta.abs()
                )
            } else {
                format!(
                    "BPM moyen {average:.0} vs cible {target:.0} : tempo trop rapide ({delta:.0} bpm de trop)."
                )
            }
        }
        (None, _) => "BPM moyen inconnu : renseignez le tempo des titres.".to_string(),
        (_, None) => "BPM cible inconnu : renseignez une allure pour l'obtenir.".to_string(),
    };

    let gauge_percent = match coverage.race_duration_s {
        Some(race_s) if race_s > 0.0 => {
            let target = race_s * (1.0 + MUSIC_MARGIN_RATIO);
            (coverage.playlist_duration_s / target * 100.0).clamp(0.0, 100.0)
        }
        _ => 0.0,
    };
    let bpm_marker_percent = coverage.average_bpm.map(|bpm| {
        let span = (config.max_bpm - config.min_bpm).max(1.0);
        ((bpm - config.min_bpm) / span * 100.0).clamp(2.0, 98.0)
    });

    CoverageView {
        verdict_class,
        verdict,
        playlist_duration: format_duration(coverage.playlist_duration_s),
        race_duration: coverage
            .race_duration_s
            .map(format_duration)
            .unwrap_or_else(|| "-".to_string()),
        margin: coverage
            .margin_s
            .map(signed_minutes)
            .unwrap_or_else(|| "-".to_string()),
        tracks_needed: coverage
            .tracks_needed
            .map(|needed| format!("{needed} titres"))
            .unwrap_or_else(|| "-".to_string()),
        average_bpm: coverage
            .average_bpm
            .map(|bpm| format!("{bpm:.0}"))
            .unwrap_or_else(|| "-".to_string()),
        target_bpm: coverage
            .target_bpm
            .map(|bpm| format!("{bpm:.0}"))
            .unwrap_or_else(|| "-".to_string()),
        tempo,
        gauge_percent,
        bpm_marker_percent,
        race_id: race.map(|race| race.id.clone()).unwrap_or_default(),
        distance_km,
        allure,
        temps,
    }
}

/// Jeton d'acces Spotify valide, rafraichi si necessaire.
///
/// Un rafraichissement refuse n'est pas une erreur fatale : la page invite
/// simplement a reconnecter le compte.
async fn spotify_access_token(state: &AppState, user_id: &str) -> AppResult<Option<String>> {
    let Some(account) = crate::db::get_spotify_account(&state.pool, user_id).await? else {
        return Ok(None);
    };
    // 60 s de marge : une requete partie juste avant l'expiration ne doit pas
    // echouer sur un jeton perime.
    if account.expires_at_ms > state.now_ms() + 60_000 {
        return Ok(Some(account.access_token));
    }
    let Some(refresh) = account.refresh_token.as_deref() else {
        tracing::warn!(utilisateur = %user_id, "jeton Spotify expire sans refresh_token");
        return Ok(None);
    };
    match crate::spotify::refresh_token(&state.http, &state.config, refresh).await {
        Ok(token) => {
            let expires_at_ms = state.now_ms() + token.expires_in.unwrap_or(3600) * 1000;
            crate::db::update_spotify_access_token(
                &state.pool,
                user_id,
                &token.access_token,
                expires_at_ms,
            )
            .await?;
            Ok(Some(token.access_token))
        }
        Err(error) => {
            tracing::warn!(error = %error, "rafraichissement du jeton Spotify refuse");
            Ok(None)
        }
    }
}

// L'ecriture des fichiers audio et le nettoyage vivent dans `crate::media` :
// le navigateur (cookie) et l'application compagnon (jeton d'appareil)
// televersent exactement par le meme chemin de code.

/// Page /music : sources, playlists, titres, MP3 a preparer et couverture.
async fn music_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(query): Query<MusicQuery>,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let spotify_ready = state.config.spotify_configured();
    let deezer_ready = state.config.deezer_configured();
    let account = crate::db::get_spotify_account(&state.pool, &user.id).await?;
    let deezer_account = crate::db::get_deezer_account(&state.pool, &user.id).await?;
    let playlists = crate::db::list_music_playlist_summaries(&state.pool, &user.id).await?;

    // Playlist affichee : celle demandee, sinon la plus recemment modifiee.
    let selected_id = query
        .playlist
        .clone()
        .filter(|id| playlists.iter().any(|playlist| &playlist.id == id))
        .or_else(|| playlists.first().map(|playlist| playlist.id.clone()));
    let selected_playlist = match &selected_id {
        Some(id) => crate::db::get_music_playlist(&state.pool, &user.id, id).await?,
        None => None,
    };
    let tracks = match &selected_id {
        Some(id) => crate::db::list_music_tracks(&state.pool, &user.id, id).await?,
        None => Vec::new(),
    };
    let selected_summary = selected_id
        .as_ref()
        .and_then(|id| playlists.iter().find(|playlist| &playlist.id == id));

    // Source interrogee : recherche (terme) ou playlists du compte (`vue=mes`).
    // Aucun appel reseau quand la source n'est pas configuree.
    let source = normalize_source(query.source.as_deref());
    let term = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_string);
    let mine = query.vue.as_deref().map(str::trim) == Some("mes");

    let mut source_playlists: Vec<SourcePlaylist> = Vec::new();
    let mut search_error: Option<String> = None;
    if term.is_some() || mine {
        let ready = if source == "deezer" {
            deezer_ready
        } else {
            spotify_ready
        };
        let connected = if source == "deezer" {
            deezer_account.is_some()
        } else {
            account.is_some()
        };
        if !ready {
            search_error = Some(source_unconfigured_message(source).to_string());
        } else if !connected {
            search_error = Some(format!(
                "Connectez votre compte {} pour continuer.",
                source_label(source)
            ));
        } else {
            let token = if source == "deezer" {
                deezer_access_token(&state, &user.id).await?
            } else {
                spotify_access_token(&state, &user.id).await?
            };
            match token {
                Some(token) => {
                    let outcome: AppResult<Vec<SourcePlaylist>> = if source == "deezer" {
                        if mine {
                            crate::deezer::list_user_playlists(&state.http, &token)
                                .await
                                .map(|items| {
                                    items.into_iter().map(deezer_source_playlist).collect()
                                })
                        } else {
                            crate::deezer::search_playlists(
                                &state.http,
                                &token,
                                term.as_deref().unwrap_or_default(),
                            )
                            .await
                            .map(|items| items.into_iter().map(deezer_source_playlist).collect())
                        }
                    } else if mine {
                        crate::spotify::list_user_playlists(&state.http, &token)
                            .await
                            .map(|items| items.into_iter().map(spotify_source_playlist).collect())
                    } else {
                        crate::spotify::search_playlists(
                            &state.http,
                            &token,
                            term.as_deref().unwrap_or_default(),
                        )
                        .await
                        .map(|items| items.into_iter().map(spotify_source_playlist).collect())
                    };
                    match outcome {
                        Ok(results) => source_playlists = results,
                        Err(error) => {
                            tracing::warn!(error = %error, source, "requete de playlists refusee");
                            search_error =
                                Some(format!("La requete {} a echoue.", source_label(source)));
                        }
                    }
                }
                None => {
                    search_error = Some(format!(
                        "Session {} expiree : reconnectez le compte.",
                        source_label(source)
                    ));
                }
            }
        }
    }

    // Courses a venir : elles alimentent le formulaire de couverture (bloc 5).
    let races = crate::db::list_upcoming_races(&state.pool, &user.id, state.now_ms()).await?;
    let coverage = coverage_view(&tracks, &races, &query);

    // Le total des durees est affiche dans la liste des playlists.
    let total_duration_s: f64 = playlists.iter().map(|playlist| playlist.duration_s).sum();
    let total_tracks: i64 = playlists.iter().map(|playlist| playlist.track_count).sum();

    let target_label = selected_summary
        .and_then(|playlist| playlist.target_bpm)
        .map(|bpm| format!("{bpm:.0}"))
        .unwrap_or_else(|| "auto".to_string());
    // Nom du manifeste et commande a recopier : c'est ce que l'utilisateur lance
    // sur son ordinateur pour copier l'audio par USB.
    let manifest_name = selected_playlist
        .as_ref()
        .map(|playlist| crate::models::manifest_file_name(&playlist.name));
    let manifest_command = manifest_name.as_deref().map(transfer_command);
    // Bloc 4 : les noms de fichiers attendus sur la montre pour cette playlist.
    let prepared = prepared_files(&tracks);
    // Nom affiche du compte lie, quel que soit le fournisseur.
    let spotify_name = account.as_ref().and_then(|account| {
        account
            .display_name
            .clone()
            .or_else(|| account.spotify_user_id.clone())
    });
    let deezer_name = deezer_account.as_ref().and_then(|account| {
        account
            .display_name
            .clone()
            .or_else(|| account.deezer_user_id.clone())
    });

    let content = html! {
        section class="hero" {
            h1 { "Musique" }
            p class="muted" {
                "Playlists de course et tempo (BPM) : le serveur publie les fiches et le "
                "manifeste de transfert, l'audio est copie sur la montre par USB."
            }
        }
        @if let Some(erreur) = query.erreur.as_deref() {
            p class="alert" {
                span class="icon icon-alert" {}
                span { (music_error_message(erreur)) }
            }
        }
        @if let Some(ok) = query.ok.as_deref() {
            p class="alert ok" {
                span class="icon icon-check" {}
                span { (music_ok_message(ok)) }
            }
        }

        // ------------------------------------ 1. sources Spotify et Deezer
        div class="section-head" { h2 { "1. Source des playlists (Spotify ou Deezer)" } }
        div class="music-grid" {
            (source_panel(
                "spotify",
                spotify_ready,
                spotify_name,
                &source_playlists,
                source == "spotify",
                term.as_deref().unwrap_or_default(),
                search_error.as_deref(),
            ))
            (source_panel(
                "deezer",
                deezer_ready,
                deezer_name,
                &source_playlists,
                source == "deezer",
                term.as_deref().unwrap_or_default(),
                search_error.as_deref(),
            ))
        }

        // ------------------------------------------------ 2. playlists preparees
        div class="section-head" { h2 { "2. Playlists preparees" } }
        @if playlists.is_empty() {
            div class="empty" {
                span class="icon icon-music" {}
                p { "Aucune playlist pour l'instant." }
                p class="tiny" { "Importez une playlist Spotify pour commencer." }
            }
        } @else {
            div class="table-wrap" {
                table {
                    thead {
                        tr {
                            th { "Playlist" }
                            th { "Source" }
                            th { "Titres" }
                            th { "Duree" }
                            th { "BPM cible" }
                            th {}
                        }
                    }
                    tbody {
                        @for playlist in &playlists {
                            tr {
                                td { a href={ "/music?playlist=" (playlist.id) } { (playlist.name) } }
                                td { span class="pill" { (playlist.source) } }
                                td { (playlist.track_count) }
                                td { (format_duration(playlist.duration_s)) }
                                td {
                                    form class="inline-form" method="post"
                                         action={ "/music/playlists/" (playlist.id) "/target" } {
                                        input type="text" name="target_bpm" inputmode="numeric" placeholder="auto"
                                              value=(playlist.target_bpm.map(|bpm| format!("{bpm:.0}")).unwrap_or_default());
                                        button class="small ghost" type="submit" { "Valider" }
                                    }
                                }
                                td {
                                    div class="actions" {
                                        a class="button small ghost" href={ "/music?playlist=" (playlist.id) } {
                                            span class="icon icon-playlist" {}
                                            "Ouvrir"
                                        }
                                        form class="inline-form" method="post"
                                             action={ "/music/playlists/" (playlist.id) "/rename" } {
                                            input type="text" name="name" value=(playlist.name) required maxlength="200"
                                                  aria-label="Nouveau nom de la playlist";
                                            button class="small ghost" type="submit" { "Renommer" }
                                        }
                                        form method="post" action={ "/music/playlists/" (playlist.id) "/delete" }
                                             data-confirm="Supprimer cette playlist et ses metadonnees ?" {
                                            button class="ghost danger small" type="submit"
                                                   aria-label="Supprimer la playlist" { "Supprimer" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    tfoot {
                        tr {
                            td { "Total" }
                            td {}
                            td { (total_tracks) }
                            td { (format_duration(total_duration_s)) }
                            td {}
                            td {}
                        }
                    }
                }
            }
        }

        // ------------------------------------------------ 3. titres
        div class="section-head" { h2 { "3. Titres (playlist selectionnee)" } }
        @if let Some(playlist) = &selected_playlist {
            p class="muted" { (playlist.name) " - " (tracks.len()) " titres" }
            @if tracks.is_empty() {
                p class="muted" { "Cette playlist ne contient aucun titre." }
            } @else {
                div class="table-wrap" {
                    table {
                        thead {
                            tr {
                                th { "#" }
                                th { "Titre" }
                                th { "Artiste" }
                                th { "Duree" }
                                th { "BPM" }
                                th {}
                            }
                        }
                        tbody {
                            @for (index, track) in tracks.iter().enumerate() {
                                tr {
                                    td { (index + 1) }
                                    td { (track.title) }
                                    td { (track.artist.clone().unwrap_or_else(|| "-".to_string())) }
                                    td { (track.duration_s.map(format_duration).unwrap_or_else(|| "-".to_string())) }
                                    td {
                                        @match track.bpm {
                                            Some(bpm) => {
                                                (format!("{bpm:.0}"))
                                                @if let Some(source) = &track.bpm_source {
                                                    span class="tiny muted" { " " (source) }
                                                }
                                            }
                                            None => { span class="muted" { "inconnu" } }
                                        }
                                    }
                                    td {
                                        form class="inline-form bpm-form" method="post"
                                             action={ "/music/playlists/" (playlist.id) "/track-bpm" }
                                             data-bpm-track=(track.id) {
                                            input type="hidden" name="track_id" value=(track.id);
                                            input type="hidden" name="source" value="manual" class="bpm-source";
                                            input type="text" name="bpm" inputmode="numeric" placeholder="BPM"
                                                  value=(track.bpm.map(|bpm| format!("{bpm:.0}")).unwrap_or_default());
                                            button class="small" type="submit" { "saisir" }
                                        }
                                        button class="small ghost tap" type="button" data-tap=(track.id) { "tapper" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } @else {
            p class="muted" { "Selectionnez une playlist dans le bloc 2 pour saisir les BPM." }
        }

        // -------------------------------------- 4. fichiers MP3 a preparer
        div class="section-head" { h2 { "4. Fichiers a preparer (MP3)" } }
        @if let Some(playlist) = &selected_playlist {
            @if prepared.is_empty() {
                p class="muted" { "Cette playlist ne contient aucun titre." }
            } @else {
                p class="muted" {
                    (prepared.len()) " fichier(s) MP3 a mettre en place : un nom par piste, "
                    "exactement celui ecrit sur la montre par mpacer-music."
                }
                div class="actions" {
                    a class="button" href={ "/music/playlists/" (playlist.id) "/files" } {
                        "Telecharger la liste (.txt)"
                    }
                }
                div class="table-wrap" {
                    table {
                        thead {
                            tr {
                                th { "#" }
                                th { "Titre" }
                                th { "Artiste" }
                                th { "Fichier MP3 attendu" }
                            }
                        }
                        tbody {
                            @for file in &prepared {
                                tr {
                                    td { (file.position) }
                                    td { (file.title) }
                                    td { (file.artist.clone().unwrap_or_else(|| "-".to_string())) }
                                    td { code { (file.file_name) } }
                                }
                            }
                        }
                    }
                }
            }
        } @else {
            p class="muted" { "Selectionnez une playlist dans le bloc 2 pour lister ses fichiers MP3." }
        }

        // ------------------------------------------------ 5. transfert USB
        div class="section-head" { h2 { "5. Transfert vers la montre (USB)" } }
        @if let (Some(playlist), Some(name), Some(command)) =
            (&selected_playlist, &manifest_name, &manifest_command)
        {
            div class="panel" {
                div class="illustration-usb" { (PreEscaped(crate::assets::ILLUSTRATION_USB_SVG)) }
                ol class="usb-steps" {
                    li {
                        a class="button" href={ "/music/playlists/" (playlist.id) "/manifest" } {
                            "Telecharger le manifeste"
                        }
                        span class="muted" { " " (name) }
                    }
                    li {
                        "Sur l'ordinateur : "
                        code { (command) }
                    }
                    li { "Brancher la montre en USB, puis lancer la commande." }
                    li { "Sur la montre : Musique > [ Importer (USB) ]." }
                }
                div class="mini-cards" {
                    (mini_card("BPM cible", &target_label))
                    (mini_card("Titres", &tracks.len().to_string()))
                    (mini_card("Playlist", &playlist.name))
                }
                p class="muted" {
                    "Le serveur ne stocke aucun fichier audio : les morceaux restent sur votre "
                    "disque et sont copies par l'outil local mpacer-music."
                }
            }
        } @else {
            p class="muted" { "Selectionnez une playlist dans le bloc 2 pour preparer son transfert." }
        }

        // ------------------------------------------------ 6. couverture musicale
        div class="section-head" { h2 { "6. Assez de musique pour la course ?" } }
        @if let Some(playlist) = &selected_playlist {
            form class="coverage-form" method="get" action="/music" {
                input type="hidden" name="playlist" value=(playlist.id);
                div class="grid-2" {
                    div class="field" {
                        label for="race" { "Course" }
                        select id="race" name="race" {
                            option value="" { "Aucune course" }
                            @for race in &races {
                                option value=(race.id) selected[coverage.race_id == race.id] { (race.name) }
                            }
                        }
                    }
                    (text_field("distance_km", "ou distance (km)", &coverage.distance_km, "text", "10"))
                    (text_field("allure", "Allure (min/km ou secondes)", &coverage.allure, "text", "5:00"))
                    (text_field("temps", "Temps vise (h:mm:ss)", &coverage.temps, "text", "1:00:00"))
                }
                div class="actions" {
                    button type="submit" { "Valider" }
                }
            }
            section class="coverage" {
                div class={ "coverage-verdict " (coverage.verdict_class) } { (coverage.verdict) }
                div class="coverage-gauge" {
                    div class="coverage-gauge-fill" style=(format!("width: {:.0}%", coverage.gauge_percent)) {}
                }
                div class="coverage-metrics" {
                    (mini_card("Duree playlist", &coverage.playlist_duration))
                    (mini_card("Duree course", &coverage.race_duration))
                    (mini_card("Marge", &coverage.margin))
                    (mini_card("Titres necessaires", &coverage.tracks_needed))
                    (mini_card("BPM moyen", &coverage.average_bpm))
                    (mini_card("BPM cible", &coverage.target_bpm))
                }
                p class="muted" { (coverage.tempo) }
                @if let Some(marker) = coverage.bpm_marker_percent {
                    div class="bpm-gauge" {
                        div class="bpm-gauge-marker" style=(format!("left: {marker:.0}%")) {}
                    }
                }
            }
        } @else {
            p class="muted" { "Selectionnez une playlist dans le bloc 2 pour verifier sa couverture." }
        }
    };

    Ok(page(layout("Musique", "music", Some(&user), content)))
}

/// Alias de /music : la recherche renvoie ses resultats dans la meme page.
async fn music_search(
    state: State<AppState>,
    user: OptionalUser,
    query: Query<MusicQuery>,
) -> AppResult<Response> {
    music_page(state, user, query).await
}

// ---------------------------------------------------------------- Spotify (web)

/// Demarre l'OAuth Spotify (code + PKCE S256).
async fn spotify_start(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> AppResult<Response> {
    if !state.config.spotify_configured() {
        return Ok(Redirect::to("/music?erreur=spotify_non_configure").into_response());
    }
    let verifier = crate::auth::random_urlsafe(32);
    let challenge = crate::auth::pkce_challenge(&verifier);
    let oauth_state = crate::auth::random_urlsafe(24);
    let now = state.now_ms();
    // Meme table que Google : un etat OAuth est a usage unique, quelle que soit
    // la provenance du callback.
    sqlx::query(
        "INSERT INTO oauth_states (state, pkce_verifier, redirect_to, created_at_ms, expires_at_ms)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&oauth_state)
    .bind(&verifier)
    .bind("/music")
    .bind(now)
    .bind(now + 600_000)
    .execute(&state.pool)
    .await?;

    let url = crate::spotify::authorize_url(&state.config, &oauth_state, &challenge)?;
    Ok(Redirect::to(&url).into_response())
}

/// Echange le code Spotify, enregistre le compte lie et revient sur /music.
pub(crate) async fn spotify_callback(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(query): Query<CallbackQuery>,
) -> AppResult<Response> {
    if let Some(error) = query.error {
        return Ok(Redirect::to(&format!("/music?erreur={error}")).into_response());
    }
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let (Some(code), Some(oauth_state)) = (query.code, query.state) else {
        return Err(AppError::bad_request("parametres OAuth manquants"));
    };

    let now = state.now_ms();
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT pkce_verifier, redirect_to FROM oauth_states WHERE state = $1 AND expires_at_ms > $2",
    )
    .bind(&oauth_state)
    .bind(now)
    .fetch_optional(&state.pool)
    .await?;
    let Some((verifier, _redirect_to)) = row else {
        return Err(AppError::bad_request("etat OAuth inconnu ou expire"));
    };
    sqlx::query("DELETE FROM oauth_states WHERE state = $1")
        .bind(&oauth_state)
        .execute(&state.pool)
        .await?;

    let token =
        match crate::spotify::exchange_code(&state.http, &state.config, &code, &verifier).await {
            Ok(token) => token,
            Err(error) => {
                tracing::warn!(error = %error, "echange de jeton Spotify refuse");
                return Ok(Redirect::to("/music?erreur=spotify_refuse").into_response());
            }
        };
    // Le profil n'est qu'un confort d'affichage : un echec ne remet pas en
    // cause la liaison, qui a bien recu ses jetons.
    let profile = crate::spotify::current_user(&state.http, &token.access_token)
        .await
        .unwrap_or(crate::spotify::Profile {
            id: None,
            display_name: None,
        });

    let account = SpotifyAccount {
        user_id: user.id.clone(),
        spotify_user_id: profile.id,
        display_name: profile.display_name,
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at_ms: now + token.expires_in.unwrap_or(3600) * 1000,
        scope: token
            .scope
            .or_else(|| Some(crate::spotify::SCOPES.to_string())),
        connected_at_ms: now,
    };
    crate::db::upsert_spotify_account(&state.pool, &account).await?;
    tracing::info!(user = %user.email, "compte Spotify connecte");
    Ok(Redirect::to("/music?ok=spotify_connecte").into_response())
}

/// Supprime la liaison Spotify (les jetons sont oublies).
async fn spotify_disconnect(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Response> {
    crate::db::delete_spotify_account(&state.pool, &user.id).await?;
    tracing::info!(user = %user.email, "compte Spotify deconnecte");
    Ok(Redirect::to("/music?ok=spotify_deconnecte").into_response())
}

// ---------------------------------------------------------------- Deezer (web)

/// Jeton d'acces Deezer du compte lie.
///
/// Deezer n'emet pas de jeton de rafraichissement : `expires_at_ms = 0` signifie
/// "duree inconnue" et le jeton reste utilisable jusqu'a un refus de l'API, qui
/// invite alors a reconnecter le compte.
async fn deezer_access_token(state: &AppState, user_id: &str) -> AppResult<Option<String>> {
    let Some(account) = crate::db::get_deezer_account(&state.pool, user_id).await? else {
        return Ok(None);
    };
    if account.expires_at_ms == 0 || account.expires_at_ms > state.now_ms() + 60_000 {
        return Ok(Some(account.access_token));
    }
    tracing::warn!(utilisateur = %user_id, "jeton Deezer expire : reconnexion necessaire");
    Ok(None)
}

/// Demarre l'OAuth Deezer (code d'autorisation, sans PKCE).
async fn deezer_start(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> AppResult<Response> {
    if !state.config.deezer_configured() {
        return Ok(Redirect::to("/music?erreur=deezer_non_configure").into_response());
    }
    let oauth_state = crate::auth::random_urlsafe(24);
    let now = state.now_ms();
    // Meme table que Google et Spotify : l'etat est a usage unique. Deezer n'a
    // pas de PKCE, le verificateur porte donc un marqueur de provenance.
    sqlx::query(
        "INSERT INTO oauth_states (state, pkce_verifier, redirect_to, created_at_ms, expires_at_ms)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&oauth_state)
    .bind("deezer")
    .bind("/music/deezer")
    .bind(now)
    .bind(now + 600_000)
    .execute(&state.pool)
    .await?;

    let url = crate::deezer::authorize_url(&state.config, &oauth_state)?;
    Ok(Redirect::to(&url).into_response())
}

/// Echange le code Deezer, enregistre le compte lie et revient sur /music.
pub(crate) async fn deezer_callback(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(query): Query<CallbackQuery>,
) -> AppResult<Response> {
    if let Some(error) = query.error {
        return Ok(Redirect::to(&format!("/music?erreur={error}")).into_response());
    }
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let Some(code) = query.code else {
        return Err(AppError::bad_request("parametres OAuth manquants"));
    };
    let now = state.now_ms();
    // Deezer renvoie `state` quand il est fourni ; s'il manque, on retombe sur le
    // dernier etat Deezer en attente : une protection minimale est conservee et
    // la connexion ne casse pas sur un fournisseur qui omet le parametre.
    let row: Option<(String, String)> = match query.state.as_deref() {
        Some(oauth_state) => {
            sqlx::query_as(
                "SELECT state, pkce_verifier FROM oauth_states
                  WHERE state = $1 AND expires_at_ms > $2",
            )
            .bind(oauth_state)
            .bind(now)
            .fetch_optional(&state.pool)
            .await?
        }
        None => {
            sqlx::query_as(
                "SELECT state, pkce_verifier FROM oauth_states
                  WHERE redirect_to = '/music/deezer' AND expires_at_ms > $1
                  ORDER BY created_at_ms DESC LIMIT 1",
            )
            .bind(now)
            .fetch_optional(&state.pool)
            .await?
        }
    };
    let Some((oauth_state, _provenance)) = row else {
        return Err(AppError::bad_request("etat OAuth inconnu ou expire"));
    };
    sqlx::query("DELETE FROM oauth_states WHERE state = $1")
        .bind(&oauth_state)
        .execute(&state.pool)
        .await?;

    let token = match crate::deezer::exchange_code(&state.http, &state.config, &code).await {
        Ok(token) => token,
        Err(error) => {
            tracing::warn!(error = %error, "echange de jeton Deezer refuse");
            return Ok(Redirect::to("/music?erreur=deezer_refuse").into_response());
        }
    };
    // Le profil n'est qu'un confort d'affichage : un echec ne remet pas en cause
    // la liaison, qui a bien recu son jeton.
    let profile = crate::deezer::current_user(&state.http, &token.access_token)
        .await
        .unwrap_or(crate::deezer::Profile {
            id: None,
            display_name: None,
        });

    let account = DeezerAccount {
        user_id: user.id.clone(),
        deezer_user_id: profile.id,
        display_name: profile.display_name,
        access_token: token.access_token,
        expires_at_ms: token.expires_in.map_or(0, |seconds| now + seconds * 1000),
        scope: Some(crate::deezer::PERMS.to_string()),
        connected_at_ms: now,
    };
    crate::db::upsert_deezer_account(&state.pool, &account).await?;
    tracing::info!(user = %user.email, "compte Deezer connecte");
    Ok(Redirect::to("/music?ok=deezer_connecte").into_response())
}

/// Supprime la liaison Deezer (le jeton est oublie).
async fn deezer_disconnect(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Response> {
    crate::db::delete_deezer_account(&state.pool, &user.id).await?;
    tracing::info!(user = %user.email, "compte Deezer deconnecte");
    Ok(Redirect::to("/music?ok=deezer_deconnecte").into_response())
}

// ---------------------------------------------------------------- actions musique

#[derive(Debug, Deserialize)]
struct MusicImportForm {
    /// Source demandee : `spotify` (defaut) ou `deezer`.
    #[serde(default)]
    source: String,
    /// Reference generique : lien, URI ou identifiant chez le fournisseur.
    #[serde(default, rename = "ref")]
    reference: String,
    /// Ancien nom du champ Spotify : conserve pour ne pas casser un formulaire
    /// deja ouvert dans un navigateur.
    #[serde(default)]
    spotify_ref: String,
    #[serde(default)]
    deezer_ref: String,
    #[serde(default)]
    target_bpm: String,
}

/// Importe une playlist d'une source externe (Spotify ou Deezer).
async fn music_import(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Form(form): Form<MusicImportForm>,
) -> AppResult<Response> {
    let requested = form.source.trim();
    if !requested.is_empty() && !matches!(requested, "spotify" | "deezer") {
        return Ok(Redirect::to("/music?erreur=source_inconnue").into_response());
    }
    let source = normalize_source(Some(requested));
    let reference = if !form.reference.trim().is_empty() {
        form.reference.trim().to_string()
    } else if source == "deezer" {
        form.deezer_ref.trim().to_string()
    } else {
        form.spotify_ref.trim().to_string()
    };
    let target_bpm = match parse_number(&form.target_bpm, "le BPM cible") {
        Ok(value) => crate::models::clean_bpm(value),
        Err(_) => return Ok(Redirect::to("/music?erreur=bpm_invalide").into_response()),
    };
    if source == "deezer" {
        import_deezer(&state, &user, &reference, target_bpm).await
    } else {
        import_spotify(&state, &user, &reference, target_bpm).await
    }
}

/// Importe une playlist Spotify (fiche + tempo quand l'API l'autorise encore).
async fn import_spotify(
    state: &AppState,
    user: &User,
    reference: &str,
    target_bpm: Option<f64>,
) -> AppResult<Response> {
    if !state.config.spotify_configured() {
        return Ok(Redirect::to("/music?erreur=spotify_non_configure").into_response());
    }
    let Some(spotify_id) = crate::spotify::playlist_id_from_ref(reference) else {
        return Ok(Redirect::to("/music?erreur=spotify_ref_invalide").into_response());
    };
    let Some(token) = spotify_access_token(state, &user.id).await? else {
        return Ok(Redirect::to("/music?erreur=spotify_non_connecte").into_response());
    };
    let detail = match crate::spotify::get_playlist(&state.http, &token, &spotify_id).await {
        Ok(detail) => detail,
        Err(error) => {
            tracing::warn!(error = %error, "import Spotify refuse");
            return Ok(Redirect::to("/music?erreur=spotify_refuse").into_response());
        }
    };

    // Le tempo vient d'audio-features quand le compte y a encore acces ; un
    // refus laisse simplement le BPM inconnu (balise, tap ou saisie manuelle).
    let ids: Vec<String> = detail
        .tracks
        .iter()
        .filter_map(|track| track.spotify_id.clone())
        .collect();
    let tempos = match crate::spotify::audio_features(&state.http, &token, &ids).await {
        Ok(features) => features,
        Err(error) => {
            tracing::warn!(error = %error, "audio-features indisponible : BPM inconnu");
            None
        }
    };
    let tempo_for = |track_id: &str| -> Option<f64> {
        tempos
            .as_ref()
            .and_then(|features| {
                features
                    .iter()
                    .find(|feature| feature.id == track_id)
                    .map(|feature| feature.tempo)
            })
            .and_then(|tempo| crate::models::clean_bpm(Some(tempo)))
    };

    // Un reimport remplace la playlist existante : aucune fiche en double.
    if let Some(existing) =
        crate::db::find_music_playlist_by_spotify(&state.pool, &user.id, &spotify_id).await?
    {
        crate::db::delete_music_playlist(&state.pool, &user.id, &existing.id).await?;
    }

    let playlist = crate::db::insert_music_playlist(
        &state.pool,
        &user.id,
        &MusicPlaylistInput {
            name: detail.name.chars().take(200).collect(),
            source: "spotify".to_string(),
            spotify_id: Some(spotify_id.clone()),
            deezer_id: None,
            cover_url: detail.cover_url.clone(),
            target_bpm,
        },
        state.now_ms(),
    )
    .await?;

    for (position, track) in detail.tracks.iter().enumerate() {
        let bpm = track.spotify_id.as_deref().and_then(tempo_for);
        crate::db::insert_music_track(
            &state.pool,
            &user.id,
            &playlist.id,
            &MusicTrackInput {
                position: position as i32,
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                duration_s: track.duration_s,
                bpm,
                bpm_source: bpm.map(|_| "spotify".to_string()),
                spotify_uri: track.spotify_uri.clone(),
            },
            state.now_ms(),
        )
        .await?;
    }

    tracing::info!(
        user = %user.email,
        playlist = %playlist.id,
        titres = detail.tracks.len(),
        "playlist Spotify importee"
    );
    Ok(Redirect::to(&format!(
        "/music?playlist={}&ok=playlist_importee",
        playlist.id
    ))
    .into_response())
}

/// Importe une playlist Deezer (fiche seule : Deezer n'expose aucun tempo).
///
/// Le BPM reste inconnu a l'import ; il se complete ensuite par la balise du
/// fichier MP3, le tap-tempo ou la saisie manuelle sur la page.
async fn import_deezer(
    state: &AppState,
    user: &User,
    reference: &str,
    target_bpm: Option<f64>,
) -> AppResult<Response> {
    if !state.config.deezer_configured() {
        return Ok(Redirect::to("/music?erreur=deezer_non_configure").into_response());
    }
    let Some(deezer_id) = crate::deezer::playlist_id_from_ref(reference) else {
        return Ok(Redirect::to("/music?erreur=deezer_ref_invalide").into_response());
    };
    let Some(token) = deezer_access_token(state, &user.id).await? else {
        return Ok(Redirect::to("/music?erreur=deezer_non_connecte").into_response());
    };
    let detail = match crate::deezer::get_playlist(&state.http, &token, &deezer_id).await {
        Ok(detail) => detail,
        Err(error) => {
            tracing::warn!(error = %error, "import Deezer refuse");
            return Ok(Redirect::to("/music?erreur=deezer_refuse").into_response());
        }
    };

    // Un reimport remplace la playlist existante : aucune fiche en double.
    if let Some(existing) =
        crate::db::find_music_playlist_by_deezer(&state.pool, &user.id, &deezer_id).await?
    {
        crate::db::delete_music_playlist(&state.pool, &user.id, &existing.id).await?;
    }

    let playlist = crate::db::insert_music_playlist(
        &state.pool,
        &user.id,
        &MusicPlaylistInput {
            name: detail.name.chars().take(200).collect(),
            source: "deezer".to_string(),
            spotify_id: None,
            deezer_id: Some(deezer_id.clone()),
            cover_url: detail.cover_url.clone(),
            target_bpm,
        },
        state.now_ms(),
    )
    .await?;

    for (position, track) in detail.tracks.iter().enumerate() {
        crate::db::insert_music_track(
            &state.pool,
            &user.id,
            &playlist.id,
            &MusicTrackInput {
                position: position as i32,
                title: track.title.clone(),
                artist: track.artist.clone(),
                album: track.album.clone(),
                duration_s: track.duration_s,
                bpm: None,
                bpm_source: None,
                spotify_uri: None,
            },
            state.now_ms(),
        )
        .await?;
    }

    tracing::info!(
        user = %user.email,
        playlist = %playlist.id,
        titres = detail.tracks.len(),
        "playlist Deezer importee"
    );
    Ok(Redirect::to(&format!(
        "/music?playlist={}&ok=playlist_importee",
        playlist.id
    ))
    .into_response())
}

#[derive(Debug, Deserialize)]
struct MusicTargetForm {
    #[serde(default)]
    target_bpm: String,
}

/// Consigne de tempo d'une playlist (vide = tempo automatique).
async fn music_target(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Form(form): Form<MusicTargetForm>,
) -> AppResult<Response> {
    let requested = match parse_number(&form.target_bpm, "le BPM cible") {
        Ok(value) => value,
        Err(_) => return Ok(Redirect::to("/music?erreur=bpm_invalide").into_response()),
    };
    if requested.is_some() && crate::models::clean_bpm(requested).is_none() {
        return Ok(Redirect::to("/music?erreur=bpm_invalide").into_response());
    }
    let updated = crate::db::set_music_playlist_target(
        &state.pool,
        &user.id,
        &id,
        crate::models::clean_bpm(requested),
        state.now_ms(),
    )
    .await?;
    if !updated {
        return Ok(Redirect::to("/music?erreur=playlist_inconnue").into_response());
    }
    Ok(Redirect::to(&format!("/music?playlist={id}")).into_response())
}

#[derive(Debug, Deserialize)]
struct MusicTrackBpmForm {
    #[serde(default)]
    track_id: String,
    #[serde(default)]
    bpm: String,
    /// Instants de tap (ms) separes par des virgules : le serveur calcule le
    /// tempo median avec la meme fonction que la montre.
    #[serde(default)]
    taps: String,
    #[serde(default)]
    source: String,
}

/// Enregistre le BPM d'un titre (tap-tempo ou saisie manuelle).
async fn music_track_bpm(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Form(form): Form<MusicTrackBpmForm>,
) -> AppResult<Response> {
    let (bpm, source) = if !form.taps.trim().is_empty() {
        let measured = crate::bpm::bpm_from_taps(&crate::bpm::parse_taps(&form.taps));
        if measured.is_none() {
            return Ok(Redirect::to("/music?erreur=bpm_invalide").into_response());
        }
        (measured, "tap")
    } else {
        let requested = match parse_number(&form.bpm, "le BPM") {
            Ok(value) => value,
            Err(_) => return Ok(Redirect::to("/music?erreur=bpm_invalide").into_response()),
        };
        if requested.is_some() && crate::models::clean_bpm(requested).is_none() {
            return Ok(Redirect::to("/music?erreur=bpm_invalide").into_response());
        }
        let source = if form.source.trim().eq_ignore_ascii_case("tap") {
            "tap"
        } else {
            "manual"
        };
        (crate::models::clean_bpm(requested), source)
    };

    let updated = crate::db::set_music_track_bpm(
        &state.pool,
        &user.id,
        &id,
        &form.track_id,
        bpm,
        Some(source),
        state.now_ms(),
    )
    .await?;
    if !updated {
        return Ok(Redirect::to("/music?erreur=playlist_inconnue").into_response());
    }
    Ok(Redirect::to(&format!("/music?playlist={id}&ok=bpm_enregistre")).into_response())
}

#[derive(Debug, Deserialize)]
struct MusicRenameForm {
    #[serde(default)]
    name: String,
}

/// Renomme une playlist (le nom est nettoye et borne a 200 caracteres).
async fn music_rename(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Form(form): Form<MusicRenameForm>,
) -> AppResult<Response> {
    let name = form.name.trim();
    if name.is_empty() {
        return Ok(Redirect::to("/music?erreur=nom_invalide").into_response());
    }
    let name: String = name.chars().take(200).collect();
    if !crate::db::rename_music_playlist(&state.pool, &user.id, &id, &name, state.now_ms()).await? {
        return Ok(Redirect::to("/music?erreur=playlist_inconnue").into_response());
    }
    tracing::info!(user = %user.email, playlist = %id, "playlist renommee");
    Ok(Redirect::to(&format!("/music?playlist={id}&ok=playlist_renommee")).into_response())
}

/// Supprime une playlist (ses metadonnees ; aucun audio n'est stocke ici).
async fn music_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    if !crate::db::delete_music_playlist(&state.pool, &user.id, &id).await? {
        return Err(AppError::NotFound);
    }
    tracing::info!(user = %user.email, playlist = %id, "playlist musique supprimee");
    Ok(Redirect::to("/music?ok=playlist_supprimee").into_response())
}

/// Telecharge le manifeste de transfert (session navigateur, piece jointe).
async fn music_manifest(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    super::manifest_response(&state, &user.id, &id).await
}

/// Liste des fichiers MP3 a mettre en place pour une playlist (piece jointe).
///
/// C'est la reponse a "quels fichiers me manque-t-il ?" : un nom par piste,
/// exactement celui que `mpacer-music` ecrit sur la montre, a rassembler dans un
/// dossier puis a laisser apparier par l'outil local.
async fn music_files(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let playlist = crate::db::get_music_playlist(&state.pool, &user.id, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    let tracks = crate::db::list_music_tracks(&state.pool, &user.id, &id).await?;
    let files = prepared_files(&tracks);
    let body = prepared_files_text(&playlist.name, &files);
    let base = crate::models::manifest_file_name(&playlist.name);
    let base = base.strip_suffix(".json").unwrap_or(&base);
    Response::builder()
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{base}.txt\""),
        )
        .body(Body::from(body))
        .map_err(|error| AppError::internal(error.to_string()))
}

// ------------------------------------------------------------- suivi en direct
//
// Pendant une seance, la montre publie sa position sur un broker MQTT
// (docs/10). Le service s'y abonne et garde la trace en memoire ; cette page
// l'affiche, sans JavaScript et sans service de cartographie tiers : la trace
// est dessinee en SVG, et un lien ouvre OpenStreetMap pour la position exacte.

/// Nombre de montres affichees sur la page.
const MAX_LIVE_SESSIONS: usize = 4;

/// Periode de rafraichissement de la page (s) tant qu'une montre publie.
const LIVE_REFRESH_S: u64 = 10;

/// Page de suivi en direct.
async fn live_page(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };
    let now_ms = state.now_ms();
    let sessions = state.live.snapshot(now_ms, MAX_LIVE_SESSIONS);
    let en_direct = sessions.iter().any(|session| session.is_live(now_ms));
    let content = live_content(&sessions, state.live.status(), &state.config, now_ms);
    Ok(page(layout_refresh(
        "Suivi en direct",
        "live",
        Some(&user),
        en_direct.then_some(LIVE_REFRESH_S),
        content,
    )))
}

/// Meme donnee que la page, en JSON, pour un outil externe (tableau de bord
/// personnel, Home Assistant, statistiques maison).
async fn live_json(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> AppResult<Response> {
    if user.is_none() {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }
    let now_ms = state.now_ms();
    let status = state.live.status();
    Ok(Json(serde_json::json!({
        "now_ms": now_ms,
        "broker": {
            "connected": status.connected(),
            "connections": status.connections(),
            "messages": status.messages(),
            "last_message_ms": status.last_message_ms(),
        },
        "sessions": state.live.snapshot(now_ms, MAX_LIVE_SESSIONS),
    }))
    .into_response())
}

/// Contenu de la page : etat du broker, puis une carte par montre.
fn live_content(
    sessions: &[LiveSessionView],
    status: &MqttStatus,
    config: &crate::config::Config,
    now_ms: i64,
) -> Markup {
    html! {
        section class="hero" {
            h1 { "Suivi en direct" }
            p class="muted" {
                "Pendant la seance, la montre publie sa position sur le broker MQTT : "
                "cette page suit la trace en temps reel. Rien n'est ecrit en base, la "
                "seance arrive ici a la fin, comme d'habitude."
            }
        }
        (broker_panel(status, config, now_ms))
        @if sessions.is_empty() {
            section class="empty" {
                span class="icon icon-watch" {}
                p { "Aucune position recue pour l'instant." }
                p class="tiny muted" {
                    "Renseignez l'adresse du broker MQTT dans les reglages de la montre, "
                    "puis lancez une seance : la trace apparaitra ici."
                }
            }
        } @else {
            @for session in sessions {
                (live_card(session, now_ms))
            }
            p class="tiny muted" {
                "Trace en memoire seulement : le fichier de la seance reste sur la montre "
                "et arrive au backend a la fin."
            }
        }
    }
}

/// Bandeau d'etat de la liaison avec le broker.
fn broker_panel(status: &MqttStatus, config: &crate::config::Config, now_ms: i64) -> Markup {
    let (classe, libelle) = if !config.mqtt_configured() {
        ("off", "desactive")
    } else if status.connected() {
        ("on", "connecte")
    } else {
        ("off", "deconnecte")
    };
    html! {
        section class="panel" {
            div class="section-head" {
                h2 { "Broker MQTT" }
                span class={ "pill " (classe) } { (libelle) }
            }
            @if config.mqtt_configured() {
                p class="tiny muted" {
                    "Filtre " code { (config.mqtt_topic) } " - "
                    (status.messages()) " message(s) recu(s) - "
                    (status.connections()) " connexion(s) - "
                    "dernier message " (status_age(status, now_ms))
                }
            } @else {
                p class="tiny muted" {
                    "Definissez " code { "MPACER_MQTT_URL" } " (par exemple "
                    code { "mqtt://mosquitto.mpacer.svc:1883" } ") pour activer le suivi en direct."
                }
            }
        }
    }
}

/// Carte d'une montre : etat, chiffres cles, trace et position exacte.
fn live_card(session: &LiveSessionView, now_ms: i64) -> Markup {
    let (classe, libelle) = match session.state.as_str() {
        "stop" => ("off", "seance terminee"),
        "pause" | "paused" => ("brand", "en pause"),
        _ if session.is_live(now_ms) => ("on", "en direct"),
        _ => ("off", "silence"),
    };
    let dernier = session.last.as_ref();
    html! {
        section class="card live-card" {
            div class="section-head" {
                h2 { (session.device) }
                span class={ "pill " (classe) } { (libelle) }
            }
            div class="mini-cards" {
                (mini_card("Distance", &option_distance(session.distance_m)))
                (mini_card("Duree", &option_duration(session.duration_s)))
                (mini_card("Allure", &format_pace(session.pace_s_per_km)))
                (mini_card("Cardio", &option_bpm(dernier.and_then(|point| point.heart_rate_bpm))))
                (mini_card("Batterie", &option_percent(dernier.and_then(|point| point.battery_percent))))
                (mini_card("Precision", &option_accuracy(dernier.and_then(|point| point.accuracy_m))))
                (mini_card("Points", &session.points.to_string()))
            }
            (trace_svg(&session.trace))
            @if let Some(point) = dernier {
                div class="live-foot" {
                    p class="tiny muted" {
                        "Derniere position " (since_label(now_ms - point.t_ms)) " - "
                        (format!("{:.5}, {:.5}", point.lat, point.lon))
                    }
                    a class="button ghost" href=(osm_url(point.lat, point.lon))
                      target="_blank" rel="noopener noreferrer" { "Ouvrir dans OpenStreetMap" }
                }
            }
        }
    }
}

/// Trace GPS dessinee en SVG, echelle uniforme (la forme du parcours est juste).
fn trace_svg(points: &[LivePoint]) -> Markup {
    const LARGEUR: f64 = 1000.0;
    const HAUTEUR: f64 = 420.0;
    const MARGE: f64 = 18.0;
    /// Metres par degre de latitude (valeur moyenne suffisante pour un apercu).
    const METRES_PAR_DEGRE: f64 = 111_320.0;

    if points.is_empty() {
        return html! {};
    }
    let nombre = points.len() as f64;
    let lat0 = points.iter().map(|point| point.lat).sum::<f64>() / nombre;
    let lon0 = points.iter().map(|point| point.lon).sum::<f64>() / nombre;
    // Projection locale : les longitudes sont compressees par le cosinus de la
    // latitude, sinon la trace est etiree d'est en ouest.
    let cosinus = lat0.to_radians().cos().abs().max(0.02);
    let xs: Vec<f64> = points
        .iter()
        .map(|point| (point.lon - lon0) * cosinus)
        .collect();
    let ys: Vec<f64> = points.iter().map(|point| point.lat - lat0).collect();

    let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut etendue_x = max_x - min_x;
    let mut etendue_y = max_y - min_y;
    let immobile = etendue_x < 1e-6 && etendue_y < 1e-6;
    if immobile {
        // Montre immobile : un point, pas une division par zero.
        etendue_x = 1e-5;
        etendue_y = 1e-5;
    }
    // Cadre de 5 % autour de la trace (le point isole tombe au centre).
    let (origine_x, cadre_x) = if immobile {
        (min_x - etendue_x / 2.0, etendue_x)
    } else {
        (min_x - etendue_x * 0.05, etendue_x * 1.1)
    };
    let (origine_y, cadre_y) = if immobile {
        (min_y - etendue_y / 2.0, etendue_y)
    } else {
        (min_y - etendue_y * 0.05, etendue_y * 1.1)
    };
    let echelle = ((LARGEUR - 2.0 * MARGE) / cadre_x).min((HAUTEUR - 2.0 * MARGE) / cadre_y);
    let marge_x = (LARGEUR - cadre_x * echelle) / 2.0;
    let marge_y = (HAUTEUR - cadre_y * echelle) / 2.0;

    let mut trace = String::with_capacity(points.len() * 12);
    let mut dernier = (LARGEUR / 2.0, HAUTEUR / 2.0);
    for (x, y) in xs.iter().zip(ys.iter()) {
        let px = marge_x + (x - origine_x) * echelle;
        // L'axe vertical est inverse : le nord est en haut.
        let py = HAUTEUR - marge_y - (y - origine_y) * echelle;
        let _ = write!(trace, "{px:.1},{py:.1} ");
        dernier = (px, py);
    }

    let (largeur_m, hauteur_m) = (etendue_x * METRES_PAR_DEGRE, etendue_y * METRES_PAR_DEGRE);
    let (cx, cy) = dernier;
    html! {
        figure class="trace-map" {
            svg viewBox=(format!("0 0 {LARGEUR:.0} {HAUTEUR:.0}"))
                role="img" aria-label="Trace GPS de la seance en cours" {
                polyline points=(trace.trim_end()) {}
                circle class="now" cx=(format!("{cx:.1}")) cy=(format!("{cy:.1}")) r="9" {}
            }
            figcaption class="tiny muted" {
                "Trace sur " (format!("{largeur_m:.0} m x {hauteur_m:.0} m")) " - "
                (points.len()) " point(s) - nord en haut"
            }
        }
    }
}

/// Lien OpenStreetMap sur la derniere position connue.
fn osm_url(lat: f64, lon: f64) -> String {
    format!("https://www.openstreetmap.org/?mlat={lat:.6}&mlon={lon:.6}#map=16/{lat:.6}/{lon:.6}")
}

/// "il y a 12 s", "il y a 3 min", "il y a 2 h".
fn since_label(ecart_ms: i64) -> String {
    if ecart_ms < 0 {
        return "a l'instant".to_string();
    }
    let secondes = ecart_ms / 1000;
    if secondes < 5 {
        "a l'instant".to_string()
    } else if secondes < 60 {
        format!("il y a {secondes} s")
    } else if secondes < 3600 {
        format!("il y a {} min", secondes / 60)
    } else {
        format!("il y a {} h", secondes / 3600)
    }
}

/// Age du dernier message recu du broker.
fn status_age(status: &MqttStatus, now_ms: i64) -> String {
    if status.messages() == 0 {
        return "aucun depuis le demarrage".to_string();
    }
    since_label(now_ms - status.last_message_ms())
}

/// Distance lissee : metres sous le kilometre, kilometres ensuite.
fn option_distance(valeur: Option<f64>) -> String {
    match valeur {
        Some(metres) if metres >= 1000.0 => format!("{:.2} km", metres / 1000.0),
        Some(metres) => format!("{metres:.0} m"),
        None => "-".to_string(),
    }
}

fn option_duration(valeur: Option<f64>) -> String {
    // `format_duration` rend "--:--" pour une valeur non finie : meme convention
    // que le reste de l'interface quand la montre n'a pas encore duree.
    format_duration(valeur.unwrap_or(f64::NAN))
}

fn option_bpm(valeur: Option<i64>) -> String {
    valeur
        .map(|bpm| format!("{bpm} bpm"))
        .unwrap_or_else(|| "-".to_string())
}

fn option_percent(valeur: Option<i64>) -> String {
    valeur
        .map(|pourcent| format!("{pourcent} %"))
        .unwrap_or_else(|| "-".to_string())
}

fn option_accuracy(valeur: Option<f64>) -> String {
    valeur
        .map(|metres| format!("{metres:.0} m"))
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod music_web_tests {
    use super::*;

    #[test]
    fn the_transfer_command_shows_the_manifest_and_the_folder() {
        let command = transfer_command("run-170.json");
        assert!(command.starts_with("mpacer-music transfer"), "{command}");
        assert!(command.contains("--manifest run-170.json"), "{command}");
        assert!(command.contains("--folder"), "{command}");
    }

    #[test]
    fn music_messages_are_explicit() {
        assert!(music_error_message("spotify_non_configure").contains("MPACER_SPOTIFY_CLIENT_ID"));
        assert!(music_error_message("bpm_invalide").contains("30"));
        assert!(music_error_message("nom_invalide").contains("vide"));
        assert!(music_ok_message("playlist_supprimee").contains("supprimee"));
        assert!(music_ok_message("playlist_renommee").contains("renommee"));
        // Un code inconnu reste lisible plutot que vide.
        assert!(music_error_message("inconnu").contains("inconnu"));
        assert!(music_ok_message("inconnu").contains("inconnu"));
    }

    #[test]
    fn the_source_falls_back_to_spotify() {
        assert_eq!(normalize_source(None), "spotify");
        assert_eq!(normalize_source(Some("")), "spotify");
        assert_eq!(normalize_source(Some(" Spotify ")), "spotify");
        assert_eq!(normalize_source(Some("DEEZER")), "deezer");
        assert_eq!(source_label("deezer"), "Deezer");
        assert_eq!(source_label("spotify"), "Spotify");
        // Les messages de developpement restent explicites.
        assert!(music_error_message("deezer_non_configure").contains("MPACER_DEEZER_APP_ID"));
        assert!(music_error_message("deezer_ref_invalide").contains("deezer.com"));
        assert!(music_error_message("source_inconnue").contains("Spotify"));
        assert!(music_ok_message("deezer_connecte").contains("Deezer"));
    }

    #[test]
    fn prepared_files_follow_the_watch_scheme() {
        let tracks = vec![
            music_track(Some(249.0), None),
            MusicTrack {
                artist: None,
                title: "Levels".into(),
                position: 4,
                ..music_track(Some(200.0), None)
            },
        ];
        let files = prepared_files(&tracks);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].file_name, "01 - Artiste - Titre.mp3");
        assert_eq!(files[1].file_name, "04 - Levels.mp3");

        let text = prepared_files_text("Run 170", &files);
        assert!(text.contains("# Run 170 : 2 fichier(s) MP3 a mettre en place"));
        assert!(text.contains("01 - Artiste - Titre.mp3"));
        assert!(text.contains("04 - Levels.mp3"));

        let empty = prepared_files_text("Vide", &[]);
        assert!(empty.contains("0 fichier(s) MP3"));
    }

    #[test]
    fn paces_and_durations_are_read_as_written() {
        assert_eq!(parse_pace_input("5:00"), Some(300.0));
        assert_eq!(parse_pace_input("5:00/km"), Some(300.0));
        assert_eq!(parse_pace_input("4:30"), Some(270.0));
        assert_eq!(parse_pace_input("300"), Some(300.0));
        assert_eq!(parse_pace_input(""), None);
        assert_eq!(parse_pace_input("vite"), None);
        assert_eq!(parse_pace_input("0"), None);

        assert_eq!(human_duration(3600.0), "1 h");
        assert_eq!(human_duration(3900.0), "1 h 05");
        assert_eq!(human_duration(2700.0), "45 min");
        assert_eq!(human_duration(0.0), "0 min");
        assert_eq!(signed_minutes(420.0), "+7 min");
        assert_eq!(signed_minutes(-420.0), "-7 min");
        assert_eq!(signed_minutes(5.0), "0 min");
    }

    /// Piste minimale, telle que la base la renvoie.
    fn music_track(duration_s: Option<f64>, bpm: Option<f64>) -> MusicTrack {
        MusicTrack {
            id: "t1".into(),
            playlist_id: "p1".into(),
            user_id: "u1".into(),
            position: 0,
            title: "Titre".into(),
            artist: Some("Artiste".into()),
            album: None,
            duration_s,
            bpm,
            bpm_source: None,
            spotify_uri: None,
            mime: None,
            size_bytes: None,
            storage_path: None,
            created_at_ms: 0,
            downloaded_at_ms: None,
        }
    }

    /// Formulaire du bloc 6, sans course selectionnee.
    fn coverage_query(distance_km: &str, allure: &str) -> MusicQuery {
        MusicQuery {
            playlist: Some("p1".into()),
            q: None,
            source: None,
            vue: None,
            race: None,
            distance_km: Some(distance_km.into()),
            allure: Some(allure.into()),
            temps: None,
            erreur: None,
            ok: None,
        }
    }

    #[test]
    fn the_coverage_verdict_follows_the_playlist_duration() {
        let tracks = vec![
            music_track(Some(3600.0), Some(150.0)),
            music_track(Some(3600.0), Some(150.0)),
        ];
        // 10 km a 5:00/km = 50 min de course ; 2 h de musique : large marge.
        let view = coverage_view(&tracks, &[], &coverage_query("10", "5:00"));
        assert_eq!(view.verdict_class, "coverage-ok");
        assert!(view.verdict.contains("OK"), "{}", view.verdict);
        assert_eq!(view.playlist_duration, "2:00:00");
        assert_eq!(view.average_bpm, "150");
        // Le BPM cible vient de la calibration du coeur (170 a 5:00/km).
        assert_eq!(view.target_bpm, "170");
        assert!(view.gauge_percent > 99.0, "{}", view.gauge_percent);
        assert!(view.bpm_marker_percent.is_some());

        // Une seule piste de 10 min ne couvre pas 50 min de course.
        let short = coverage_view(
            &[music_track(Some(600.0), None)],
            &[],
            &coverage_query("10", "5:00"),
        );
        assert_eq!(short.verdict_class, "coverage-bad");
        assert!(short.verdict.contains("insuffisant"), "{}", short.verdict);
        // Sans BPM renseigne, aucun tempo n'est invente.
        assert_eq!(short.average_bpm, "-");
        assert_eq!(short.bpm_marker_percent, None);
        assert!(short.margin.starts_with('-'), "{}", short.margin);

        // Sans duree de course, le verdict reste en attente.
        let unknown = coverage_view(&tracks, &[], &coverage_query("", ""));
        assert!(unknown.distance_km.is_empty());
        assert_eq!(unknown.verdict_class, "coverage-warn");
        assert!(unknown.verdict.contains("inconnue"), "{}", unknown.verdict);
        assert_eq!(unknown.race_duration, "-");
    }
}

#[cfg(test)]
mod navigation_web_tests {
    use super::*;

    #[test]
    fn the_header_tabs_include_settings_and_the_dropdown_is_gone() {
        // Reglages est un onglet comme les autres (docs/07 section 10.3) : plus
        // de menu deroulant, l'appairage et les jetons vivent sur /settings.
        assert_eq!(NAV.len(), 8);
        assert!(NAV.iter().any(|(cle, ..)| *cle == "live"));
        assert!(NAV.iter().any(|(cle, ..)| *cle == "dashboards"));
        assert!(NAV
            .iter()
            .any(|(cle, libelle, icone, chemin)| *cle == "reglages"
                && *libelle == "Reglages"
                && *icone == "icon-settings"
                && *chemin == "/settings"));
        // /link reste servi (la montre y renvoie l'utilisateur) mais n'est plus
        // dans la navigation.
        assert!(!NAV.iter().any(|(cle, ..)| *cle == "link"));

        let user = User {
            id: "u1".into(),
            google_sub: None,
            email: "coureur@example.org".into(),
            name: Some("Coureur".into()),
            picture_url: None,
            created_at_ms: 0,
            last_seen_ms: 0,
        };
        let markup = layout("Reglages", "reglages", Some(&user), html! {}).into_string();
        assert!(markup.contains("href=\"/settings\""), "{markup}");
        assert!(markup.contains("Reglages"), "{markup}");
        // Le logo du chantier design remplace le simple texte de marque.
        assert!(markup.contains("class=\"brand logo\""), "{markup}");
        // Plus de menu deroulant ni de deconnexion dans l'en-tete.
        assert!(!markup.contains("menu-panel"), "{markup}");
        assert!(!markup.contains("action=\"/logout\""), "{markup}");
    }
}

#[cfg(test)]
mod live_web_tests {
    use super::*;

    fn point(t_ms: i64, lat: f64, lon: f64) -> LivePoint {
        LivePoint {
            t_ms,
            lat,
            lon,
            accuracy_m: Some(4.0),
            distance_m: Some(1_200.0),
            pace_s_per_km: Some(300.0),
            heart_rate_bpm: Some(145),
            lap: Some(1),
            battery_percent: Some(72),
            state: Some("run".to_string()),
            device: None,
        }
    }

    fn session(points: Vec<LivePoint>) -> LiveSessionView {
        let dernier = points.last().cloned();
        LiveSessionView {
            device: "montre-a1b2".to_string(),
            state: "run".to_string(),
            started_ms: points.first().map(|p| p.t_ms).unwrap_or(0),
            last_ms: dernier.as_ref().map(|p| p.t_ms).unwrap_or(0),
            received: points.len() as u64,
            points: points.len(),
            trace: points,
            distance_m: Some(1_200.0),
            duration_s: Some(600.0),
            pace_s_per_km: Some(300.0),
            last: dernier,
        }
    }

    fn config() -> crate::config::Config {
        crate::config::Config::for_tests("http://localhost:8080", "postgresql://exemple")
    }

    /// Coordonnees de la polyligne, dans l'ordre du document.
    fn polyline_points(markup: &str) -> Vec<(f64, f64)> {
        let brut = markup
            .split("points=\"")
            .nth(1)
            .expect("polyligne presente")
            .split('"')
            .next()
            .unwrap();
        brut.split(' ')
            .map(|paire| {
                let (x, y) = paire.split_once(',').expect("couple x,y");
                (x.parse().unwrap(), y.parse().unwrap())
            })
            .collect()
    }

    #[test]
    fn the_trace_is_drawn_inside_the_view_box() {
        let markup = trace_svg(&[
            point(0, 48.85, 2.35),
            point(1, 48.86, 2.36),
            point(2, 48.87, 2.35),
        ])
        .into_string();
        assert!(markup.contains("viewBox=\"0 0 1000 420\""), "{markup}");
        let points = polyline_points(&markup);
        assert_eq!(points.len(), 3);
        for (x, y) in &points {
            assert!((0.0..=1000.0).contains(x), "x hors cadre : {x}");
            assert!((0.0..=420.0).contains(y), "y hors cadre : {y}");
        }
        // Le nord est en haut : le point le plus au nord a le plus petit y.
        assert!(points[1].1 < points[0].1, "{points:?}");
    }

    #[test]
    fn a_stationary_watch_still_draws_a_point() {
        let markup = trace_svg(&[point(0, 48.85, 2.35)]).into_string();
        assert!(markup.contains("<circle"), "{markup}");
        let points = polyline_points(&markup);
        assert_eq!(points.len(), 1);
        // La trace immobile est centree dans le cadre.
        assert!((points[0].0 - 500.0).abs() < 1.0, "{points:?}");
        assert!((points[0].1 - 210.0).abs() < 1.0, "{points:?}");
    }

    #[test]
    fn the_current_position_closes_the_trace() {
        let markup = trace_svg(&[point(0, 48.0, 2.0), point(1, 48.1, 2.2)]).into_string();
        let points = polyline_points(&markup);
        let (x, y) = points.last().unwrap();
        assert!(markup.contains(&format!("cx=\"{x:.1}\"")), "{markup}");
        assert!(markup.contains(&format!("cy=\"{y:.1}\"")), "{markup}");
    }

    #[test]
    fn session_age_is_written_in_plain_french() {
        assert_eq!(since_label(1_000), "a l'instant");
        assert_eq!(since_label(12_000), "il y a 12 s");
        assert_eq!(since_label(180_000), "il y a 3 min");
        assert_eq!(since_label(7_200_000), "il y a 2 h");
        assert_eq!(since_label(-5), "a l'instant");
    }

    #[test]
    fn the_page_shows_the_device_its_state_and_a_map_link() {
        let status = MqttStatus::default();
        let markup = live_content(
            &[session(vec![point(10, 48.85, 2.35)])],
            &status,
            &config(),
            20,
        )
        .into_string();
        assert!(markup.contains("montre-a1b2"), "{markup}");
        assert!(markup.contains("en direct"), "{markup}");
        assert!(markup.contains("1.20 km"), "{markup}");
        assert!(markup.contains("5:00"), "allure formatee : {markup}");
        assert!(
            markup.contains("openstreetmap.org/?mlat=48.850000&amp;mlon=2.350000"),
            "{markup}"
        );
        // Sans broker configure, la page explique le reglage au lieu d'un vide.
        assert!(markup.contains("MPACER_MQTT_URL"), "{markup}");
    }

    #[test]
    fn without_any_session_the_page_says_so() {
        let markup = live_content(&[], &MqttStatus::default(), &config(), 1_000).into_string();
        assert!(markup.contains("Aucune position recue"), "{markup}");
        assert!(!markup.contains("<polyline"), "{markup}");
    }
}
