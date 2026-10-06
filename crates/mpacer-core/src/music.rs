//! Musique de course : calibration tempo/allure, directeur d'orchestre,
//! tap-tempo et lecture des balises BPM.
//!
//! Le module est **pur** : aucune E/S, aucun acces fichier. Les octets d'un
//! fichier audio arrivent en memoire (bpm_from_tags) et la montre pousse
//! l'instantane de la piste en cours (on_now_playing). Le resultat est publie
//! dans EngineOutput.music et serialise tel quel vers le shell Android.
//!
//! Regle d'or : l'etat est **stable**. Deux evaluations consecutives avec les
//! memes entrees donnent le meme resultat (aucun tirage aleatoire, aucune
//! horloge cachee). Seules les transitions pause/reprise sont ponctuelles.

use crate::workout::WorkoutState;
use serde::{Deserialize, Serialize};

/// Foulee de reference (m) : longueur de pas a l'allure de reference.
pub const REFERENCE_STRIDE_M: f64 = 1.15;
/// Exposant d'allongement de la foulee avec la vitesse.
pub const STRIDE_SPEED_EXPONENT: f64 = 0.4;
/// Vitesse de reference de la foulee (m/s) : 5:00/km (defaut de calibration).
pub const STRIDE_REFERENCE_SPEED_MPS: f64 = 1000.0 / 300.0;
/// Cadence minimale estimee (pas/minute).
pub const CADENCE_MIN_SPM: f64 = 140.0;
/// Cadence maximale estimee (pas/minute).
pub const CADENCE_MAX_SPM: f64 = 210.0;
/// Ecart au plan (m) qui declenche Boost/Relax.
const PLAN_GAP_M: f64 = 20.0;
/// Ecart d'allure (s/km) qui declenche Boost/Relax.
const PACE_GAP_S_PER_KM: f64 = 5.0;
/// Intervalle de tap minimal plausible (ms) : 300 BPM.
const TAP_MIN_INTERVAL_MS: i64 = 200;
/// Intervalle de tap maximal plausible (ms) : 30 BPM.
const TAP_MAX_INTERVAL_MS: i64 = 2_000;

/// Piste d'une playlist, telle que le backend la publie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    pub duration_s: f64,
    /// None = tempo inconnu : piste neutre.
    #[serde(default)]
    pub bpm: Option<f64>,
    #[serde(default)]
    pub position: u32,
}

/// Playlist de course, telle que preparee dans l'interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    /// Consigne fixe decidee dans l'interface (None = calibration auto).
    #[serde(default)]
    pub target_bpm: Option<f64>,
    #[serde(default)]
    pub tracks: Vec<Track>,
}

/// Origine d'une valeur de BPM (affichage et confiance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum BpmSource {
    Spotify,
    Tag,
    Tap,
    Manual,
    Estimated,
}

/// Reglages musique du moteur (miroir de l'ecran Reglages de la montre).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MusicConfig {
    pub enabled: bool,
    /// BPM de reference a l'allure de reference.
    pub reference_bpm: f64,
    /// Allure de reference (s/km).
    pub reference_pace_s_per_km: f64,
    /// Elasticite du tempo par rapport a l'allure (0 = tempo fixe).
    pub pace_elasticity: f64,
    pub min_bpm: f64,
    pub max_bpm: f64,
    /// Ecart de BPM qui declenche un changement de morceau.
    pub switch_threshold_bpm: f64,
    /// Gain applique quand le coureur est en retard sur le plan.
    pub boost_bpm: f64,
    /// Perte appliquee quand il est en avance (ou cardio trop haut).
    pub relax_bpm: f64,
    /// Annoncer les changements de consigne a la voix.
    pub announce: bool,
    /// Ne pas rejouer une des N dernieres pistes lors d'une selection.
    pub avoid_last: usize,
}

impl Default for MusicConfig {
    fn default() -> Self {
        Self {
            // Musique active par defaut (opt-out) : sans playlist, le directeur
            // renvoie NoPlaylist et rien ne se declenche.
            enabled: true,
            reference_bpm: 170.0,
            reference_pace_s_per_km: 300.0,
            pace_elasticity: 0.35,
            min_bpm: 100.0,
            max_bpm: 200.0,
            switch_threshold_bpm: 8.0,
            boost_bpm: 6.0,
            relax_bpm: 6.0,
            announce: true,
            avoid_last: 3,
        }
    }
}

/// Instantane de la piste en cours, pousse par la montre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NowPlaying {
    pub track_id: String,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub bpm: Option<f64>,
    #[serde(default)]
    pub position_s: f64,
}

/// Consigne envoyee au lecteur de la montre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum MusicDirective {
    #[default]
    None,
    Play,
    Keep,
    Boost,
    Relax,
    SkipTo,
    Pause,
    Resume,
}

/// Justification d'une directive (affichage et journal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum DirectiveReason {
    #[default]
    Disabled,
    NoPlaylist,
    Steady,
    OnPlan,
    BehindPlan,
    AheadOfPlan,
    PaceSlow,
    PaceFast,
    HeartRateHigh,
    Paused,
    Resumed,
    TrackBpmMismatch,
}

/// Resultat publie dans EngineOutput.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicState {
    pub enabled: bool,
    pub playlist_id: Option<String>,
    pub playlist_name: Option<String>,
    /// BPM cible du tick (apres boost/relax).
    pub target_bpm: Option<f64>,
    pub target_cadence_spm: Option<f64>,
    pub cadence_spm: Option<f64>,
    pub current: Option<NowPlaying>,
    pub next_track_id: Option<String>,
    pub directive: MusicDirective,
    pub reason: DirectiveReason,
}

impl Default for MusicState {
    fn default() -> Self {
        Self {
            enabled: false,
            playlist_id: None,
            playlist_name: None,
            target_bpm: None,
            target_cadence_spm: None,
            cadence_spm: None,
            current: None,
            next_track_id: None,
            directive: MusicDirective::None,
            reason: DirectiveReason::Disabled,
        }
    }
}

/// Entree du directeur pour un tick du moteur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MusicInput {
    pub t_ms: i64,
    pub state: WorkoutState,
    /// Allure cible du plan (s/km), si un plan est actif.
    pub target_pace_s_per_km: Option<f64>,
    /// Allure courante lissee (s/km).
    pub current_pace_s_per_km: Option<f64>,
    /// Ecart au shadow runner (m) : positif = en avance.
    pub shadow_delta_m: Option<f64>,
    pub shadow_on_plan: bool,
    pub heart_rate_zone: Option<u8>,
    pub speed_mps: Option<f64>,
}

// ------------------------------------------------------------ loi tempo/allure

/// BPM cible pour une allure cible (s/km).
///
/// target = clamp(reference_bpm * (reference_pace / pace)^elasticity, min, max)
pub fn target_bpm_for_pace(pace_s_per_km: f64, cfg: &MusicConfig) -> f64 {
    let (low, high) = sorted_bounds(cfg);
    if !pace_s_per_km.is_finite() || pace_s_per_km <= 0.0 {
        return cfg.reference_bpm.clamp(low, high);
    }
    let ratio = cfg.reference_pace_s_per_km / pace_s_per_km;
    let bpm = cfg.reference_bpm * ratio.powf(cfg.pace_elasticity);
    bpm.clamp(low, high)
}

/// Cadence de pas visee (pas/minute) pour une allure cible (s/km).
///
/// cadence = 60 * vitesse / foulee, avec foulee = 1.15 * (v / v_ref)^0.4
/// et v_ref = 1000 / reference_pace_s_per_km.
///
/// Bornee a 140..=210 pas/minute comme `cadence_from_speed` : au-dela d'environ
/// 3:30/km la cadence sature, la vitesse venant alors de l'allongement de la
/// foulee et non d'un pas plus rapide.
pub fn target_cadence_spm(pace_s_per_km: f64, cfg: &MusicConfig) -> f64 {
    if !pace_s_per_km.is_finite()
        || pace_s_per_km <= 0.0
        || !cfg.reference_pace_s_per_km.is_finite()
        || cfg.reference_pace_s_per_km <= 0.0
    {
        return CADENCE_MIN_SPM;
    }
    let speed = 1000.0 / pace_s_per_km;
    let reference_speed = 1000.0 / cfg.reference_pace_s_per_km;
    let stride = stride_for_speed(speed, reference_speed);
    (60.0 * speed / stride).clamp(CADENCE_MIN_SPM, CADENCE_MAX_SPM)
}

/// Cadence estimee a partir de la vitesse (repli si aucun capteur de pas).
///
/// Utilise la foulee de reference et sa loi d'allongement, bornee a
/// 140..=210 pas/minute.
pub fn cadence_from_speed(speed_mps: f64) -> f64 {
    if !speed_mps.is_finite() || speed_mps <= 0.0 {
        return CADENCE_MIN_SPM;
    }
    let stride = stride_for_speed(speed_mps, STRIDE_REFERENCE_SPEED_MPS);
    (60.0 * speed_mps / stride).clamp(CADENCE_MIN_SPM, CADENCE_MAX_SPM)
}

/// Longueur de foulee pour une vitesse et une vitesse de reference donnees.
fn stride_for_speed(speed_mps: f64, reference_speed_mps: f64) -> f64 {
    let ratio = speed_mps / reference_speed_mps;
    REFERENCE_STRIDE_M * ratio.powf(STRIDE_SPEED_EXPONENT)
}

/// Bornes de BPM triees : une configuration incoherente ne doit pas paniquer.
fn sorted_bounds(cfg: &MusicConfig) -> (f64, f64) {
    (cfg.min_bpm.min(cfg.max_bpm), cfg.min_bpm.max(cfg.max_bpm))
}

// ------------------------------------------------------------------- tap-tempo

/// BPM estime a partir d'instants de tap (ms).
///
/// Median des intervalles, au moins 4 taps, sinon None. Les intervalles
/// aberrants (hors 200..=2000 ms, soit 30 a 300 BPM) sont ignores.
pub fn tap_tempo(taps_ms: &[i64]) -> Option<f64> {
    if taps_ms.len() < 4 {
        return None;
    }
    let mut intervals: Vec<i64> = taps_ms
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .filter(|interval| (TAP_MIN_INTERVAL_MS..=TAP_MAX_INTERVAL_MS).contains(interval))
        .collect();
    if intervals.is_empty() {
        return None;
    }
    intervals.sort_unstable();
    let middle = intervals.len() / 2;
    let median = if intervals.len() % 2 == 0 {
        (intervals[middle - 1] + intervals[middle]) as f64 / 2.0
    } else {
        intervals[middle] as f64
    };
    if median <= 0.0 {
        return None;
    }
    Some(60_000.0 / median)
}

// ------------------------------------------------------------------ balises BPM

