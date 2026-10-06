<span class="eyebrow">Documentation</span>
# M-pacer, en deux minutes

<p class="lead">
Une application de <strong>contrôle d'allure</strong> pour montre Android (Wear OS),
et un <strong>site web auto-hébergé</strong> pour relire, analyser et préparer vos courses.
Tout le calcul tient dans un cœur Rust unique, testé sans montre et sans serveur.
</p>

<div class="mockups">
  <figure class="mockup">
    <svg class="watch" viewBox="0 0 200 200" role="img" aria-label="Écran de montre : allure, distance, temps et voyant GPS">
      <circle cx="100" cy="100" r="93" fill="#0b0d10" stroke="rgba(255,255,255,.14)" stroke-width="2"/>
      <circle cx="100" cy="24" r="6" fill="#2fbf71"/>
      <text x="100" y="96" text-anchor="middle" font-family="system-ui, sans-serif" font-size="46" font-weight="700" fill="#f3f5f8">5:12</text>
      <text x="100" y="122" text-anchor="middle" font-family="system-ui, sans-serif" font-size="13" fill="#98a2b3">8,42 km &#183; 42:10</text>
      <text x="100" y="150" text-anchor="middle" font-family="system-ui, sans-serif" font-size="14" font-weight="600" fill="#2fbf71">sur le plan</text>
      <g font-family="system-ui, sans-serif" font-size="11" font-weight="600">
        <rect x="46" y="164" width="48" height="22" rx="11" fill="#fc4c02"/>
        <text x="70" y="179" text-anchor="middle" fill="#ffffff">Pause</text>
        <rect x="104" y="164" width="46" height="22" rx="11" fill="#242933"/>
        <text x="127" y="179" text-anchor="middle" fill="#f3f5f8">Stop</text>
      </g>
      <rect x="188" y="86" width="8" height="28" rx="4" fill="#2a2f38"/>
    </svg>
    <figcaption>La montre pendant l'effort</figcaption>
  </figure>
  <figure class="mockup">
    <svg class="phone" viewBox="0 0 180 320" role="img" aria-label="Page web : liste des séances et barre d'onglets">
      <rect x="4" y="4" width="172" height="312" rx="22" fill="#0b0d10" stroke="rgba(255,255,255,.14)" stroke-width="2"/>
      <text x="20" y="42" font-family="system-ui, sans-serif" font-size="15" font-weight="700" fill="#f3f5f8">Vos séances</text>
      <g font-family="system-ui, sans-serif">
        <rect x="14" y="58" width="152" height="46" rx="12" fill="#16191f" stroke="rgba(255,255,255,.08)"/>
        <text x="26" y="78" font-size="11" fill="#f3f5f8">mardi 6 octobre</text>
        <text x="26" y="94" font-size="10" fill="#98a2b3">12,04 km &#183; 5:02 /km</text>
        <text x="152" y="88" text-anchor="end" font-size="12" font-weight="700" fill="#fc4c02">1h00</text>
        <rect x="14" y="112" width="152" height="46" rx="12" fill="#16191f" stroke="rgba(255,255,255,.08)"/>
        <text x="26" y="132" font-size="11" fill="#f3f5f8">dimanche 4 octobre</text>
        <text x="26" y="148" font-size="10" fill="#98a2b3">21,10 km &#183; 5:28 /km</text>
        <text x="152" y="142" text-anchor="end" font-size="12" font-weight="700" fill="#fc4c02">1h55</text>
        <rect x="14" y="166" width="152" height="46" rx="12" fill="#16191f" stroke="rgba(255,255,255,.08)"/>
        <text x="26" y="186" font-size="11" fill="#f3f5f8">jeudi 1er octobre</text>
        <text x="26" y="202" font-size="10" fill="#98a2b3">8,00 km &#183; 4:58 /km</text>
        <text x="152" y="196" text-anchor="end" font-size="12" font-weight="700" fill="#fc4c02">39:44</text>
        <rect x="14" y="228" width="152" height="52" rx="12" fill="#16191f" stroke="rgba(252,76,2,.38)"/>
        <text x="26" y="248" font-size="10" fill="#98a2b3">PROCHAIN DÉPART</text>
        <text x="26" y="266" font-size="11" fill="#f3f5f8">Marathon de La Rochelle &#183; J-48</text>
        <g fill="#6b7480" font-size="9">
          <circle cx="26" cy="302" r="4"/><circle cx="56" cy="302" r="4" fill="#fc4c02"/>
          <circle cx="86" cy="302" r="4"/><circle cx="116" cy="302" r="4"/><circle cx="146" cy="302" r="4"/>
        </g>
      </g>
    </svg>
    <figcaption>Le site web sur téléphone</figcaption>
  </figure>
</div>

## Le principe

