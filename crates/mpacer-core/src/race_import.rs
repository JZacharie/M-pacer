//! Import d'une ancienne course depuis un export Strava ou Garmin (GPX ou TCX).
//!
//! Le coureur exporte une course deja courue depuis Strava ou Garmin Connect
//! (formats GPX et TCX, les deux exports proposes par les deux services), puis
//! la depose dans M-pacer : elle devient une **course de reference**, rangee
//! avec les courses deja courues, avec sa date, sa distance, ses temps et son
//! denivele. Aucune donnee n'est inventee : ce que le fichier ne dit pas reste
//! vide.
//!
//! Le module est pur (aucune E/S) : il prend le nom du fichier et son contenu.
//! Le lecteur XML est minimal et volontairement tolerant (prefixes de namespace,
//! attributs dans n'importe quel ordre, espaces libres) : il n'existe que pour
//! ces deux formats, donc aucune dependance XML n'est ajoutee au coeur.

use crate::best_distances::TrackPoint;
use crate::geo::{haversine_m, Position};
use serde::{Deserialize, Serialize};

/// Taille maximale acceptee pour un fichier importe (octets).
///
/// Un GPX de marathon pese environ 1,5 Mo ; au-dela de 16 Mo le fichier n'est
/// plus un export de course.
pub const MAX_IMPORT_BYTES: usize = 16 * 1024 * 1024;

/// Nombre maximal de points conserves dans la trace stockee.
///
/// Au-dela, la trace est echantillonnee uniformement : un profil d'altitude
/// reste juste, et la fiche ne stocke pas des dizaines de milliers de lignes.
pub const MAX_TRACK_POINTS: usize = 2_000;

/// Vitesse minimale consideree comme du mouvement (m/s) : 0,5 m/s, soit environ
/// 1,8 km/h. En dessous, c'est un arret, pas une allure de course.
const MOVING_SPEED_MPS: f64 = 0.5;

/// Ecart maximal entre deux points pour compter du temps (s) : au-dela, c'est
/// une coupure GPS (ou une pause longue), pas une portion courue.
const MAX_GAP_S: f64 = 120.0;

/// Origine d'une course importee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportSource {
    /// Export GPX de Strava (creator StravaGPX).
    Strava,
    /// Export GPX ou TCX de Garmin Connect.
    Garmin,
    /// GPX d'une autre origine.
    Gpx,
    /// TCX d'une autre origine.
    Tcx,
}

impl ImportSource {
    /// Code stable stocke en base.
    pub fn as_str(self) -> &'static str {
        match self {
            ImportSource::Strava => "strava",
            ImportSource::Garmin => "garmin",
            ImportSource::Gpx => "gpx",
            ImportSource::Tcx => "tcx",
        }
    }

    /// Libelle affiche.
    pub fn label(self) -> &'static str {
        match self {
            ImportSource::Strava => "Strava",
            ImportSource::Garmin => "Garmin Connect",
            ImportSource::Gpx => "GPX",
            ImportSource::Tcx => "TCX",
        }
    }

    /// Relit un code stocke en base.
    pub fn from_code(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "strava" => Some(ImportSource::Strava),
            "garmin" => Some(ImportSource::Garmin),
            "gpx" => Some(ImportSource::Gpx),
            "tcx" => Some(ImportSource::Tcx),
            _ => None,
        }
    }

    fn from_creator(creator: &str) -> Option<Self> {
        let lowered = creator.to_ascii_lowercase();
        if lowered.contains("strava") {
            Some(ImportSource::Strava)
        } else if lowered.contains("garmin") {
            Some(ImportSource::Garmin)
        } else {
            None
        }
    }
}

/// Erreur d'import, avec un message affichable tel quel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// Le fichier est trop volumineux pour etre une trace de course.
    TooLarge,
    /// Format inconnu (FIT binaire, CSV, fichier HTML d'une page de connexion...).
    UnsupportedFormat(String),
    /// Le fichier est lisible mais ne contient aucun point de trace.
    NoTrack,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::TooLarge => write!(
                formatter,
                "fichier trop volumineux ({} Mo maximum)",
                MAX_IMPORT_BYTES / (1024 * 1024)
            ),
            ImportError::UnsupportedFormat(detail) => write!(
                formatter,
                "format non pris en charge : {detail} (exportez la course en GPX ou TCX)"
            ),
            ImportError::NoTrack => write!(
                formatter,
                "aucun point de trace dans le fichier : ce n'est pas un export de course"
            ),
        }
    }
}

