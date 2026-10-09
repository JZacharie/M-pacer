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

  // ------------------------------------------------ copie d'un code (page Amis)
  // Le code d'invitation se dicte mal : un bouton le met dans le presse-papiers.
  document.querySelectorAll("[data-copier]").forEach(function (bouton) {
    bouton.addEventListener("click", function () {
      var cible = document.querySelector(bouton.getAttribute("data-copier"));
      if (!cible) return;
      var texte = cible.textContent.trim();
      var confirmer = function () {
        var initial = bouton.textContent;
        bouton.textContent = "Copie !";
        window.setTimeout(function () { bouton.textContent = initial; }, 2000);
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(texte).then(confirmer, confirmer);
        return;
      }
      // Repli pour les navigateurs sans API presse-papiers (contexte non securise).
      var zone = document.createElement("textarea");
      zone.value = texte;
      zone.setAttribute("readonly", "readonly");
      zone.style.position = "absolute";
      zone.style.left = "-9999px";
      document.body.appendChild(zone);
      zone.select();
      try { document.execCommand("copy"); confirmer(); } catch (erreur) { /* rien */ }
      document.body.removeChild(zone);
    });
  });

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

  // ------------------------------------------------ depot des MP3 (page /music)
  // Le serveur garde les fichiers le temps que la montre ou le telephone les
  // recupere (docs/16). Sans JavaScript, rien n'est casse : la zone n'apparait
  // qu'avec le script, et le transfert USB reste disponible plus bas.
  (function () {
    var zone = document.getElementById("music-upload");
    if (!zone) return;
    var playlist = zone.getAttribute("data-playlist");
    var maxOctets = parseInt(zone.getAttribute("data-max-bytes") || "0", 10);
    var drop = document.getElementById("music-upload-drop");
    var entree = document.getElementById("music-upload-input");
    var barre = document.getElementById("music-upload-bar");
    var statut = document.getElementById("music-upload-status");
    var resultats = document.getElementById("music-upload-results");
    if (!playlist || !drop || !entree) return;

    function lisible(octets) {
      var unites = ["o", "Ko", "Mo", "Go"];
      var valeur = Number(octets) || 0;
      var unite = 0;
      while (valeur >= 1024 && unite + 1 < unites.length) { valeur = valeur / 1024; unite += 1; }
      return unite === 0 ? valeur + " o" : valeur.toFixed(1) + " " + unites[unite];
    }

    function ligne(nom, texte, ok) {
      if (!resultats) return;
      var item = document.createElement("li");
      item.className = ok ? "ok" : "erreur";
      var titre = document.createElement("strong");
      titre.textContent = nom + " : ";
      item.appendChild(titre);
      item.appendChild(document.createTextNode(texte));
      resultats.appendChild(item);
    }

    function envoyer(fichier) {
      return new Promise(function (resoudre) {
        if (maxOctets && fichier.size > maxOctets) {
          ligne(fichier.name, "trop volumineux (" + lisible(fichier.size) + ")", false);
          resoudre(false);
          return;
        }
        var donnees = new FormData();
        donnees.append("file", fichier, fichier.name);
        var requete = new XMLHttpRequest();
        requete.open("POST", "/music/playlists/" + encodeURIComponent(playlist) + "/upload");
        requete.upload.addEventListener("progress", function (evenement) {
          if (!barre || !evenement.lengthComputable) return;
          barre.style.width = Math.round((evenement.loaded / evenement.total) * 100) + "%";
        });
        requete.addEventListener("load", function () {
          var reponse = null;
          try { reponse = JSON.parse(requete.responseText); } catch (erreur) { reponse = null; }
          if (requete.status >= 200 && requete.status < 300 && reponse) {
            ligne(fichier.name, "depose (" + lisible(reponse.size_bytes) + ")", true);
            resoudre(true);
          } else {
            var message = (reponse && (reponse.message || reponse.error)) || ("HTTP " + requete.status);
            ligne(fichier.name, message, false);
            resoudre(false);
          }
        });
        requete.addEventListener("error", function () {
          ligne(fichier.name, "reseau indisponible", false);
          resoudre(false);
        });
        requete.send(donnees);
      });
    }

    var enCours = false;
    function traiter(fichiers) {
      if (enCours) return;
      var liste = Array.prototype.slice.call(fichiers || []);
      if (!liste.length) return;
      enCours = true;
      if (resultats) resultats.innerHTML = "";
      if (barre) barre.style.width = "0%";
      var reussis = 0;
      var suite = Promise.resolve();
      liste.forEach(function (fichier, index) {
        suite = suite.then(function () {
          if (statut) statut.textContent = "envoi " + (index + 1) + "/" + liste.length + " : " + fichier.name;
          return envoyer(fichier).then(function (ok) {
            if (ok) reussis += 1;
            if (barre) barre.style.width = Math.round(((index + 1) / liste.length) * 100) + "%";
          });
        });
      });
      suite.then(function () {
        if (statut) {
          statut.textContent = reussis + "/" + liste.length + " fichier(s) depose(s)"
            + (reussis ? " — rechargez la page pour voir la liste" : "");
        }
        enCours = false;
      });
    }

    ["dragenter", "dragover"].forEach(function (nom) {
      drop.addEventListener(nom, function (evenement) {
        evenement.preventDefault();
        drop.classList.add("actif");
      });
    });
    ["dragleave", "drop"].forEach(function (nom) {
      drop.addEventListener(nom, function (evenement) {
        evenement.preventDefault();
        drop.classList.remove("actif");
      });
    });
    drop.addEventListener("drop", function (evenement) {
      if (evenement.dataTransfer) traiter(evenement.dataTransfer.files);
    });
    entree.addEventListener("change", function () {
      traiter(entree.files);
      entree.value = "";
    });
  })();

  // ------------------------------------------------ agent local (chemin A, docs/16)
  // Meme depot, mais sans passer par le serveur : quand mpacer-music tourne sur
  // cet ordinateur, les fichiers partent directement en USB vers la montre ou le
  // telephone. Le panneau reste cache tant que l'agent ne repond pas.
  (function () {
    var zone = document.getElementById("music-agent");
    if (!zone) return;
    var base = "http://127.0.0.1:8077";
    var playlist = zone.getAttribute("data-playlist");
    var manifestUrl = zone.getAttribute("data-manifest");
    var statut = document.getElementById("music-agent-status");
    var corps = document.getElementById("music-agent-body");
    var selection = document.getElementById("music-agent-target");
    var rafraichir = document.getElementById("music-agent-refresh");
    var drop = document.getElementById("music-agent-drop");
    var entree = document.getElementById("music-agent-input");
    var barre = document.getElementById("music-agent-bar");
    var progression = document.getElementById("music-agent-progress");
    var resultats = document.getElementById("music-agent-results");
    if (!playlist || !selection || !drop || !entree) return;

    function lisible(octets) {
      var unites = ["o", "Ko", "Mo", "Go"];
      var valeur = Number(octets) || 0;
      var unite = 0;
      while (valeur >= 1024 && unite + 1 < unites.length) { valeur = valeur / 1024; unite += 1; }
      return unite === 0 ? valeur + " o" : valeur.toFixed(1) + " " + unites[unite];
    }

    function ligne(nom, texte, ok) {
      if (!resultats) return;
      var item = document.createElement("li");
      item.className = ok ? "ok" : "erreur";
      var titre = document.createElement("strong");
      titre.textContent = nom + " : ";
      item.appendChild(titre);
      item.appendChild(document.createTextNode(texte));
      resultats.appendChild(item);
    }

    function chargerCibles() {
      if (statut) statut.textContent = "recherche de l'agent local...";
      var controleur = new AbortController();
      var minuteur = window.setTimeout(function () { controleur.abort(); }, 2000);
      fetch(base + "/api/targets", { mode: "cors", signal: controleur.signal })
        .then(function (reponse) {
          if (!reponse.ok) throw new Error("HTTP " + reponse.status);
          return reponse.json();
        })
        .then(function (donnees) {
          window.clearTimeout(minuteur);
          var cibles = donnees.targets || [];
          selection.innerHTML = "";
          cibles.forEach(function (cible) {
            var option = document.createElement("option");
            option.value = cible.serial;
            option.setAttribute("data-dir", cible.music_dir || "");
            var genre = cible.kind === "watch" ? "montre" : (cible.kind === "phone" ? "telephone" : "appareil");
            option.textContent = (cible.model || cible.serial) + " (" + genre + ")";
            selection.appendChild(option);
          });
          var autorisees = cibles.filter(function (cible) { return cible.state === "device"; });
          if (!autorisees.length) {
            if (corps) corps.hidden = true;
            if (statut) {
              statut.textContent = donnees.adb === "absent"
                ? "agent local detecte, mais adb est introuvable : installez les platform-tools Android."
                : "agent local detecte, mais aucun appareil autorise : branchez la montre ou le telephone en USB (debogage USB active).";
            }
            return;
          }
          if (corps) corps.hidden = false;
          if (statut) statut.textContent = "agent local detecte (" + (donnees.adb || "adb") + ")";
        })
        .catch(function () {
          window.clearTimeout(minuteur);
          if (corps) corps.hidden = true;
          if (statut) {
            statut.textContent = "agent local absent : lancez mpacer-music sur cet ordinateur. Si besoin, autorisez cette page avec --allow-origin.";
          }
        });
    }

    function envoyer(fichier) {
      return new Promise(function (resoudre) {
        var requete = new XMLHttpRequest();
        var url = base + "/api/library/" + encodeURIComponent(playlist) +
          "?name=" + encodeURIComponent(fichier.name);
        requete.open("POST", url);
        requete.addEventListener("load", function () {
          var reponse = null;
          try { reponse = JSON.parse(requete.responseText); } catch (erreur) { reponse = null; }
          if (requete.status >= 200 && requete.status < 300 && reponse) {
            ligne(fichier.name, "depose dans l'agent", true);
            resoudre(true);
          } else {
            var message = (reponse && (reponse.error || reponse.message)) || ("HTTP " + requete.status);
            ligne(fichier.name, message, false);
            resoudre(false);
          }
        });
        requete.addEventListener("error", function () {
          ligne(fichier.name, "agent local injoignable", false);
          resoudre(false);
        });
        requete.send(fichier);
      });
    }

    function suivreTransfert(jobId) {
      return new Promise(function (resoudre) {
        function interroger() {
          fetch(base + "/api/transfer/" + jobId, { mode: "cors" })
            .then(function (reponse) { return reponse.json(); })
            .then(function (etat) {
              if (progression) {
                progression.textContent = etat.step + " " + etat.current + "/" + etat.total +
                  "  " + lisible(etat.bytes_sent);
              }
              if (barre && etat.total > 0) {
                barre.style.width = Math.round((etat.current / etat.total) * 100) + "%";
              }
              if (etat.state === "running") {
                window.setTimeout(interroger, 500);
                return;
              }
              var message = etat.state === "done"
                ? "copie terminee"
                : ("transfert " + etat.state + (etat.error ? " : " + etat.error : ""));
              if (progression) progression.textContent = message;
              ligne("transfert", message, etat.state === "done");
              resoudre(etat.state === "done");
            })
            .catch(function (erreur) {
              if (progression) progression.textContent = "suivi impossible : " + erreur.message;
              resoudre(false);
            });
        }
        interroger();
      });
    }

    function transferer() {
      var option = selection.options[selection.selectedIndex];
      var dossier = option ? option.getAttribute("data-dir") : "";
      if (progression) progression.textContent = "lecture du manifeste...";
      return fetch(manifestUrl)
        .then(function (reponse) { return reponse.text(); })
        .then(function (manifeste) {
          var charge = { manifest_json: manifeste, serial: selection.value };
          if (dossier) charge.watch_dir = dossier;
          return fetch(base + "/api/transfer", {
            method: "POST",
            mode: "cors",
            headers: { "content-type": "application/json" },
            body: JSON.stringify(charge)
          });
        })
        .then(function (reponse) { return reponse.json(); })
        .then(function (donnees) {
          if (!donnees.job_id) throw new Error(donnees.error || "transfert refuse");
          return suivreTransfert(donnees.job_id);
        })
        .catch(function (erreur) {
          if (progression) progression.textContent = "transfert impossible : " + erreur.message;
        });
    }

    var enCours = false;
    function traiter(fichiers) {
      if (enCours) return;
      var liste = Array.prototype.slice.call(fichiers || []);
      if (!liste.length) return;
      enCours = true;
      if (resultats) resultats.innerHTML = "";
      var reussis = 0;
      var suite = Promise.resolve();
      liste.forEach(function (fichier, index) {
        suite = suite.then(function () {
          if (progression) {
            progression.textContent = "envoi " + (index + 1) + "/" + liste.length + " : " + fichier.name;
          }
          return envoyer(fichier).then(function (ok) {
            if (ok) reussis += 1;
            if (barre) barre.style.width = Math.round(((index + 1) / liste.length) * 100) + "%";
          });
        });
      });
      suite
        .then(function () {
          if (!reussis) return undefined;
          return transferer();
        })
        .then(function () { enCours = false; });
    }

    ["dragenter", "dragover"].forEach(function (nom) {
      drop.addEventListener(nom, function (evenement) {
        evenement.preventDefault();
        drop.classList.add("actif");
      });
    });
    ["dragleave", "drop"].forEach(function (nom) {
      drop.addEventListener(nom, function (evenement) {
        evenement.preventDefault();
        drop.classList.remove("actif");
      });
    });
    drop.addEventListener("drop", function (evenement) {
      if (evenement.dataTransfer) traiter(evenement.dataTransfer.files);
    });
    entree.addEventListener("change", function () {
      traiter(entree.files);
      entree.value = "";
    });
    if (rafraichir) rafraichir.addEventListener("click", chargerCibles);
    chargerCibles();
  })();
})();
