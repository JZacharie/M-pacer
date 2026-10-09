//! Recherche de courses a venir : source Finishers.com.
//!
//! Le calendrier publie sur <https://www.finishers.com/ou-courir/europe/france>
//! est servi par un index de recherche Typesense : la page `/courses` du site
//! interroge cet index depuis le navigateur avec une cle de recherche publique
//! (livree dans le JavaScript du site, elle n'autorise que la lecture). Ce module
//! interroge le meme index, directement et sans analyser de HTML :
//!
//! ```text
//! GET https://<hote>/collections/races/documents/search
//!     ?q=marathon&query_by=eventName,city
//!     &filter_by=countryCode:=FR && months:=4 && raceDistance:>=40000
//!     &group_by=eventId&group_limit=1&sort_by=boosted:desc,raceDate:asc
//!     &per_page=20&page=1
//! X-TYPESENSE-API-KEY: <cle de recherche>
//! ```
//!
//! Les donnees arrivent structurees (nom, ville, region, departement,
//! coordonnees, date d'edition, distances, denivele, discipline, liens), ce qui
//! permet de pre-remplir une fiche de course M-pacer sans rien inventer : tout ce
//! qui n'est pas publie reste vide et se complete a la main.
//!
//! La source est optionnelle et remplacable :
//! * `MPACER_FINISHERS_DISABLED=1` eteint la recherche (aucun appel reseau) ;
//! * `MPACER_FINISHERS_HOST` et `MPACER_FINISHERS_API_KEY` changent d'index.

use crate::config::Config;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

/// Hote Typesense du site Finishers (celui qu'interroge sa page `/courses`).
pub const DEFAULT_HOST: &str = "vn2qtcjsbg0ea481p.a1.typesense.net";
/// Cle de recherche publique de Finishers.
///
/// Elle est livree telle quelle dans le JavaScript du site et ne permet que la
/// lecture de l'index ; `MPACER_FINISHERS_API_KEY` la remplace si Finishers
/// publie une nouvelle cle.
pub const DEFAULT_API_KEY: &str = "G1BPjGr3KDU7n6yylcfOREpRVGUBpKYW";
/// Collection Typesense interrogee (une entree par edition de course).
pub const DEFAULT_COLLECTION: &str = "races";
/// Site public, pour reconstruire les liens absolus.
pub const SITE: &str = "https://www.finishers.com";
/// Pays par defaut : la source demandee est le calendrier francais.
pub const DEFAULT_COUNTRY: &str = "FR";

/// Nombre de resultats par page par defaut.
pub const DEFAULT_PER_PAGE: u32 = 20;
/// Plafond de resultats par page (l'index en accepte davantage, la page non).
pub const MAX_PER_PAGE: u32 = 50;
/// Plafond de pagination : au-dela, l'utilisateur affine sa recherche.
pub const MAX_PAGE: u32 = 100;

/// Regions francaises telles que les nomme Finishers (champ `level1`).
///
/// La liste est figee : `level1` n'est pas une facette de l'index, elle ne peut
/// donc pas etre decouverte par une requete. Les regions d'outre-mer ne figurent
/// pas au calendrier Finishers : leur recherche passe par le texte libre.
pub const FRENCH_REGIONS: [&str; 13] = [
    "Auvergne-Rhône-Alpes",
    "Bourgogne-Franche-Comté",
    "Bretagne",
    "Centre-Val de Loire",
    "Corse",
    "Grand Est",
    "Hauts-de-France",
    "Île-de-France",
    "Normandie",
    "Nouvelle-Aquitaine",
    "Occitanie",
    "Pays de la Loire",
    "Provence-Alpes-Côte d'Azur",
];

/// Disciplines publiees par Finishers : code de l'index, libelle francais.
pub const DISCIPLINES: [(&str, &str); 29] = [
    ("trail", "Trail"),
    ("road", "Route"),
    ("walking", "Marche"),
    ("obstacle_race", "Course a obstacles"),
    ("nordic", "Marche nordique"),
    ("cycling", "Cyclisme"),
    ("mountain_biking", "VTT"),
    ("cyclo_cross", "Cyclo-cross"),
    ("gravel_biking", "Gravel"),
    ("gravel_running", "Gravel running"),
    ("swimming", "Natation"),
    ("bike_and_run", "Bike and run"),
    ("cross", "Cross"),
    ("canoe", "Canoe"),
    ("ski_mountaineering", "Ski alpinisme"),
    ("cross_country_skiing", "Ski de fond"),
    ("fast_hiking", "Randonnee rapide"),
    ("other", "Autre"),
    ("aquathlon", "Aquathlon"),
    ("cross_triathlon", "Cross triathlon"),
    ("cyclathlon", "Cyclathlon"),
    ("duathlon", "Duathlon"),
    ("swimrun", "Swimrun"),
    ("swimbike", "Swim and bike"),
    ("triathlon", "Triathlon"),
    ("multisports", "Multisports"),
    ("fitnessrace", "Fitness race"),
    ("winter_duathlon", "Duathlon d'hiver"),
    ("winter_triathlon", "Triathlon d'hiver"),
];