impl std::error::Error for ImportError {}

/// Course relue dans un export, prete a etre enregistree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportedRace {
    /// Nom de la trace (nom du fichier a defaut).
    pub name: String,
    /// Origine detectee dans le fichier.
    pub source: ImportSource,
    /// Debut de la course (ms UNIX), absent si le fichier ne porte pas d'heure.
    pub started_at_ms: Option<i64>,
    /// Distance totale (m).
    pub distance_m: f64,
    /// Temps en mouvement (s), pauses exclues, absent sans horodatage.
    pub moving_time_s: Option<f64>,
    /// Temps ecoule entre le premier et le dernier point (s).
    pub elapsed_time_s: Option<f64>,
    /// Denivele positif cumule (m).
    pub elevation_gain_m: f64,
    /// Trace echantillonnee, temps relatifs au depart.
    pub points: Vec<TrackPoint>,
}

impl ImportedRace {
    /// Allure moyenne de la course (s/km), sur le temps en mouvement.
    pub fn average_pace_s_per_km(&self) -> Option<f64> {
        let moving_s = self.moving_time_s?;
        if self.distance_m <= 0.0 || moving_s <= 0.0 {
            return None;
        }
        Some(moving_s / (self.distance_m / 1000.0))
    }

    /// GPX normalise de la trace importee (conservation et reexport).
    pub fn to_gpx(&self) -> String {
        crate::gpx::export_points_gpx(&self.name, &self.points, self.started_at_ms)
    }
}

/// Importe une course depuis le contenu brut d'un fichier.
pub fn import_race(filename: &str, bytes: &[u8]) -> Result<ImportedRace, ImportError> {
    if bytes.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        ImportError::UnsupportedFormat(
            "le fichier n'est pas du texte XML (le FIT binaire n'est pas lu)".to_string(),
        )
    })?;
    import_race_text(filename, text)
}

/// Importe une course depuis un export deja decode en texte.
pub fn import_race_text(filename: &str, text: &str) -> Result<ImportedRace, ImportError> {
    let lowered = filename.to_ascii_lowercase();
    let looks_like_tcx = lowered.ends_with(".tcx") || text.contains("TrainingCenterDatabase");
    let looks_like_gpx = lowered.ends_with(".gpx") || text.contains("<gpx");
    if looks_like_tcx {
        return parse_tcx(filename, text);
    }
    if looks_like_gpx {
        return parse_gpx(filename, text);
    }
    Err(ImportError::UnsupportedFormat(
        "le fichier n'est ni un GPX ni un TCX".to_string(),
    ))
}

/// Point brut relu dans un fichier, avant calcul des temps et distances.
#[derive(Debug, Clone, Copy)]
struct RawPoint {
    lat: f64,
    lon: f64,
    elevation_m: Option<f64>,
    time_ms: Option<i64>,
    /// Distance cumulee fournie par le fichier (TCX), absente en GPX.
    distance_m: Option<f64>,
}

// ------------------------------------------------------------------ GPX

fn parse_gpx(filename: &str, text: &str) -> Result<ImportedRace, ImportError> {
    let root = elements(text, "gpx");
    let mut source = ImportSource::Gpx;
    if let Some(root) = root.first() {
        if let Some(creator) = attr(&root.attrs, "creator") {
            source = ImportSource::from_creator(creator).unwrap_or(ImportSource::Gpx);
        }
    }

    let name = element_text(text, "metadata", "name")
        .or_else(|| element_text(text, "trk", "name"))
        .map(str::to_string);

    let mut raw = Vec::new();
    let segments = elements(text, "trkseg");
    if segments.is_empty() {
        // GPX sans trkseg (trace a plat) : on relit tous les points du document.
        collect_gpx_points(text, &mut raw);
    } else {
        for segment in &segments {
            collect_gpx_points(segment.inner, &mut raw);
        }
    }

    let metadata_time =
        element_text(text, "metadata", "time").and_then(crate::gpx::parse_iso8601_utc);
    build(filename, name, source, raw, None, metadata_time)
}

