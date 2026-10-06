# Simulations de courses

Traces GPS **synthetiques** produites par `mpacer-sim`, pour tester l'application
(analyse d'allure, tours, meilleures distances, shadow runner, annonces vocales, export
GPX) sans sortir courir. Elles passent par le meme moteur que la montre : ce que montre le
tableau de bord apres import est exactement ce que le coeur calcule.

| Scenario | Distance | Temps realise | Fichier |
|---|---|---|---|
| Equivalent marathon de Bordeaux | 42,195 km | **3:00:07** | `marathon-bordeaux-3h00.gpx` |
| Equivalent semi-marathon | 21,097 km | **1:30:02** | `semi-bordeaux-1h30.gpx` |
| 10 km | 10,000 km | **45:01** | `10k-bordeaux-45min.gpx` |

Les temps realises sont ceux annonces par le moteur (`Realise : ...`) : l'ecart de
quelques secondes a la cible vient du bruit GPS et des deux pauses inserees dans la seance.

## Le parcours

L'option `--course bordeaux` fait suivre une boucle de **7,97 km** inspiree des quais de
Bordeaux : place des Quinconces, quais des Chartrons, pont Chaban-Delmas, parc aux
Angeliques, quais de la rive droite, pont de pierre, place de la Bourse, retour aux
Quinconces. Un marathon correspond donc a un peu plus de cinq tours.

> **C'est une trace synthetique.** Ce n'est pas le parcours officiel du Marathon de
> Bordeaux : la geographie est respectee (points reels sur les quais), pas le trace exact
> de l'epreuve.

Le bruit GPS par defaut (`--noise 3`, soit 3 m) est realiste. Consequence a connaitre :
si l'on additionne betement les points du GPX on trouve ~51 km au lieu de 42,195 km — c'est
precisement ce que le moteur doit filtrer. La meme trace generee avec `--noise 0` mesure
**42 178 m** (0,04 % d'ecart), ce qui valide la geometrie du parcours.

## Regenerer

```bash
cargo run -p mpacer-sim -- --mode plan --course bordeaux \
  --distance 42195 --time 10800 --minutes 220 \
  --split 0.03 --noise 3 --seed 20261006 --pauses 2 \
  --gpx simulations/marathon-bordeaux-3h00.gpx
```

Options utiles : `--split R` (negative split, 0 pour une allure reguliere),
`--pauses N` (0 pour une seance sans arret), `--seed N` (bruit reproductible),
`--course line` (revenir a une trace en ligne droite), `--every S` (frequence
d'affichage), `--units imperial`.

## Importer une simulation dans le backend

```bash
cargo run -p mpacer-sim -- --mode plan --course bordeaux \
  --distance 42195 --time 10800 --minutes 220 --noise 3 \
  --api-url https://mpacer.p.zacharie.org --api-token <jeton d'appareil>
```

La seance apparait ensuite dans le tableau de bord (tours, meilleures distances, trace,
export GPX). Sans `--api-token`, le simulateur propose l'appairage par code : il affiche un
code, on l'accepte sur `https://mpacer.p.zacharie.org/link`, et le jeton renvoye est
reutilisable pour les envois suivants.

## Autres modes

`--mode predict` (prediction de temps), `--mode track` (suivi d'allure),
`--mode remote` (course virtuelle contre un adversaire). Voir `--help`.
