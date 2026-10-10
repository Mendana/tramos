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
