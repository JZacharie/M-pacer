//! Ressources statiques embarquees dans le binaire (aucun serveur de fichiers).

pub const APP_CSS: &str = include_str!("../static/app.css");
pub const APP_JS: &str = include_str!("../static/app.js");
/// Carte OpenStreetMap maison (tuiles, marqueurs, traces) : sert la page
/// Amis du navigateur et la vue Carte de l'application Android, qui charge ce
/// fichier dans une WebView.
pub const MAP_JS: &str = include_str!("../static/map.js");

/// Logo complet (marque + mot-cle), affiche dans l'en-tete.
pub const LOGO_SVG: &str = include_str!("../static/logo.svg");
/// Marque seule (carre arrondi orange), utilisee en favicon.
pub const LOGO_MARK_SVG: &str = include_str!("../static/logo-mark.svg");
/// Schema du bloc 4 : playlist, cable USB, montre.
pub const ILLUSTRATION_USB_SVG: &str = include_str!("../static/illustration-usb.svg");

/// Image marathon & pacer pour la page de presentation.
pub const MARATHON_PACER_JPG: &[u8] = include_bytes!("../static/marathon-pacer.jpg");
/// Image trail running pour la page de presentation.
pub const TRAIL_RUNNING_JPG: &[u8] = include_bytes!("../static/trail-running.jpg");