/// Mois de l'annee, pour le filtre et l'affichage.
pub const MONTHS: [&str; 12] = [
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

/// Libelle francais d'une discipline de l'index.
pub fn discipline_label(code: &str) -> String {
    DISCIPLINES
        .iter()
        .find(|(value, _)| *value == code)
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| code.to_string())
}

/// Ordre de tri propose par la recherche.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Prochaines courses (les mises en avant d'abord), tri par defaut.
    Date,
    /// Distance croissante.
    Distance,
    /// Les plus populaires (finishers des editions passees).
    Popularity,
}

impl Sort {
    /// Valeur du parametre `tri`.
    pub fn code(self) -> &'static str {
        match self {
            Sort::Date => "date",
            Sort::Distance => "distance",
            Sort::Popularity => "popularite",
        }
    }

    /// Libelle du menu deroulant.
    pub fn label(self) -> &'static str {
        match self {
            Sort::Date => "Date (prochaines courses)",
            Sort::Distance => "Distance croissante",
            Sort::Popularity => "Popularite",
        }
    }

    /// Parametre `sort_by` envoye a Typesense.
    pub fn typesense(self) -> &'static str {
        match self {
            Sort::Date => "boosted:desc,raceDate:asc",
            Sort::Distance => "raceDistance:asc",
            Sort::Popularity => "popularity:desc",
        }
    }

    /// Les trois modes, dans l'ordre du menu.
    pub const ALL: [Sort; 3] = [Sort::Date, Sort::Distance, Sort::Popularity];
}

/// Parametres bruts d'une recherche, tels qu'ils arrivent dans l'URL.
///
/// Tous facultatifs et tous en texte : une valeur illisible produit un message
/// clair (voir [RaceSearchQuery::parse]) au lieu d'un refus brut du cadre HTTP.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RaceSearchParams {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub pays: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub departement: Option<String>,
    #[serde(default)]
    pub ville: Option<String>,
    #[serde(default)]
    pub discipline: Option<String>,
    #[serde(default)]
    pub mois: Option<String>,
    #[serde(default)]
    pub annee: Option<String>,
    #[serde(default)]
    pub dmin: Option<String>,
    #[serde(default)]
    pub dmax: Option<String>,
    #[serde(default)]
    pub tri: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub taille: Option<String>,
}

/// Recherche validee, prete a partir dans une requete Typesense.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceSearchQuery {
    /// Texte libre (`*` quand il est vide : tout le calendrier filtre).
    pub q: String,
    /// Code pays a deux lettres (`FR` par defaut).
    pub country: String,
    pub region: Option<String>,
    pub department: Option<String>,
    pub city: Option<String>,
    pub discipline: Option<String>,
    pub month: Option<u32>,
    pub year: Option<i32>,
    pub distance_min_m: Option<f64>,
    pub distance_max_m: Option<f64>,
    pub sort: Sort,
    pub page: u32,
    pub per_page: u32,
}

impl Default for RaceSearchQuery {
    fn default() -> Self {
        Self {
            q: String::new(),
            country: DEFAULT_COUNTRY.to_string(),
            region: None,
            department: None,
            city: None,
            discipline: None,
            month: None,
            year: None,
            distance_min_m: None,
            distance_max_m: None,
            sort: Sort::Date,
            page: 1,
            per_page: DEFAULT_PER_PAGE,
        }
    }
}

