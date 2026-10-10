// Paneles de análisis que se pueden ocultar (#130, `docs/app.md`, "Gráficas"): el catálogo, por
// pantalla y pestaña, el contexto con los ocultos y el cuadro «Personalizar». Lo oculto se guarda
// en los ajustes del usuario (`ui.hidden_panels`) y vale para todas sus carreras y las que vea de
// otros corredores. Las Estadísticas de un grupo (#145) usan los mismos paneles, con los mismos
// identificadores: ocultar uno lo oculta en las dos pantallas.
import { ReactNode, createContext, useCallback, useContext, useEffect, useState } from "react";
import { hiddenPanels, setHiddenPanels } from "./api";
import { CloseIcon, Notice } from "./ui";

export interface PanelInfo {
  id: string;
  title: string;
}

export interface PanelGroup {
  /** «Estadísticas · ¿Dónde fallo?». */
  label: string;
  panels: PanelInfo[];
}

/** Todos los paneles que se pueden ocultar, en el orden en que salen. */
export const PANEL_GROUPS: PanelGroup[] = [
  {
    label: "Estadísticas · Resumen",
    panels: [
      { id: "format-performance", title: "IR medio por formato" },
      { id: "format-error-rate", title: "Tasa de error por formato" },
      { id: "format-loss", title: "Pérdida media por tramo" },
    ],
  },
  {
    label: "Estadísticas · ¿Dónde fallo?",
    panels: [
      { id: "leg-length", title: "Pérdida según duración del tramo" },
      { id: "common-errors", title: "Tipos de error" },
      { id: "slope-performance", title: "IR medio según desnivel" },
      { id: "slope-error-rate", title: "Tasa de error según desnivel" },
      { id: "breakdown", title: "De qué está hecha la pérdida de tus errores" },
    ],
  },
  {
    label: "Estadísticas · ¿Cómo evoluciono?",
    panels: [
      { id: "consistency", title: "Consistencia por carrera" },
      { id: "days-off-entry", title: "IR al entrar en mapa" },
      { id: "days-off-start", title: "Errores al principio de la carrera" },
    ],
  },
  {
    label: "Estadísticas · Cabeza y piernas",
    panels: [
      { id: "after-error", title: "¿Un error trae otro?" },
      { id: "clean-streaks", title: "Rachas limpias" },
      { id: "fatigue-drift", title: "Deriva del pulso" },
      { id: "fatigue-heart-rate", title: "Pulso antes del error" },
      { id: "fatigue-effort", title: "Esfuerzo percibido" },
    ],
  },
  {
    // Lo que lleva cada tarjeta de Mis atletas (#142).
    label: "Atletas · Tarjetas de Mis atletas",
    panels: [
      { id: "athlete-races", title: "Carreras" },
      { id: "athlete-performance", title: "Rendimiento" },
      { id: "athlete-error-rate", title: "Tasa de error" },
      { id: "athlete-top-error", title: "Error más común" },
      { id: "athlete-trend", title: "Evolución del rendimiento" },
      { id: "athlete-groups", title: "Grupos" },
    ],
  },
  {
    label: "Atletas · Comparar grupos",
    panels: [
      { id: "groups-error-types", title: "Tipos de error de cada grupo" },
      { id: "groups-leg-length", title: "Tasa de error según duración del tramo" },
      { id: "groups-slope", title: "IR medio según desnivel" },
    ],
  },
  {
    label: "Carrera · Resumen",
    panels: [{ id: "race-loss", title: "Pérdida por tramo" }],
  },
  {
    label: "Carrera · Frente al grupo",
    panels: [
      { id: "race-group-ideal", title: "Diferencia con el tiempo ideal" },
      { id: "race-group-loss", title: "Pérdida por tramo comparada" },
    ],
  },
  {
    label: "Carrera · Análisis",
    panels: [
      { id: "race-cumulative", title: "Pérdida acumulada" },
      { id: "race-gain-loss", title: "Dónde gano y dónde pierdo" },
      { id: "race-performance", title: "Rendimiento por tramo" },
      { id: "race-breakdown", title: "¿Lento o desorientado?" },
    ],
  },
];

export interface PanelVisibility {
  hidden: ReadonlySet<string>;
  /** Oculta (`true`) o enseña un panel, y lo guarda. */
  setHidden: (id: string, hidden: boolean) => void;
  showAll: () => void;
  /** Abre el cuadro «Personalizar». */
  customize: () => void;
  error: string | null;
}

/** Sin proveedor, nada se oculta y no se ofrece ocultar. */
export const PanelVisibilityContext = createContext<PanelVisibility | null>(null);

export const usePanelVisibility = () => useContext(PanelVisibilityContext);

/** Los paneles ocultos, cargados de los ajustes, y el cuadro para elegirlos. */
export function PanelVisibilityProvider({ children }: { children: ReactNode }) {
  const [hidden, setHiddenSet] = useState<ReadonlySet<string>>(new Set());
  const [choosing, setChoosing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    hiddenPanels()
      .then((ids) => setHiddenSet(new Set(ids)))
      .catch((err: unknown) => setError(String(err)));
  }, []);

  const save = useCallback((next: ReadonlySet<string>) => {
    setHiddenSet(next);
    setError(null);
    setHiddenPanels([...next]).catch((err: unknown) => setError(String(err)));
  }, []);

  const value: PanelVisibility = {
    hidden,
    setHidden: (id, hide) => {
      const next = new Set(hidden);
      if (hide) next.add(id);
      else next.delete(id);
      save(next);
    },
    showAll: () => save(new Set()),
    customize: () => setChoosing(true),
    error,
  };

  return (
    <PanelVisibilityContext.Provider value={value}>
      {children}
      {choosing && <PanelChooser visibility={value} onClose={() => setChoosing(false)} />}
    </PanelVisibilityContext.Provider>
  );
}

/** ¿Están ocultos todos estos paneles? Para quitar también el título de su apartado. */
export function allHidden(visibility: PanelVisibility | null, ids: string[]): boolean {
  return visibility !== null && ids.every((id) => visibility.hidden.has(id));
}

/** Cuadro «Personalizar»: una casilla por panel, agrupados por pantalla y pestaña. */
function PanelChooser({
  visibility,
  onClose,
}: {
  visibility: PanelVisibility;
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="drawer-backdrop" onClick={onClose}>
      <aside
        className="drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="panel-chooser-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="card-title">
          <h2 id="panel-chooser-title">Personalizar</h2>
          <button
            type="button"
            className="btn btn-ghost btn-icon"
            onClick={onClose}
            aria-label="Cerrar"
            autoFocus
          >
            <CloseIcon />
          </button>
        </div>
        <p className="small muted">
          Elige qué paneles ves. Vale para todas tus carreras y para las que veas de otros
          corredores.
        </p>
        {visibility.error !== null && <Notice kind="error">{visibility.error}</Notice>}
        {PANEL_GROUPS.map((group) => (
          <fieldset key={group.label} className="drawer-group">
            <legend className="eyebrow">{group.label}</legend>
            {group.panels.map((panel) => (
              <label key={panel.id} className="choice">
                <input
                  type="checkbox"
                  checked={!visibility.hidden.has(panel.id)}
                  onChange={(e) => visibility.setHidden(panel.id, !e.target.checked)}
                />
                <span>{panel.title}</span>
              </label>
            ))}
          </fieldset>
        ))}
        <div className="row">
          <button type="button" className="btn btn-primary" onClick={onClose}>
            Hecho
          </button>
          <button type="button" className="btn btn-ghost" onClick={visibility.showAll}>
            Enseñar todos
          </button>
        </div>
      </aside>
    </div>
  );
}