/// Extrait un BPM d'une balise binaire de fichier audio, sans dependance :
/// ID3v2 (TBPM), commentaire Vorbis/FLAC (BPM=), atome MP4 (tmpo).
///
/// Le nom de fichier sert d'indice de priorite ; la reconnaissance reste
/// basee sur les octets, donc une extension erronee ne masque pas la balise.
pub fn bpm_from_tags(bytes: &[u8], filename: &str) -> Option<f64> {
    let lower = filename.to_ascii_lowercase();
    let vorbis_first = lower.ends_with(".ogg")
        || lower.ends_with(".oga")
        || lower.ends_with(".opus")
        || lower.ends_with(".flac");
    let mp4_first = lower.ends_with(".m4a")
        || lower.ends_with(".mp4")
        || lower.ends_with(".m4b")
        || lower.ends_with(".aac");
    if vorbis_first {
        if let Some(bpm) = bpm_from_vorbis_comment(bytes) {
            return Some(bpm);
        }
    }
    if mp4_first {
        if let Some(bpm) = bpm_from_mp4(bytes) {
            return Some(bpm);
        }
    }
    bpm_from_id3v2(bytes)
        .or_else(|| bpm_from_vorbis_comment(bytes))
        .or_else(|| bpm_from_mp4(bytes))
}

/// TBPM dans un tag ID3v2.2/2.3/2.4.
fn bpm_from_id3v2(bytes: &[u8]) -> Option<f64> {
    if bytes.len() < 10 || !bytes.starts_with(b"ID3") {
        return None;
    }
    let major = bytes[3];
    if !(2..=4).contains(&major) {
        return None;
    }
    let tag_size = syncsafe_u32(bytes.get(6..10)?)? as usize;
    let end = (10 + tag_size).min(bytes.len());
    let mut pos = 10;
    // En-tete etendu : le drapeau 0x40 est commun aux versions 2.3 et 2.4.
    if bytes[5] & 0x40 != 0 {
        if major == 4 {
            let size = syncsafe_u32(bytes.get(pos..pos + 4)?)? as usize;
            pos += size.max(6);
        } else {
            let size = be_u32(bytes.get(pos..pos + 4)?)? as usize;
            pos += 4 + size;
        }
    }
    while pos + 6 <= end {
        let (id_len, header_len, frame_size) = if major == 2 {
            (
                3usize,
                6usize,
                be_u24(bytes.get(pos + 3..pos + 6)?)? as usize,
            )
        } else if major == 4 {
            (
                4usize,
                10usize,
                syncsafe_u32(bytes.get(pos + 4..pos + 8)?)? as usize,
            )
        } else {
            (
                4usize,
                10usize,
                be_u32(bytes.get(pos + 4..pos + 8)?)? as usize,
            )
        };
        let id = bytes.get(pos..pos + id_len)?;
        if id.iter().all(|byte| *byte == 0) {
            break;
        }
        if frame_size == 0 {
            pos += header_len;
            continue;
        }
        let body_start = pos + header_len;
        let body = bytes.get(body_start..body_start + frame_size)?;
        if id == b"TBPM".as_slice() || id == b"TBP".as_slice() {
            // Trame texte normale (octet d'encodage) ; certains encodeurs
            // ecrivent le nombre directement : on accepte les deux formes.
            return parse_bpm_text(body).or_else(|| parse_bpm_ascii(body));
        }
        pos = body_start + frame_size;
    }
    None
}

/// BPM=... dans un bloc de commentaires Vorbis (OGG, FLAC, Opus).
fn bpm_from_vorbis_comment(bytes: &[u8]) -> Option<f64> {
    for needle in [b"BPM=".as_slice(), b"bpm=".as_slice()] {
        let mut from = 0usize;
        while let Some(relative) = find(&bytes[from..], needle) {
            let start = from + relative + needle.len();
            let end = bytes[start..]
                .iter()
                .position(|byte| *byte == 0 || *byte == b'\n' || *byte == b'\r')
                .map(|offset| start + offset)
                .unwrap_or(bytes.len());
            if let Some(bpm) = parse_bpm_ascii(&bytes[start..end]) {
                return Some(bpm);
            }
            from = start;
        }
    }
    None
}

/// Atome tmpo d'un fichier MP4/M4A (sous-atome de donnees data).
fn bpm_from_mp4(bytes: &[u8]) -> Option<f64> {
    let tmpo = find(bytes, b"tmpo")?;
    let after = tmpo + 4;
    let window_end = (after + 64).min(bytes.len());
    if let Some(relative) = find(bytes.get(after..window_end)?, b"data") {
        let data = after + relative;
        let value_pos = data + 12;
        // La taille de l'atome data vaut 16 + taille de la valeur.
        let declared = if data >= 4 {
            be_u32(bytes.get(data - 4..data)?)? as usize
        } else {
            18
        };
        let width = declared.saturating_sub(16);
        let width = if (1..=8).contains(&width) { width } else { 2 };
        let mut value: u64 = 0;
        for offset in 0..width {
            value = (value << 8) | *bytes.get(value_pos + offset)? as u64;
        }
        return (value > 0).then_some(value as f64);
    }
    // Repli : atome tmpo portant directement un entier 16 bits.
    let value = u16::from_be_bytes(bytes.get(after..after + 2)?.try_into().ok()?);
    (value > 0).then_some(value as f64)
}

/// Decode le corps d'une trame texte ID3 puis lit le premier nombre.
///
/// Renvoie `None` si le premier octet n'est pas un encodage connu : la trame
/// est alors traitee comme du texte brut par l'appelant.
fn parse_bpm_text(body: &[u8]) -> Option<f64> {
    let (encoding, text) = body.split_first()?;
    let decoded = match encoding {
        0 => text.iter().map(|byte| *byte as char).collect::<String>(),
        1 => {
            if text.len() >= 2 && text[0] == 0xFF && text[1] == 0xFE {
                decode_utf16(&text[2..], true)
            } else {
                let text = if text.len() >= 2 && text[0] == 0xFE && text[1] == 0xFF {
                    &text[2..]
                } else {
                    text
                };
                decode_utf16(text, false)
            }
        }
        2 => decode_utf16(text, false),
        3 => String::from_utf8_lossy(text).into_owned(),
        _ => return None,
    };
    parse_bpm_ascii(decoded.as_bytes())
}

/// Decode une suite de paires UTF-16 jusqu'au premier NUL.
fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let mut text = String::new();
    for pair in bytes.chunks_exact(2) {
        let unit = if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        };
        if unit == 0 {
            break;
        }
        text.push(char::from_u32(unit as u32).unwrap_or('?'));
    }
    text
}

/// Lit le premier nombre d'un texte ASCII (172, 172.5, 172/120).
fn parse_bpm_ascii(bytes: &[u8]) -> Option<f64> {
    let text = String::from_utf8_lossy(bytes);
    let mut number = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() {
            number.push(character);
        } else if (character == '.' || character == ',') && !number.is_empty() {
            number.push('.');
        } else if !number.is_empty() {
            break;
        }
    }
    if number.is_empty() {
        return None;
    }
    number
        .parse::<f64>()
        .ok()
        .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn be_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?))
}