impl RaceSearchQuery {
    /// Traduit les parametres bruts en requete valide.
    ///
    /// Les valeurs hors bornes (mois 13, annee 1500, distance negative) sont
    /// refusees avec un message : une recherche silencieusement vide serait plus
    /// difficile a comprendre qu'une erreur explicite.
    pub fn parse(params: &RaceSearchParams) -> Result<Self, String> {
        let country = match text(&params.pays, 64) {
            Some(value) => value.to_uppercase(),
            None => DEFAULT_COUNTRY.to_string(),
        };
        if country.len() != 2 || !country.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err("le pays doit etre un code a deux lettres (FR, BE, CH...)".to_string());
        }
        let discipline = match text(&params.discipline, 40) {
            Some(code) if DISCIPLINES.iter().any(|(value, _)| *value == code) => Some(code),
            Some(code) => return Err(format!("discipline inconnue : {code}")),
            None => None,
        };
        let month = match text(&params.mois, 2) {
            Some(value) => {
                let month: u32 = value
                    .parse()
                    .map_err(|_| "le mois doit etre un nombre de 1 a 12".to_string())?;
                if !(1..=12).contains(&month) {
                    return Err("le mois doit etre compris entre 1 et 12".to_string());
                }
                Some(month)
            }
            None => None,
        };
        let year = match text(&params.annee, 4) {
            Some(value) => {
                let year: i32 = value
                    .parse()
                    .map_err(|_| "l'annee doit etre un nombre a quatre chiffres".to_string())?;
                if !(2000..=2100).contains(&year) {
                    return Err("l'annee doit etre comprise entre 2000 et 2100".to_string());
                }
                Some(year)
            }
            None => None,
        };
        let distance_min_m =
            parse_km(&params.dmin, "la distance minimale")?.map(|km| (km * 1000.0).round());
        let distance_max_m =
            parse_km(&params.dmax, "la distance maximale")?.map(|km| (km * 1000.0).round());
        if let (Some(min), Some(max)) = (distance_min_m, distance_max_m) {
            if min > max {
                return Err("la distance minimale depasse la distance maximale".to_string());
            }
        }
        let sort = match text(&params.tri, 20).as_deref() {
            None | Some("date") => Sort::Date,
            Some("distance") => Sort::Distance,
            Some("popularite") => Sort::Popularity,
            Some(other) => return Err(format!("tri inconnu : {other}")),
        };
        let per_page = match text(&params.taille, 3) {
            Some(value) => value
                .parse::<u32>()
                .map_err(|_| "la taille de page doit etre un nombre".to_string())?
                .clamp(1, MAX_PER_PAGE),
            None => DEFAULT_PER_PAGE,
        };
        let page = match text(&params.page, 4) {
            Some(value) => {
                let page: u32 = value
                    .parse()
                    .map_err(|_| "le numero de page doit etre un nombre".to_string())?;
                if page == 0 {
                    return Err("le numero de page commence a 1".to_string());
                }
                page.min(MAX_PAGE)
            }
            None => 1,
        };
        Ok(Self {
            q: text(&params.q, 120).unwrap_or_default(),
            country,
            region: text(&params.region, 80),
            department: text(&params.departement, 80),
            city: text(&params.ville, 80),
            discipline,
            month,
            year,
            distance_min_m,
            distance_max_m,
            sort,
            page,
            per_page,
        })
    }

    /// Texte envoye a Typesense (`*` quand la recherche est vide).
    pub fn keyword(&self) -> &str {
        let trimmed = self.q.trim();
        if trimmed.is_empty() {
            "*"
        } else {
            trimmed
        }
    }

    /// Filtre `filter_by` : un `&&` par critere renseigne.
    pub fn filter_by(&self) -> String {
        let mut filters = vec![format!("countryCode:={}", escape(&self.country))];
        if let Some(region) = &self.region {
            filters.push(format!("level1:={}", quote(region)));
        }
        if let Some(department) = &self.department {
            filters.push(format!("level2:={}", quote(department)));
        }
        if let Some(city) = &self.city {
            filters.push(format!("city:={}", quote(city)));
        }
        if let Some(discipline) = &self.discipline {
            filters.push(format!("raceDiscipline:={}", escape(discipline)));
        }
        if let Some(month) = self.month {
            filters.push(format!("months:={month}"));
        }
        if let Some(year) = self.year {
            if let Some((start, end)) = year_bounds(year) {
                filters.push(format!("raceDate:>={start} && raceDate:<={end}"));
            }
        }
        if let Some(min) = self.distance_min_m {
            filters.push(format!("raceDistance:>={min:.0}"));
        }
        if let Some(max) = self.distance_max_m {
            filters.push(format!("raceDistance:<={max:.0}"));
        }
        filters.join(" && ")
    }

    /// Nombre de pages annonce par la source (au moins 1).
    pub fn pages(&self, found: u64) -> u32 {
        let per_page = u64::from(self.per_page.max(1));
        found.div_ceil(per_page).clamp(1, u64::from(u32::MAX)) as u32
    }
}

/// Texte nettoye (`None` si vide), borne a `max` caracteres.
fn text(value: &Option<String>, max: usize) -> Option<String> {
    let trimmed = value.as_deref()?.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(max).collect())
}

/// Distance en kilometres, convertie depuis un champ de formulaire.
fn parse_km(value: &Option<String>, label: &str) -> Result<Option<f64>, String> {
    let Some(raw) = text(value, 12) else {
        return Ok(None);
    };
    let number: f64 = raw
        .replace(',', ".")
        .parse()
        .map_err(|_| format!("{label} doit etre un nombre"))?;
    if !number.is_finite() || number < 0.0 {
        return Err(format!("{label} doit etre positive"));
    }
    if number > 1000.0 {
        return Err(format!("{label} ne peut pas depasser 1000 km"));
    }
    Ok(Some(number))
}

/// Bornes UNIX (s) d'une annee civile, pour le filtre `raceDate`.
fn year_bounds(year: i32) -> Option<(i64, i64)> {
    let start = chrono::NaiveDate::from_ymd_opt(year, 1, 1)?.and_hms_opt(0, 0, 0)?;
    let end = chrono::NaiveDate::from_ymd_opt(year, 12, 31)?.and_hms_opt(23, 59, 59)?;
    Some((start.and_utc().timestamp(), end.and_utc().timestamp()))
}

