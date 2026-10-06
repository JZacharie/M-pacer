# Documentation M-pacer

Index des documents. Le point d'entrée reste le [README principal](../README.md)
(présentation, plan d'action complet et carte du dépôt).

| Document | À lire si vous voulez… |
|---|---|
| [01 - Analyse des fonctionnalités](01-analyse-features.md) | comprendre ce qui a été repris de Pace Control, pourquoi, et avec quelles priorités (37 fonctionnalités inventoriées, formules du negative split et des meilleures distances) |
| [02 - Architecture Rust / Wear OS](02-architecture-rust-wearos.md) | savoir pourquoi le calcul est en Rust et l'interface en Kotlin, comment fonctionne l'algorithme d'allure sur 2 minutes, quelles permissions sont nécessaires et ce que consomme la batterie |
| [03 - Plan d'action (application montre)](03-plan-action.md) | planifier le travail côté Wear OS : phases, tâches, critères d'acceptation, risques, stratégie de test |
| [04 - Backend, interface web et déploiement](04-backend-web-et-deploiement.md) | comprendre l'API, l'authentification Google, le modèle de données PostgreSQL et l'exploitation du service |
| [05 - Courses à venir : fiches, planning et suivi](05-courses-et-planning.md) | préparer une course : dossard, horaires, lieux, live, hébergement, nutrition, informations importantes et éléments à cocher |
| [06 - Analyse d'une séance](06-analyse-seance.md) | lire un écran d'analyse : veille concurrente (Strava, Garmin, Polar, Coros…), plan contre réalisé, zones de fréquence cardiaque, découplage et temps d'accélération |
| [07 - Musique, BPM et playlists de course](07-musique-bpm-et-playlists.md) | ajouter de la musique à la montre : playlists Spotify ou fichiers personnels, tempo cible, préparation avant une course, contrat d'interface du coeur, de l'API et des écrans |
| [Guide de déploiement](../deploy/README.md) | déployer concrètement sur le cluster k3s **jo3** : image, secrets, Helm, ingress, TLS, sauvegardes, dépannage |

## Parcours conseillés

**Je veux juste essayer** → [README § 5](../README.md#5-démarrage-rapide) : `cargo test` puis `mpacer-sim`.

**Je veux comprendre le produit** → 01 (fonctionnalités) puis 02 (architecture).

**Je veux déployer** → [deploy/README.md](../deploy/README.md).

**Je veux reprendre le développement montre** → 03 puis [android/README.md](../android/README.md).

**Je veux toucher au backend** → 04, puis le code de `crates/mpacer-api/` (chaque module est documenté en tête de fichier).
