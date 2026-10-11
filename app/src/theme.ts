import { Theme } from "./api";

/** Los temas que se pueden elegir, con su nombre en Ajustes. */
export const THEMES: { value: Theme; label: string }[] = [
  { value: "system", label: "Sistema" },
  { value: "light", label: "Claro" },
  { value: "dark", label: "Oscuro" },
];

/**
 * Pone el tema en la raíz del documento (`docs/app.md`, "Apariencia"): `data-theme="light"` o
 * `"dark"` lo fuerzan; sin atributo manda el del sistema (`prefers-color-scheme` en `tokens.css`).
 */
export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

/**
 * Clave de la copia del tema en `localStorage`. La lee `public/theme-init.js` antes de pintar;
 * si la cambias, cámbiala también allí.
 */
export const THEME_STORAGE_KEY = "tramos.theme";

/**
 * Guarda la copia del tema para que la próxima vez se aplique antes de pintar. El núcleo manda: la
 * copia solo evita ver un instante el tema equivocado. Si `localStorage` falla, no pasa nada.
 */
export function storeThemeCopy(theme: Theme) {
  try {
    if (theme === "system") window.localStorage.removeItem(THEME_STORAGE_KEY);
    else window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // Sin copia, la app sigue igual: se ve el tema del sistema hasta que responde el núcleo.
  }
}

/** Aplica el tema y guarda su copia: lo que se hace al elegirlo y al leerlo del núcleo. */
export function applyAndStoreTheme(theme: Theme) {
  applyTheme(theme);
  storeThemeCopy(theme);
}