/// Echappe une valeur de filtre Typesense.
fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('`', "\\`")
}

/// Encadre une valeur de filtre pouvant contenir espaces, accents ou apostrophes.
fn quote(value: &str) -> String {
    format!("`{}`", escape(value))
}

/// Une course a venir, telle que la publie Finishers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FinishersRace {
    /// Identifiant de l'epreuve (`eventId`), stable d'une annee a l'autre.
    pub id: String,
    pub name: String,
    pub city: Option<String>,
    /// Region administrative (`level1`).
    pub region: Option<String>,
    /// Departement (`level2`).
    pub department: Option<String>,
    pub country: Option<String>,
    /// Debut de l'edition, a minuit heure locale (ms UNIX).
    pub start_at_ms: Option<i64>,
    /// Fin de l'edition (courses a etapes), meme convention.
    pub end_at_ms: Option<i64>,
    /// Date de debut telle que publiee (`AAAA-MM-JJ`), sans ambiguite de fuseau.
    pub start_date: Option<String>,
    /// Distance de la course mise en avant (m).
    pub distance_m: Option<f64>,
    /// Toutes les distances proposees (m), croissantes et sans doublon.
    pub distances_m: Vec<f64>,
    /// Code de discipline de l'index (`trail`, `road`...).
    pub discipline: Option<String>,
    /// Libelle francais de la discipline.
    pub discipline_label: Option<String>,
    /// Denivele positif annonce (m).
    pub elevation_gain_m: Option<f64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// Finishers de la derniere edition, quand le chiffre est publie.
    pub finishers: Option<i64>,
    /// Statut de l'edition (`confirmed`, `tba`, `cancelled`...).
    pub status: Option<String>,
    /// Fiche Finishers de l'epreuve.
    pub url: String,
    /// Page d'inscription, absolue.
    pub registration_url: Option<String>,
    /// Thematiques publiees par Finishers.
    pub tags: Vec<String>,
}

impl FinishersRace {
    /// Distance lisible : `42,2 km`.
    pub fn distance_label(&self) -> Option<String> {
        self.distance_m.map(format_distance_km)
    }

    /// Toutes les distances proposees, en kilometres.
    pub fn distances_labels(&self) -> Vec<String> {
        self.distances_m
            .iter()
            .map(|meters| format_distance_km(*meters))
            .collect()
    }
}

/// Distance en kilometres, sans zero inutile (`10 km`, `42,2 km`).
pub fn format_distance_km(meters: f64) -> String {
    let km = meters / 1000.0;
    let text = if (km - km.round()).abs() < 0.05 {
        format!("{}", km.round() as i64)
    } else {
        let mut value = format!("{km:.1}");
        if value.ends_with('0') {
            value.pop();
        }
        if value.ends_with('.') {
            value.pop();
        }
        value.replace('.', ",")
    };
    format!("{text} km")
}

/// Reponse d'une recherche : les courses de la page et le total disponible.
#[derive(Debug, Clone, Serialize)]
pub struct RaceSearchResponse {
    /// Source des donnees, affichee par l'interface.
    pub source: &'static str,
    /// Nombre total de courses correspondant a la recherche.
    pub total: u64,
    pub page: u32,
    pub pages: u32,
    pub per_page: u32,
    pub items: Vec<FinishersRace>,
}

/// Reponse brute de Typesense (seuls les champs utilises sont declares).
///
/// Avec `group_by=eventId`, l'index range les documents sous `grouped_hits`
/// (un groupe par epreuve) ; sans regroupement, ils sont sous `hits`.
#[derive(Debug, Deserialize)]
struct TsResponse {
    #[serde(default)]
    found: u64,
    #[serde(default)]
    hits: Vec<TsHit>,
    #[serde(default)]
    grouped_hits: Vec<TsGroup>,
}

#[derive(Debug, Deserialize)]
struct TsGroup {
    #[serde(default)]
    hits: Vec<TsHit>,
}

#[derive(Debug, Deserialize)]
struct TsHit {
    document: TsDocument,
}

impl TsResponse {
    /// Tous les documents de la reponse, groupes ou non.
    fn into_documents(self) -> Vec<TsDocument> {
        let TsResponse {
            hits, grouped_hits, ..
        } = self;
        let mut documents: Vec<TsDocument> = hits.into_iter().map(|hit| hit.document).collect();
        for group in grouped_hits {
            documents.extend(group.hits.into_iter().map(|hit| hit.document));
        }
        documents
    }
}

