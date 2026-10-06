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
