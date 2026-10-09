// Resumen en frases (#126, docs/frases.md): unas pocas frases arriba de Estadísticas y de la
// pestaña Resumen de cada carrera. Las escribe el núcleo; aquí solo se enseñan y cada una lleva
// a la vista que la justifica.
import { Insight } from "./api";
import { ChevronRight } from "./ui";

/** Las frases, cada una como enlace a su vista. Sin frases, nada. */
export function Insights<T extends string>({
  insights,
  onOpen,
}: {
  insights: Insight<T>[];
  /** Abre la vista que justifica una frase. */
  onOpen: (target: T) => void;
}) {
  if (insights.length === 0) return null;
  return (
    <section className="card insights" aria-label="Resumen en frases">
      <ul className="insight-list">
        {insights.map((insight) => (
          <li key={insight.rule}>
            <button type="button" className="insight" onClick={() => onOpen(insight.target)}>
              <span className="insight-text">
                {insight.text}
                {insight.caveat !== null && (
                  <span className="pill pill-warning insight-caveat">
                    <span aria-hidden="true">⚠</span> {insight.caveat}
                  </span>
                )}
              </span>
              <span className="insight-go small">
                Ver <ChevronRight size={14} />
              </span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