fn be_u24(bytes: &[u8]) -> Option<u32> {
    let bytes = bytes.get(..3)?;
    Some(((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | bytes[2] as u32)
}

/// Entier syncsafe ID3v2 : 7 bits utiles par octet.
fn syncsafe_u32(bytes: &[u8]) -> Option<u32> {
    let bytes = bytes.get(..4)?;
    if bytes.iter().any(|byte| byte & 0x80 != 0) {
        return None;
    }
    Some(
        ((bytes[0] as u32) << 21)
            | ((bytes[1] as u32) << 14)
            | ((bytes[2] as u32) << 7)
            | bytes[3] as u32,
    )
}

// ------------------------------------------------------------------- directeur

/// Directeur d'orchestre : choisit la consigne de tempo du tick courant.
///
/// Il ne fait aucune E/S et ne connait ni le lecteur ni le systeme de
/// fichiers : la montre lui pousse l'etat de lecture (on_now_playing) et la
/// cadence mesuree (on_cadence), le moteur lui fournit le contexte de course.
#[derive(Debug, Clone)]
pub struct MusicDirector {
    config: MusicConfig,
    playlist: Option<Playlist>,
    current: Option<NowPlaying>,
    /// Cadence issue d'un capteur de pas (prime sur l'estimation).
    cadence_spm: Option<f64>,
    /// Dernieres pistes jouees (la plus recente en dernier).
    history: Vec<String>,
    /// Vrai entre l'annonce d'une pause et le retour en course.
    in_pause: bool,
}

impl Default for MusicDirector {
    fn default() -> Self {
        Self::new(MusicConfig::default())
    }
}

impl MusicDirector {
    pub fn new(config: MusicConfig) -> Self {
        Self {
            config,
            playlist: None,
            current: None,
            cadence_spm: None,
            history: Vec::new(),
            in_pause: false,
        }
    }

    pub fn config(&self) -> MusicConfig {
        self.config
    }

    pub fn set_config(&mut self, config: MusicConfig) {
        self.config = config;
    }

    pub fn set_playlist(&mut self, playlist: Option<Playlist>) {
        let changed = match (&self.playlist, &playlist) {
            (Some(previous), Some(next)) => previous.id != next.id,
            (None, None) => false,
            _ => true,
        };
        if changed {
            self.current = None;
            self.history.clear();
        }
        self.playlist = playlist;
    }

    pub fn playlist(&self) -> Option<&Playlist> {
        self.playlist.as_ref()
    }

    pub fn on_now_playing(&mut self, now: Option<NowPlaying>) {
        if let Some(now) = &now {
            let changed = self
                .current
                .as_ref()
                .map(|current| current.track_id != now.track_id)
                .unwrap_or(true);
            if changed {
                self.remember(&now.track_id);
            }
        }
        self.current = now;
    }

    /// Cadence mesuree par un capteur de pas (0 ou non fini = pas de capteur).
    pub fn on_cadence(&mut self, spm: f64) {
        self.cadence_spm = if spm.is_finite() && spm > 0.0 {
            Some(spm)
        } else {
            None
        };
    }

    /// Nouvelle seance : oublie la lecture en cours, garde playlist et reglages.
    pub fn reset(&mut self) {
        self.current = None;
        self.cadence_spm = None;
        self.history.clear();
        self.in_pause = false;
    }

    /// Consigne du tick courant.
    pub fn evaluate(&mut self, input: MusicInput) -> MusicState {
        let cadence = self.current_cadence(&input);

        // 1. Musique coupee, ou aucune playlist preparee.
        if !self.config.enabled {
            return self.state(
                &input,
                MusicDirective::None,
                DirectiveReason::Disabled,
                None,
                None,
                cadence,
            );
        }
        if self.playlist.is_none() {
            return self.state(
                &input,
                MusicDirective::None,
                DirectiveReason::NoPlaylist,
                None,
                None,
                cadence,
            );
        }

        // 2. Pause / reprise : une seule emission par transition.
        if input.state.is_paused() {
            let target = Some(self.base_target_bpm(&input));
            if self.in_pause {
                return self.state(
                    &input,
                    MusicDirective::None,
                    DirectiveReason::Paused,
                    target,
                    None,
                    cadence,
                );
            }
            self.in_pause = true;
            return self.state(
                &input,
                MusicDirective::Pause,
                DirectiveReason::Paused,
                target,
                None,
                cadence,
            );
        }
        let resumed = self.in_pause;
        self.in_pause = false;

        // 3 a 5. Cible de tempo du tick.
        let (desired_bpm, reason) = self.desired_bpm(&input);

        // 6. Comparaison a la piste en cours et selection eventuelle.
        let (directive, next_track_id, reason) = self.compare_with_current(desired_bpm, reason);

        if resumed && input.state == WorkoutState::Running {
            return self.state(
                &input,
                MusicDirective::Resume,
                DirectiveReason::Resumed,
                Some(desired_bpm),
                None,
                cadence,
            );
        }
        self.state(
            &input,
            directive,
            reason,
            Some(desired_bpm),
            next_track_id,
            cadence,
        )
    }

    /// Cible de base : consigne fixe, sinon loi de calibration, sinon
    /// reference. Le booleen indique si une consigne existe (raison OnPlan
    /// plutot que Steady).
    fn base_bpm(&self, input: &MusicInput) -> (f64, bool) {
        if let Some(fixed) = self.fixed_target_bpm() {
            return (fixed, true);
        }
        if let Some(pace) = input
            .target_pace_s_per_km
            .filter(|pace| pace.is_finite() && *pace > 0.0)
        {
            return (target_bpm_for_pace(pace, &self.config), true);
        }
        (self.config.reference_bpm, false)
    }

    fn base_target_bpm(&self, input: &MusicInput) -> f64 {
        self.base_bpm(input).0
    }

    fn fixed_target_bpm(&self) -> Option<f64> {
        self.playlist
            .as_ref()
            .and_then(|playlist| playlist.target_bpm)
            .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
    }

    /// Applique les regles 3 a 5 et borne le resultat dans min..=max.
    fn desired_bpm(&self, input: &MusicInput) -> (f64, DirectiveReason) {
        let (base, has_plan) = self.base_bpm(input);
        let behind_plan = input
            .shadow_delta_m
            .map(|delta| delta < -PLAN_GAP_M)
            .unwrap_or(false)
            && !input.shadow_on_plan;
        let ahead_of_plan = input
            .shadow_delta_m
            .map(|delta| delta > PLAN_GAP_M)
            .unwrap_or(false)
            && !input.shadow_on_plan;
        let pace_slow = match (input.current_pace_s_per_km, input.target_pace_s_per_km) {
            (Some(current), Some(target)) => current > target + PACE_GAP_S_PER_KM,
            _ => false,
        };
        let pace_fast = match (input.current_pace_s_per_km, input.target_pace_s_per_km) {
            (Some(current), Some(target)) => current < target - PACE_GAP_S_PER_KM,
            _ => false,
        };
        let heart_rate_high = matches!(input.heart_rate_zone, Some(zone) if zone >= 5);

        let (desired, reason) = if behind_plan {
            (base + self.config.boost_bpm, DirectiveReason::BehindPlan)
        } else if pace_slow {
            (base + self.config.boost_bpm, DirectiveReason::PaceSlow)
        } else if ahead_of_plan {
            (base - self.config.relax_bpm, DirectiveReason::AheadOfPlan)
        } else if heart_rate_high {
            (base - self.config.relax_bpm, DirectiveReason::HeartRateHigh)
        } else if pace_fast {
            (base - self.config.relax_bpm, DirectiveReason::PaceFast)
        } else if has_plan {
            (base, DirectiveReason::OnPlan)
        } else {
            (base, DirectiveReason::Steady)
        };
        let (low, high) = sorted_bounds(&self.config);
        (desired.clamp(low, high), reason)
    }

    /// Regle 6 : garder la piste si son tempo colle, sinon en choisir une autre.
    fn compare_with_current(
        &self,
        desired_bpm: f64,
        default_reason: DirectiveReason,
    ) -> (MusicDirective, Option<String>, DirectiveReason) {
        let Some(current) = self.current.as_ref() else {
            let next = self.pick_track(desired_bpm, false);
            return (MusicDirective::Play, next, default_reason);
        };
        let Some(bpm) = current.bpm.filter(|bpm| bpm.is_finite()) else {
            // Piste neutre : jamais choisie pour un changement de tempo.
            return (MusicDirective::Keep, None, default_reason);
        };
        if (bpm - desired_bpm).abs() <= self.config.switch_threshold_bpm {
            return (MusicDirective::Keep, None, default_reason);
        }
        match self.pick_track(desired_bpm, true) {
            Some(next) => (
                MusicDirective::SkipTo,
                Some(next),
                DirectiveReason::TrackBpmMismatch,
            ),
            // Aucune piste de rechange : on ne coupe pas la musique en cours.
            None => (MusicDirective::Keep, None, default_reason),
        }
    }

    /// Meilleure piste pour un BPM desire.
    ///
    /// require_known_bpm interdit les pistes au tempo inconnu (changement de
    /// tempo) ; sinon une piste neutre peut servir de repli. Les avoid_last
    /// dernieres pistes sont ecartees, sauf s'il ne reste rien.
    fn pick_track(&self, desired_bpm: f64, require_known_bpm: bool) -> Option<String> {
        let playlist = self.playlist.as_ref()?;
        let excluded = self.recent_tracks();
        let mut allowed: Vec<&Track> = playlist
            .tracks
            .iter()
            .filter(|track| !excluded.iter().any(|id| *id == track.id))
            .collect();
        if allowed.is_empty() {
            allowed = playlist.tracks.iter().collect();
        }
        let mut known: Vec<&Track> = allowed
            .iter()
            .copied()
            .filter(|track| track.bpm.is_some_and(|bpm| bpm.is_finite()))
            .collect();
        if !known.is_empty() {
            known.sort_by(|left, right| {
                let left_delta = (left.bpm.unwrap_or(f64::MAX) - desired_bpm).abs();
                let right_delta = (right.bpm.unwrap_or(f64::MAX) - desired_bpm).abs();
                left_delta
                    .partial_cmp(&right_delta)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(left.position.cmp(&right.position))
            });
            return known.first().map(|track| track.id.clone());
        }
        if require_known_bpm {
            return None;
        }
        allowed.first().map(|track| track.id.clone())
    }

    /// Identifiants des avoid_last dernieres pistes, plus recente d'abord.
    fn recent_tracks(&self) -> Vec<&str> {
        let mut recent: Vec<&str> = Vec::new();
        for id in self.history.iter().rev() {
            if recent.len() >= self.config.avoid_last {
                break;
            }
            if !recent.contains(&id.as_str()) {
                recent.push(id);
            }
        }
        recent
    }

    fn remember(&mut self, track_id: &str) {
        if self.history.last().map(String::as_str) == Some(track_id) {
            return;
        }
        self.history.push(track_id.to_string());
        let keep = (self.config.avoid_last.max(1) * 4).min(64);
        if self.history.len() > keep {
            let excess = self.history.len() - keep;
            self.history.drain(0..excess);
        }
    }

    /// Cadence du tick : capteur, sinon estimation depuis la vitesse.
    fn current_cadence(&self, input: &MusicInput) -> Option<f64> {
        if let Some(spm) = self.cadence_spm.filter(|spm| spm.is_finite() && *spm > 0.0) {
            return Some(spm);
        }
        input
            .speed_mps
            .filter(|speed| speed.is_finite() && *speed > 0.0)
            .map(cadence_from_speed)
    }

    #[allow(clippy::too_many_arguments)]
    fn state(
        &self,
        input: &MusicInput,
        directive: MusicDirective,
        reason: DirectiveReason,
        target_bpm: Option<f64>,
        next_track_id: Option<String>,
        cadence_spm: Option<f64>,
    ) -> MusicState {
        MusicState {
            enabled: self.config.enabled,
            playlist_id: self.playlist.as_ref().map(|playlist| playlist.id.clone()),
            playlist_name: self.playlist.as_ref().map(|playlist| playlist.name.clone()),
            target_bpm,
            target_cadence_spm: input
                .target_pace_s_per_km
                .filter(|pace| pace.is_finite() && *pace > 0.0)
                .map(|pace| target_cadence_spm(pace, &self.config)),
            cadence_spm,
            current: self.current.clone(),
            next_track_id,
            directive,
            reason,
        }
    }
}

// ------------------------------------------- couverture musicale (v3 10.1)

/// Marge de securite par defaut appliquee a la duree de course (5 %).
pub const MUSIC_MARGIN_RATIO: f64 = 0.05;

/// Tolerance relative qui absorbe les arrondis flottants sur les bornes : une
/// playlist exactement a la marge reste suffisante, et un rapport entier de
/// tracks_needed n'est pas pousse a l'entier superieur par du bruit numerique.
const COVERAGE_EPSILON_RATIO: f64 = 1e-9;

/// Validation de la couverture musicale d'une course : la playlist dure-t-elle
/// assez longtemps, et a quel tempo ?
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MusicCoverage {
    /// Somme des durees valides de la playlist (s).
    pub playlist_duration_s: f64,
    /// Duree estimee de la course (s), si elle est connue.
    pub race_duration_s: Option<f64>,
    /// Marge brute playlist - course (negatif si insuffisant).
    pub margin_s: Option<f64>,
    /// Vrai si la playlist couvre la course avec la marge demandee.
    pub sufficient: Option<bool>,
    /// Titres necessaires pour couvrir la course avec la marge.
    pub tracks_needed: Option<u32>,
    /// BPM moyen des titres renseignes.
    pub average_bpm: Option<f64>,
    /// BPM cible deduit de l'allure cible.
    pub target_bpm: Option<f64>,
    /// Ecart BPM moyen - BPM cible.
    pub bpm_delta: Option<f64>,
    /// Vrai si l'ecart tient dans MusicConfig::switch_threshold_bpm.
    pub tempo_ok: Option<bool>,
}

/// Somme des durees valides (finies et strictement positives) d'une playlist.
///
/// Une duree absente, non finie, nulle ou negative est ignoree : une playlist
/// vide vaut 0 s, sans panic.
pub fn playlist_duration_s(tracks: &[Track]) -> f64 {
    tracks
        .iter()
        .map(|track| track.duration_s)
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .sum()
}

/// Duree de course (s) : le temps cible s'il est fourni (> 0), sinon
/// distance x allure. None si l'information manque ou est invalide.
pub fn race_duration_s(
    distance_m: Option<f64>,
    target_time_s: Option<f64>,
    target_pace_s_per_km: Option<f64>,
) -> Option<f64> {
    if let Some(time) = target_time_s.filter(|time| time.is_finite() && *time > 0.0) {
        return Some(time);
    }
    let distance = distance_m.filter(|distance| distance.is_finite() && *distance > 0.0)?;
    let pace = target_pace_s_per_km.filter(|pace| pace.is_finite() && *pace > 0.0)?;
    Some(distance / 1000.0 * pace)
}

/// Validation : la playlist couvre-t-elle la course, et a quel tempo ?
///
/// Le verdict n'est jamais invente : sans duree de course connue, sufficient,
/// margin_s et tracks_needed restent None. Aucune division par zero et aucun
/// panic sur une playlist vide.
pub fn music_coverage(
    tracks: &[Track],
    race_duration_s: Option<f64>,
    target_pace_s_per_km: Option<f64>,
    cfg: &MusicConfig,
    margin_ratio: f64,
) -> MusicCoverage {
    let playlist_duration = playlist_duration_s(tracks);
    let race = race_duration_s.filter(|duration| duration.is_finite() && *duration > 0.0);
    let margin_ratio = if margin_ratio.is_finite() {
        margin_ratio
    } else {
        0.0
    };
    let safety = 1.0 + margin_ratio;

    let sufficient = race.map(|race| {
        let required = race * safety;
        let tolerance = required.abs().max(1.0) * COVERAGE_EPSILON_RATIO;
        playlist_duration + tolerance >= required
    });
    let tracks_needed = match (race, average_track_duration_s(tracks)) {
        (Some(race), Some(average)) => {
            let raw = race * safety / average;
            let tolerance = raw.abs().max(1.0) * COVERAGE_EPSILON_RATIO;
            Some((raw - tolerance).ceil().max(1.0) as u32)
        }
        _ => None,
    };

    let average_bpm = average_bpm(tracks);
    let target_bpm = target_pace_s_per_km
        .filter(|pace| pace.is_finite() && *pace > 0.0)
        .map(|pace| target_bpm_for_pace(pace, cfg));
    let bpm_delta = average_bpm
        .zip(target_bpm)
        .map(|(average, target)| average - target);
    let tempo_ok = bpm_delta.map(|delta| delta.abs() <= cfg.switch_threshold_bpm);

    MusicCoverage {
        playlist_duration_s: playlist_duration,
        race_duration_s: race,
        margin_s: race.map(|race| playlist_duration - race),
        sufficient,
        tracks_needed,
        average_bpm,
        target_bpm,
        bpm_delta,
        tempo_ok,
    }
}

/// Duree moyenne des titres dont la duree est valide (s) ; None si aucun.
fn average_track_duration_s(tracks: &[Track]) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0u32;
    for track in tracks {
        if track.duration_s.is_finite() && track.duration_s > 0.0 {
            total += track.duration_s;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let average = total / f64::from(count);
    (average.is_finite() && average > 0.0).then_some(average)
}

/// Moyenne des BPM renseignes (finis et strictement positifs) ; None si aucun.
fn average_bpm(tracks: &[Track]) -> Option<f64> {
    let mut total = 0.0;
    let mut count = 0u32;
    for bpm in tracks
        .iter()
        .filter_map(|track| track.bpm)
        .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
    {
        total += bpm;
        count += 1;
    }
    if count == 0 {
        return None;
    }
    Some(total / f64::from(count))
}

// ------------------------------------------- appariement fichiers <-> pistes

/// Extensions audio reconnues par l'appariement (contrat, section 3.1).
pub const AUDIO_EXTENSIONS: [&str; 6] = ["mp3", "m4a", "ogg", "opus", "flac", "wav"];
/// Seuil de validation d'un appariement : une piste sous ce score reste non
/// appariee et renommee, plutot que copiee au hasard.
pub const MATCH_THRESHOLD: f64 = 0.60;
/// Score de base quand le nom de fichier porte le titre.
const TITLE_SCORE: f64 = 0.60;
/// Bonus quand le nom porte aussi l'artiste.
const ARTIST_BONUS: f64 = 0.25;
/// Bonus quand la duree du fichier colle a moins de 3 s.
const DURATION_BONUS: f64 = 0.15;
/// Score de repli (artiste + duree a moins de 2 s) : volontairement **sous**
/// le seuil, pour demander a l'utilisateur de renommer son fichier.
const ARTIST_DURATION_SCORE: f64 = 0.55;
/// Tolerance de duree qui donne le bonus complet (s).
const DURATION_BONUS_TOLERANCE_S: f64 = 3.0;
/// Tolerance de duree du repli artiste seul (s).
const ARTIST_DURATION_TOLERANCE_S: f64 = 2.0;

/// Fichier audio trouve sur le disque de l'utilisateur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalFile {
    /// Chemin complet.
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    /// Duree estimee du fichier, si elle est connue.
    #[serde(default)]
    pub duration_s: Option<f64>,
    /// BPM lu dans les balises du fichier, sinon None.
    #[serde(default)]
    pub bpm: Option<f64>,
}

/// Piste attendue par un manifeste de transfert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WantedTrack {
    pub id: String,
    #[serde(default)]
    pub position: u32,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration_s: Option<f64>,
    #[serde(default)]
    pub bpm: Option<f64>,
    /// Nom du fichier une fois copie sur la montre (None tant qu'il est inconnu).
    #[serde(default)]
    pub file: Option<String>,
    /// Taille du fichier copie (None tant qu'il est inconnu).
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

/// Manifeste de transfert (produit par le backend, ecrit sur la montre).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferManifest {
    /// Version du format : 1.
    #[serde(default = "default_manifest_version")]
    pub version: u32,
    pub playlist_id: String,
    pub name: String,
    /// "spotify" | "manual".
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target_bpm: Option<f64>,
    #[serde(default)]
    pub tracks: Vec<WantedTrack>,
}

fn default_manifest_version() -> u32 {
    1
}

/// Resultat de l'appariement d'une piste : fichier trouve, ou None.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileMatch {
    pub track_id: String,
    #[serde(default)]
    pub file: Option<LocalFile>,
    /// Score du couple, entre 0.0 et 1.0.
    pub score: f64,
}

