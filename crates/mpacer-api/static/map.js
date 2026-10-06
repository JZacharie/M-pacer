// M-pacer : carte OpenStreetMap minimale, sans dependance.
//
// Pourquoi une carte maison plutot que Leaflet : le service se veut sans
// dependance externe (aucun CDN, aucun script tiers), et le besoin tient en
// trois choses : afficher des tuiles OpenStreetMap, poser des marqueurs, tracer
// une polyligne. Le tout fait quelques centaines de lignes lisibles, et sert
// aussi bien la page /amis du navigateur que la vue Carte de l'application
// Android (qui charge ce meme fichier dans une WebView).
//
// Les tuiles viennent de tile.openstreetmap.org, avec l'attribution exigee par
// la politique d'usage : « (c) OpenStreetMap contributeurs », cliquable.
(function () {
  "use strict";

  var TUILE = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
  var TAILLE = 256;
  var ATTRIBUTION = "&copy; <a href=\"https://www.openstreetmap.org/copyright\">OpenStreetMap</a> contributeurs";
  var ZOOM_MIN = 3;
  var ZOOM_MAX = 18;

  // ------------------------------------------------------------- projection

  function projeter(lat, lon, zoom) {
    var n = Math.pow(2, zoom);
    var x = ((lon + 180) / 360) * n;
    var rad = (lat * Math.PI) / 180;
    var y = ((1 - Math.log(Math.tan(rad) + 1 / Math.cos(rad)) / Math.PI) / 2) * n;
    return { x: x * TAILLE, y: y * TAILLE };
  }

  function deprojeter(x, y, zoom) {
    var n = Math.pow(2, zoom);
    var lon = (x / (n * TAILLE)) * 360 - 180;
    var m = 1 - (2 * y) / (n * TAILLE);
    var lat = (180 / Math.PI) * Math.atan(0.5 * (Math.exp(m * Math.PI) - Math.exp(-m * Math.PI)));
    return { lat: lat, lon: lon };
  }

  function age(secondes) {
    if (secondes === null || secondes === undefined) return "";
    if (secondes < 60) return "il y a " + secondes + " s";
    if (secondes < 3600) return "il y a " + Math.round(secondes / 60) + " min";
    return "il y a " + Math.round(secondes / 3600) + " h";
  }

  function couleurEtat(etat) {
    if (etat === "pause") return "#f7b955";
    if (etat === "arm") return "#98a2b3";
    return "#fc4c02";
  }

  // ------------------------------------------------------------------ carte

  function Carte(conteneur, options) {
    this.conteneur = conteneur;
    this.options = options || {};
    this.zoom = this.options.zoom || 13;
    this.centre = null;
    this.points = [];
    // Trace fixe (fiche de seance) et reperes de distance, distincts des
    // positions vivantes du cercle d'amis.
    this.trace = this.options.trace || [];
    this.reperes = this.options.reperes || [];
    this.tuiles = {};
    this.suivi = true;
    this.source = conteneur.getAttribute("data-source") || "";
    this.construire();
  }

  Carte.prototype.construire = function () {
    var conteneur = this.conteneur;
    conteneur.classList.add("carte");
    conteneur.innerHTML = "";

    this.fond = document.createElement("div");
    this.fond.className = "carte-fond";
    conteneur.appendChild(this.fond);

    this.calque = document.createElement("div");
    this.calque.className = "carte-calque";
    conteneur.appendChild(this.calque);

    var attribution = document.createElement("div");
    attribution.className = "carte-attribution";
    attribution.innerHTML = ATTRIBUTION;
    conteneur.appendChild(attribution);

    var commandes = document.createElement("div");
    commandes.className = "carte-commandes";
    commandes.appendChild(this.bouton("+", "Zoom avant", this.zoomer.bind(this, 1)));
    commandes.appendChild(this.bouton("-", "Zoom arriere", this.zoomer.bind(this, -1)));
    commandes.appendChild(this.bouton("o", "Recentrer sur le cercle", this.recentrer.bind(this)));
    conteneur.appendChild(commandes);

    var message = document.createElement("div");
    message.className = "carte-message";
    this.message = message;
    conteneur.appendChild(message);

    this.installerGestes();
    this.redessiner();
  };

  Carte.prototype.bouton = function (texte, titre, action) {
    var bouton = document.createElement("button");
    bouton.type = "button";
    bouton.textContent = texte;
    bouton.title = titre;
    bouton.setAttribute("aria-label", titre);
    bouton.addEventListener("click", function (evenement) {
      evenement.preventDefault();
      action();
    });
    return bouton;
  };

  // ------------------------------------------------------------ interactions

  Carte.prototype.installerGestes = function () {
    var carte = this;
    var derniere = null;

    this.conteneur.addEventListener("pointerdown", function (evenement) {
      if (evenement.target.closest(".carte-commandes, .carte-attribution")) return;
      derniere = { x: evenement.clientX, y: evenement.clientY };
      carte.suivi = false;
      carte.conteneur.setPointerCapture(evenement.pointerId);
      carte.conteneur.classList.add("deplacement");
    });

    this.conteneur.addEventListener("pointermove", function (evenement) {
      if (!derniere) return;
      var dx = evenement.clientX - derniere.x;
      var dy = evenement.clientY - derniere.y;
      derniere = { x: evenement.clientX, y: evenement.clientY };
      var centre = projeter(carte.centre.lat, carte.centre.lon, carte.zoom);
      var nouveau = deprojeter(centre.x - dx, centre.y - dy, carte.zoom);
      carte.centre = nouveau;
      carte.redessiner();
    });

    var relacher = function () {
      derniere = null;
      carte.conteneur.classList.remove("deplacement");
    };
    this.conteneur.addEventListener("pointerup", relacher);
    this.conteneur.addEventListener("pointercancel", relacher);

    this.conteneur.addEventListener(
      "wheel",
      function (evenement) {
        evenement.preventDefault();
        carte.zoomer(evenement.deltaY < 0 ? 1 : -1);
      },
      { passive: false }
    );

    // Deux doigts : pincement pour zoomer (tablette, telephone).
    var pincement = null;
    this.conteneur.addEventListener("touchstart", function (evenement) {
      if (evenement.touches.length === 2) {
        pincement = distance(evenement.touches);
      }
    });
    this.conteneur.addEventListener("touchmove", function (evenement) {
      if (evenement.touches.length !== 2 || pincement === null) return;
      evenement.preventDefault();
      var maintenant = distance(evenement.touches);
      if (Math.abs(maintenant - pincement) > 40) {
        carte.zoomer(maintenant > pincement ? 1 : -1);
        pincement = maintenant;
      }
    });
    this.conteneur.addEventListener("touchend", function () {
      pincement = null;
    });

    window.addEventListener("resize", function () {
      carte.redessiner();
    });
  };

  function distance(touches) {
    var dx = touches[0].clientX - touches[1].clientX;
    var dy = touches[0].clientY - touches[1].clientY;
    return Math.sqrt(dx * dx + dy * dy);
  }

  Carte.prototype.zoomer = function (pas) {
    this.zoom = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, this.zoom + pas));
    this.redessiner();
  };

  Carte.prototype.recentrer = function () {
    this.suivi = true;
    this.cadrer();
    this.redessiner();
  };

  /**
   * Cadre la vue sur tout ce qui doit etre visible : positions vivantes, trace
   * fixe et reperes de distance. A defaut, la France entiere.
   */
  Carte.prototype.cadrer = function () {
    var vivants = [];
    var ajouter = function (lat, lon) {
      if (typeof lat === "number" && typeof lon === "number") vivants.push({ lat: lat, lon: lon });
    };
    this.points.forEach(function (point) {
      ajouter(point.lat, point.lon);
    });
    (this.trace || []).forEach(function (couple) {
      ajouter(couple[0], couple[1]);
    });
    (this.reperes || []).forEach(function (repere) {
      ajouter(repere.lat, repere.lon);
    });
    if (!vivants.length) {
      if (!this.centre) this.centre = { lat: 46.6, lon: 2.4 };
      return;
    }
    var lat = 0;
    var lon = 0;
    var minLat = 90;
    var maxLat = -90;
    var minLon = 180;
    var maxLon = -180;
    vivants.forEach(function (point) {
      lat += point.lat;
      lon += point.lon;
      minLat = Math.min(minLat, point.lat);
      maxLat = Math.max(maxLat, point.lat);
      minLon = Math.min(minLon, point.lon);
      maxLon = Math.max(maxLon, point.lon);
    });
    this.centre = { lat: lat / vivants.length, lon: lon / vivants.length };
    if (vivants.length === 1) {
      this.zoom = Math.max(this.zoom, 14);
      return;
    }
    // Zoom calcule sur la projection reelle : la trace entiere tient dans la vue.
    var largeur = this.conteneur.clientWidth || 640;
    var hauteur = this.conteneur.clientHeight || 360;
    var zoom = ZOOM_MAX;
    while (zoom > ZOOM_MIN) {
      var coinHautGauche = projeter(maxLat, minLon, zoom);
      var coinBasDroit = projeter(minLat, maxLon, zoom);
      if (
        Math.abs(coinBasDroit.x - coinHautGauche.x) <= largeur - 40 &&
        Math.abs(coinBasDroit.y - coinHautGauche.y) <= hauteur - 40
      ) {
        break;
      }
      zoom -= 1;
    }
    this.zoom = zoom;
  };

  // ---------------------------------------------------------------- rendu

  Carte.prototype.redessiner = function () {
    if (!this.centre) this.cadrer();
    var largeur = this.conteneur.clientWidth || 320;
    var hauteur = this.conteneur.clientHeight || 260;
    var centre = projeter(this.centre.lat, this.centre.lon, this.zoom);
    var gauche = centre.x - largeur / 2;
    var haut = centre.y - hauteur / 2;

    var premiereX = Math.floor(gauche / TAILLE);
    var premiereY = Math.floor(haut / TAILLE);
    var derniereX = Math.floor((gauche + largeur) / TAILLE);
    var derniereY = Math.floor((haut + hauteur) / TAILLE);
    var n = Math.pow(2, this.zoom);
    var vues = {};

    for (var x = premiereX; x <= derniereX; x += 1) {
      for (var y = premiereY; y <= derniereY; y += 1) {
        if (x < 0 || y < 0 || x >= n || y >= n) continue;
        var cle = this.zoom + "/" + x + "/" + y;
        vues[cle] = true;
        var tuile = this.tuiles[cle];
        if (!tuile) {
          tuile = document.createElement("img");
          tuile.className = "carte-tuile";
          tuile.alt = "";
          tuile.setAttribute("loading", "lazy");
          tuile.src = TUILE.replace("{z}", this.zoom).replace("{x}", x).replace("{y}", y);
          this.fond.appendChild(tuile);
          this.tuiles[cle] = tuile;
        } else if (tuile.parentNode !== this.fond) {
          this.fond.appendChild(tuile);
        }
        tuile.style.left = Math.round(x * TAILLE - gauche) + "px";
        tuile.style.top = Math.round(y * TAILLE - haut) + "px";
      }
    }

    // Les tuiles sorties de l'ecran sont retirees du DOM : la memoire reste
    // bornee, meme apres un long deplacement.
    var carte = this;
    Object.keys(this.tuiles).forEach(function (cle) {
      if (vues[cle]) return;
      var ancienne = carte.tuiles[cle];
      if (ancienne && ancienne.parentNode) ancienne.parentNode.removeChild(ancienne);
      delete carte.tuiles[cle];
    });

    this.dessinerTraces(gauche, haut);
    this.dessinerTraceFixe(gauche, haut);
    this.dessinerReperes(gauche, haut);
    this.dessinerMarqueurs(gauche, haut);
    var vide = !this.points.length && (!this.trace || this.trace.length < 2);
    this.message.textContent = vide ? "Aucune position a afficher pour l'instant." : "";
  };

  Carte.prototype.dessinerTraces = function (gauche, haut) {
    var carte = this;
    this.calque.innerHTML = "";
    this.points.forEach(function (point) {
      if (!point.trace || point.trace.length < 2) return;
      var chemin = point.trace
        .map(function (couple) {
          var pixel = projeter(couple[0], couple[1], carte.zoom);
          return Math.round(pixel.x - gauche) + "," + Math.round(pixel.y - haut);
        })
        .join(" ");
      var ligne = document.createElementNS("http://www.w3.org/2000/svg", "polyline");
      ligne.setAttribute("points", chemin);
      ligne.setAttribute("fill", "none");
      ligne.setAttribute("stroke", point.couleur || "#fc4c02");
      ligne.setAttribute("stroke-width", point.moi ? "3" : "2.5");
      ligne.setAttribute("stroke-linejoin", "round");
      ligne.setAttribute("stroke-linecap", "round");
      ligne.setAttribute("opacity", point.vivant ? "0.95" : "0.4");
      carte.calque.appendChild(ligne);
    });
  };

  /** Trace enregistree d'une seance : polyligne unique, sans marqueur mobile. */
  Carte.prototype.dessinerTraceFixe = function (gauche, haut) {
    var carte = this;
    if (!this.trace || this.trace.length < 2) return;
    var chemin = this.trace
      .map(function (couple) {
        var pixel = projeter(couple[0], couple[1], carte.zoom);
        return Math.round(pixel.x - gauche) + "," + Math.round(pixel.y - haut);
      })
      .join(" ");
    var ligne = document.createElementNS("http://www.w3.org/2000/svg", "polyline");
    ligne.setAttribute("points", chemin);
    ligne.setAttribute("fill", "none");
    ligne.setAttribute("stroke", "#fc4c02");
    ligne.setAttribute("stroke-width", "3");
    ligne.setAttribute("stroke-linejoin", "round");
    ligne.setAttribute("stroke-linecap", "round");
    this.calque.appendChild(ligne);
  };

  /** Reperes de distance (depart, kilometres, arrivee) de la trace fixe. */
  Carte.prototype.dessinerReperes = function (gauche, haut) {
    var carte = this;
    if (!this.reperes || !this.reperes.length) return;
    this.reperes.forEach(function (repere) {
      if (typeof repere.lat !== "number" || typeof repere.lon !== "number") return;
      var pixel = projeter(repere.lat, repere.lon, carte.zoom);
      var element = document.createElement("div");
      element.className = "carte-repere";
      element.style.left = Math.round(pixel.x - gauche) + "px";
      element.style.top = Math.round(pixel.y - haut) + "px";
      element.innerHTML = '<span class="carte-repere-point"></span><span class="carte-repere-nom"></span>';
      element.querySelector(".carte-repere-nom").textContent = repere.nom || "";
      carte.calque.appendChild(element);
    });
  };

  Carte.prototype.dessinerMarqueurs = function (gauche, haut) {
    var carte = this;
    this.points.forEach(function (point) {
      if (typeof point.lat !== "number" || typeof point.lon !== "number") return;
      var pixel = projeter(point.lat, point.lon, carte.zoom);
      var marqueur = point.element;
      if (!marqueur) {
        marqueur = document.createElement("div");
        marqueur.className = "carte-marqueur";
        marqueur.innerHTML = '<span class="carte-point"></span><span class="carte-etiquette"></span>';
        carte.calque.appendChild(marqueur);
        point.element = marqueur;
      }
      if (marqueur.parentNode !== carte.calque) carte.calque.appendChild(marqueur);
      marqueur.style.left = Math.round(pixel.x - gauche) + "px";
      marqueur.style.top = Math.round(pixel.y - haut) + "px";
      marqueur.classList.toggle("moi", !!point.moi);
      marqueur.classList.toggle("immobile", !point.vivant);
      var pastille = marqueur.querySelector(".carte-point");
      pastille.style.background = point.couleur || "#fc4c02";
      var etiquette = marqueur.querySelector(".carte-etiquette");
      var texte = point.nom || "Ami";
      if (point.age_texte) texte += " - " + point.age_texte;
      etiquette.textContent = texte;
    });

    // Les marqueurs dont la position a disparu sont retires.
    Array.prototype.slice.call(this.calque.querySelectorAll(".carte-marqueur")).forEach(function (element) {
      var connu = carte.points.some(function (point) {
        return point.element === element;
      });
      if (!connu) element.parentNode.removeChild(element);
    });
  };

  /** Applique une charge utile /amis.json (ou /api/v1/friends/live). */
  Carte.prototype.mettreAJour = function (donnees) {
    var points = [];
    if (donnees && donnees.me && donnees.me.live) {
      points.push(this.pointDe(donnees.me.live, "Ma position", true));
    }
    var amis = (donnees && donnees.friends) || [];
    amis.forEach(
      function (ami) {
        if (!ami.live) return;
        points.push(this.pointDe(ami.live, ami.name || ami.email, false));
      }.bind(this)
    );
    // Conserve les elements DOM existants pour eviter que les marqueurs
    // clignotent a chaque rafraichissement.
    var carte = this;
    points.forEach(function (point) {
      var ancien = carte.points.filter(function (existant) {
        return existant.cle === point.cle;
      })[0];
      if (ancien && ancien.element) {
        point.element = ancien.element;
        if (ancien.trace && ancien.trace.length && (!point.trace || !point.trace.length)) {
          point.trace = ancien.trace;
        }
      }
    });
    this.points = points;
    if (this.suivi) this.cadrer();
    this.redessiner();
  };

  Carte.prototype.pointDe = function (position, nom, moi) {
    return {
      cle: (moi ? "moi:" : "ami:") + position.device,
      nom: nom,
      moi: moi,
      lat: position.lat,
      lon: position.lon,
      trace: position.trace || [],
      vivant: position.state !== "stop",
      age_texte: age(position.age_s),
      couleur: moi ? "#2f2fbf" : couleurEtat(position.state),
    };
  };

  // --------------------------------------------------------------- donnees

  Carte.prototype.charger = function () {
    if (!this.source) return;
    var carte = this;
    window
      .fetch(this.source, { headers: { Accept: "application/json" }, credentials: "same-origin" })
      .then(function (reponse) {
        if (!reponse.ok) throw new Error("HTTP " + reponse.status);
        return reponse.json();
      })
      .then(function (donnees) {
        carte.message.textContent = "";
        carte.mettreAJour(donnees);
      })
      .catch(function (erreur) {
        carte.message.textContent = "Carte indisponible : " + erreur.message;
      });
  };

  Carte.prototype.demarrer = function (periode_ms) {
    this.charger();
    var carte = this;
    window.setInterval(function () {
      // Rien ne sert de tirer quand l'onglet est en arriere-plan.
      if (document.hidden) return;
      carte.charger();
    }, periode_ms || 10000);
  };

  window.MpacerCarte = {
    creer: function (conteneur, options) {
      return new Carte(conteneur, options);
    },
  };

  // Instances vivantes : l'application Android charge cette page dans une
  // WebView et pousse les positions par evaluateJavascript("MpacerCartes[0]...").
  window.MpacerCartes = [];

  /** Lit un attribut JSON (trace, reperes) sans faire echouer la page. */
  function lireJson(conteneur, attribut) {
    var brut = conteneur.getAttribute(attribut);
    if (!brut) return [];
    try {
      var valeur = JSON.parse(brut);
      return Array.isArray(valeur) ? valeur : [];
    } catch (erreur) {
      return [];
    }
  }

  function initialiser() {
    var conteneurs = Array.prototype.slice.call(document.querySelectorAll("[data-carte]"));
    conteneurs.forEach(function (conteneur) {
      var carte = new Carte(conteneur, {
        zoom: parseInt(conteneur.getAttribute("data-zoom"), 10) || 13,
        trace: lireJson(conteneur, "data-trace"),
        reperes: lireJson(conteneur, "data-reperes"),
      });
      window.MpacerCartes.push(carte);
      var periode = parseInt(conteneur.getAttribute("data-periode"), 10);
      if (conteneur.getAttribute("data-source")) carte.demarrer(periode > 0 ? periode : 10000);
      else carte.redessiner();
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initialiser);
  } else {
    initialiser();
  }
})();
