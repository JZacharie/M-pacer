// M-pacer - documentation interactive.
//
// Aucune dependance, aucun reseau : theme clair/sombre, barre de progression,
// sommaire suivi, visionneuse d'images, boutons de copie, onglets, filtres de
// galerie et demonstration du composeur de tableau de bord.
(function () {
  "use strict";

  var racine = document.documentElement;

  // ------------------------------------------------------- theme clair / sombre
  // Le choix explicite est memorise ; sans choix, le theme suit le systeme.
  var CLE_THEME = "mpacer-theme";
  var boutonTheme = document.querySelector("[data-theme-toggle]");

  function estClair() {
    var choisi = racine.getAttribute("data-theme");
    if (choisi) return choisi === "light";
    return !!(window.matchMedia && window.matchMedia("(prefers-color-scheme: light)").matches);
  }

  function majBoutonTheme() {
    if (!boutonTheme) return;
    var clair = estClair();
    boutonTheme.textContent = clair ? "Thème sombre" : "Thème clair";
    boutonTheme.setAttribute("aria-pressed", clair ? "true" : "false");
  }

  if (boutonTheme) {
    majBoutonTheme();
    boutonTheme.addEventListener("click", function () {
      var suivant = estClair() ? "dark" : "light";
      racine.setAttribute("data-theme", suivant);
      try { localStorage.setItem(CLE_THEME, suivant); } catch (e) { /* navigation privee */ }
      majBoutonTheme();
    });
    if (window.matchMedia) {
      var mediaTheme = window.matchMedia("(prefers-color-scheme: light)");
      var surChangement = function () { majBoutonTheme(); };
      if (mediaTheme.addEventListener) mediaTheme.addEventListener("change", surChangement);
      else if (mediaTheme.addListener) mediaTheme.addListener(surChangement);
    }
  }

  // --------------------------------------------------- progression de lecture
  var barre = document.querySelector("[data-progress]");
  if (barre) {
    var majBarre = function () {
      var hauteur = document.documentElement.scrollHeight - window.innerHeight;
      var ratio = hauteur > 0 ? Math.min(1, Math.max(0, window.scrollY / hauteur)) : 0;
      barre.style.transform = "scaleX(" + ratio.toFixed(4) + ")";
    };
    window.addEventListener("scroll", majBarre, { passive: true });
    window.addEventListener("resize", majBarre);
    majBarre();
  }

  // ------------------------------------------------------ sommaire de la page
  var zoneSommaire = document.querySelector("[data-toc]");
  // Seules les sections du document comptent : les titres de cartes, de
  // vignettes ou de panneaux ne polluent pas le sommaire.
  var titres = [].slice.call(document.querySelectorAll("main h2, main h3")).filter(function (titre) {
    return !titre.closest(".card, .grid, .steps, .vignette, .kpi, .schema, .composeur, .onglet, figure, pre, details");
  });

  if (zoneSommaire && titres.length > 1) {
    var liste = document.createElement("ol");
    liste.className = "toc";
    titres.forEach(function (titre, index) {
      if (!titre.id) titre.id = "section-" + (index + 1);
      var item = document.createElement("li");
      item.className = titre.tagName === "H3" ? "niveau-3" : "niveau-2";
      var lien = document.createElement("a");
      lien.href = "#" + titre.id;
      lien.textContent = titre.textContent;
      item.appendChild(lien);
      liste.appendChild(item);
    });
    zoneSommaire.appendChild(liste);

    // Le sommaire se place juste apres le titre (et son chapeau) plutot qu'au
    // tout debut de page ; il est deplie sur grand ecran, replie sur telephone.
    var titrePrincipal = document.querySelector("main h1");
    var point = titrePrincipal;
    if (point && point.nextElementSibling && point.nextElementSibling.classList.contains("lead")) {
      point = point.nextElementSibling;
    }
    if (point && point.parentNode) {
      point.parentNode.insertBefore(zoneSommaire, point.nextSibling);
    }
    zoneSommaire.hidden = false;
    zoneSommaire.open = window.innerWidth >= 900;

    var liens = [].slice.call(liste.querySelectorAll("a"));
    var marquer = function (id) {
      liens.forEach(function (lien) {
        var actif = lien.getAttribute("href") === "#" + id;
        lien.className = actif ? "actif" : "";
        if (actif) lien.setAttribute("aria-current", "true");
        else lien.removeAttribute("aria-current");
      });
    };
    var majSommaire = function () {
      var courant = titres[0].id;
      titres.forEach(function (titre) {
        if (titre.getBoundingClientRect().top <= 140) courant = titre.id;
      });
      marquer(courant);
    };
    var planifie = false;
    window.addEventListener("scroll", function () {
      if (planifie) return;
      planifie = true;
      window.requestAnimationFrame(function () { planifie = false; majSommaire(); });
    }, { passive: true });
    majSommaire();
  }

  // -------------------------------------------------------- retour en haut
  var boutonHaut = document.querySelector("[data-top]");
  if (boutonHaut) {
    var majHaut = function () {
      boutonHaut.hidden = window.scrollY < 600;
    };
    window.addEventListener("scroll", majHaut, { passive: true });
    majHaut();
    boutonHaut.addEventListener("click", function () {
      window.scrollTo({ top: 0, behavior: "smooth" });
    });
  }

  // ---------------------------------------------------- visionneuse d'images
  var images = [].slice.call(document.querySelectorAll("main img"));
  var voile = null;
  var voileImage = null;
  var voileLegende = null;
  var position = 0;
  var dernierFoyer = null;

  function legendeDe(image) {
    var figure = image.closest ? image.closest("figure") : null;
    var legende = figure ? figure.querySelector("figcaption") : null;
    return legende ? legende.textContent.trim() : (image.getAttribute("alt") || "");
  }

  function construireVoile() {
    voile = document.createElement("div");
    voile.className = "visionneuse";
    voile.setAttribute("role", "dialog");
    voile.setAttribute("aria-modal", "true");
    voile.setAttribute("aria-label", "Image agrandie");
    voile.hidden = true;

    var fermer = document.createElement("button");
    fermer.type = "button";
    fermer.className = "visionneuse-fermer";
    fermer.setAttribute("data-fermer", "");
    fermer.setAttribute("aria-label", "Fermer");
    fermer.textContent = "\u00d7";

    var precedent = document.createElement("button");
    precedent.type = "button";
    precedent.className = "visionneuse-nav precedent";
    precedent.setAttribute("aria-label", "Image précédente");
    precedent.textContent = "\u2039";

    var suivant = document.createElement("button");
    suivant.type = "button";
    suivant.className = "visionneuse-nav suivant";
    suivant.setAttribute("aria-label", "Image suivante");
    suivant.textContent = "\u203a";

    var figure = document.createElement("figure");
    voileImage = document.createElement("img");
    voileImage.alt = "";
    voileLegende = document.createElement("figcaption");
    figure.appendChild(voileImage);
    figure.appendChild(voileLegende);

    voile.appendChild(fermer);
    voile.appendChild(precedent);
    voile.appendChild(figure);
    voile.appendChild(suivant);
    document.body.appendChild(voile);

    fermer.addEventListener("click", fermerVoile);
    precedent.addEventListener("click", function (evenement) {
      evenement.stopPropagation();
      naviguer(-1);
    });
    suivant.addEventListener("click", function (evenement) {
      evenement.stopPropagation();
      naviguer(1);
    });
    voile.addEventListener("click", function (evenement) {
      if (evenement.target === voile) fermerVoile();
    });
  }

  function afficherPosition() {
    var image = images[position];
    if (!image) return;
    voileImage.src = image.currentSrc || image.src;
    voileImage.alt = image.getAttribute("alt") || "";
    voileLegende.textContent = legendeDe(image) + " (" + (position + 1) + " / " + images.length + ")";
    var plusieurs = images.length > 1;
    voile.querySelector(".visionneuse-nav.precedent").hidden = !plusieurs;
    voile.querySelector(".visionneuse-nav.suivant").hidden = !plusieurs;
  }

  function ouvrirVoile(index) {
    if (!voile) construireVoile();
    position = index;
    dernierFoyer = document.activeElement;
    voile.hidden = false;
    document.body.classList.add("voile-ouverte");
    afficherPosition();
    voile.querySelector("[data-fermer]").focus();
  }

  function fermerVoile() {
    if (!voile || voile.hidden) return;
    voile.hidden = true;
    document.body.classList.remove("voile-ouverte");
    if (dernierFoyer && dernierFoyer.focus) dernierFoyer.focus();
  }

  function naviguer(pas) {
    if (!images.length) return;
    position = (position + pas + images.length) % images.length;
    afficherPosition();
  }

  images.forEach(function (image, index) {
    image.classList.add("agrandissable");
    var bouton = image.closest ? image.closest("[data-zoom]") : null;
    if (bouton) {
      bouton.addEventListener("click", function (evenement) {
        evenement.preventDefault();
        ouvrirVoile(index);
      });
    } else {
      image.addEventListener("click", function () { ouvrirVoile(index); });
    }
  });

  document.addEventListener("keydown", function (evenement) {
    if (!voile || voile.hidden) return;
    if (evenement.key === "Escape") { fermerVoile(); return; }
    if (evenement.key === "ArrowLeft") { naviguer(-1); return; }
    if (evenement.key === "ArrowRight") { naviguer(1); }
  });

  // ------------------------------------------------- blocs de code copiables
  [].slice.call(document.querySelectorAll("main pre")).forEach(function (bloc) {
    var bouton = document.createElement("button");
    bouton.type = "button";
    bouton.className = "copier";
    bouton.textContent = "Copier";
    bouton.addEventListener("click", function () {
      var texte = bloc.querySelector("code") ? bloc.querySelector("code").innerText : bloc.innerText;
      var reussi = function () {
        bouton.textContent = "Copié";
        window.setTimeout(function () { bouton.textContent = "Copier"; }, 1600);
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(texte).then(reussi, function () { bouton.textContent = "Copie impossible"; });
      } else {
        bouton.textContent = "Copie impossible";
      }
    });
    bloc.appendChild(bouton);
  });

  // ------------------------------------------------------------------ onglets
  [].slice.call(document.querySelectorAll("[data-tabs]")).forEach(function (bloc) {
    var panneaux = [].slice.call(bloc.children).filter(function (enfant) {
      return enfant.classList && enfant.classList.contains("onglet");
    });
    if (panneaux.length < 2) return;

    var barre = document.createElement("div");
    barre.className = "onglets-barre";
    barre.setAttribute("role", "tablist");

    var activer = function (index) {
      panneaux.forEach(function (panneau, rang) {
        var actif = rang === index;
        panneau.hidden = !actif;
        boutons[rang].setAttribute("aria-selected", actif ? "true" : "false");
        boutons[rang].tabIndex = actif ? 0 : -1;
      });
    };

    var boutons = panneaux.map(function (panneau, index) {
      var bouton = document.createElement("button");
      bouton.type = "button";
      bouton.className = "onglet-bouton";
      bouton.setAttribute("role", "tab");
      bouton.textContent = panneau.getAttribute("data-title") || "Onglet " + (index + 1);
      bouton.addEventListener("click", function () { activer(index); });
      bouton.addEventListener("keydown", function (evenement) {
        if (evenement.key !== "ArrowRight" && evenement.key !== "ArrowLeft") return;
        evenement.preventDefault();
        var pas = evenement.key === "ArrowRight" ? 1 : -1;
        var suivant = (index + pas + panneaux.length) % panneaux.length;
        activer(suivant);
        boutons[suivant].focus();
      });
      barre.appendChild(bouton);
      panneau.classList.add("onglet-panneau");
      return bouton;
    });

    bloc.insertBefore(barre, bloc.firstChild);
    activer(0);
  });

  // -------------------------------------------------------- filtres de galerie
  var filtres = document.querySelector("[data-filters]");
  var galerie = document.querySelector("[data-galerie]");
  if (filtres && galerie) {
    var vignettes = [].slice.call(galerie.querySelectorAll("[data-group]"));
    var boutonsFiltre = [].slice.call(filtres.querySelectorAll("button[data-filter]"));
    var filtrer = function (groupe) {
      boutonsFiltre.forEach(function (bouton) {
        bouton.classList.toggle("actif", bouton.getAttribute("data-filter") === groupe);
      });
      var visibles = 0;
      vignettes.forEach(function (vignette) {
        var garde = groupe === "*" || vignette.getAttribute("data-group") === groupe;
        vignette.hidden = !garde;
        if (garde) visibles++;
      });
      var compteur = galerie.parentNode.querySelector("[data-galerie-compteur]");
      if (compteur) {
        compteur.textContent = visibles + (visibles > 1 ? " images" : " image");
      }
    };
    boutonsFiltre.forEach(function (bouton) {
      bouton.addEventListener("click", function () {
        filtrer(bouton.getAttribute("data-filter"));
      });
    });
  }

  // ------------------------------------ demonstration du composeur de tableaux
  var composeur = document.querySelector("[data-composeur]");
  if (composeur) {
    // Le catalogue est lu dans la page elle-meme (cases et boutons de gabarit) :
    // aucune donnee injectee dans un script, donc rien qui puisse etre echappe
    // ou perdu par le moteur de rendu.
    var cases = [].slice.call(composeur.querySelectorAll("input[data-widget]"));
    var boutonsGabarit = [].slice.call(composeur.querySelectorAll("[data-gabarit]"));
    var parCle = {};
    cases.forEach(function (caze) {
      var etiquette = caze.closest("label");
      var titre = etiquette ? etiquette.querySelector("strong") : null;
      var details = etiquette ? etiquette.querySelector("span") : null;
      parCle[caze.value] = {
        key: caze.value,
        label: titre ? titre.textContent.trim() : caze.value,
        description: details ? details.textContent.trim() : ""
      };
    });
    var gabarits = boutonsGabarit.map(function (bouton) {
      return {
        name: bouton.textContent.trim(),
        widgets: (bouton.getAttribute("data-widgets") || "").split(",").filter(Boolean)
      };
    });

    var choisis = [];
    var listeApercu = composeur.querySelector("[data-composeur-liste]");
    var messageVide = composeur.querySelector("[data-composeur-vide]");
    var compteurApercu = composeur.querySelector("[data-composeur-compteur]");

    function petitBouton(libelle, titre, action) {
      var bouton = document.createElement("button");
      bouton.type = "button";
      bouton.className = "aprecu-bouton";
      bouton.textContent = libelle;
      bouton.title = titre;
      bouton.setAttribute("aria-label", titre);
      bouton.addEventListener("click", action);
      return bouton;
    }

    function dessinerApercu() {
      if (!listeApercu) return;
      listeApercu.innerHTML = "";
      choisis.forEach(function (cle, index) {
        var widget = parCle[cle] || { label: cle, description: "" };
        var item = document.createElement("li");
        item.className = "aprecu-widget";

        var numero = document.createElement("span");
        numero.className = "aprecu-numero";
        numero.textContent = String(index + 1);

        var texte = document.createElement("div");
        texte.className = "aprecu-texte";
        var titre = document.createElement("strong");
        titre.textContent = widget.label;
        var details = document.createElement("span");
        details.textContent = widget.description;
        texte.appendChild(titre);
        texte.appendChild(details);

        var actions = document.createElement("div");
        actions.className = "aprecu-actions";
        actions.appendChild(petitBouton("\u2191", "Monter " + widget.label, function () { echanger(index, index - 1); }));
        actions.appendChild(petitBouton("\u2193", "Descendre " + widget.label, function () { echanger(index, index + 1); }));
        actions.appendChild(petitBouton("\u00d7", "Retirer " + widget.label, function () { basculer(cle, false); }));

        item.appendChild(numero);
        item.appendChild(texte);
        item.appendChild(actions);
        listeApercu.appendChild(item);
      });
      if (messageVide) messageVide.hidden = choisis.length > 0;
      if (compteurApercu) {
        compteurApercu.textContent = choisis.length + (choisis.length > 1 ? " widgets" : " widget");
      }
      cases.forEach(function (caze) {
        caze.checked = choisis.indexOf(caze.value) !== -1;
      });
      boutonsGabarit.forEach(function (bouton) {
        var cles = (bouton.getAttribute("data-widgets") || "").split(",").filter(Boolean);
        var identique = cles.length === choisis.length && cles.every(function (cle, index) {
          return choisis[index] === cle;
        });
        bouton.classList.toggle("actif", identique);
      });
    }

    function basculer(cle, actif) {
      var rang = choisis.indexOf(cle);
      if (actif && rang === -1) choisis.push(cle);
      if (!actif && rang !== -1) choisis.splice(rang, 1);
      dessinerApercu();
    }

    function echanger(depuis, vers) {
      if (vers < 0 || vers >= choisis.length) return;
      var valeur = choisis[depuis];
      choisis[depuis] = choisis[vers];
      choisis[vers] = valeur;
      dessinerApercu();
    }

    cases.forEach(function (caze) {
      caze.addEventListener("change", function () {
        basculer(caze.value, caze.checked);
      });
    });

    boutonsGabarit.forEach(function (bouton) {
      bouton.addEventListener("click", function () {
        choisis = (bouton.getAttribute("data-widgets") || "").split(",").filter(function (cle) {
          return !!parCle[cle];
        });
        dessinerApercu();
      });
    });

    var vider = composeur.querySelector("[data-composeur-vider]");
    if (vider) {
      vider.addEventListener("click", function () {
        choisis = [];
        dessinerApercu();
      });
    }

    // Au chargement : le gabarit « Pace Control », comme dans l'application.
    if (gabarits.length) {
      choisis = gabarits[0].widgets.slice();
    }
    dessinerApercu();
  }
})();