/// Normalise un libelle pour l'appariement : minuscules, sans accents,
/// ponctuation reduite a des espaces, suffixes (feat., remaster, official
/// video, lyrics) retires, numero de piste en tete retire.
///
/// La fonction est stable : deux libelles equivalents donnent la meme chaine.
pub fn normalize_label(text: &str) -> String {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in fold_text(text).chars() {
        if character.is_ascii_alphanumeric() {
            current.push(character);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    // "feat. ..." : l'invite et tout ce qui suit sont retires du libelle.
    if let Some(index) = tokens.iter().position(|token| is_featured_token(token)) {
        tokens.truncate(index);
    }
    // Bruit des noms de fichiers (remaster, official video, lyrics...).
    tokens.retain(|token| !is_noise_token(token));
    // Numero de piste en tete ("01 - ...", "12. ...").
    let start = tokens
        .iter()
        .position(|token| !token.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(tokens.len());
    tokens.drain(..start);
    tokens.join(" ")
}

/// Minuscules sans accents : chaque caractere accentue devient sa lettre de
/// base (oe pour oe-ligature, ss pour eszett).
fn fold_text(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for character in text.chars() {
        for lowered in character.to_lowercase() {
            match lowered {
                'a' | '\u{e0}' | '\u{e1}' | '\u{e2}' | '\u{e3}' | '\u{e4}' | '\u{e5}' => {
                    folded.push('a')
                }
                'c' | '\u{e7}' => folded.push('c'),
                'e' | '\u{e8}' | '\u{e9}' | '\u{ea}' | '\u{eb}' => folded.push('e'),
                'i' | '\u{ec}' | '\u{ed}' | '\u{ee}' | '\u{ef}' => folded.push('i'),
                'n' | '\u{f1}' => folded.push('n'),
                'o' | '\u{f2}' | '\u{f3}' | '\u{f4}' | '\u{f5}' | '\u{f6}' | '\u{f8}' => {
                    folded.push('o')
                }
                'u' | '\u{f9}' | '\u{fa}' | '\u{fb}' | '\u{fc}' => folded.push('u'),
                'y' | '\u{fd}' | '\u{ff}' => folded.push('y'),
                '\u{153}' => folded.push_str("oe"),
                '\u{df}' => folded.push_str("ss"),
                other => folded.push(other),
            }
        }
    }
    folded
}

/// Vrai pour les marqueurs d'invite : feat, feat., featuring, ft.
fn is_featured_token(token: &str) -> bool {
    matches!(token, "feat" | "featuring" | "ft")
}

/// Vrai pour le bruit courant d'un nom de fichier.
fn is_noise_token(token: &str) -> bool {
    matches!(
        token,
        "remaster" | "remastered" | "official" | "video" | "lyrics" | "lyric" | "hd" | "hq"
    )
}

/// Vrai si `haystack` contient `needle` comme suite de mots entiers.
fn contains_phrase(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let padded_haystack = format!(" {haystack} ");
    let padded_needle = format!(" {needle} ");
    padded_haystack.contains(&padded_needle)
}

/// Nom de fichier sans son extension.
fn file_stem(file_name: &str) -> &str {
    match file_name.rfind('.') {
        Some(index) if index > 0 => &file_name[..index],
        _ => file_name,
    }
}

/// Score d'un couple (piste, fichier), de 0.0 a 1.0.
fn match_score(wanted: &WantedTrack, file: &LocalFile) -> f64 {
    let file_label = normalize_label(file_stem(&file.file_name));
    let title = normalize_label(&wanted.title);
    let artist = wanted
        .artist
        .as_deref()
        .map(normalize_label)
        .unwrap_or_default();
    let title_found = contains_phrase(&file_label, &title);
    let artist_found = contains_phrase(&file_label, &artist);
    let duration_gap = match (wanted.duration_s, file.duration_s) {
        (Some(wanted), Some(found)) if wanted.is_finite() && found.is_finite() => {
            Some((wanted - found).abs())
        }
        _ => None,
    };

    if title_found {
        let mut score = TITLE_SCORE;
        if artist_found {
            score += ARTIST_BONUS;
        }
        if duration_gap.is_some_and(|gap| gap < DURATION_BONUS_TOLERANCE_S) {
            score += DURATION_BONUS;
        }
        return score;
    }
    if artist_found && duration_gap.is_some_and(|gap| gap < ARTIST_DURATION_TOLERANCE_S) {
        return ARTIST_DURATION_SCORE;
    }
    0.0
}

/// Apparie des pistes a des fichiers. Deterministe : les couples
/// (piste, fichier) sont tries par score decroissant puis par position de
/// piste ; un fichier ne sert qu'a une piste ; seuil de validation 0.6.
///
/// Renvoie une entree par piste, dans l'ordre du manifeste. Une piste non
/// appariee porte `file: None` et le meilleur score rencontre.
pub fn match_tracks(wanted: &[WantedTrack], files: &[LocalFile]) -> Vec<FileMatch> {
    struct Candidate {
        track_index: usize,
        file_index: usize,
        score: f64,
    }

    let mut best = vec![0.0_f64; wanted.len()];
    let mut candidates = Vec::new();
    for (track_index, track) in wanted.iter().enumerate() {
        for (file_index, file) in files.iter().enumerate() {
            let score = match_score(track, file);
            if score > best[track_index] {
                best[track_index] = score;
            }
            if score >= MATCH_THRESHOLD {
                candidates.push(Candidate {
                    track_index,
                    file_index,
                    score,
                });
            }
        }
    }

    candidates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                wanted[left.track_index]
                    .position
                    .cmp(&wanted[right.track_index].position)
            })
            .then_with(|| {
                files[left.file_index]
                    .file_name
                    .cmp(&files[right.file_index].file_name)
            })
            .then_with(|| {
                files[left.file_index]
                    .path
                    .cmp(&files[right.file_index].path)
            })
    });

    let mut file_taken = vec![false; files.len()];
    let mut assigned: Vec<Option<(usize, f64)>> = vec![None; wanted.len()];
    for candidate in candidates {
        if assigned[candidate.track_index].is_some() || file_taken[candidate.file_index] {
            continue;
        }
        assigned[candidate.track_index] = Some((candidate.file_index, candidate.score));
        file_taken[candidate.file_index] = true;
    }

    wanted
        .iter()
        .enumerate()
        .map(|(index, track)| match assigned[index] {
            Some((file_index, score)) => FileMatch {
                track_id: track.id.clone(),
                file: Some(files[file_index].clone()),
                score,
            },
            None => FileMatch {
                track_id: track.id.clone(),
                file: None,
                score: best[index],
            },
        })
        .collect()
}

