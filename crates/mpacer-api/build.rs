//! Date de compilation de `mpacer-api`.
//!
//! `SOURCE_DATE_EPOCH` (secondes UNIX) fixe la date quand la chaine de
//! construction la fournit : c'est la variable normalisee de reproductibilite,
//! et elle donne la date de la **release** plutot que celle du poste qui
//! rejoue le build. A defaut, l'horloge de la machine est lue en **UTC** : le
//! meme instant se lit de la meme facon sur un poste, dans la CI et dans
//! l'image DEBIAN.
//!
//! La conversion du calendrier vit dans `src/date_iso.rs`, importe ici : la
//! suite de tests ordinaire le couvre, contrairement a un script de build.

use std::time::{SystemTime, UNIX_EPOCH};

#[path = "src/date_iso.rs"]
mod date_iso;

fn main() {
    let epoch = if let Ok(valeur) = std::env::var("SOURCE_DATE_EPOCH") {
        valeur
            .trim()
            .parse::<i64>()
            .ok()
            .unwrap_or_else(instant_actuel)
    } else {
        instant_actuel()
    };

    println!(
        "cargo:rustc-env=MPACER_BUILD_DATE={}",
        date_iso::horodatage_iso(epoch)
    );
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
}

fn instant_actuel() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duree| duree.as_secs() as i64)
        .unwrap_or(0)
}
