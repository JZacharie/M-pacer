# 05 - Courses a venir : fiches, planning et suivi

Ce document decrit la partie « courses » de l'interface web : ce qu'elle affiche,
comment les donnees sont stockees et quelles decisions ont ete prises.

## 1. Le besoin

Une seance enregistree raconte le passe. Une course a venir demande autre chose :
retrouver vite, le jour J, tout ce qui a ete prepare des semaines a l'avance.

| Element a suivre | Ou il apparait |
|---|---|
| Numero de dossard | carte, fiche, rappel |
| Horaire de depart | carte, planning, fiche |
| Lieu de depart | carte, planning, fiche, lien carte |
| Lien du live (tracking) | carte, fiche, bouton d'acces direct |
| Hotel reserve (nom, adresse, telephone, lien, arrivee, depart) | fiche, planning |
| Rendez-vous de prise de dossard (date, heure, lieu) | planning, fiche |
| Autres solutions pour dormir | fiche |
| Nutrition et ravitaillement | fiche |
| Informations importantes pour la course | fiche |
| Autres informations (transport, accompagnants...) | fiche |
| Elements a preparer, a cocher | carte (progression), fiche, planning |

## 2. Les trois ecrans

### 2.1 Les cartes des courses (`/courses`)

Une carte par course a venir, avec ce qu'on veut savoir sans rien ouvrir :
compte a rebours en jours, date et lieu, pastilles (dossard, distance, discipline,
hotel reserve ou a confirmer, live disponible), et une barre de progression du suivi
avec le prochain element a faire. En dessous, la liste des courses deja courues.

### 2.2 Le planning (`/courses/planning`)

L'agenda des echeances a venir, tous types confondus, groupes par mois :

- **Depart** : la date et l'heure de la course ;
- **Dossard** : le rendez-vous de prise de dossard ;
- **Hotel** : l'arrivee et le depart de l'hebergement ;
- **Suivi** : les elements a preparer qui portent une echeance.

Tout est trie chronologiquement : le planning ne montre que ce qu'il reste a faire,
et chaque ligne renvoie a la fiche de la course concernee.

### 2.3 La fiche de course (`/courses/{id}`)

Toutes les informations de la course, en sections : la course, dossard, suivi en
direct, hebergement, autres solutions pour dormir, nutrition, informations
importantes, autres informations, puis le suivi des elements importants.

Chaque champ non renseigne est affiche comme tel (« non renseigne ») : la fiche
montre d'un coup d'oeil ce qu'il reste a completer, au lieu de masquer les trous.

## 3. Le suivi des elements importants

Chaque course est creee avec huit elements par defaut : inscription confirmee,
certificat medical / PPS, dossard retire, hotel reserve, transport reserve, sac de
course prepare, nutrition prevue, lien du live partage. Chacun peut etre coche,
decoche, retire, et complete par d'autres elements propres a la course, avec une
echeance facultative qui le fait apparaitre dans le planning.

Les elements coches restent visibles, barres : c'est la memoire de ce qui a ete fait.

## 4. La carte

Si la latitude et la longitude sont renseignees, la fiche pointe directement sur le
point exact dans OpenStreetMap ([openstreetmap.org](https://www.openstreetmap.org/)) ;
sinon la recherche se fait sur le nom du lieu de depart ou de la ville. Aucune
bibliotheque cartographique n'est embarquee : c'est un lien, pas une carte interactive,
donc aucun script tiers et aucune fuite de donnees vers un service externe.

## 5. L'API

L'API JSON expose les memes donnees, pour la montre ou un futur client mobile :

| Methode | Chemin | Description |
|---|---|---|
| GET | `/api/v1/races` | liste (option `?upcoming=true`) |
| POST | `/api/v1/races` | creation (201, avec les elements de suivi par defaut) |
| GET | `/api/v1/races/{id}` | fiche complete et son suivi |
| PUT | `/api/v1/races/{id}` | mise a jour de la fiche |
| DELETE | `/api/v1/races/{id}` | suppression (le suivi suit par cascade) |

Authentification identique au reste de l'API (`Authorization: Bearer <jeton>`) ou
session du navigateur. Les erreurs gardent le format
`{"error":"code_stable","message":"explication"}`.

Les horodatages sont des millisecondes UNIX, comme partout ailleurs : le client
decide de l'affichage, le serveur ne stocke jamais de date locale.

## 6. Modele de donnees

```text
races(id, user_id, name, start_at_ms, distance_m, discipline, location, start_location,
      bib_number, bib_pickup_at_ms, bib_pickup_location, live_url, registration_url,
      website_url, latitude, longitude, hotel_name, hotel_address, hotel_phone,
      hotel_url, hotel_booked, hotel_check_in_ms, hotel_check_out_ms, lodging_notes,
      nutrition_notes, important_info, notes, goal_time_s, created_at_ms, updated_at_ms)

race_tasks(id, race_id, user_id, label, due_at_ms, done, done_at_ms, position, created_at_ms)
```

Migration : `crates/mpacer-api/migrations/0002_races.sql`, idempotente et rejouee au
demarrage comme la premiere. `ON DELETE CASCADE` garantit qu'aucun element de suivi
ne survit a sa course, ni a la suppression du compte.

## 7. Decisions

- **Un seul formulaire, une seule validation.** Le formulaire web et l'API
  construisent la meme structure `RaceInput`, normalisee puis validee une seule fois :
  aucun ecart possible entre les deux entrees.
- **Les champs vides sont `NULL`, jamais une chaine vide.** L'affichage distingue
  ainsi « non renseigne » de « renseigne vide ».
- **Les liens sont verifies a l'enregistrement** : seuls `http://` et `https://` sont
  acceptes. Sans ce controle, un lien `javascript:` stocke dans une fiche serait
  rendu dans un `href` et deviendrait un script executable au clic.
- **Aucune dependance front supplementaire.** Les pages restent rendues en Rust
  (maud), le CSS est ecrit a la main, et le JavaScript se limite au strict necessaire.
- **Une echeance sans date ne disparait pas** : un element de suivi sans echeance
  reste sur la fiche, simplement il n'encombre pas le planning.