/// Lit un manifeste JSON (format ci-dessus). None si illisible.
pub fn parse_manifest(json: &str) -> Option<TransferManifest> {
    serde_json::from_str(json).ok()
}

/// Nom de fichier propose pour la montre : "01 - Artiste - Titre.mp3"
/// (extension conservee a l'ecriture reelle).
pub fn suggested_file_name(track: &WantedTrack, extension: &str) -> String {
    let extension = extension.trim_start_matches('.');
    let number = track.position.max(1);
    let title = sanitise_file_part(&track.title, "Titre");
    match track
        .artist
        .as_deref()
        .map(|artist| sanitise_file_part(artist, ""))
        .filter(|artist| !artist.is_empty())
    {
        Some(artist) => format!("{number:02} - {artist} - {title}.{extension}"),
        None => format!("{number:02} - {title}.{extension}"),
    }
}

/// Nettoie un fragment de nom de fichier : caracteres interdits, espaces
/// multiples, bords. `fallback` evite un nom vide.
fn sanitise_file_part(text: &str, fallback: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_control()
            || matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        {
            cleaned.push(' ');
        } else {
            cleaned.push(character);
        }
    }
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches(['.', ' ']).trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> MusicConfig {
        MusicConfig {
            enabled: true,
            ..Default::default()
        }
    }

    fn track(id: &str, bpm: Option<f64>, position: u32) -> Track {
        Track {
            id: id.to_string(),
            title: format!("Titre {id}"),
            artist: Some("Artiste".to_string()),
            duration_s: 200.0,
            bpm,
            position,
        }
    }

    fn playlist() -> Playlist {
        Playlist {
            id: "p1".to_string(),
            name: "Run 170".to_string(),
            target_bpm: Some(170.0),
            tracks: vec![
                track("t1", Some(170.0), 0),
                track("t2", Some(160.0), 1),
                track("t3", Some(176.0), 2),
                track("t4", Some(180.0), 3),
            ],
        }
    }

    fn input(state: WorkoutState) -> MusicInput {
        MusicInput {
            t_ms: 1_700_000_000_000,
            state,
            target_pace_s_per_km: Some(300.0),
            current_pace_s_per_km: Some(300.0),
            shadow_delta_m: None,
            shadow_on_plan: false,
            heart_rate_zone: None,
            speed_mps: Some(1000.0 / 300.0),
        }
    }

    fn now_playing(id: &str, bpm: Option<f64>) -> NowPlaying {
        NowPlaying {
            track_id: id.to_string(),
            title: format!("Titre {id}"),
            artist: None,
            bpm,
            position_s: 0.0,
        }
    }

    // -------------------------------------------------------- configuration

    #[test]
    fn defaults_match_the_contract() {
        let cfg = MusicConfig::default();
        assert!(cfg.enabled);
        assert!((cfg.reference_bpm - 170.0).abs() < 1e-9);
        assert!((cfg.reference_pace_s_per_km - 300.0).abs() < 1e-9);
        assert!((cfg.pace_elasticity - 0.35).abs() < 1e-9);
        assert!((cfg.min_bpm - 100.0).abs() < 1e-9);
        assert!((cfg.max_bpm - 200.0).abs() < 1e-9);
        assert!((cfg.switch_threshold_bpm - 8.0).abs() < 1e-9);
        assert!((cfg.boost_bpm - 6.0).abs() < 1e-9);
        assert!((cfg.relax_bpm - 6.0).abs() < 1e-9);
        assert!(cfg.announce);
        assert_eq!(cfg.avoid_last, 3);
    }

    // ------------------------------------------------------------- calibration

    #[test]
    fn calibration_follows_the_documented_law() {
        let cfg = MusicConfig::default();
        assert!((target_bpm_for_pace(300.0, &cfg) - 170.0).abs() < 0.5);
        assert!((target_bpm_for_pace(240.0, &cfg) - 183.8).abs() < 0.5);
        // Loi explicite du contrat : 170 * (300 / 360)^0.35 = 159.5 BPM.
        assert!((target_bpm_for_pace(360.0, &cfg) - 159.5).abs() < 0.5);
        // Le tempo decroit quand l'allure ralentit.
        assert!(target_bpm_for_pace(360.0, &cfg) < target_bpm_for_pace(300.0, &cfg));
        assert!(target_bpm_for_pace(240.0, &cfg) > target_bpm_for_pace(300.0, &cfg));
    }

    #[test]
    fn calibration_is_clamped_and_never_panics() {
        let cfg = MusicConfig::default();
        assert!((target_bpm_for_pace(60.0, &cfg) - 200.0).abs() < 1e-9);
        assert!((target_bpm_for_pace(3600.0, &cfg) - 100.0).abs() < 1e-9);
        assert!((target_bpm_for_pace(0.0, &cfg) - 170.0).abs() < 1e-9);
        assert!((target_bpm_for_pace(f64::NAN, &cfg) - 170.0).abs() < 1e-9);
        // Bornes inversees : pas de panique de clamp.
        let inverted = MusicConfig {
            min_bpm: 200.0,
            max_bpm: 100.0,
            ..Default::default()
        };
        assert!(target_bpm_for_pace(300.0, &inverted).is_finite());
    }

    #[test]
    fn cadence_holds_the_steps_at_the_reference_pace() {
        let cfg = MusicConfig::default();
        // 60 * 3.3333 / 1.15 = 173.9 pas/minute.
        assert!((target_cadence_spm(300.0, &cfg) - 173.9).abs() < 0.5);
        // La cadence visee augmente avec la vitesse.
        assert!(target_cadence_spm(240.0, &cfg) > target_cadence_spm(300.0, &cfg));
        assert!(target_cadence_spm(360.0, &cfg) < target_cadence_spm(300.0, &cfg));
    }

    #[test]
    fn target_cadence_saturates_at_both_bounds() {
        let cfg = MusicConfig::default();
        // 3:00/km : ~236 spm sans borne, sature a 210.
        assert!((target_cadence_spm(180.0, &cfg) - CADENCE_MAX_SPM).abs() < 1e-9);
        // 15:00/km : ~90 spm sans borne, sature a 140.
        assert!((target_cadence_spm(900.0, &cfg) - CADENCE_MIN_SPM).abs() < 1e-9);
        // 4:00/km reste dans l'intervalle.
        let at_four = target_cadence_spm(240.0, &cfg);
        assert!(at_four > CADENCE_MIN_SPM && at_four < CADENCE_MAX_SPM);
    }

    #[test]
    fn cadence_from_speed_is_clamped_and_consistent() {
        // Meme loi que la calibration, a l'allure de reference par defaut.
        assert!((cadence_from_speed(1000.0 / 300.0) - 173.9).abs() < 0.5);
        assert!((cadence_from_speed(0.0) - CADENCE_MIN_SPM).abs() < 1e-9);
        assert!((cadence_from_speed(-3.0) - CADENCE_MIN_SPM).abs() < 1e-9);
        assert!((cadence_from_speed(20.0) - CADENCE_MAX_SPM).abs() < 1e-9);
        assert!((cadence_from_speed(f64::NAN) - CADENCE_MIN_SPM).abs() < 1e-9);
    }

    // --------------------------------------------------------------- tap-tempo

    #[test]
    fn tap_tempo_needs_at_least_four_taps() {
        assert_eq!(tap_tempo(&[]), None);
        assert_eq!(tap_tempo(&[0, 500, 1000]), None);
        let bpm = tap_tempo(&[0, 500, 1000, 1500]).unwrap();
        assert!((bpm - 120.0).abs() < 0.5);
    }

    #[test]
    fn tap_tempo_uses_the_median_and_ignores_outliers() {
        // 500, 500, 4000 (aberrant), 500, 500 -> median 500 -> 120 BPM.
        let bpm = tap_tempo(&[0, 500, 1000, 5000, 5500, 6000]).unwrap();
        assert!((bpm - 120.0).abs() < 0.5);
        // Taps colles (intervalle nul) : aucun intervalle retenu.
        assert_eq!(tap_tempo(&[100, 100, 100, 100]), None);
        // Taps trop espaces : plus de 2 s, hors bornes.
        assert_eq!(tap_tempo(&[0, 3000, 6000, 9000]), None);
    }

    // ------------------------------------------------------------------ balises

    /// Construit un tag ID3v2.3 minimal avec une trame TBPM.
    fn id3v2_tbpm(text: &[u8]) -> Vec<u8> {
        let mut body = vec![0u8]; // encodage ISO-8859-1
        body.extend_from_slice(text);
        let frame_size = body.len() as u32;
        let total = 10 + frame_size;
        let mut tag = Vec::new();
        tag.extend_from_slice(b"ID3");
        tag.extend_from_slice(&[3, 0, 0]);
        tag.push((total >> 21) as u8 & 0x7F);
        tag.push((total >> 14) as u8 & 0x7F);
        tag.push((total >> 7) as u8 & 0x7F);
        tag.push(total as u8 & 0x7F);
        tag.extend_from_slice(b"TBPM");
        tag.extend_from_slice(&frame_size.to_be_bytes());
        tag.extend_from_slice(&[0, 0]);
        tag.extend_from_slice(&body);
        tag
    }

    /// Construit un atome MP4 tmpo avec son sous-atome data.
    fn mp4_tmpo(bpm: u16) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&18u32.to_be_bytes());
        data.extend_from_slice(b"data");
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&bpm.to_be_bytes());
        let mut atom = Vec::new();
        atom.extend_from_slice(&((8 + data.len()) as u32).to_be_bytes());
        atom.extend_from_slice(b"tmpo");
        atom.extend_from_slice(&data);
        atom
    }

    /// Variante sans octet d'encodage (encodeurs qui ecrivent le nombre seul).
    fn id3v2_tbpm_raw(text: &[u8]) -> Vec<u8> {
        let frame_size = text.len() as u32;
        let tag_size = 10 + frame_size;
        let mut tag = Vec::new();
        tag.extend_from_slice(b"ID3");
        tag.extend_from_slice(&[3, 0, 0]);
        tag.push((tag_size >> 21) as u8 & 0x7F);
        tag.push((tag_size >> 14) as u8 & 0x7F);
        tag.push((tag_size >> 7) as u8 & 0x7F);
        tag.push(tag_size as u8 & 0x7F);
        tag.extend_from_slice(b"TBPM");
        tag.extend_from_slice(&frame_size.to_be_bytes());
        tag.extend_from_slice(&[0, 0]);
        tag.extend_from_slice(text);
        tag
    }

    #[test]
    fn bpm_from_id3v2_accepts_both_text_layouts() {
        assert_eq!(
            bpm_from_tags(&id3v2_tbpm_raw(b"172\x00"), "titre.mp3"),
            Some(172.0)
        );
        assert_eq!(
            bpm_from_tags(&id3v2_tbpm_raw(b"128"), "titre.mp3"),
            Some(128.0)
        );
    }

    #[test]
    fn bpm_from_id3v2_reads_tbpm() {
        assert_eq!(bpm_from_tags(&id3v2_tbpm(b"172"), "song.mp3"), Some(172.0));
        assert_eq!(
            bpm_from_tags(&id3v2_tbpm(b"172/120"), "song.mp3"),
            Some(172.0)
        );
        // Une trame sans TBPM ne doit rien donner.
        assert_eq!(
            bpm_from_tags(b"ID3\x03\x00\x00\x00\x00\x00\x00", "song.mp3"),
            None
        );
    }

    #[test]
    fn bpm_from_vorbis_comment_reads_bpm() {
        let bytes = b"\x0bM-pacer test\x00TITLE=Wake me up\x00BPM=173.5\x00";
        assert_eq!(bpm_from_tags(bytes, "song.ogg"), Some(173.5));
        assert_eq!(bpm_from_tags(bytes, "song.flac"), Some(173.5));
        assert_eq!(
            bpm_from_tags(b"TITLE=x\x00bpm=128\x00", "song.opus"),
            Some(128.0)
        );
    }

    #[test]
    fn bpm_from_mp4_reads_tmpo() {
        assert_eq!(bpm_from_tags(&mp4_tmpo(174), "song.m4a"), Some(174.0));
        assert_eq!(bpm_from_tags(&mp4_tmpo(128), "song.mp4"), Some(128.0));
    }

    #[test]
    fn bpm_from_tags_is_silent_without_a_tag() {
        assert_eq!(bpm_from_tags(b"", "song.mp3"), None);
        assert_eq!(bpm_from_tags(b"\xff\xfb\x90\x00", "song.mp3"), None);
        assert_eq!(bpm_from_tags(b"BPM=0\x00", "song.ogg"), None);
    }

    // --------------------------------------------------------------- directeur

    #[test]
    fn disabled_music_emits_none_for_disabled_reason() {
        let mut director = MusicDirector::new(MusicConfig {
            enabled: false,
            ..Default::default()
        });
        director.set_playlist(Some(playlist()));
        let state = director.evaluate(input(WorkoutState::Running));
        assert!(!state.enabled);
        assert_eq!(state.directive, MusicDirective::None);
        assert_eq!(state.reason, DirectiveReason::Disabled);
        assert_eq!(state.target_bpm, None);
    }

    #[test]
    fn missing_playlist_emits_none_for_no_playlist_reason() {
        let mut director = MusicDirector::new(config());
        let state = director.evaluate(input(WorkoutState::Running));
        assert!(state.enabled);
        assert_eq!(state.directive, MusicDirective::None);
        assert_eq!(state.reason, DirectiveReason::NoPlaylist);
        assert_eq!(state.playlist_id, None);
    }

    #[test]
    fn first_evaluation_plays_the_best_track() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        let state = director.evaluate(input(WorkoutState::Running));
        assert_eq!(state.directive, MusicDirective::Play);
        assert_eq!(state.reason, DirectiveReason::OnPlan);
        assert_eq!(state.next_track_id.as_deref(), Some("t1")); // 170 pile
        assert_eq!(state.target_bpm, Some(170.0));
        assert_eq!(state.playlist_id.as_deref(), Some("p1"));
        assert_eq!(state.playlist_name.as_deref(), Some("Run 170"));
        assert!(state.target_cadence_spm.is_some());
        assert!(state.cadence_spm.is_some());
    }

    #[test]
    fn a_matching_track_is_kept() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        director.on_now_playing(Some(now_playing("t1", Some(172.0))));
        let state = director.evaluate(input(WorkoutState::Running));
        assert_eq!(state.directive, MusicDirective::Keep);
        assert_eq!(state.next_track_id, None);
        assert_eq!(state.current.as_ref().unwrap().track_id, "t1");
    }

    #[test]
    fn a_mismatched_track_is_switched() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        director.on_now_playing(Some(now_playing("t2", Some(160.0))));
        let state = director.evaluate(input(WorkoutState::Running));
        assert_eq!(state.directive, MusicDirective::SkipTo);
        assert_eq!(state.reason, DirectiveReason::TrackBpmMismatch);
        assert_eq!(state.next_track_id.as_deref(), Some("t1")); // 170 le plus proche
    }

    #[test]
    fn a_track_without_bpm_is_never_replaced_for_tempo() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        director.on_now_playing(Some(now_playing("t9", None)));
        let state = director.evaluate(input(WorkoutState::Running));
        assert_eq!(state.directive, MusicDirective::Keep);
        assert_eq!(state.next_track_id, None);
    }

    #[test]
    fn being_behind_the_plan_boosts_the_tempo() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        let mut entry = input(WorkoutState::Running);
        entry.shadow_delta_m = Some(-50.0);
        let state = director.evaluate(entry);
        assert_eq!(state.target_bpm, Some(176.0));
        assert_eq!(state.directive, MusicDirective::Play);
        assert_eq!(state.reason, DirectiveReason::BehindPlan);
    }

    #[test]
    fn being_ahead_of_the_plan_relaxes_the_tempo() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        let mut entry = input(WorkoutState::Running);
        entry.shadow_delta_m = Some(50.0);
        let state = director.evaluate(entry);
        assert_eq!(state.target_bpm, Some(164.0));
        assert_eq!(state.reason, DirectiveReason::AheadOfPlan);
    }

    #[test]
    fn pace_gaps_boost_or_relax() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        let mut slow = input(WorkoutState::Running);
        slow.current_pace_s_per_km = Some(310.0);
        let state = director.evaluate(slow);
        assert_eq!(state.reason, DirectiveReason::PaceSlow);
        assert_eq!(state.target_bpm, Some(176.0));

        let mut fast = input(WorkoutState::Running);
        fast.current_pace_s_per_km = Some(290.0);
        let state = director.evaluate(fast);
        assert_eq!(state.reason, DirectiveReason::PaceFast);
        assert_eq!(state.target_bpm, Some(164.0));
    }

    #[test]
    fn a_high_heart_rate_relaxes_the_tempo() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        let mut entry = input(WorkoutState::Running);
        entry.heart_rate_zone = Some(5);
        let state = director.evaluate(entry);
        assert_eq!(state.reason, DirectiveReason::HeartRateHigh);
        assert_eq!(state.target_bpm, Some(164.0));
    }

    #[test]
    fn a_plan_gap_within_tolerance_stays_on_plan() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        let mut entry = input(WorkoutState::Running);
        entry.shadow_delta_m = Some(-18.0);
        entry.shadow_on_plan = true;
        let state = director.evaluate(entry);
        assert_eq!(state.reason, DirectiveReason::OnPlan);
        assert_eq!(state.target_bpm, Some(170.0));
    }

    #[test]
    fn without_any_target_the_reason_is_steady() {
        let mut director = MusicDirector::new(config());
        let mut playlist = playlist();
        playlist.target_bpm = None;
        director.set_playlist(Some(playlist));
        let mut entry = input(WorkoutState::Running);
        entry.target_pace_s_per_km = None;
        let state = director.evaluate(entry);
        assert_eq!(state.reason, DirectiveReason::Steady);
        assert_eq!(state.target_bpm, Some(170.0));
    }

    #[test]
    fn the_selection_avoids_the_last_tracks() {
        let mut director = MusicDirector::new(MusicConfig {
            avoid_last: 3,
            ..config()
        });
        let mut playlist = playlist();
        playlist.tracks = vec![
            track("t1", Some(200.0), 0),
            track("t2", Some(130.0), 1),
            track("t3", Some(130.0), 2),
            track("t4", Some(130.0), 3),
            track("t5", Some(170.0), 4),
        ];
        director.set_playlist(Some(playlist));
        for id in ["t2", "t3", "t4", "t1"] {
            director.on_now_playing(Some(now_playing(id, Some(200.0))));
        }
        let state = director.evaluate(input(WorkoutState::Running));
        assert_eq!(state.directive, MusicDirective::SkipTo);
        // t3, t4 et t1 sont ecartees : il ne reste que t2 (130) et t5 (170).
        assert_eq!(state.next_track_id.as_deref(), Some("t5"));
    }

    #[test]
    fn pause_and_resume_are_emitted_once_per_transition() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));

        let mut paused = input(WorkoutState::Paused);
        let first = director.evaluate(paused);
        assert_eq!(first.directive, MusicDirective::Pause);
        assert_eq!(first.reason, DirectiveReason::Paused);
        let second = director.evaluate(paused);
        assert_eq!(second.directive, MusicDirective::None);
        assert_eq!(second.reason, DirectiveReason::Paused);

        paused.state = WorkoutState::AutoPaused;
        assert_eq!(director.evaluate(paused).directive, MusicDirective::None);

        let running = director.evaluate(input(WorkoutState::Running));
        assert_eq!(running.directive, MusicDirective::Resume);
        assert_eq!(running.reason, DirectiveReason::Resumed);
        let after = director.evaluate(input(WorkoutState::Running));
        assert_ne!(after.directive, MusicDirective::Resume);
    }

    #[test]
    fn the_state_is_stable_for_identical_inputs() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        director.on_now_playing(Some(now_playing("t2", Some(160.0))));
        let entry = input(WorkoutState::Running);
        let first = director.evaluate(entry);
        let second = director.evaluate(entry);
        let third = director.evaluate(entry);
        assert_eq!(first, second);
        assert_eq!(second, third);
    }

    #[test]
    fn cadence_prefers_the_sensor_and_falls_back_to_speed() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        let entry = input(WorkoutState::Running);
        let estimated = director.evaluate(entry).cadence_spm.unwrap();
        assert!((estimated - 173.9).abs() < 0.5);

        director.on_cadence(168.0);
        assert_eq!(director.evaluate(entry).cadence_spm, Some(168.0));

        let mut without_speed = entry;
        without_speed.speed_mps = None;
        assert_eq!(director.evaluate(without_speed).cadence_spm, Some(168.0));
    }

    #[test]
    fn changing_the_playlist_forgets_the_previous_track() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        director.on_now_playing(Some(now_playing("t1", Some(170.0))));
        director.set_playlist(Some(Playlist {
            id: "p2".to_string(),
            name: "Autre".to_string(),
            target_bpm: None,
            tracks: vec![track("x1", Some(150.0), 0)],
        }));
        let state = director.evaluate(input(WorkoutState::Running));
        assert_eq!(state.current, None);
        assert_eq!(state.playlist_id.as_deref(), Some("p2"));
        assert_eq!(state.directive, MusicDirective::Play);
        assert_eq!(state.next_track_id.as_deref(), Some("x1"));
    }

    #[test]
    fn reset_forgets_the_current_track_but_keeps_the_playlist() {
        let mut director = MusicDirector::new(config());
        director.set_playlist(Some(playlist()));
        director.on_now_playing(Some(now_playing("t1", Some(170.0))));
        director.on_cadence(170.0);
        director.reset();
        assert!(director.playlist().is_some());
        let state = director.evaluate(input(WorkoutState::Running));
        assert_eq!(state.current, None);
        assert_eq!(state.cadence_spm.map(|spm| spm.round()), Some(174.0));
    }

    #[test]
    fn directives_and_reasons_serialise_in_pascal_case() {
        let state = MusicState {
            directive: MusicDirective::Keep,
            reason: DirectiveReason::OnPlan,
            ..Default::default()
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"directive\":\"Keep\""), "{json}");
        assert!(json.contains("\"reason\":\"OnPlan\""), "{json}");
        assert!(json.contains("\"playlist_id\":null"), "{json}");

        let round_trip: MusicState = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip, state);
    }

    #[test]
    fn music_config_accepts_partial_json() {
        let cfg: MusicConfig = serde_json::from_str(r#"{"enabled":true,"avoid_last":5}"#).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.avoid_last, 5);
        assert!((cfg.reference_bpm - 170.0).abs() < 1e-9);
    }
}

