// Atajo F1: abre la ayuda de la pantalla abierta, como el botón «?» (`docs/app.md`, "Ayuda").
import { useEffect, useRef } from "react";

/** Con `open`, F1 lo llama; con `null` (en la propia ayuda, en el recorrido), no hace nada. */
export function useHelpShortcut(open: (() => void) | null) {
  const latest = useRef(open);
  useEffect(() => {
    latest.current = open;
  });
  const active = open !== null;
  useEffect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "F1") return;
      e.preventDefault();
      if (!e.repeat) latest.current?.();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [active]);
}
