// Quién mira la app. Quien entrena ve las carreras de un atleta en solo lectura (docs/app.md,
// "Atletas"): las vistas esconden todo control que modifique algo.
import { createContext, useContext } from "react";

export interface Viewer {
  /** Se ve a un atleta: no hay ningún control de edición ni de etiquetado. */
  readOnly: boolean;
  /** Nombre del atleta que se ve; `null` = el propio usuario. */
  runnerName: string | null;
}

export const ViewerContext = createContext<Viewer>({ readOnly: false, runnerName: null });

export const useViewer = () => useContext(ViewerContext);