#[cfg(test)]
mod match_tests {
    use super::*;

    fn wanted(id: &str, position: u32, title: &str) -> WantedTrack {
        WantedTrack {
            id: id.to_string(),
            position,
            title: title.to_string(),
            artist: None,
            album: None,
            duration_s: None,
            bpm: None,
            file: None,
            size_bytes: None,
        }
    }

    fn local(file_name: &str) -> LocalFile {
        LocalFile {
            path: format!("D:/Musique/{file_name}"),
            file_name: file_name.to_string(),
            size_bytes: 4_523_112,
            duration_s: None,
            bpm: None,
        }
    }

    fn matched(matches: &[FileMatch], index: usize) -> &LocalFile {
        matches[index]
            .file
            .as_ref()
            .unwrap_or_else(|| panic!("piste {} non appariee", matches[index].track_id))
    }

    // ----------------------------------------------------------- normalisation

    #[test]
    fn normalize_label_folds_accents_and_case() {
        assert_eq!(normalize_label("Été ÉTÉ"), "ete ete");
        assert_eq!(normalize_label("Ça va, déjà ?"), "ca va deja");
        assert_eq!(normalize_label("Cœur"), "coeur");
    }

    #[test]
    fn normalize_label_drops_track_numbers_and_noise() {
        assert_eq!(normalize_label("01 - Wake Me Up"), "wake me up");
        assert_eq!(normalize_label("12. Titre (Remastered)"), "titre");
        assert_eq!(normalize_label("03_Titre_Officiel"), "titre officiel");
        assert_eq!(normalize_label("Wake Me Up (Official Video)"), "wake me up");
    }