/// Document `races` de l'index Finishers.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct TsDocument {
    #[serde(rename = "eventId")]
    event_id: Option<String>,
    #[serde(rename = "eventName")]
    event_name: Option<String>,
    #[serde(rename = "eventSlug")]
    event_slug: Option<String>,
    city: Option<String>,
    level1: Option<String>,
    level2: Option<String>,
    country: Option<String>,
    coordinates: Option<Vec<f64>>,
    #[serde(rename = "editionStartDate")]
    edition_start_date: Option<String>,
    #[serde(rename = "editionEndDate")]
    edition_end_date: Option<String>,
    #[serde(rename = "editionStatus")]
    edition_status: Option<String>,
    #[serde(rename = "raceDate")]
    race_date: Option<i64>,
    #[serde(rename = "raceDistance")]
    race_distance: Option<f64>,
    #[serde(rename = "raceDistanceUnit")]
    race_distance_unit: Option<String>,
    #[serde(rename = "raceDistanceVariants")]
    race_distance_variants: Option<Vec<Option<f64>>>,
    #[serde(rename = "raceDiscipline")]
    race_discipline: Option<String>,
    #[serde(rename = "raceElevationGain")]
    race_elevation_gain: Option<f64>,
    #[serde(rename = "eventLastEditionFinisherCount")]
    finisher_count: Option<i64>,
    #[serde(rename = "eventRegistrationUrl")]
    registration_url: Option<String>,
    #[serde(rename = "eventTags")]
    tags: Option<Vec<String>>,
}

impl TsDocument {
    /// Convertit un document en course ; `None` s'il manque l'identite de l'epreuve.
    fn into_race(self) -> Option<FinishersRace> {
        let id = self.event_id.clone()?;
        let name = self.event_name.clone()?;
        if id.is_empty() || name.is_empty() {
            return None;
        }
        let start_date = normalized_date(self.edition_start_date.as_deref());
        let end_date = normalized_date(self.edition_end_date.as_deref());
        let miles = self
            .race_distance_unit
            .as_deref()
            .is_some_and(|unit| !unit.eq_ignore_ascii_case("meters"));
        let distance_m = if miles { None } else { self.race_distance };
        let mut distances_m: Vec<f64> = self
            .race_distance_variants
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .filter(|value| value.is_finite() && *value > 0.0)
            .collect();
        if let Some(distance) = distance_m {
            distances_m.push(distance);
        }
        distances_m.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        distances_m.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        let coordinates = self.coordinates.unwrap_or_default();
        let (latitude, longitude) = match coordinates.as_slice() {
            [latitude, longitude, ..] => (Some(*latitude), Some(*longitude)),
            _ => (None, None),
        };
        let start_at_ms = start_date
            .as_deref()
            .and_then(local_midnight_ms)
            .or_else(|| self.race_date.map(|seconds| seconds * 1000));
        let end_at_ms = end_date.as_deref().and_then(local_midnight_ms);
        let url = self
            .event_slug
            .as_deref()
            .filter(|slug| !slug.is_empty())
            .map(|slug| format!("{SITE}/course/{slug}"))
            .unwrap_or_else(|| format!("{SITE}/courses"));
        let discipline = self.race_discipline.filter(|code| !code.is_empty());
        Some(FinishersRace {
            id,
            name,
            city: non_empty(self.city),
            region: non_empty(self.level1),
            department: non_empty(self.level2),
            country: non_empty(self.country),
            start_at_ms,
            end_at_ms,
            start_date,
            distance_m,
            distances_m,
            discipline_label: discipline.as_deref().map(discipline_label),
            discipline,
            elevation_gain_m: self.race_elevation_gain.filter(|value| *value > 0.0),
            latitude,
            longitude,
            finishers: self.finisher_count.filter(|count| *count > 0),
            status: non_empty(self.edition_status),
            url,
            registration_url: self.registration_url.as_deref().and_then(absolute_url),
            tags: self.tags.unwrap_or_default(),
        })
    }
}

/// `None` pour une chaine vide ou blanche.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

/// Resout un lien relatif du site (`/book/event/xyz`) en lien absolu.
fn absolute_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("//") {
        return Some(format!("https://{rest}"));
    }
    if trimmed.starts_with('/') {
        return Some(format!("{SITE}{trimmed}"));
    }
    Some(format!("{SITE}/{trimmed}"))
}

/// Verifie qu'une date est bien `AAAA-MM-JJ` et la renvoie telle quelle.
fn normalized_date(value: Option<&str>) -> Option<String> {
    let date = chrono::NaiveDate::parse_from_str(value?.trim(), "%Y-%m-%d").ok()?;
    Some(date.format("%Y-%m-%d").to_string())
}

/// Minuit local d'une date `AAAA-MM-JJ`, en millisecondes UNIX.
///
/// M-pacer enregistre les horaires en heure locale du serveur : une course dont
/// seul le jour est connu doit s'afficher comme une date, pas comme un rendez-vous
/// a deux heures du matin.
fn local_midnight_ms(date: &str) -> Option<i64> {
    let naive = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()?
        .and_hms_opt(0, 0, 0)?;
    naive
        .and_local_timezone(chrono::Local)
        .earliest()
        .map(|value| value.timestamp_millis())
}