fn collect_gpx_points(xml: &str, raw: &mut Vec<RawPoint>) {
    for point in elements(xml, "trkpt") {
        let Some(lat) = attr(&point.attrs, "lat").and_then(parse_f64) else {
            continue;
        };
        let Some(lon) = attr(&point.attrs, "lon").and_then(parse_f64) else {
            continue;
        };
        let elevation_m = first_text(&point, "ele").and_then(parse_f64);
        let time_ms = first_text(&point, "time").and_then(crate::gpx::parse_iso8601_utc);
        raw.push(RawPoint {
            lat,
            lon,
            elevation_m,
            time_ms,
            distance_m: None,
        });
    }
}

// ------------------------------------------------------------------ TCX

fn parse_tcx(filename: &str, text: &str) -> Result<ImportedRace, ImportError> {
    let creator = element_text(text, "Creator", "Name").unwrap_or("");
    let source = ImportSource::from_creator(creator).unwrap_or(ImportSource::Tcx);

    // Garmin Connect ne nomme pas toujours l'activite : le nom du fichier reste
    // alors la seule information disponible (ex. "Marathon de Bordeaux.tcx").
    let name = element_text(text, "Activity", "Notes").map(str::to_string);

    let mut raw = Vec::new();
    for point in elements(text, "Trackpoint") {
        let position = elements(point.inner, "Position").into_iter().next();
        let (lat, lon) = match position {
            Some(position) => {
                let lat = first_text(&position, "LatitudeDegrees").and_then(parse_f64);
                let lon = first_text(&position, "LongitudeDegrees").and_then(parse_f64);
                match (lat, lon) {
                    (Some(lat), Some(lon)) => (lat, lon),
                    _ => continue,
                }
            }
            None => continue,
        };
        raw.push(RawPoint {
            lat,
            lon,
            elevation_m: first_text(&point, "AltitudeMeters").and_then(parse_f64),
            time_ms: first_text(&point, "Time").and_then(crate::gpx::parse_iso8601_utc),
            distance_m: first_text(&point, "DistanceMeters").and_then(parse_f64),
        });
    }

    // Distance officielle : le dernier point porte la distance cumulee ; a
    // defaut, on additionne les tours.
    let declared_distance_m = raw
        .iter()
        .rev()
        .find_map(|point| point.distance_m)
        .or_else(|| {
            let laps = elements(text, "Lap");
            if laps.is_empty() {
                return None;
            }
            let total: f64 = laps
                .iter()
                .filter_map(|lap| first_text(lap, "DistanceMeters").and_then(parse_f64))
                .sum();
            (total > 0.0).then_some(total)
        });

    let id_time = element_text(text, "Activity", "Id").and_then(crate::gpx::parse_iso8601_utc);
    build(filename, name, source, raw, declared_distance_m, id_time)
}

// ------------------------------------------------------------------ calculs

