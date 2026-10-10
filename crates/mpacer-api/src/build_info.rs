//! Date et heure de compilation du binaire.
//!
//! Sert a afficher, dans l'interface web, la version ET le moment ou le serveur
//! a ete compile : sans cela, deux deploiements d'une meme version sont
//! indiscernables, et il faut se fier a l'horodatage d'une image conteneur.
//! La precision est la minute : c'est ce qui distingue deux compilations du
//! meme jour, sans alourdir la ligne affichee.
//!
//! L'horodatage vient de `SOURCE_DATE_EPOCH` quand l'environnement le fournit
//! (variable normalisee de reproductibilite, posee par les chaines de
//! construction) ; sinon de l'horloge de la machine, en **UTC** : le meme
//! instant est lu de la meme facon sur un poste, dans la CI et dans l'image
//! DEBIAN. Le calcul est fait par `build.rs` (voir `crate::date_iso`) et
//! transmis au code par `MPACER_BUILD_DATE`.

/// Version du serveur et du coeur, reprise du `Cargo.toml` de l'espace de travail.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Horodatage de compilation du binaire, au format `AAAA-MM-JJ HH:MM` (UTC).
pub const BUILD_DATE: &str = env!("MPACER_BUILD_DATE");

/// Resume affichable dans l'interface : « version du moment de compilation ».
pub fn resume() -> String {
    format!("{VERSION} (compile le {BUILD_DATE})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_date_est_un_horodatage_iso() {
        // « AAAA-MM-JJ HH:MM » : le format lisible de partout (interface web,
        // reglages des applications), precise a la minute.
        assert_eq!(BUILD_DATE.len(), 16, "{BUILD_DATE}");
        assert_eq!(&BUILD_DATE[4..5], "-");
        assert_eq!(&BUILD_DATE[7..8], "-");
        assert_eq!(&BUILD_DATE[10..11], " ");
        assert_eq!(&BUILD_DATE[13..14], ":");
        let chiffres =
            |plage: std::ops::Range<usize>| BUILD_DATE[plage].chars().all(|c| c.is_ascii_digit());
        assert!(chiffres(0..4), "{BUILD_DATE}");
        assert!(chiffres(5..7), "{BUILD_DATE}");
        assert!(chiffres(8..10), "{BUILD_DATE}");
        assert!(chiffres(11..13), "{BUILD_DATE}");
        assert!(chiffres(14..16), "{BUILD_DATE}");
    }

    #[test]
    fn lheure_est_plausible() {
        // Le fuseau est UTC : l'heure reste dans 00..=23 et la minute dans 00..=59.
        let heures: u32 = BUILD_DATE[11..13].parse().expect("heures");
        let minutes: u32 = BUILD_DATE[14..16].parse().expect("minutes");
        assert!(heures < 24, "{BUILD_DATE}");
        assert!(minutes < 60, "{BUILD_DATE}");
    }

    #[test]
    fn le_resume_porte_la_version_et_la_date() {
        let texte = resume();
        assert!(texte.contains(VERSION), "{texte}");
        assert!(texte.contains(BUILD_DATE), "{texte}");
    }
}
