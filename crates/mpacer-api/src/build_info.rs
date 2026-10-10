//! Date de compilation du binaire.
//!
//! Sert a afficher, dans l'interface web, la version ET le moment ou le serveur
//! a ete compile : sans cela, deux deploiements d'une meme version sont
//! indiscernables, et il faut se fier a l'horodatage d'une image conteneur.
//!
//! La date vient de `SOURCE_DATE_EPOCH` quand l'environnement la fournit
//! (variable normalisee de reproductibilite, posee par les chaines de
//! construction) ; sinon de l'horloge de la machine, en **UTC** : le meme
//! instant est lu de la meme facon sur un poste, dans la CI et dans l'image
//! DEBIAN. Le calcul est fait par `build.rs` (voir `crate::date_iso`) et
//! transmis au code par `MPACER_BUILD_DATE`.

/// Version du serveur et du coeur, reprise du `Cargo.toml` de l'espace de travail.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Jour de compilation du binaire, au format `AAAA-MM-JJ` (UTC).
pub const BUILD_DATE: &str = env!("MPACER_BUILD_DATE");

/// Resume affichable dans l'interface : « version du jour de compilation ».
pub fn resume() -> String {
    format!("{VERSION} (compile le {BUILD_DATE})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_date_est_un_jour_iso() {
        // Dix caracteres « AAAA-MM-JJ », separes de tirets : le meme format que
        // les dates de seance, pour que l'affichage reste lisible.
        assert_eq!(BUILD_DATE.len(), 10, "{BUILD_DATE}");
        assert_eq!(&BUILD_DATE[4..5], "-");
        assert_eq!(&BUILD_DATE[7..8], "-");
        assert!(
            BUILD_DATE[..4].chars().all(|c| c.is_ascii_digit()),
            "{BUILD_DATE}"
        );
        assert!(
            BUILD_DATE[5..7].chars().all(|c| c.is_ascii_digit()),
            "{BUILD_DATE}"
        );
        assert!(
            BUILD_DATE[8..].chars().all(|c| c.is_ascii_digit()),
            "{BUILD_DATE}"
        );
    }

    #[test]
    fn le_resume_porte_la_version_et_la_date() {
        let texte = resume();
        assert!(texte.contains(VERSION), "{texte}");
        assert!(texte.contains(BUILD_DATE), "{texte}");
    }
}
