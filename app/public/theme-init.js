// Aplica el tema elegido antes de que la ventana pinte (`docs/app.md`, "Apariencia").
// Es un fichero aparte, sin `type="module"`, para que bloquee el análisis del HTML: la CSP solo
// deja scripts de la app (nada en línea) y un módulo se ejecutaría ya con la ventana pintada.
// Lee la copia del tema en `localStorage`; el núcleo es la fuente de verdad (`src/theme.ts`
// la corrige al leerlo). Si cambias la clave o los valores, cambia también `theme.ts`.
(function () {
  try {
    var theme = window.localStorage.getItem("tramos.theme");
    if (theme === "light" || theme === "dark") {
      document.documentElement.setAttribute("data-theme", theme);
    }
  } catch (_) {
    // Sin localStorage: se queda el tema del sistema hasta que responda el núcleo.
  }
})();
