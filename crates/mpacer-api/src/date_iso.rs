//! Horodatage d'un instant UNIX, en UTC.
//!
//! Ce module ne contient que du calcul de calendrier, sans entree/sortie ni
//! acces a l'environnement : il sert a la fois au script de compilation
//! (`build.rs`, qui importe ce fichier par `#[path]`) et au code, qui garde
//! ainsi la date sous les yeux de la suite de tests ordinaire.
//!
//! Deux precisions sont disponibles : le jour seul (`AAAA-MM-JJ`) et
//! l'horodatage a la minute (`AAAA-MM-JJ HH:MM`). Les applications affichent
//! l'horodatage : deux compilations du meme jour restent ainsi discernables.

const SECONDES_PAR_JOUR: i64 = 86_400;
const SECONDES_PAR_HEURE: i64 = 3_600;

/// Annee bissextile du calendrier gregorien.
fn bissextile(annee: i64) -> bool {
    annee % 4 == 0 && (annee % 100 != 0 || annee % 400 == 0)
}

/// Nombre de jours du mois `mois` (1 = janvier).
fn jours_du_mois(annee: i64, mois: i64) -> i64 {
    match mois {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if bissextile(annee) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Jour `AAAA-MM-JJ` correspondant a un instant UNIX (UTC).
pub fn jour_iso(epoch: i64) -> String {
    let mut jours = epoch.div_euclid(SECONDES_PAR_JOUR);

    let mut annee = 1970;
    loop {
        let longueur = if bissextile(annee) { 366 } else { 365 };
        if jours < longueur {
            break;
        }
        jours -= longueur;
        annee += 1;
    }

    let mut mois = 1;
    loop {
        let longueur = jours_du_mois(annee, mois);
        if jours < longueur {
            break;
        }
        jours -= longueur;
        mois += 1;
    }

    format!("{annee:04}-{mois:02}-{:02}", jours + 1)
}

/// Heure et minute `(HH, MM)` d'un instant UNIX (UTC).
fn heure_minute(epoch: i64) -> (i64, i64) {
    let secondes_du_jour = epoch.rem_euclid(SECONDES_PAR_JOUR);
    (
        secondes_du_jour / SECONDES_PAR_HEURE,
        (secondes_du_jour % SECONDES_PAR_HEURE) / 60,
    )
}

/// Horodatage `AAAA-MM-JJ HH:MM` (UTC), precis a la minute.
pub fn horodatage_iso(epoch: i64) -> String {
    let (heures, minutes) = heure_minute(epoch);
    format!("{} {heures:02}:{minutes:02}", jour_iso(epoch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lepoque_donne_le_premier_jour() {
        assert_eq!(jour_iso(0), "1970-01-01");
        assert_eq!(jour_iso(SECONDES_PAR_JOUR - 1), "1970-01-01");
        assert_eq!(jour_iso(SECONDES_PAR_JOUR), "1970-01-02");
    }

    #[test]
    fn les_annees_bissextiles_sont_respectees() {
        // 2024-02-29 : veille et lendemain encadrent le 29.
        let vingt_neuf = 1_709_164_800; // 2024-02-29T00:00:00Z
        assert_eq!(jour_iso(vingt_neuf), "2024-02-29");
        assert_eq!(jour_iso(vingt_neuf - SECONDES_PAR_JOUR), "2024-02-28");
        assert_eq!(jour_iso(vingt_neuf + SECONDES_PAR_JOUR), "2024-03-01");
    }

    #[test]
    fn les_bornes_dannee_sont_justes() {
        assert_eq!(jour_iso(1_735_603_200), "2024-12-31");
        assert_eq!(jour_iso(1_735_689_600), "2025-01-01");
        assert_eq!(jour_iso(1_791_590_400), "2026-10-10");
    }

    #[test]
    fn lheure_est_precise_a_la_minute() {
        // 2026-10-10 00:00:00 UTC, puis 06:12 et 23:59 le meme jour.
        let minuit = 1_791_590_400;
        assert_eq!(horodatage_iso(minuit), "2026-10-10 00:00");
        assert_eq!(
            horodatage_iso(minuit + 6 * 3_600 + 12 * 60),
            "2026-10-10 06:12"
        );
        assert_eq!(
            horodatage_iso(minuit + 23 * 3_600 + 59 * 60),
            "2026-10-10 23:59"
        );
    }

    #[test]
    fn lheure_avance_avec_les_secondes() {
        // 2024-02-29T01:01:01Z : la minute courante, pas la suivante.
        let instant = 1_709_164_800 + 3_661; // 01:01:01
        assert_eq!(horodatage_iso(instant), "2024-02-29 01:01");
        assert_eq!(horodatage_iso(instant + 58), "2024-02-29 01:01");
        assert_eq!(horodatage_iso(instant + 59), "2024-02-29 01:02");
        assert_eq!(horodatage_iso(instant + 60), "2024-02-29 01:02");
    }

    #[test]
    fn la_minute_precedente_reste_la_veille() {
        // Une seconde avant minuit : la date ne doit pas basculer avant l'heure.
        let minuit = 1_791_590_400;
        assert_eq!(horodatage_iso(minuit - 1), "2026-10-09 23:59");
    }
}