<ol class="steps">
  <li><strong>Vous courez, la montre mesure.</strong> Le GPS est lu à 1 Hz et poussé dans le
  moteur Rust, qui lisse l'allure sur 2 minutes, découpe les tours et détecte les pauses.
  Aucun calcul de course n'est écrit en Kotlin : la montre ne fait que piloter la plateforme.</li>
  <li><strong>La montre guide et conserve.</strong> Elle affiche l'allure courante, l'écart au
  plan (shadow runner), le feu de statut GPS, et annonce les informations à la voix. Chaque
  séance est archivée localement au format <code>.pac</code> : courir ne dépend jamais du réseau.</li>
  <li><strong>Le site web restitue.</strong> À la fin de la séance, la montre l'envoie au backend
  auto-hébergé, qui l'analyse (plan contre réalisé, zones cardiaques, pauses, accélération) et
  alimente le tableau de bord, les statistiques et les fiches de course.</li>
</ol>

## Les deux produits

<div class="grid two">
  <a class="card" href="{{ '/montre/' | relative_url }}">
    <span class="icon" aria-hidden="true"><svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="#fc4c02" stroke-width="1.9" stroke-linecap="round"><circle cx="12" cy="12" r="6"/><path d="M9 2h6M9 22h6"/></svg></span>
    <h3>L'application montre</h3>
    <p>Wear OS : allure lissée, assistant à quatre modes, shadow runner, voix, tours,
    historique local et synchronisation. Un module téléphone compagnon l'accompagne.</p>
  </a>
  <a class="card" href="{{ '/site-web/' | relative_url }}">
    <span class="icon" aria-hidden="true"><svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="#fc4c02" stroke-width="1.9" stroke-linecap="round"><rect x="2.5" y="4" width="19" height="14" rx="2.5"/><path d="M8 21h8M12 18v3"/></svg></span>
    <h3>Le site web</h3>
    <p>Tableau de bord, analyse complète d'une séance, statistiques hebdomadaires,
    fiches de course et planning des échéances, appairage des montres, export GPX.</p>
  </a>
</div>

## Ce que M-pacer n'est pas

<div class="note">
<p><strong>Non affilié à Pace Control</strong> (PBkSoft). Le produit d'origine a servi de
référence fonctionnelle et de source d'inspiration ; aucune ligne de code ni ressource ne
provient de cette application. Les fonctionnalités sont réimplémentées d'après la
documentation publique.</p>
</div>

- **Pas de compte tiers obligatoire** : l'accès au site se fait avec Google (OAuth 2.0 + PKCE)
  ou, en développement, par une connexion locale. Aucun mot de passe n'est stocké par M-pacer.
- **Pas de publicité ni de revente de données** : la base PostgreSQL et le service tournent
  chez vous (cluster k3s personnel, chart Helm fourni).
- **Pas une application de réseau social** : pas de fil d'actualité, pas de segments partagés.

## État du projet

<div class="kpis">
  <div class="kpi"><strong>123</strong><span>tests Rust verts</span></div>
  <div class="kpi"><strong>4</strong><span>modes d'assistant</span></div>
  <div class="kpi"><strong>2</strong><span>modules Android</span></div>
  <div class="kpi"><strong>37</strong><span>fonctionnalités analysées</span></div>
</div>

<div class="table-wrap">

| Brique | État |
|---|---|
| Cœur Rust (<code>mpacer-core</code>) | <span class="tag ok">Fait</span> 93 tests |
| Backend + site web (<code>mpacer-api</code>) | <span class="tag ok">Fait</span> tests d'intégration PostgreSQL |
| Chart Helm / déploiement k3s | <span class="tag ok">Validé</span> sur le cluster jo3 |
| Applications Android (montre + téléphone) | <span class="tag ok">Compilent</span> validation terrain à faire |
| Capteur cardio, boutons du casque, mode ambiant | <span class="tag todo">À faire</span> |

</div>

## Où aller ensuite

<ul class="toc">
  <li><a href="{{ '/montre/' | relative_url }}">L'application montre</a> : écrans, modes d'assistant, voix, synchronisation.</li>
  <li><a href="{{ '/site-web/' | relative_url }}">Le site web</a> : pages, analyse de séance, courses, API.</li>
  <li><a href="{{ '/architecture/' | relative_url }}">Architecture</a> : cœur Rust partagé, pont JNI, idempotence, sécurité.</li>
  <li><a href="{{ '/galerie/' | relative_url }}">Galerie</a> : architecture, écrans du site et propositions de logo, agrandissables.</li>
  <li><a href="{{ '/composeur/' | relative_url }}">Composeur</a> : essayez le constructeur de tableaux de bord, en interactif.</li>
  <li><a href="{{ '/demarrage/' | relative_url }}">Démarrage</a> : essayer le cœur, lancer le backend, déployer.</li>
  <li><a href="https://github.com/JZacharie/M-pacer">Le dépôt</a> : code, README, documentation technique détaillée.</li>
</ul>
