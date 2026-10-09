// Qué página de ayuda abre el botón «?» en cada pantalla y pestaña (`docs/app.md`, "Ayuda").
//
// Esto es el test de que cada vista tiene su página: los tres mapas son `Record` sobre los tipos
// de pantalla y de pestaña, así que si se añade una pantalla (`Screen` en `App.tsx`) o una
// pestaña (`RaceTab`, `HistoryTab`) sin darle página, `tsc` falla y con él `npm run build`, que
// corre en la CI. Que el fichero de la página exista lo asegura `pages.ts` (import estático).
import type { Screen } from "../App";
import type { HistoryTab } from "../HistoryScreen";
import type { RaceTab } from "../RaceView";
import type { HelpPageId } from "./pages";

/** Página de cada pantalla. Las que tienen pestañas, la de su primera pestaña. */
const SCREEN_HELP: Record<Screen["kind"], HelpPageId> = {
  home: "inicio",
  races: "mis-carreras",
  race: "carrera",
  history: "estadisticas",
  group: "grupo",
  import: "importar",
  profile: "perfil",
  settings: "ajustes",
  help: "indice",
};

/** Página de cada pestaña de la vista de carrera. */
const RACE_TAB_HELP: Record<RaceTab, HelpPageId> = {
  summary: "carrera",
  legs: "carrera-tramos",
  map: "carrera-mapa",
  group: "carrera-grupo",
  analysis: "carrera-analisis",
};

/** Página de cada pestaña de Estadísticas. */
const HISTORY_TAB_HELP: Record<HistoryTab, HelpPageId> = {
  summary: "estadisticas",
  where: "estadisticas-donde",
  progress: "estadisticas-evolucion",
  body: "estadisticas-cabeza",
};

/** La página de ayuda de lo que se está viendo. */
export function helpFor(screen: Screen, historyTab: HistoryTab): HelpPageId {
  switch (screen.kind) {
    case "race":
      return RACE_TAB_HELP[screen.tab ?? "summary"];
    case "history":
      return HISTORY_TAB_HELP[historyTab];
    case "help":
      return screen.page;
    default:
      return SCREEN_HELP[screen.kind];
  }
}
