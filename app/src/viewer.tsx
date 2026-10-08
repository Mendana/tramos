// Quién mira la app. En modo entrenadora se ven las carreras de un corredor en solo lectura
// (docs/app.md, "Modo entrenadora"): las vistas esconden todo control que modifique algo.
import { createContext, useContext } from "react";

export interface Viewer {
  /** Modo entrenadora: no hay ningún control de edición ni de etiquetado. */
  readOnly: boolean;
  /** Nombre del corredor que se ve en modo entrenadora; `null` = el propio usuario. */
  runnerName: string | null;
}

export const ViewerContext = createContext<Viewer>({ readOnly: false, runnerName: null });

export const useViewer = () => useContext(ViewerContext);
