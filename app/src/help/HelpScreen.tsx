import { useEffect, useRef } from "react";
import { PageHeader } from "../ui";
import { Markdown } from "./markdown";
import { HELP_GROUPS, HELP_PAGES, HelpPageId, helpPagesOf, helpTitle, isHelpPageId } from "./pages";

/** Cuánto se queda marcada la sección a la que se llega desde un «?» (ms). */
const TARGET_MS = 2500;

/** Pantalla Ayuda: índice de páginas a la izquierda y la página abierta (`docs/app.md`). */
function HelpScreen({
  page,
  section,
  onOpen,
  onTour,
}: {
  page: HelpPageId;
  /** Título de la sección a la que bajar (el «?» de una gráfica); sin él, se lee desde arriba. */
  section?: string;
  /** Abre otra página: es otra pantalla, así que entra en «volver». */
  onOpen: (page: HelpPageId) => void;
  /** Repite el recorrido guiado de la primera vez (#141); se ofrece en la portada. */
  onTour: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  // Una página nueva se lee desde arriba; con `section`, baja a ella, la marca un momento y le
  // lleva el foco para que el teclado y el lector de pantalla sigan desde ahí.
  useEffect(() => {
    ref.current?.closest(".content")?.scrollTo(0, 0);
    if (section === undefined) return;
    const headings = ref.current?.querySelectorAll<HTMLElement>(".help-doc h2, .help-doc h3");
    const heading = [...(headings ?? [])].find((h) => h.textContent === section);
    if (heading === undefined) return;
    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    heading.tabIndex = -1;
    heading.scrollIntoView({ behavior: still ? "auto" : "smooth", block: "start" });
    heading.focus({ preventScroll: true });
    heading.classList.add("is-target");
    const fading = window.setTimeout(() => heading.classList.remove("is-target"), TARGET_MS);
    return () => {
      window.clearTimeout(fading);
      heading.classList.remove("is-target");
    };
  }, [page, section]);
  const group = HELP_GROUPS.find((g) => g.id === HELP_PAGES[page].group);
  return (
    <>
      <PageHeader
        title={helpTitle(page)}
        subtitle={page === "indice" ? "Funciona sin conexión." : `Ayuda · ${group?.label ?? ""}`}
        actions={
          page === "indice" ? (
            <button type="button" className="btn" onClick={onTour}>
              Ver el recorrido guiado
            </button>
          ) : undefined
        }
      />
      <div className="with-index" ref={ref}>
        <nav className="page-index help-index" aria-label="Páginas de ayuda">
          {HELP_GROUPS.map((g) => (
            <div key={g.id} className="help-index-group">
              <span className="help-index-label">{g.label}</span>
              {helpPagesOf(g.id).map((id) => (
                <a
                  key={id}
                  className="page-index-link"
                  href={`#ayuda-${id}`}
                  aria-current={id === page ? "page" : undefined}
                  onClick={(e) => {
                    e.preventDefault();
                    if (id !== page) onOpen(id);
                  }}
                >
                  {helpTitle(id)}
                </a>
              ))}
            </div>
          ))}
        </nav>
        <div className="with-index-body">
          <article className="card help-doc">
            <Markdown
              source={HELP_PAGES[page].source}
              withoutTitle
              isPage={isHelpPageId}
              onOpenPage={(id) => {
                if (isHelpPageId(id) && id !== page) onOpen(id);
              }}
            />
          </article>
        </div>
      </div>
    </>
  );
}

export default HelpScreen;
