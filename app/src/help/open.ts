// Cómo abre la ayuda una gráfica (`docs/app.md`, "Gráficas" y "Ayuda"): la app da la función y el
// «?» de cada panel la usa para llevar a la sección de su página.
import { createContext, useContext } from "react";
import type { HelpTarget } from "./pages";

/** Abre la ayuda en `target`. Sin proveedor, el «?» solo enseña la explicación. */
export const HelpOpenContext = createContext<((target: HelpTarget) => void) | null>(null);

export const useOpenHelp = () => useContext(HelpOpenContext);
