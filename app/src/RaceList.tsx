import { FORMAT_LABELS, RaceFormat, RaceRow, SummaryRace, clock, statusLabel } from "./api";
import {
  NO_FILTER,
  PAGE_SIZE,
  RaceFilter,
  RaceSort,
  SORT_LABELS,
  filterRaces,
  isFiltered,
  seasons,
} from "./raceFilter";
import { ChevronRight, EmptyState, FileIcon, PageHeader, TagIcon, WatchIcon } from "./ui";
import { useViewer } from "./viewer";

/** Filtros y página de la lista: los guarda la app para que sigan al volver de una carrera. */
export interface RaceListState {
  filter: RaceFilter;
  /** Desde 1. */
  page: number;
}

export const RACE_LIST_START: RaceListState = { filter: NO_FILTER, page: 1 };

const FORMAT_CHOICES: (RaceFormat | "all" | "none")[] = ["all", "sprint", "middle", "long", "none"];

/**
 * Carreras del usuario, de la más reciente a la más antigua, con búsqueda, filtros, orden y
 * paginación (#128). Una fila abre su carrera.
 */
function RaceList({
  races,
  state = RACE_LIST_START,
  onStateChange,
  onOpen,
  onImport,
  summaryOnly = [],
}: {
  races: RaceRow[] | null;
  state?: RaceListState;
  onStateChange?: (state: RaceListState) => void;
  onOpen: (resultId: number) => void;
  onImport: () => void;
  /** Al ver a un atleta, las carreras que ha compartido solo con el resumen. */
  summaryOnly?: SummaryRace[];
}) {
  const { readOnly, runnerName } = useViewer();
  const count = races?.length ?? 0;
  const setFilter = (filter: RaceFilter) => onStateChange?.({ filter, page: 1 });
  const shown = races === null ? [] : filterRaces(races, state.filter);
  const pages = Math.max(1, Math.ceil(shown.length / PAGE_SIZE));
  const page = Math.min(state.page, pages);
  const pageRows = shown.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);
  if (readOnly && runnerName === null) {
    return (
      <>
        <PageHeader title="Carreras" />
        <div className="card">
          <EmptyState icon={<FileIcon size={40} />} title="Elige un corredor">
            <p>
              Sus carreras llegan por la carpeta compartida. Elige de quién verlas en la barra
              lateral.
            </p>
          </EmptyState>
        </div>
      </>
    );
  }
  return (
    <>
      <PageHeader
        title={runnerName === null ? "Tus carreras" : `Carreras de ${runnerName}`}
        subtitle={
          races === null
            ? "Cargando…"
            : count === 0
              ? undefined
              : `${count} ${count === 1 ? (readOnly ? "carrera compartida" : "carrera importada") : readOnly ? "carreras compartidas" : "carreras importadas"}`
        }
        actions={
          !readOnly &&
          count > 0 && (
            <button type="button" className="btn btn-primary" onClick={onImport}>
              Importar carrera
            </button>
          )
        }
      />

      {readOnly && races !== null && races.length === 0 && summaryOnly.length === 0 && (
        <div className="card">
          <EmptyState icon={<FileIcon size={40} />} title="Aún no ha compartido carreras">
            <p>Aparecerán aquí cuando las deje en la carpeta compartida.</p>
          </EmptyState>
        </div>
      )}

      {!readOnly && races !== null && races.length === 0 && (
        <div className="card">
          <EmptyState icon={<FileIcon size={40} />} title="Aún no hay carreras">
            <p>Importa el .spl de WinSplits de una carrera y, si lo tienes, el FIT de tu reloj.</p>
            <button type="button" className="btn btn-primary btn-lg" onClick={onImport}>
              Importar tu primera carrera
            </button>
          </EmptyState>
        </div>
      )}

      {races !== null && races.length > 0 && (
        <>
          <RaceToolbar races={races} filter={state.filter} onChange={setFilter} />
          {shown.length === 0 ? (
            <div className="card">
              <EmptyState icon={<FileIcon size={40} />} title="Ninguna carrera con estos filtros">
                <button type="button" className="btn" onClick={() => setFilter(NO_FILTER)}>
                  Quitar filtros
                </button>
              </EmptyState>
            </div>
          ) : (
            <div className="card card-flush">
              <div className="table-wrap">
                <table className="table">
                  <thead>
                    <tr>
                      <th>Fecha</th>
                      <th>Carrera</th>
                      <th>Categoría</th>
                      <th>Resultado</th>
                      <th className="num">Tiempo</th>
                      <th className="num">Perdido</th>
                      <th>
                        <span className="visually-hidden">Abrir</span>
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {pageRows.map((race) => (
                      <tr
                        key={race.result_id}
                        className="clickable"
                        tabIndex={0}
                        onClick={() => onOpen(race.result_id)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") onOpen(race.result_id);
                        }}
                      >
                        <td className="num muted">{race.date}</td>
                        <td>
                          <div className="strong">{race.name ?? "Sin nombre"}</div>
                          <div className="meta">
                            {race.format !== null && (
                              <span className="pill pill-accent">{FORMAT_LABELS[race.format]}</span>
                            )}
                            {race.has_track && (
                              <span className="pill" title="Con el FIT del reloj">
                                <WatchIcon size={14} /> Reloj
                              </span>
                            )}
                          </div>
                        </td>
                        <td>{race.class_name}</td>
                        <td>{statusLabel(race.status, race.place)}</td>
                        <td className="num">{clock(race.total_s)}</td>
                        <td className="num">
                          <span className={race.error_count > 0 ? "loss-bad" : undefined}>
                            {clock(race.lost_time_s)}
                          </span>
                          {race.error_count > 0 && (
                            <div className="small muted">
                              {race.error_count} {race.error_count === 1 ? "error" : "errores"}
                            </div>
                          )}
                        </td>
                        <td className="chevron">
                          {!readOnly && race.unreviewed_count > 0 && (
                            <span
                              className="row-flag"
                              title={`${race.unreviewed_count} ${race.unreviewed_count === 1 ? "error" : "errores"} sin revisar`}
                              aria-label={`${race.unreviewed_count} sin revisar`}
                            >
                              <TagIcon size={16} />
                            </span>
                          )}
                          <ChevronRight />
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <Pager
                page={page}
                pages={pages}
                total={shown.length}
                onPage={(p) => onStateChange?.({ ...state, page: p })}
              />
            </div>
          )}
        </>
      )}

      {summaryOnly.length > 0 && <SummaryOnlyRaces races={summaryOnly} />}
    </>
  );
}

/** Búsqueda, formato, temporada, «solo con track» y orden. */
function RaceToolbar({
  races,
  filter,
  onChange,
}: {
  races: RaceRow[];
  filter: RaceFilter;
  onChange: (filter: RaceFilter) => void;
}) {
  const years = seasons(races);
  return (
    <div className="card list-toolbar">
      <input
        className="input list-search"
        type="search"
        placeholder="Buscar carrera o categoría"
        aria-label="Buscar carrera o categoría"
        value={filter.query}
        onChange={(e) => onChange({ ...filter, query: e.target.value })}
      />
      <div className="segmented" role="radiogroup" aria-label="Formato">
        {FORMAT_CHOICES.map((format) => (
          <label key={format}>
            <input
              type="radio"
              name="race-list-format"
              checked={filter.format === format}
              onChange={() => onChange({ ...filter, format })}
            />
            {format === "all" ? "Todas" : format === "none" ? "Sin formato" : FORMAT_LABELS[format]}
          </label>
        ))}
      </div>
      <select
        className="select"
        aria-label="Temporada"
        value={filter.season ?? ""}
        onChange={(e) =>
          onChange({ ...filter, season: e.target.value === "" ? null : Number(e.target.value) })
        }
      >
        <option value="">Todas las temporadas</option>
        {years.map((year) => (
          <option key={year} value={year}>
            {year}
          </option>
        ))}
      </select>
      <select
        className="select"
        aria-label="Orden"
        value={filter.sort}
        onChange={(e) => onChange({ ...filter, sort: e.target.value as RaceSort })}
      >
        {(Object.keys(SORT_LABELS) as RaceSort[]).map((sort) => (
          <option key={sort} value={sort}>
            {SORT_LABELS[sort]}
          </option>
        ))}
      </select>
      <label className="check">
        <input
          type="checkbox"
          checked={filter.trackOnly}
          onChange={(e) => onChange({ ...filter, trackOnly: e.target.checked })}
        />
        Solo con track
      </label>
      {isFiltered(filter) && (
        <button
          type="button"
          className="btn btn-ghost"
          onClick={() => onChange({ ...NO_FILTER, sort: filter.sort })}
        >
          Quitar filtros
        </button>
      )}
    </div>
  );
}

/** «1–25 de 60» y los botones de página. Con una sola página, solo el recuento. */
function Pager({
  page,
  pages,
  total,
  onPage,
}: {
  page: number;
  pages: number;
  total: number;
  onPage: (page: number) => void;
}) {
  const first = (page - 1) * PAGE_SIZE + 1;
  const last = Math.min(total, page * PAGE_SIZE);
  return (
    <div className="pager">
      <span className="small muted num">
        {first}–{last} de {total}
      </span>
      {pages > 1 && (
        <nav className="row" aria-label="Páginas">
          <button
            type="button"
            className="btn"
            disabled={page === 1}
            onClick={() => onPage(page - 1)}
          >
            Anterior
          </button>
          {Array.from({ length: pages }, (_, i) => i + 1).map((p) => (
            <button
              key={p}
              type="button"
              className={p === page ? "btn btn-primary" : "btn"}
              aria-current={p === page ? "page" : undefined}
              onClick={() => onPage(p)}
            >
              {p}
            </button>
          ))}
          <button
            type="button"
            className="btn"
            disabled={page === pages}
            onClick={() => onPage(page + 1)}
          >
            Siguiente
          </button>
        </nav>
      )}
    </div>
  );
}

/** Carreras compartidas solo con el resumen: sin tramos, no se pueden abrir. */
function SummaryOnlyRaces({ races }: { races: SummaryRace[] }) {
  return (
    <section>
      <h3 className="section-title">Solo con el resumen</h3>
      <p className="small muted">
        De estas carreras solo ha compartido el resumen: no se pueden abrir ni entran en el
        histórico.
      </p>
      <div className="card card-flush">
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th>Fecha</th>
                <th>Carrera</th>
                <th>Categoría</th>
                <th>Resultado</th>
                <th className="num">Tiempo</th>
                <th className="num">Perdido</th>
              </tr>
            </thead>
            <tbody>
              {races.map((race) => (
                <tr key={`${race.date}-${race.name ?? ""}-${race.summary.class_name}`}>
                  <td className="num muted">{race.date}</td>
                  <td>
                    <div className="strong">{race.name ?? "Sin nombre"}</div>
                    {race.format !== null && (
                      <div className="meta">
                        <span className="pill pill-accent">{FORMAT_LABELS[race.format]}</span>
                      </div>
                    )}
                  </td>
                  <td>{race.summary.class_name}</td>
                  <td>{statusLabel(race.summary.status, race.summary.place)}</td>
                  <td className="num">{clock(race.summary.total_s)}</td>
                  <td className="num">
                    <span className={race.summary.error_count > 0 ? "loss-bad" : undefined}>
                      {clock(race.summary.lost_time_s)}
                    </span>
                    {race.summary.error_count > 0 && (
                      <div className="small muted">
                        {race.summary.error_count}{" "}
                        {race.summary.error_count === 1 ? "error" : "errores"}
                      </div>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}

export default RaceList;