fn build(
    filename: &str,
    name: Option<String>,
    source: ImportSource,
    raw: Vec<RawPoint>,
    declared_distance_m: Option<f64>,
    declared_start_ms: Option<i64>,
) -> Result<ImportedRace, ImportError> {
    if raw.is_empty() {
        return Err(ImportError::NoTrack);
    }

    let started_at_ms = raw
        .iter()
        .find_map(|point| point.time_ms)
        .or(declared_start_ms);
    let file_distances = raw
        .iter()
        .filter(|point| point.distance_m.is_some())
        .count()
        > 1;

    let mut points = Vec::with_capacity(raw.len());
    let mut cumulative_m = 0.0_f64;
    let mut previous_position: Option<Position> = None;
    let mut previous_t_ms: Option<i64> = None;

    for (index, point) in raw.iter().enumerate() {
        let position = Position::new(point.lat, point.lon);
        if let Some(previous) = previous_position {
            cumulative_m += haversine_m(previous, position);
        }
        previous_position = Some(position);

        let absolute_t_ms = match (point.time_ms, started_at_ms) {
            (Some(time), Some(start)) => (time - start).max(0),
            _ => previous_t_ms
                .map(|t| t + 1000)
                .unwrap_or(index as i64 * 1000),
        };
        let t_ms = previous_t_ms
            .map(|previous| absolute_t_ms.max(previous))
            .unwrap_or(absolute_t_ms);
        previous_t_ms = Some(t_ms);

        let dist_m = if file_distances {
            point.distance_m.unwrap_or(cumulative_m)
        } else {
            cumulative_m
        };
        points.push(TrackPoint {
            t_ms,
            dist_m,
            lat: point.lat,
            lon: point.lon,
            elevation_m: point.elevation_m,
        });
    }

    let timed = started_at_ms.is_some();
    let elapsed_time_s = timed.then(|| {
        let first = points.first().map(|point| point.t_ms).unwrap_or(0);
        let last = points.last().map(|point| point.t_ms).unwrap_or(0);
        (last - first).max(0) as f64 / 1000.0
    });
    let moving_time_s = timed.then(|| moving_time_s(&points));

    let distance_m = declared_distance_m
        .filter(|distance| distance.is_finite() && *distance > 0.0)
        .unwrap_or_else(|| points.last().map(|point| point.dist_m).unwrap_or(0.0));
    let elevation_gain_m = crate::analysis::elevation_gain_m(&points);

    Ok(ImportedRace {
        name: name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| name_from_filename(filename)),
        source,
        started_at_ms,
        distance_m,
        moving_time_s,
        elapsed_time_s,
        elevation_gain_m,
        points: decimate(points, MAX_TRACK_POINTS),
    })
}

/// Temps en mouvement : seules les portions courues (vitesse suffisante, sans
/// coupure GPS) sont comptees.
fn moving_time_s(points: &[TrackPoint]) -> f64 {
    let mut moving = 0.0;
    for window in points.windows(2) {
        let (previous, next) = (window[0], window[1]);
        let dt_s = (next.t_ms - previous.t_ms) as f64 / 1000.0;
        if dt_s <= 0.0 || dt_s > MAX_GAP_S {
            continue;
        }
        let distance_m = (next.dist_m - previous.dist_m).max(0.0);
        if distance_m / dt_s >= MOVING_SPEED_MPS {
            moving += dt_s;
        }
    }
    moving
}

/// Echantillonne une trace trop longue, en gardant toujours le dernier point.
fn decimate(points: Vec<TrackPoint>, maximum: usize) -> Vec<TrackPoint> {
    if points.len() <= maximum || maximum == 0 {
        return points;
    }
    let step = points.len().div_ceil(maximum);
    let last = points.len() - 1;
    points
        .into_iter()
        .enumerate()
        .filter(|(index, _)| index % step == 0 || *index == last)
        .map(|(_, point)| point)
        .collect()
}

/// Nom de repli : le nom du fichier, sans dossier ni extension.
fn name_from_filename(filename: &str) -> String {
    let base = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(filename)
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(filename);
    let cleaned = base.replace(['_', '-'], " ").trim().to_string();
    if cleaned.is_empty() {
        "Course importee".to_string()
    } else {
        cleaned
    }
}

// ------------------------------------------------------------------ lecteur XML

/// Balise XML ouverte, reduite a ce qui est utile ici.
struct Tag<'a> {
    /// Nom local, sans prefixe de namespace.
    name: &'a str,
    attrs: Vec<(&'a str, &'a str)>,
    closing: bool,
    self_closing: bool,
    /// Index du caractere ouvrant.
    start: usize,
    /// Index juste apres la balise fermante.
    end: usize,
}

/// Element XML : ses attributs et son contenu.
struct Element<'a> {
    attrs: Vec<(&'a str, &'a str)>,
    inner: &'a str,
}

