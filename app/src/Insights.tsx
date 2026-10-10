// Resumen en frases (#126, docs/frases.md): unas pocas frases arriba de Estadísticas y de la
// pestaña Resumen de cada carrera. Las escribe el núcleo; aquí solo se enseñan y cada una lleva
// al panel concreto que la justifica (#144): abre su pestaña, se desplaza hasta él y lo resalta un
// momento. Si el panel está oculto (#130), avisa con «Enseñar este panel».
import { useEffect, useState } from "react";
import { Insight } from "./api";
import { PANEL_GROUPS, usePanelVisibility } from "./panels";
import { ChevronRight } from "./ui";

/** Dónde está lo que justifica una frase: su pestaña y el panel al que se lleva. */
export interface InsightPlace<Tab extends string> {
  tab: Tab;
  /**
   * Identificadores (`panels.tsx`, o `data-panel` de un bloque que no se oculta) del panel al que
   * se lleva, por orden de preferencia: se va al primero que no esté oculto. Si están todos
   * ocultos, se avisa del primero.
   */
  panels: string[];
}

/** Cuánto dura el resalte del panel. */
const HIGHLIGHT_MS = 2400;
/** El panel puede tardar en salir (se pinta con la pestaña o cuando llegan sus datos). */
const FIND_TRIES = 40;
const FIND_EVERY_MS = 50;

/** Lo que `Insights` necesita para llevar a un panel; sale de [`useInsightFocus`]. */
export interface InsightFocus<Target extends string> {
  /** Abre la pestaña de la frase y se desplaza hasta su panel. */
  open: (target: Target) => void;
  /** Si el panel de la última frase abierta está oculto: su título y cómo enseñarlo. */
  hidden: { title: string; show: () => void } | null;
}

/**
 * Lleva de una frase a su panel. `places` dice dónde está cada destino; `tab` es la pestaña
 * actual y `onTab` la cambia.
 */
export function useInsightFocus<Target extends string, Tab extends string>(
  places: Record<Target, InsightPlace<Tab>>,
  tab: Tab,
  onTab: (tab: Tab) => void,
): InsightFocus<Target> {
  const visibility = usePanelVisibility();
  const hiddenIds = visibility?.hidden;
  // `count` hace que abrir otra vez la misma frase vuelva a desplazarse.
  const [request, setRequest] = useState<{ place: InsightPlace<Tab>; count: number } | null>(null);

  // Una petición vale mientras se esté en su pestaña.
  useEffect(() => {
    setRequest((r) => (r !== null && r.place.tab !== tab ? null : r));
  }, [tab]);

  const isHidden = (id: string) => hiddenIds?.has(id) ?? false;
  const focusId = request?.place.panels.find((id) => !isHidden(id));
  const blockedId =
    request !== null && focusId === undefined ? (request.place.panels[0] ?? null) : null;

  useEffect(() => {
    if (request === null || focusId === undefined) return;
    let tries = 0;
    let finding: number | undefined;
    let fading: number | undefined;
    const find = () => {
      const panel = document.querySelector<HTMLElement>(`[data-panel="${focusId}"]`);
      if (panel === null) {
        tries += 1;
        if (tries < FIND_TRIES) finding = window.setTimeout(find, FIND_EVERY_MS);
        return;
      }
      const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      panel.scrollIntoView({ behavior: still ? "auto" : "smooth", block: "start" });
      panel.classList.add("is-highlighted");
      fading = window.setTimeout(() => panel.classList.remove("is-highlighted"), HIGHLIGHT_MS);
    };
    // Tras pintar la pestaña.
    finding = window.setTimeout(find, 0);
    return () => {
      window.clearTimeout(finding);
      window.clearTimeout(fading);
      document
        .querySelectorAll("[data-panel].is-highlighted")
        .forEach((panel) => panel.classList.remove("is-highlighted"));
    };
  }, [request, focusId]);

  const title =
    blockedId === null
      ? null
      : (PANEL_GROUPS.flatMap((g) => g.panels).find((p) => p.id === blockedId)?.title ?? blockedId);

  return {
    open: (target) => {
      const place = places[target];
      onTab(place.tab);
      setRequest((r) => ({ place, count: (r?.count ?? 0) + 1 }));
    },
    hidden:
      blockedId === null || title === null || visibility === null
        ? null
        : {
            title,
            show: () => {
              visibility.setHidden(blockedId, false);
              // Para que, al salir, se desplace hasta él.
              setRequest((r) => (r === null ? r : { place: r.place, count: r.count + 1 }));
            },
          },
  };
}

/** Las frases, cada una como enlace a su panel. Sin frases, nada. */
export function Insights<T extends string>({
  insights,
  focus,
}: {
  insights: Insight<T>[];
  focus: InsightFocus<T>;
}) {
  if (insights.length === 0) return null;
  return (
    <>
      <section className="card insights" aria-label="Resumen en frases">
        <ul className="insight-list">
          {insights.map((insight) => (
            <li key={insight.rule}>
              <button type="button" className="insight" onClick={() => focus.open(insight.target)}>
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
      {focus.hidden !== null && (
        <div className="notice notice-warning notice-action">
          <div>El panel «{focus.hidden.title}» está oculto, así que esta frase no lo enseña.</div>
          <button type="button" className="btn" onClick={focus.hidden.show}>
            Enseñar este panel
          </button>
        </div>
      )}
    </>
  );
}
