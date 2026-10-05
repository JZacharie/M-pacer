// M-pacer : interactions minimales, aucune dependance.
(function () {
  "use strict";

  // Champ de code d'appairage : mise en forme AB12-CD34 au fil de la saisie.
  var input = document.querySelector('input[name="user_code"]');
  if (input) {
    input.addEventListener("input", function () {
      var raw = input.value
        .toUpperCase()
        .replace(/[^A-Z0-9]/g, "")
        .slice(0, 8);
      input.value = raw.length > 4 ? raw.slice(0, 4) + "-" + raw.slice(4) : raw;
    });
  }

  // Confirmation avant suppression d'une seance.
  document.querySelectorAll("form[data-confirm]").forEach(function (form) {
    form.addEventListener("submit", function (event) {
      if (!window.confirm(form.getAttribute("data-confirm"))) {
        event.preventDefault();
      }
    });
  });
})();