/// Cherche la prochaine balise a partir de l'index donne.
fn next_tag<'a>(xml: &'a str, from: usize) -> Option<Tag<'a>> {
    let mut cursor = from;
    while cursor < xml.len() {
        let open = cursor + xml[cursor..].find('<')?;
        if xml[open..].starts_with("<!--") {
            cursor = open + xml[open..].find("-->")? + 3;
            continue;
        }
        if xml[open..].starts_with("<!") || xml[open..].starts_with("<?") {
            cursor = open + xml[open..].find('>')? + 1;
            continue;
        }
        let close = open + xml[open..].find('>')?;
        let raw = xml[open + 1..close].trim();
        let self_closing = raw.ends_with('/');
        let closing = raw.starts_with('/');
        let body = raw.trim_start_matches('/').trim_end_matches('/').trim();
        let name_end = body.find(char::is_whitespace).unwrap_or(body.len());
        let qualified = &body[..name_end];
        let name = qualified.rsplit(':').next().unwrap_or(qualified);
        let attrs = parse_attrs(&body[name_end..]);
        return Some(Tag {
            name,
            attrs,
            closing,
            self_closing,
            start: open,
            end: close + 1,
        });
    }
    None
}

/// Tous les elements de premier niveau portant ce nom.
fn elements<'a>(xml: &'a str, wanted: &str) -> Vec<Element<'a>> {
    let mut found = Vec::new();
    let mut cursor = 0;
    while let Some(tag) = next_tag(xml, cursor) {
        cursor = tag.end;
        if tag.closing || !tag.name.eq_ignore_ascii_case(wanted) {
            continue;
        }
        if tag.self_closing {
            found.push(Element {
                attrs: tag.attrs,
                inner: "",
            });
            continue;
        }
        let mut depth = 1_usize;
        let mut scan = tag.end;
        while let Some(next) = next_tag(xml, scan) {
            scan = next.end;
            if !next.name.eq_ignore_ascii_case(wanted) {
                continue;
            }
            if next.closing {
                depth -= 1;
                if depth == 0 {
                    found.push(Element {
                        attrs: tag.attrs,
                        inner: &xml[tag.end..next.start],
                    });
                    cursor = next.end;
                    break;
                }
            } else if !next.self_closing {
                depth += 1;
            }
        }
    }
    found
}

/// Contenu texte du premier element enfant (vide si absent).
fn first_text<'a>(element: &Element<'a>, child: &str) -> Option<&'a str> {
    elements(element.inner, child)
        .into_iter()
        .next()
        .map(|child| child.inner.trim())
        .filter(|text| !text.is_empty())
}

/// Contenu texte du premier enfant, a l'interieur du premier parent donne.
fn element_text<'a>(xml: &'a str, parent: &str, child: &str) -> Option<&'a str> {
    let parent = elements(xml, parent).into_iter().next()?;
    first_text(&parent, child)
}

/// Valeur d'un attribut, sans tenir compte de la casse du nom.
fn attr<'a>(attrs: &[(&'a str, &'a str)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| *value)
}

