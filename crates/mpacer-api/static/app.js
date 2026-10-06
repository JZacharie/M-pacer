// M-pacer : interactions minimales, aucune dependance.
(function () {
  "use strict";

  var reduit = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  // ------------------------------------------------ code d'appairage AB12-CD34
  var input = document.querySelector('input[name="user_code"]');
  if (input) {
    input.addEventListener("input", function () {
      var raw = input.value.toUpperCase().replace(/[^A-Z0-9]/g, "").slice(0, 8);
      input.value = raw.length > 4 ? raw.slice(0, 4) + "-" + raw.slice(4) : raw;
    });
  }

  // ------------------------------------------------ menu des reglages
  // Le menu est un `details` natif : il fonctionne sans JavaScript. Le script
  // ne fait que le refermer quand on clique ailleurs ou qu'on appuie sur Echap.
  var menuReglages = document.querySelector("header.site details.menu");
  if (menuReglages) {
    document.addEventListener("click", function (event) {
      if (menuReglages.open && !menuReglages.contains(event.target)) {
        menuReglages.open = false;
      }
    });
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape" && menuReglages.open) {
        menuReglages.open = false;
        var bouton = menuReglages.querySelector("summary");
        if (bouton) bouton.focus();
      }
    });
  }

  // ------------------------------------------------ confirmation de suppression
  document.querySelectorAll("form[data-confirm]").forEach(function (form) {
    form.addEventListener("submit", function (event) {
      if (!window.confirm(form.getAttribute("data-confirm"))) event.preventDefault();
    });
  });

  // ------------------------------------------------ compte a rebours vivant
  function formater(ms) {
    if (ms <= 0) return "c'est parti";
    var s = Math.floor(ms / 1000);
    var j = Math.floor(s / 86400);
    var h = Math.floor((s % 86400) / 3600);
    var m = Math.floor((s % 3600) / 60);
    var sec = s % 60;
    if (j > 0) return "J-" + j + " " + h + " h";
    if (h > 0) return h + " h " + String(m).padStart(2, "0");
    return m + " min " + String(sec).padStart(2, "0") + " s";
  }

  var compte = Array.prototype.slice.call(document.querySelectorAll(".countdown[data-at]"));
  if (compte.length) {
    var rafraichir = function () {
      var maintenant = Date.now();
      compte.forEach(function (el) {
        var cible = parseInt(el.getAttribute("data-at"), 10);
        if (!cible) return;
        // le libelle est le dernier noeud texte du bloc
        var texte = formater(cible - maintenant);
        var noeuds = el.childNodes;
        for (var i = noeuds.length - 1; i >= 0; i--) {
          if (noeuds[i].nodeType === 3) { noeuds[i].nodeValue = " " + texte + " "; return; }
        }
      });
    };
    rafraichir();
    setInterval(rafraichir, 1000);
  }

  // ------------------------------------------------ apparitions au defilement
  var aReveler = Array.prototype.slice.call(document.querySelectorAll(".reveal"));
  if (aReveler.length && "IntersectionObserver" in window && !reduit) {
    var observateur = new IntersectionObserver(function (entrees) {
      entrees.forEach(function (entree, index) {
        if (!entree.isIntersecting) return;
        var el = entree.target;
        el.style.animationDelay = index * 60 + "ms";
        el.classList.add("vu");
        observateur.unobserve(el);
      });
    }, { rootMargin: "0px 0px -40px 0px" });
    aReveler.forEach(function (el) { observateur.observe(el); });
  }

  // ------------------------------------------------ chiffres qui montent
  if (!reduit) {
    document.querySelectorAll(".card strong, .card .value").forEach(function (el) {
      var brut = el.textContent.trim();
      var mesure = brut.match(/^([0-9]+(?:[.,][0-9]+)?)/);
      if (!mesure) return;
      var cible = parseFloat(mesure[1].replace(",", "."));
      if (!isFinite(cible) || cible < 3 || cible > 100000) return;
      var suffixe = brut.slice(mesure[1].length);
      var decimales = (mesure[1].split(/[.,]/)[1] || "").length;
      var debut = null;
      var duree = 520;
      var pas = function (instant) {
        if (debut === null) debut = instant;
        var t = Math.min(1, (instant - debut) / duree);
        var eased = 1 - Math.pow(1 - t, 3);
        el.textContent = (cible * eased).toFixed(decimales).replace(".", ",") + suffixe;
        if (t < 1) requestAnimationFrame(pas);
        else el.textContent = brut;
      };
      requestAnimationFrame(pas);
    });
  }

  // ------------------------------------------------ barres du graphique
  document.querySelectorAll(".chart rect").forEach(function (barre, index) {
    barre.style.animationDelay = Math.min(index * 28, 400) + "ms";
  });

  // ------------------------------------------------ tap-tempo (page /music)
  // L'utilisateur tape en rythme : la mediane des intervalles donne le BPM.
  // Le calcul est fait aussi cote serveur (mpacer_core::music::tap_tempo) quand
  // les instants sont transmis, mais l'affichage immediat evite un aller-retour.
  (function () {
    "use strict";

    function tempoMedian(instants) {
      var intervalles = [];
      for (var i = 1; i < instants.length; i++) {
        var ecart = instants[i] - instants[i - 1];
        // Un intervalle aberrant (hesitation, double clic) fausserait la mediane.
        if (ecart >= 200 && ecart <= 2000) intervalles.push(ecart);
      }
      if (intervalles.length < 3) return null;
      intervalles.sort(function (a, b) { return a - b; });
      var milieu = Math.floor(intervalles.length / 2);
      var mediane = intervalles.length % 2
        ? intervalles[milieu]
        : (intervalles[milieu - 1] + intervalles[milieu]) / 2;
      if (mediane <= 0) return null;
      return 60000 / mediane;
    }

    var formulaires = Array.prototype.slice.call(
      document.querySelectorAll("form[data-bpm-track]")
    );
    formulaires.forEach(function (formulaire) {
      var id = formulaire.getAttribute("data-bpm-track");
      var champ = formulaire.querySelector('input[name="bpm"]');
      var source = formulaire.querySelector('input[name="source"]');
      var bouton = document.querySelector('button[data-tap="' + id + '"]');
      if (!champ || !bouton) return;

      var instants = [];
      var minuterie = null;

      bouton.addEventListener("click", function () {
        var maintenant = Date.now();
        // Une pause de plus de 3 s repart de zero : c'est une nouvelle mesure.
        if (instants.length && maintenant - instants[instants.length - 1] > 3000) {
          instants = [];
        }
        instants.push(maintenant);
        if (instants.length > 12) instants.shift();

        var bpm = tempoMedian(instants);
        bouton.textContent = bpm ? "tapper " + Math.round(bpm) + " (" + instants.length + ")" : "tapper (" + instants.length + ")";
        if (bpm) {
          champ.value = String(Math.round(bpm));
          if (source) source.value = "tap";
          bouton.classList.add("ok");
        }

        if (minuterie) window.clearTimeout(minuterie);
        minuterie = window.setTimeout(function () {
          bouton.textContent = "tapper";
          bouton.classList.remove("ok");
        }, 4000);
      });
    });
  })();

  // ------------------------------------------------ gabarits de tableau de bord
  // Choisir un modele coche les widgets correspondants. Le formulaire reste
  // utilisable sans JavaScript : le serveur applique alors le modele.
  var gabarit = document.querySelector("select[data-widget-target]");
  if (gabarit) {
    var cibles = Array.prototype.slice.call(
      document.querySelectorAll('input[name="' + gabarit.getAttribute("data-widget-target") + '"]')
    );
    gabarit.addEventListener("change", function () {
      var option = gabarit.options[gabarit.selectedIndex];
      var brutes = (option && option.getAttribute("data-widgets")) || "";
      var cles = brutes.split(",").filter(Boolean);
      if (!cles.length) return;
      cibles.forEach(function (cible) {
        cible.checked = cles.indexOf(cible.value) !== -1;
      });
    });
  }
})();
