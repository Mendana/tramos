import { useEffect, useRef } from "react";
import { PageHeader } from "../ui";
import { Markdown } from "./markdown";
import { HELP_GROUPS, HELP_PAGES, HelpPageId, helpPagesOf, helpTitle, isHelpPageId } from "./pages";

/** Pantalla Ayuda: índice de páginas a la izquierda y la página abierta (`docs/app.md`). */
function HelpScreen({
  page,
  onOpen,
}: {
  page: HelpPageId;
  /** Abre otra página: es otra pantalla, así que entra en «volver». */
  onOpen: (page: HelpPageId) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  // Una página nueva se lee desde arriba.
  useEffect(() => {
    ref.current?.closest(".content")?.scrollTo(0, 0);
  }, [page]);
  const group = HELP_GROUPS.find((g) => g.id === HELP_PAGES[page].group);
  return (
    <>
      <PageHeader
        title={helpTitle(page)}
        subtitle={page === "indice" ? "Funciona sin conexión." : `Ayuda · ${group?.label ?? ""}`}
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
