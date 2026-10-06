//! BPM des titres : balises audio, tap-tempo et saisie manuelle.
//!
//! Aucune analyse audio lourde n'est faite cote serveur (voir
//! docs/07-musique-bpm-et-playlists.md section 9 : l'analyse par autocorrelation
//! reste une suite possible). Le decodage des balises vit dans le coeur Rust
//! (`mpacer_core::music`), partage avec la montre : un meme fichier donne donc
//! exactement le meme tempo des deux cotes du reseau.

pub use mpacer_core::music::{bpm_from_tags, tap_tempo};

use crate::models::{BPM_MAX, BPM_MIN};

/// BPM lu dans les balises d'un fichier audio, borne a un intervalle plausible.
///
/// Une balise absente ou aberrante renvoie `None` : le titre reste « neutre » et
/// l'interface propose le tap-tempo ou la saisie manuelle.
pub fn bpm_from_bytes(bytes: &[u8], filename: &str) -> Option<f64> {
    bpm_from_tags(bytes, filename).filter(|bpm| plausible(*bpm))
}

/// BPM estime depuis des instants de tap (millisecondes).
pub fn bpm_from_taps(taps_ms: &[i64]) -> Option<f64> {
    tap_tempo(taps_ms).filter(|bpm| plausible(*bpm))
}

/// Lit une suite d'instants de tap saisie dans un formulaire
/// (`"0,510,1020,1530"`) ; les valeurs illisibles sont ignorees.
pub fn parse_taps(text: &str) -> Vec<i64> {
    text.split([',', ';', ' ', '\n', '\t'])
        .filter_map(|part| part.trim().parse::<i64>().ok())
        .collect()
}

/// Un BPM n'est retenu que s'il est fini et dans les bornes du coeur.
fn plausible(bpm: f64) -> bool {
    bpm.is_finite() && (BPM_MIN..=BPM_MAX).contains(&bpm)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Balise ID3v2.3 minimale portant un cadre `TBPM` (le format produit par la
    /// plupart des encodeurs MP3).
    ///
    /// Le corps d'une trame texte commence par l'octet d'encodage (0x00 =
    /// ISO-8859-1), puis le texte termine par un NUL.
    fn id3v2_tbpm(bpm: &str) -> Vec<u8> {
        let mut frame = Vec::new();
        frame.extend_from_slice(b"TBPM");
        frame.extend_from_slice(&(bpm.len() as u32 + 2).to_be_bytes());
        frame.extend_from_slice(&[0, 0]); // drapeaux
        frame.push(0); // encodage ISO-8859-1
        frame.extend_from_slice(bpm.as_bytes());
        frame.push(0);

        let mut tag = Vec::new();
        tag.extend_from_slice(b"ID3");
        tag.extend_from_slice(&[3, 0, 0]); // version 2.3, sans indicateur
        let size = frame.len() as u32;
        tag.push(((size >> 21) & 0x7f) as u8);
        tag.push(((size >> 14) & 0x7f) as u8);
        tag.push(((size >> 7) & 0x7f) as u8);
        tag.push((size & 0x7f) as u8);
        tag.extend_from_slice(&frame);
        tag
    }

    #[test]
    fn bpm_comes_from_the_id3_tag() {
        let bytes = id3v2_tbpm("172");
        assert_eq!(bpm_from_bytes(&bytes, "titre.mp3"), Some(172.0));
    }

    #[test]
    fn a_file_without_tag_has_no_bpm() {
        assert_eq!(bpm_from_bytes(b"pas de balise ici", "titre.mp3"), None);
        assert_eq!(bpm_from_bytes(b"", ""), None);
    }

    #[test]
    fn tap_tempo_needs_at_least_four_taps() {
        assert_eq!(bpm_from_taps(&[0, 500, 1000]), None);
        let bpm = bpm_from_taps(&[0, 500, 1000, 1500, 2000]).expect("tempo mesure");
        assert!((bpm - 120.0).abs() < 0.5, "bpm = {bpm}");
    }

    #[test]
    fn taps_are_read_from_a_form_field() {
        assert_eq!(parse_taps("0,501,1002,1503"), vec![0, 501, 1002, 1503]);
        assert_eq!(parse_taps(" 0 ; 500 ; 1000 "), vec![0, 500, 1000]);
        assert_eq!(parse_taps("inconnu"), Vec::<i64>::new());
    }
}