    #[test]
    fn normalize_label_strips_the_featured_credit() {
        assert_eq!(
            normalize_label("Wake Me Up (feat. Aloe Blacc)"),
            "wake me up"
        );
        assert_eq!(normalize_label("Titre featuring Invite"), "titre");
        assert_eq!(normalize_label("Titre ft. Invite"), "titre");
    }

    // -------------------------------------------------------------- appariement

    #[test]
    fn matching_ignores_accents_case_and_track_numbers() {
        let tracks = vec![wanted("t1", 1, "Soleil d'été")];
        let files = vec![local("01 - Artiste - SOLEIL D'ETE.mp3")];
        let result = match_tracks(&tracks, &files);
        assert_eq!(
            matched(&result, 0).file_name,
            "01 - Artiste - SOLEIL D'ETE.mp3"
        );
        assert!((result[0].score - 0.60).abs() < 1e-9);
    }

    #[test]
    fn matching_uses_artist_and_duration_bonuses() {
        let mut track = wanted("t1", 1, "Wake me up");
        track.artist = Some("Avicii".to_string());
        track.duration_s = Some(249.0);

        let mut file = local("01 - Avicii - Wake me up.mp3");
        file.duration_s = Some(250.0);
        let result = match_tracks(&[track.clone()], &[file]);
        assert!((result[0].score - 1.0).abs() < 1e-9);

        let mut without_duration = local("01 - Avicii - Wake me up.mp3");
        without_duration.duration_s = None;
        let result = match_tracks(&[track.clone()], &[without_duration]);
        assert!((result[0].score - 0.85).abs() < 1e-9);

        let mut far = local("01 - Avicii - Wake me up.mp3");
        far.duration_s = Some(260.0);
        let result = match_tracks(&[track], &[far]);
        assert!((result[0].score - 0.85).abs() < 1e-9);
    }

    #[test]
    fn a_title_alone_reaches_the_threshold() {
        let result = match_tracks(&[wanted("t1", 1, "Niveau")], &[local("Niveau.mp3")]);
        assert!((result[0].score - MATCH_THRESHOLD).abs() < 1e-9);
        assert!(result[0].file.is_some());
    }

    #[test]
    fn artist_and_duration_without_the_title_stay_below_the_threshold() {
        let mut track = wanted("t1", 1, "Levels");
        track.artist = Some("Avicii".to_string());
        track.duration_s = Some(200.0);

        let mut near = local("Avicii - Autre morceau.mp3");
        near.duration_s = Some(201.0);
        let result = match_tracks(&[track.clone()], &[near]);
        assert!((result[0].score - 0.55).abs() < 1e-9);
        assert!(result[0].file.is_none());

        let mut far = local("Avicii - Autre morceau.mp3");
        far.duration_s = Some(203.0);
        let result = match_tracks(&[track], &[far]);
        assert!(result[0].score.abs() < 1e-9);
        assert!(result[0].file.is_none());
    }

    #[test]
    fn a_file_is_used_by_a_single_track_only() {
        let tracks = vec![wanted("t1", 1, "Meme titre"), wanted("t2", 2, "Meme titre")];
        let files = vec![local("Meme titre.mp3")];
        let result = match_tracks(&tracks, &files);
        assert_eq!(result.len(), 2);
        assert!(result[0].file.is_some());
        assert!(result[1].file.is_none());
        assert_eq!(result[1].track_id, "t2");
        assert!((result[1].score - 0.60).abs() < 1e-9);
    }

    #[test]
    fn matching_is_deterministic() {
        let tracks = vec![
            wanted("t1", 1, "Alpha"),
            wanted("t2", 2, "Beta"),
            wanted("t3", 3, "Gamma"),
        ];
        let files = vec![
            local("Beta - copie a.mp3"),
            local("Beta - copie b.mp3"),
            local("Alpha.mp3"),
        ];
        let first = match_tracks(&tracks, &files);
        let second = match_tracks(&tracks, &files);
        assert_eq!(first, second);
        assert_eq!(matched(&first, 0).file_name, "Alpha.mp3");
        // Deux candidats de meme score pour Beta : le nom de fichier tranche.
        assert_eq!(matched(&first, 1).file_name, "Beta - copie a.mp3");
        assert!(first[2].file.is_none());
    }