/// Client de l'index de recherche Finishers.
#[derive(Debug, Clone)]
pub struct Finishers {
    http: reqwest::Client,
    /// Base de l'API, sans barre oblique finale.
    base: String,
    api_key: String,
    collection: String,
    enabled: bool,
}

impl Finishers {
    /// Client configure depuis l'environnement.
    pub fn from_config(http: reqwest::Client, config: &Config) -> Self {
        let enabled = config.finishers_enabled && !config.finishers_api_key.trim().is_empty();
        Self {
            http,
            base: normalize_host(&config.finishers_host),
            api_key: config.finishers_api_key.trim().to_string(),
            collection: config.finishers_collection.trim().to_string(),
            enabled,
        }
    }

    /// Client eteint : aucune requete ne part, l'interface l'annonce.
    pub fn disabled() -> Self {
        Self {
            http: reqwest::Client::new(),
            base: normalize_host(DEFAULT_HOST),
            api_key: String::new(),
            collection: DEFAULT_COLLECTION.to_string(),
            enabled: false,
        }
    }

    /// Vrai si la recherche peut interroger l'index.
    pub fn configured(&self) -> bool {
        self.enabled
    }

    /// Interroge l'index et renvoie la page demandee.
    pub async fn search(&self, query: &RaceSearchQuery) -> AppResult<RaceSearchResponse> {
        let response = self
            .request(&[
                ("q", query.keyword().to_string()),
                ("query_by", "eventName,city".to_string()),
                ("filter_by", query.filter_by()),
                ("sort_by", query.sort.typesense().to_string()),
                ("group_by", "eventId".to_string()),
                ("group_limit", "1".to_string()),
                (
                    "per_page",
                    query.per_page.clamp(1, MAX_PER_PAGE).to_string(),
                ),
                ("page", query.page.max(1).to_string()),
            ])
            .await?;
        let found = response.found;
        let items: Vec<FinishersRace> = response
            .into_documents()
            .into_iter()
            .filter_map(TsDocument::into_race)
            .collect();
        Ok(RaceSearchResponse {
            source: "finishers.com",
            total: found,
            page: query.page,
            pages: query.pages(found),
            per_page: query.per_page,
            items,
        })
    }

    /// Retrouve une epreuve par son identifiant Finishers (`eventId`).
    pub async fn event(&self, event_id: &str) -> AppResult<Option<FinishersRace>> {
        let id = event_id.trim();
        if id.is_empty() || id.len() > 64 {
            return Ok(None);
        }
        let response = self
            .request(&[
                ("q", "*".to_string()),
                ("query_by", "eventName".to_string()),
                ("filter_by", format!("eventId:={}", escape(id))),
                ("per_page", "1".to_string()),
                ("page", "1".to_string()),
            ])
            .await?;
        Ok(response
            .into_documents()
            .into_iter()
            .filter_map(TsDocument::into_race)
            .next())
    }

    /// Appel `documents/search`, avec la cle de recherche en en-tete.
    async fn request(&self, params: &[(&str, String)]) -> AppResult<TsResponse> {
        if !self.enabled {
            return Err(AppError::unavailable(
                "la recherche Finishers est desactivee sur ce service",
            ));
        }
        let url = format!(
            "{}/collections/{}/documents/search",
            self.base, self.collection
        );
        let response = self
            .http
            .get(&url)
            .header("X-TYPESENSE-API-KEY", &self.api_key)
            .query(params)
            .send()
            .await
            .map_err(|error| {
                AppError::unavailable(format!("recherche Finishers injoignable : {error}"))
            })?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            tracing::warn!(%status, body = %body, "recherche Finishers refusee");
            return Err(AppError::unavailable(
                "la source Finishers a refuse la recherche",
            ));
        }
        response.json::<TsResponse>().await.map_err(|error| {
            AppError::unavailable(format!("reponse Finishers illisible : {error}"))
        })
    }
}