/// Lit les attributs cle/valeur d'une balise (guillemets simples ou doubles).
fn parse_attrs(raw: &str) -> Vec<(&str, &str)> {
    // 0x27 est l'apostrophe : l'ecrire en octet evite un echappement inutile.
    const APOSTROPHE: u8 = 0x27;
    let bytes = raw.as_bytes();
    let mut attrs = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && (bytes[index].is_ascii_whitespace() || bytes[index] == b'/') {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len() && bytes[index] != b'=' && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let name = &raw[name_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            // Attribut sans valeur ou fin de balise : on s'arrete proprement.
            if name.is_empty() {
                break;
            }
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let quote = bytes[index];
        if quote != b'"' && quote != APOSTROPHE {
            continue;
        }
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if !name.is_empty() {
            attrs.push((name, &raw[value_start..index.min(raw.len())]));
        }
        index += 1;
    }
    attrs
}

/// Lit un nombre decimal (point ou virgule).
fn parse_f64(value: &str) -> Option<f64> {
    let cleaned = value.trim().replace(',', ".");
    cleaned
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpx::parse_iso8601_utc;

    /// GPX realiste : creator Strava, trois points a 1 Hz, altitudes variees.
    fn strava_gpx() -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str(
            "<gpx version=\"1.1\" creator=\"StravaGPX\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n",
        );
        out.push_str("  <metadata>\n    <name>Marathon de Bordeaux</name>\n");
        out.push_str("    <time>2026-04-05T08:00:00.000Z</time>\n  </metadata>\n");
        out.push_str("  <trk>\n    <name>Marathon de Bordeaux</name>\n    <trkseg>\n");
        let points = [
            (44.84, -0.57, 10.0),
            (44.85, -0.57, 14.0),
            (44.86, -0.57, 12.0),
        ];
        for (index, (lat, lon, elevation)) in points.iter().enumerate() {
            out.push_str(&format!(
                "      <trkpt lat=\"{lat:.5}\" lon=\"{lon:.5}\">\n"
            ));
            out.push_str(&format!("        <ele>{elevation:.1}</ele>\n"));
            out.push_str(&format!(
                "        <time>2026-04-05T08:00:0{index}.000Z</time>\n"
            ));
            out.push_str("      </trkpt>\n");
        }
        out.push_str("    </trkseg>\n  </trk>\n</gpx>\n");
        out
    }

    /// TCX Garmin : la distance est portee par chaque point.
    fn garmin_tcx() -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str("<TrainingCenterDatabase xmlns=\"http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2\">\n");
        out.push_str("  <Activities>\n    <Activity Sport=\"Running\">\n");
        out.push_str("      <Id>2026-03-01T09:30:00.000Z</Id>\n");
        out.push_str("      <Lap StartTime=\"2026-03-01T09:30:00.000Z\">\n");
        out.push_str("        <TotalTimeSeconds>7200</TotalTimeSeconds>\n");
        out.push_str("        <DistanceMeters>42195</DistanceMeters>\n");
        out.push_str("        <Track>\n");
        let points = [
            (44.84, -0.57, 8.0, 0.0),
            (44.85, -0.57, 12.0, 100.0),
            (44.86, -0.57, 9.0, 200.0),
        ];
        for (index, (lat, lon, elevation, distance)) in points.iter().enumerate() {
            out.push_str("          <Trackpoint>\n");
            out.push_str(&format!(
                "            <Time>2026-03-01T09:30:0{index}.000Z</Time>\n"
            ));
            out.push_str("            <Position>\n");
            out.push_str(&format!(
                "              <LatitudeDegrees>{lat}</LatitudeDegrees>\n"
            ));
            out.push_str(&format!(
                "              <LongitudeDegrees>{lon}</LongitudeDegrees>\n"
            ));
            out.push_str("            </Position>\n");
            out.push_str(&format!(
                "            <AltitudeMeters>{elevation}</AltitudeMeters>\n"
            ));
            out.push_str(&format!(
                "            <DistanceMeters>{distance}</DistanceMeters>\n"
            ));
            out.push_str("          </Trackpoint>\n");
        }
        out.push_str("        </Track>\n      </Lap>\n    </Activity>\n  </Activities>\n");
        out.push_str("  <Creator><Name>Garmin Connect</Name></Creator>\n");
        out.push_str("</TrainingCenterDatabase>\n");
        out
    }

    #[test]
    fn strava_gpx_is_read_as_a_reference_race() {
        let race = import_race_text("marathon-bordeaux.gpx", &strava_gpx()).expect("import");
        assert_eq!(race.name, "Marathon de Bordeaux");
        assert_eq!(race.source, ImportSource::Strava);
        assert_eq!(
            race.started_at_ms,
            parse_iso8601_utc("2026-04-05T08:00:00.000Z")
        );
        assert_eq!(race.points.len(), 3);
        assert_eq!(race.elapsed_time_s, Some(2.0));
        assert_eq!(race.moving_time_s, Some(2.0));
        // Deux points a 0,01 degre de latitude : environ 2,2 km.
        assert!(
            (2200.0..2250.0).contains(&race.distance_m),
            "distance = {}",
            race.distance_m
        );
        assert!((race.elevation_gain_m - 4.0).abs() < 1e-9);
        assert!(race.average_pace_s_per_km().is_some());
        // Le GPX reexporte reste relisible : c'est la trace conservee.
        let exported = race.to_gpx();
        assert!(exported.contains("<trkpt lat=\"44.84"));
        let reread = import_race_text("trace.gpx", &exported).expect("relecture");
        assert_eq!(reread.points.len(), 3);
    }

    #[test]
    fn garmin_tcx_uses_the_declared_distance() {
        let race = import_race_text("Marathon de Bordeaux.tcx", &garmin_tcx()).expect("import");
        assert_eq!(race.source, ImportSource::Garmin);
        // Garmin ne nomme pas la trace : le nom du fichier prend le relais.
        assert_eq!(race.name, "Marathon de Bordeaux");
        assert_eq!(race.distance_m, 200.0);
        assert_eq!(race.points.len(), 3);
        assert_eq!(race.points[2].dist_m, 200.0);
        assert_eq!(race.elapsed_time_s, Some(2.0));
    }

    #[test]
    fn stops_are_excluded_from_the_moving_time() {
        // Trois points : 10 s de course, 60 s d'arret au meme endroit, 10 s de course.
        let gpx = "<?xml version=\"1.0\"?><gpx creator=\"Garmin Connect\"><trk><trkseg>\
            <trkpt lat=\"45.0\" lon=\"3.0\"><time>2026-01-01T10:00:00Z</time></trkpt>\
            <trkpt lat=\"45.001\" lon=\"3.0\"><time>2026-01-01T10:00:10Z</time></trkpt>\
            <trkpt lat=\"45.001\" lon=\"3.0\"><time>2026-01-01T10:01:10Z</time></trkpt>\
            <trkpt lat=\"45.002\" lon=\"3.0\"><time>2026-01-01T10:01:20Z</time></trkpt>\
            </trkseg></trk></gpx>";
        let race = import_race_text("sortie.gpx", gpx).expect("import");
        assert_eq!(race.source, ImportSource::Garmin);
        assert_eq!(race.elapsed_time_s, Some(80.0));
        assert_eq!(race.moving_time_s, Some(20.0));
    }

    #[test]
    fn namespaced_and_commented_xml_is_tolerated() {
        let gpx = "<?xml version=\"1.0\"?>\
            <gpx creator=\"autre outil\" xmlns:gpxx=\"http://example.org\">\
            <!-- une trace sans metadata -->\
            <trk><name>Trail des cretes</name><trkseg>\
            <trkpt lon=\"6.5\" lat=\"45.2\"><ele>1200</ele><extensions><gpxx:hr>150</gpxx:hr></extensions></trkpt>\
            <trkpt lon=\"6.6\" lat=\"45.3\"><ele>1250</ele></trkpt>\
            </trkseg></trk></gpx>";
        let race = import_race_text("trail.gpx", gpx).expect("import");
        assert_eq!(race.name, "Trail des cretes");
        assert_eq!(race.source, ImportSource::Gpx);
        assert_eq!(race.points.len(), 2);
        assert_eq!(race.started_at_ms, None);
        assert_eq!(race.moving_time_s, None);
        assert!((race.elevation_gain_m - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_file_without_track_is_refused() {
        let error = import_race_text("inconnu.txt", "<html><body>Connexion</body></html>")
            .expect_err("refus");
        assert!(matches!(error, ImportError::UnsupportedFormat(_)));
        assert!(error.to_string().contains("GPX"));

        let empty = import_race_text(
            "vide.gpx",
            "<?xml version=\"1.0\"?><gpx creator=\"StravaGPX\"></gpx>",
        )
        .expect_err("refus");
        assert_eq!(empty, ImportError::NoTrack);
    }

    #[test]
    fn a_binary_fit_export_is_refused_with_a_clear_message() {
        let error =
            import_race("activite.fit", &[0x0e, 0x10, 0x00, 0xff, 0xfe]).expect_err("refus");
        assert!(error.to_string().contains("GPX ou TCX"), "{error}");
    }

    #[test]
    fn long_tracks_are_decimated_but_keep_the_last_point() {
        let mut points: Vec<TrackPoint> = (0..5_000)
            .map(|index| TrackPoint {
                t_ms: index as i64 * 1000,
                dist_m: index as f64,
                lat: 45.0,
                lon: 3.0,
                elevation_m: None,
            })
            .collect();
        let last = *points.last().unwrap();
        points = decimate(points, 2_000);
        assert!(points.len() <= MAX_TRACK_POINTS, "{}", points.len());
        assert!(points.len() >= 1_600, "{}", points.len());
        assert_eq!(*points.last().unwrap(), last);
    }

    #[test]
    fn file_names_become_readable_fallbacks() {
        assert_eq!(
            name_from_filename("C:/exports/ma_course-2026.gpx"),
            "ma course 2026"
        );
        assert_eq!(name_from_filename(".gpx"), "Course importee");
    }
}
