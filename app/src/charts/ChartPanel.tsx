// Panel desplegable de una gráfica: título, número de casos, la gráfica y su tabla de datos
// (la vista accesible de la gráfica: todo valor está también en la tabla).
import { ReactNode, useId, useState } from "react";
import { ChevronRight } from "../ui";

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
  const [open, setOpen] = useState(defaultOpen);
  const [view, setView] = useState<"chart" | "table">("chart");
  const bodyId = useId();
  return (
    <section className={open ? "chart-panel is-open" : "chart-panel"}>
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
