// Panel de una gráfica: título, número de casos, la gráfica y su tabla de datos (la vista
// accesible de la gráfica: todo valor está también en la tabla). Desplegable salvo dentro de
// `PanelsOpen`, donde está siempre abierto (pestañas de la vista de carrera, #129). La explicación
// fija va en un «?» junto al título (tooltip y enlace a la ayuda); a la vista solo quedan los
// avisos que dependen de los datos (`note`).
import { ReactNode, createContext, useContext, useId, useState } from "react";
import { HelpTarget } from "../help/pages";
import { useOpenHelp } from "../help/open";
import { usePanelVisibility } from "../panels";
import { ChevronRight, CloseIcon, HelpTip } from "../ui";

/** Con `true`, los paneles de dentro están siempre abiertos y sin desplegable. */
export const PanelsOpen = createContext(false);

export function ChartPanel({
  id,
  title,
  description,
  note,
  help,
  cases,
  chart,
  table,
  defaultOpen = false,
}: {
  /** Identificador estable (`panels.tsx`): con él, el panel se puede ocultar. */
  id?: string;
  title: string;
  /** Qué enseña y cómo leerla: la explicación fija, que sale en el tooltip del «?». */
  description: string;
  /**
   * Aviso corto sobre los datos de este caso (falta altitud, un resumen de lo que sale…): se ve
   * siempre, porque cambia cómo se leen los números. Nunca la explicación general.
   */
  note?: ReactNode;
  /** Sección de la ayuda que explica esta gráfica: adonde lleva el «?». */
  help: HelpTarget;
  /** Número de casos en los que se apoya (tramos, carreras…), con su unidad. */
  cases: string;
  chart: ReactNode;
  table: ReactNode;
  defaultOpen?: boolean;
}) {
  const alwaysOpen = useContext(PanelsOpen);
  const visibility = usePanelVisibility();
  const [toggled, setOpen] = useState(defaultOpen);
  const open = alwaysOpen || toggled;
  const [view, setView] = useState<"chart" | "table">("chart");
  const bodyId = useId();
  const openHelp = useOpenHelp();
  if (id !== undefined && visibility?.hidden.has(id)) return null;
  const hide =
    id !== undefined && visibility !== null ? (
      <button
        type="button"
        className="btn btn-ghost btn-icon chart-panel-hide"
        onClick={() => visibility.setHidden(id, true)}
        aria-label={`Ocultar «${title}»`}
        title="Ocultar este panel (vuelve desde Personalizar)"
      >
        <CloseIcon size={16} />
      </button>
    ) : null;
  const helpTip = (
    <HelpTip
      label={`Qué enseña «${title}»: abre la ayuda`}
      text={description}
      onOpen={openHelp === null ? undefined : () => openHelp(help)}
    />
  );
  return (
    <section className={open ? "chart-panel is-open" : "chart-panel"} data-panel={id}>
      <div className="chart-panel-head">
        {alwaysOpen ? (
          <div className="chart-panel-header is-static">
            <h3 className="chart-panel-title">{title}</h3>
            {helpTip}
            <span className="pill">{cases}</span>
          </div>
        ) : (
          <button
            type="button"
            className="chart-panel-header"
            aria-expanded={open}
            aria-controls={bodyId}
            onClick={() => setOpen(!open)}
          >
            <span className="chart-panel-chevron">
              <ChevronRight size={16} />
            </span>
            <span className="chart-panel-title">{title}</span>
            <span className="pill">{cases}</span>
          </button>
        )}
        {!alwaysOpen && helpTip}
        {hide}
      </div>
      {open && (
        <div className="chart-panel-body" id={bodyId}>
          <div className="chart-panel-toolbar">
            {note !== undefined && <p className="small muted chart-panel-note">{note}</p>}
            <div className="segmented" role="radiogroup" aria-label="Vista">
              <label>
                <input
                  type="radio"
                  name={`${bodyId}-view`}
                  checked={view === "chart"}
                  onChange={() => setView("chart")}
                />
                Gráfica
              </label>
              <label>
                <input
                  type="radio"
                  name={`${bodyId}-view`}
                  checked={view === "table"}
                  onChange={() => setView("table")}
                />
                Tabla
              </label>
            </div>
          </div>
          {view === "chart" ? chart : <div className="table-wrap">{table}</div>}
        </div>
      )}
    </section>
  );
}
