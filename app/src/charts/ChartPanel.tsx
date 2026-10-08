// Panel de una gráfica: título, número de casos, la gráfica y su tabla de datos (la vista
// accesible de la gráfica: todo valor está también en la tabla). Desplegable salvo dentro de
// `PanelsOpen`, donde está siempre abierto (pestañas de la vista de carrera, #129).
import { ReactNode, createContext, useContext, useId, useState } from "react";
import { ChevronRight } from "../ui";

/** Con `true`, los paneles de dentro están siempre abiertos y sin desplegable. */
export const PanelsOpen = createContext(false);

export function ChartPanel({
  title,
  description,
  cases,
  chart,
  table,
  defaultOpen = false,
}: {
  title: string;
  /** Qué enseña y cómo leerla, en una frase. */
  description: string;
  /** Número de casos en los que se apoya (tramos, carreras…), con su unidad. */
  cases: string;
  chart: ReactNode;
  table: ReactNode;
  defaultOpen?: boolean;
}) {
  const alwaysOpen = useContext(PanelsOpen);
  const [toggled, setOpen] = useState(defaultOpen);
  const open = alwaysOpen || toggled;
  const [view, setView] = useState<"chart" | "table">("chart");
  const bodyId = useId();
  return (
    <section className={open ? "chart-panel is-open" : "chart-panel"}>
      {alwaysOpen ? (
        <div className="chart-panel-header is-static">
          <h3 className="chart-panel-title">{title}</h3>
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
      {open && (
        <div className="chart-panel-body" id={bodyId}>
          <div className="chart-panel-toolbar">
            <p className="small muted">{description}</p>
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
