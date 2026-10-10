//! Conversion d'un instant UNIX en jour `AAAA-MM-JJ` (UTC).
//!
//! Ce module ne contient que du calcul de calendrier, sans entree/sortie ni
//! acces a l'environnement : il sert a la fois au script de compilation
//! (`build.rs`, qui importe ce fichier par `#[path]`) et au code, qui garde
//! ainsi la date sous les yeux de la suite de tests ordinaire.

const SECONDES_PAR_JOUR: i64 = 86_400;

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
}