    #[test]
    fn empty_inputs_never_panic() {
        assert!(match_tracks(&[], &[]).is_empty());
        let result = match_tracks(&[wanted("t1", 1, "Titre")], &[]);
        assert_eq!(result.len(), 1);
        assert!(result[0].file.is_none());
        assert!(result[0].score.abs() < 1e-9);

        let empty: TransferManifest =
            parse_manifest(r#"{"playlist_id":"p","name":"Vide"}"#).unwrap();
        assert_eq!(empty.version, 1);
        assert!(empty.tracks.is_empty());
        assert!(match_tracks(&empty.tracks, &[]).is_empty());
    }

    // ---------------------------------------------------------------- manifeste

    #[test]
    fn parse_manifest_reads_the_contract_schema() {
        let json = r#"{
            "version": 1,
            "playlist_id": "run-170",
            "name": "Run 170",
            "source": "spotify",
            "target_bpm": 170.0,
            "tracks": [
                {
                    "id": "t1",
                    "position": 1,
                    "title": "Wake me up",
                    "artist": "Avicii",
                    "album": "True",
                    "duration_s": 249.0,
                    "bpm": 124.0
                }
            ]
        }"#;
        let manifest = parse_manifest(json).expect("manifeste valide");
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.playlist_id, "run-170");
        assert_eq!(manifest.name, "Run 170");
        assert_eq!(manifest.source, "spotify");
        assert_eq!(manifest.target_bpm, Some(170.0));
        assert_eq!(manifest.tracks.len(), 1);
        let track = &manifest.tracks[0];
        assert_eq!(track.position, 1);
        assert_eq!(track.artist.as_deref(), Some("Avicii"));
        assert!(track.file.is_none());
        assert!(track.size_bytes.is_none());
    }

    #[test]
    fn parse_manifest_rejects_unreadable_json() {
        assert!(parse_manifest("").is_none());
        assert!(parse_manifest("pas du json").is_none());
        assert!(parse_manifest("{").is_none());
        // JSON valide mais sans playlist : inutilisable pour un transfert.
        assert!(parse_manifest(r#"{"titre":"sans playlist"}"#).is_none());
    }

    // ------------------------------------------------------------- nom de fichier

    #[test]
    fn suggested_file_name_follows_the_watch_scheme() {
        let mut track = wanted("t1", 1, "Wake me up");
        track.artist = Some("Avicii".to_string());
        assert_eq!(
            suggested_file_name(&track, "mp3"),
            "01 - Avicii - Wake me up.mp3"
        );
        assert_eq!(
            suggested_file_name(&track, ".m4a"),
            "01 - Avicii - Wake me up.m4a"
        );

        let mut without_artist = wanted("t2", 2, "Titre");
        assert_eq!(
            suggested_file_name(&without_artist, "mp3"),
            "02 - Titre.mp3"
        );

        without_artist.position = 0;
        assert_eq!(
            suggested_file_name(&without_artist, "mp3"),
            "01 - Titre.mp3"
        );

        let mut unsafe_title = wanted("t3", 12, "AC/DC: Live");
        assert_eq!(
            suggested_file_name(&unsafe_title, "mp3"),
            "12 - AC DC Live.mp3"
        );

        unsafe_title.title = "   ".to_string();
        assert_eq!(suggested_file_name(&unsafe_title, "mp3"), "12 - Titre.mp3");
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    fn cfg() -> MusicConfig {
        MusicConfig::default()
    }

    fn with_duration(id: &str, duration_s: f64, bpm: Option<f64>) -> Track {
        Track {
            id: id.to_string(),
            title: format!("Titre {id}"),
            artist: None,
            duration_s,
            bpm,
            position: 0,
        }
    }

    fn with_durations(duration_s: f64, count: usize) -> Vec<Track> {
        (0..count)
            .map(|index| with_duration(&format!("t{index}"), duration_s, None))
            .collect()
    }

    // ------------------------------------------------------------ durees

    #[test]
    fn playlist_duration_ignores_invalid_durations() {
        let tracks = vec![
            with_duration("a", 200.0, None),
            with_duration("b", f64::NAN, None),
            with_duration("c", -5.0, None),
            with_duration("d", 0.0, None),
            with_duration("e", f64::INFINITY, None),
            with_duration("f", 100.0, None),
        ];
        assert!((playlist_duration_s(&tracks) - 300.0).abs() < 1e-9);
    }

    #[test]
    fn playlist_duration_of_an_empty_playlist_is_zero() {
        assert_eq!(playlist_duration_s(&[]), 0.0);
    }

    #[test]
    fn race_duration_prefers_the_target_time() {
        assert_eq!(
            race_duration_s(Some(10_000.0), Some(3000.0), Some(300.0)),
            Some(3000.0)
        );
    }

    #[test]
    fn race_duration_falls_back_to_distance_times_pace() {
        assert_eq!(
            race_duration_s(Some(10_000.0), None, Some(300.0)),
            Some(3000.0)
        );
        // Un temps cible nul ou negatif est ignore au profit du couple distance x allure.
        assert_eq!(
            race_duration_s(Some(5000.0), Some(0.0), Some(300.0)),
            Some(1500.0)
        );
        assert_eq!(
            race_duration_s(Some(5000.0), Some(-10.0), Some(300.0)),
            Some(1500.0)
        );
    }

    #[test]
    fn race_duration_is_unknown_without_valid_inputs() {
        assert_eq!(race_duration_s(None, None, None), None);
        assert_eq!(race_duration_s(Some(10_000.0), None, None), None);
        assert_eq!(race_duration_s(None, None, Some(300.0)), None);
        assert_eq!(race_duration_s(Some(10_000.0), None, Some(0.0)), None);
        assert_eq!(
            race_duration_s(Some(f64::NAN), Some(f64::NAN), Some(300.0)),
            None
        );
        assert_eq!(
            race_duration_s(Some(10_000.0), None, Some(f64::INFINITY)),
            None
        );
    }

    // --------------------------------------------------------- couverture

    #[test]
    fn a_sufficient_playlist_reports_a_positive_margin() {
        let tracks = with_durations(120.0, 10); // 1200 s
        let coverage = music_coverage(&tracks, Some(1000.0), None, &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.playlist_duration_s, 1200.0);
        assert_eq!(coverage.race_duration_s, Some(1000.0));
        assert_eq!(coverage.margin_s, Some(200.0));
        assert_eq!(coverage.sufficient, Some(true));
    }

    #[test]
    fn an_insufficient_playlist_reports_a_negative_margin() {
        let tracks = with_durations(200.0, 2); // 400 s
        let coverage = music_coverage(&tracks, Some(1000.0), None, &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.margin_s, Some(-600.0));
        assert_eq!(coverage.sufficient, Some(false));
    }

    #[test]
    fn the_margin_boundary_is_inclusive() {
        // 5 x 210 s = 1050 s, soit exactement 1000 s x 1.05.
        let at_margin = with_durations(210.0, 5);
        let coverage = music_coverage(&at_margin, Some(1000.0), None, &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.sufficient, Some(true));

        // Un cheveu sous la marge : insuffisant.
        let below = vec![with_duration("a", 1049.0, None)];
        let coverage = music_coverage(&below, Some(1000.0), None, &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.sufficient, Some(false));
    }

    #[test]
    fn a_zero_margin_accepts_an_exact_match() {
        let tracks = with_durations(200.0, 5); // 1000 s
        let coverage = music_coverage(&tracks, Some(1000.0), None, &cfg(), 0.0);
        assert_eq!(coverage.sufficient, Some(true));
        assert_eq!(coverage.margin_s, Some(0.0));
    }

    #[test]
    fn tracks_needed_rounds_up() {
        // 2 titres de 210 s : 600 x 1.05 / 210 = 3 exactement (pas 4).
        let tracks = with_durations(210.0, 2);
        let coverage = music_coverage(&tracks, Some(600.0), None, &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.tracks_needed, Some(3));

        // 5 titres de 200 s : 1000 x 1.05 / 200 = 5.25 -> 6.
        let tracks = with_durations(200.0, 5);
        let coverage = music_coverage(&tracks, Some(1000.0), None, &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.tracks_needed, Some(6));
    }

    #[test]
    fn tracks_needed_is_none_without_a_race_or_a_duration() {
        let tracks = with_durations(210.0, 2);
        let coverage = music_coverage(&tracks, None, None, &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.tracks_needed, None);
        assert_eq!(coverage.sufficient, None);
        assert_eq!(coverage.margin_s, None);

        let empty: Vec<Track> = Vec::new();
        let coverage = music_coverage(&empty, Some(600.0), None, &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.playlist_duration_s, 0.0);
        assert_eq!(coverage.sufficient, Some(false));
        assert_eq!(coverage.tracks_needed, None);

        let invalid = vec![
            with_duration("a", f64::NAN, None),
            with_duration("b", 0.0, None),
        ];
        let coverage = music_coverage(&invalid, Some(600.0), None, &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.tracks_needed, None);
    }

    #[test]
    fn an_invalid_race_duration_gives_no_verdict() {
        for race in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            let tracks = with_durations(210.0, 2);
            let coverage = music_coverage(&tracks, Some(race), None, &cfg(), MUSIC_MARGIN_RATIO);
            assert_eq!(coverage.race_duration_s, None, "race = {race}");
            assert_eq!(coverage.sufficient, None);
            assert_eq!(coverage.margin_s, None);
            assert_eq!(coverage.tracks_needed, None);
        }
    }

    // ----------------------------------------------------------- tempo

    #[test]
    fn average_bpm_is_none_without_any_bpm() {
        let tracks = with_durations(200.0, 3);
        let coverage = music_coverage(
            &tracks,
            Some(1000.0),
            Some(300.0),
            &cfg(),
            MUSIC_MARGIN_RATIO,
        );
        assert_eq!(coverage.average_bpm, None);
        assert_eq!(coverage.target_bpm, Some(170.0));
        assert_eq!(coverage.bpm_delta, None);
        assert_eq!(coverage.tempo_ok, None);
    }

    #[test]
    fn average_bpm_and_delta_use_only_valid_values() {
        let tracks = vec![
            with_duration("a", 200.0, Some(168.0)),
            with_duration("b", 200.0, Some(172.0)),
            with_duration("c", 200.0, Some(f64::NAN)),
            with_duration("d", 200.0, Some(0.0)),
            with_duration("e", 200.0, None),
        ];
        let coverage = music_coverage(
            &tracks,
            Some(1000.0),
            Some(300.0),
            &cfg(),
            MUSIC_MARGIN_RATIO,
        );
        assert_eq!(coverage.average_bpm, Some(170.0));
        assert_eq!(coverage.target_bpm, Some(170.0));
        assert_eq!(coverage.bpm_delta, Some(0.0));
        assert_eq!(coverage.tempo_ok, Some(true));
    }

    #[test]
    fn tempo_ok_is_false_when_the_gap_exceeds_the_threshold() {
        let tracks = vec![
            with_duration("a", 200.0, Some(160.0)),
            with_duration("b", 200.0, Some(160.0)),
        ];
        let coverage = music_coverage(
            &tracks,
            Some(1000.0),
            Some(300.0),
            &cfg(),
            MUSIC_MARGIN_RATIO,
        );
        assert_eq!(coverage.average_bpm, Some(160.0));
        assert_eq!(coverage.bpm_delta, Some(-10.0));
        assert_eq!(coverage.tempo_ok, Some(false));
    }

    #[test]
    fn tempo_ok_accepts_the_threshold_exactly() {
        // Cible 170, moyenne 162 : ecart de 8 = switch_threshold_bpm.
        let tracks = vec![with_duration("a", 200.0, Some(162.0))];
        let coverage = music_coverage(
            &tracks,
            Some(1000.0),
            Some(300.0),
            &cfg(),
            MUSIC_MARGIN_RATIO,
        );
        assert_eq!(coverage.bpm_delta, Some(-8.0));
        assert_eq!(coverage.tempo_ok, Some(true));
    }

    #[test]
    fn tempo_ok_is_none_without_a_target_pace() {
        let tracks = vec![with_duration("a", 200.0, Some(170.0))];
        let coverage = music_coverage(&tracks, Some(1000.0), None, &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.average_bpm, Some(170.0));
        assert_eq!(coverage.target_bpm, None);
        assert_eq!(coverage.bpm_delta, None);
        assert_eq!(coverage.tempo_ok, None);
    }

    #[test]
    fn an_empty_playlist_never_panics() {
        let coverage = music_coverage(&[], Some(1000.0), Some(300.0), &cfg(), MUSIC_MARGIN_RATIO);
        assert_eq!(coverage.playlist_duration_s, 0.0);
        assert_eq!(coverage.margin_s, Some(-1000.0));
        assert_eq!(coverage.sufficient, Some(false));
        assert_eq!(coverage.tracks_needed, None);
        assert_eq!(coverage.average_bpm, None);
        assert_eq!(coverage.target_bpm, Some(170.0));
    }
}