/// Base d'API : `https://` ajoute si l'hote est donne sans schema.
fn normalize_host(host: &str) -> String {
    let trimmed = host.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return format!("https://{DEFAULT_HOST}");
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(pairs: &[(&str, &str)]) -> RaceSearchParams {
        let mut params = RaceSearchParams::default();
        for (key, value) in pairs {
            let slot = match *key {
                "q" => &mut params.q,
                "pays" => &mut params.pays,
                "region" => &mut params.region,
                "departement" => &mut params.departement,
                "ville" => &mut params.ville,
                "discipline" => &mut params.discipline,
                "mois" => &mut params.mois,
                "annee" => &mut params.annee,
                "dmin" => &mut params.dmin,
                "dmax" => &mut params.dmax,
                "tri" => &mut params.tri,
                "page" => &mut params.page,
                "taille" => &mut params.taille,
                other => panic!("parametre inconnu : {other}"),
            };
            *slot = Some((*value).to_string());
        }
        params
    }

    #[test]
    fn empty_search_targets_france_and_sorts_by_date() {
        let query = RaceSearchQuery::parse(&RaceSearchParams::default()).unwrap();
        assert_eq!(query.keyword(), "*");
        assert_eq!(query.filter_by(), "countryCode:=FR");
        assert_eq!(query.sort, Sort::Date);
        assert_eq!(query.page, 1);
        assert_eq!(query.per_page, DEFAULT_PER_PAGE);
        assert_eq!(Sort::Date.typesense(), "boosted:desc,raceDate:asc");
    }

    #[test]
    fn every_criterion_reaches_the_filter() {
        let query = RaceSearchQuery::parse(&params(&[
            ("q", "marathon"),
            ("region", "Provence-Alpes-Côte d'Azur"),
            ("departement", "Bouches-du-Rhone"),
            ("ville", "Marseille"),
            ("discipline", "trail"),
            ("mois", "4"),
            ("annee", "2027"),
            ("dmin", "40"),
            ("dmax", "50,5"),
            ("tri", "distance"),
        ]))
        .unwrap();
        let filter = query.filter_by();
        assert!(filter.starts_with("countryCode:=FR"), "{filter}");
        assert!(
            filter.contains("level1:=`Provence-Alpes-Côte d'Azur`"),
            "{filter}"
        );
        assert!(filter.contains("level2:=`Bouches-du-Rhone`"), "{filter}");
        assert!(filter.contains("city:=`Marseille`"), "{filter}");
        assert!(filter.contains("raceDiscipline:=trail"), "{filter}");
        assert!(filter.contains("months:=4"), "{filter}");
        assert!(filter.contains("raceDistance:>=40000"), "{filter}");
        assert!(filter.contains("raceDistance:<=50500"), "{filter}");
        assert_eq!(query.sort, Sort::Distance);
        assert_eq!(query.keyword(), "marathon");
    }

    #[test]
    fn a_year_becomes_a_date_range() {
        let query = RaceSearchQuery::parse(&params(&[("annee", "2027")])).unwrap();
        let (start, end) = year_bounds(2027).unwrap();
        assert_eq!(start, 1_798_761_600, "1er janvier 2027");
        assert_eq!(end, 1_830_297_599, "31 decembre 2027 a 23:59:59 UTC");
        let filter = query.filter_by();
        assert!(filter.contains(&format!("raceDate:>={start}")), "{filter}");
        assert!(filter.contains(&format!("raceDate:<={end}")), "{filter}");
    }

    #[test]
    fn backticks_and_backslashes_are_escaped() {
        let query = RaceSearchQuery::parse(&params(&[("ville", "Saint-`Etienne`")])).unwrap();
        assert!(
            query.filter_by().contains("city:=`Saint-\\`Etienne\\``"),
            "{}",
            query.filter_by()
        );
        assert_eq!(escape("a\\b"), "a\\\\b");
    }

    #[test]
    fn nonsense_values_are_refused_with_a_clear_message() {
        assert!(RaceSearchQuery::parse(&params(&[("mois", "13")])).is_err());
        assert!(RaceSearchQuery::parse(&params(&[("mois", "janvier")])).is_err());
        assert!(RaceSearchQuery::parse(&params(&[("annee", "1500")])).is_err());
        assert!(RaceSearchQuery::parse(&params(&[("discipline", "petanque")])).is_err());
        assert!(RaceSearchQuery::parse(&params(&[("tri", "aleatoire")])).is_err());
        assert!(RaceSearchQuery::parse(&params(&[("page", "0")])).is_err());
        assert!(RaceSearchQuery::parse(&params(&[("dmin", "-3")])).is_err());
        assert!(RaceSearchQuery::parse(&params(&[("dmin", "50"), ("dmax", "10")])).is_err());
        assert!(RaceSearchQuery::parse(&params(&[("pays", "France")])).is_err());
    }

    #[test]
    fn the_page_size_is_bounded() {
        let query = RaceSearchQuery::parse(&params(&[("taille", "500")])).unwrap();
        assert_eq!(query.per_page, MAX_PER_PAGE);
        let query = RaceSearchQuery::parse(&params(&[("page", "99999")])).unwrap();
        assert_eq!(query.page, MAX_PAGE);
        assert_eq!(query.pages(0), 1);
        assert_eq!(query.pages(21), 2, "21 courses sur des pages de 20");
    }

    #[test]
    fn a_typesense_document_becomes_a_race() {
        let document: TsDocument = serde_json::from_str(
            r#"{
              "eventId": "4268923e-3361-4956-a73f-2956610bac26",
              "eventName": "Asics Marathon de Paris",
              "eventSlug": "marathon-de-paris",
              "city": "Paris",
              "level1": "Ile-de-France",
              "level2": "Paris",
              "country": "France",
              "coordinates": [48.8583701, 2.2944813, 0],
              "editionStartDate": "2027-04-04",
              "editionEndDate": "2027-04-04",
              "editionStatus": "confirmed",
              "raceDate": 1806796800,
              "raceDistance": 4000,
              "raceDistanceUnit": "meters",
              "raceDistanceVariants": [4000, null, 42195, 42195],
              "raceDiscipline": "road",
              "raceElevationGain": 120,
              "eventLastEditionFinisherCount": 47468,
              "eventRegistrationUrl": "/book/event/4WNBE",
              "eventTags": ["city", "monuments"]
            }"#,
        )
        .unwrap();
        let race = document.into_race().unwrap();
        assert_eq!(race.id, "4268923e-3361-4956-a73f-2956610bac26");
        assert_eq!(race.name, "Asics Marathon de Paris");
        assert_eq!(race.city.as_deref(), Some("Paris"));
        assert_eq!(race.region.as_deref(), Some("Ile-de-France"));
        assert_eq!(race.start_date.as_deref(), Some("2027-04-04"));
        assert_eq!(race.distance_m, Some(4000.0));
        assert_eq!(race.distances_m, vec![4000.0, 42195.0]);
        assert_eq!(race.discipline.as_deref(), Some("road"));
        assert_eq!(race.discipline_label.as_deref(), Some("Route"));
        assert_eq!(race.latitude, Some(48.8583701));
        assert_eq!(race.longitude, Some(2.2944813));
        assert_eq!(race.finishers, Some(47468));
        assert_eq!(
            race.url,
            "https://www.finishers.com/course/marathon-de-paris"
        );
        assert_eq!(
            race.registration_url.as_deref(),
            Some("https://www.finishers.com/book/event/4WNBE")
        );
        assert_eq!(race.distance_label().as_deref(), Some("4 km"));
        assert_eq!(format_distance_km(42195.0), "42,2 km");
        assert_eq!(format_distance_km(10000.0), "10 km");
        assert_eq!(format_distance_km(21100.0), "21,1 km");
    }

    #[test]
    fn the_edition_date_is_local_midnight() {
        let document: TsDocument = serde_json::from_str(
            r#"{"eventId":"x","eventName":"Course","editionStartDate":"2027-04-04"}"#,
        )
        .unwrap();
        let race = document.into_race().unwrap();
        let date = chrono::DateTime::from_timestamp_millis(race.start_at_ms.unwrap())
            .unwrap()
            .with_timezone(&chrono::Local);
        assert_eq!(
            date.format("%Y-%m-%d %H:%M").to_string(),
            "2027-04-04 00:00",
            "une date sans horaire reste une date"
        );
    }

    #[test]
    fn a_grouped_response_is_read_like_a_flat_one() {
        let response: TsResponse = serde_json::from_str(
            r#"{
              "found": 2,
              "grouped_hits": [
                {"group_key": ["a"], "found": 1, "hits": [{"document": {"eventId": "a", "eventName": "Course A"}}]},
                {"group_key": ["b"], "found": 1, "hits": [{"document": {"eventId": "b", "eventName": "Course B"}}]}
              ]
            }"#,
        )
        .unwrap();
        let names: Vec<String> = response
            .into_documents()
            .into_iter()
            .filter_map(TsDocument::into_race)
            .map(|race| race.name)
            .collect();
        assert_eq!(names, vec!["Course A".to_string(), "Course B".to_string()]);
    }

    #[test]
    fn a_document_without_a_name_is_ignored() {
        let document: TsDocument = serde_json::from_str(r#"{"eventId":"x"}"#).unwrap();
        assert!(document.into_race().is_none());
    }

    #[test]
    fn distances_in_miles_are_not_taken_for_meters() {
        let document: TsDocument = serde_json::from_str(
            r#"{"eventId":"x","eventName":"Course","raceDistance":26.2,"raceDistanceUnit":"miles"}"#,
        )
        .unwrap();
        let race = document.into_race().unwrap();
        assert_eq!(race.distance_m, None);
    }

    #[test]
    fn hosts_are_normalised() {
        assert_eq!(normalize_host(""), format!("https://{DEFAULT_HOST}"));
        assert_eq!(normalize_host("  exemple.org/ "), "https://exemple.org");
        assert_eq!(
            normalize_host("http://127.0.0.1:8108"),
            "http://127.0.0.1:8108"
        );
    }

    #[tokio::test]
    async fn a_disabled_client_never_calls_the_network() {
        let client = Finishers::disabled();
        assert!(!client.configured());
        assert!(client.event("abc").await.is_err(), "aucune requete ne part");
        assert!(client.search(&RaceSearchQuery::default()).await.is_err());
    }

    #[test]
    fn every_discipline_has_a_label() {
        for (code, label) in DISCIPLINES {
            assert_eq!(discipline_label(code), label);
        }
        assert_eq!(discipline_label("inconnue"), "inconnue");
    }
}
