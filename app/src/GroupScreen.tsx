// Vista de grupo (P15, docs/historico.md): quien entrena compara a sus atletas. Una fila por
// atleta con lo principal de su histórico y todos contra todos en las carreras que comparten. Con
// «Incluirme» (#119), sus carreras propias cuentan como un atleta más. Se puede limitar a un grupo
// de atletas (#120), con una fila de totales.
// Los números vienen del núcleo; aquí solo se dibujan.
import { useEffect, useState } from "react";
import {
  FORMAT_LABELS,
  GroupInfo,
  GroupView,
  HistoryFilter,
  SLOPE_LABELS,
  SlopeClass,
  Taxonomy,
  decimal,
  getTaxonomy,
  athleteGroups,
  groupView,
  setIncludeSelf,
} from "./api";
import { GroupDot } from "./GroupsScreen";
import { Filters } from "./HistoryScreen";
import { percent } from "./HistoryPanels";
import { bucketLabel } from "./LegLengthPanel";
import { EmptyState, GroupIcon, Notice, PageHeader } from "./ui";

const NO_FILTER: HistoryFilter = { from: null, to: null, format: null };
const SLOPE_CLASSES = Object.keys(SLOPE_LABELS) as SlopeClass[];

const ir = (v: number | null) => (v === null ? "—" : percent(v * 100));
const rate = (v: number | null) => (v === null ? "—" : percent(v * 100));
/** Diferencia de IR en puntos: «+2,5» o «−1,0». */
const points = (v: number) => `${v > 0 ? "+" : v < 0 ? "−" : ""}${decimal(Math.abs(v) * 100, 1)}`;

function GroupScreen({
  group,
  onGroup,
  onOpenRunner,
}: {
  /** El grupo de atletas al que se limita; `null` = todos. */
  group: number | null;
  onGroup: (group: number | null) => void;
  /** Abre las carreras de un atleta; `null` = las propias. */
  onOpenRunner: (runnerId: string | null) => void;
}) {
  const [groups, setGroups] = useState<GroupInfo[]>([]);
  const [filter, setFilter] = useState<HistoryFilter>(NO_FILTER);
  const [view, setView] = useState<GroupView | null>(null);
  // Sube al cambiar «Incluirme», para volver a pedir la vista.
  const [revision, setRevision] = useState(0);
  const [taxonomy, setTaxonomy] = useState<Taxonomy | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getTaxonomy()
      .then(setTaxonomy)
      .catch(() => setTaxonomy(null));
    athleteGroups()
      .then((v) => setGroups(v.groups))
      .catch((err: unknown) => setError(String(err)));
  }, []);

  useEffect(() => {
    let current = true;
    setError(null);
    groupView(filter, group)
      .then((v) => {
        if (current) setView(v);
      })
      .catch((err: unknown) => {
        if (current) setError(String(err));
      });
    return () => {
      current = false;
    };
  }, [filter, revision, group]);

  const includeSelf = (include: boolean) => {
    setIncludeSelf(include)
      .then(() => setRevision((r) => r + 1))
      .catch((err: unknown) => setError(String(err)));
  };

  const names =
    view?.runners.map((r) =>
      r.is_self
        ? r.runner.display_name === ""
          ? "Tú"
          : `${r.runner.display_name} (tú)`
        : r.runner.display_name || "Sin nombre",
    ) ?? [];
  const athletes = view?.runners.filter((r) => !r.is_self).length ?? 0;
  const selected = groups.find((g) => g.id === group) ?? null;
  const me = view?.runners.some((r) => r.is_self) ?? false;
  const shared = view?.comparison.shared_races.length ?? 0;
  const typeName = (key: string) => taxonomy?.types.find((t) => t.key === key)?.label ?? key;

  return (
    <>
      <PageHeader
        title={selected === null ? "Comparar atletas" : selected.name}
        subtitle={
          view === null
            ? "Cargando…"
            : `${athletes} ${athletes === 1 ? "atleta" : "atletas"}${me ? " y tú" : ""} · ${shared} ${shared === 1 ? "carrera compartida" : "carreras compartidas"}`
        }
      />
      {error !== null && <Notice kind="error">{error}</Notice>}
      {selected !== null && selected.description !== "" && (
        <p className="muted">{selected.description}</p>
      )}

      {view !== null && group === null && athletes === 0 && (
        <div className="card">
          <EmptyState icon={<GroupIcon size={40} />} title="Aún no hay atletas">
            <p>Aparecerán cuando dejen sus carreras en la carpeta compartida.</p>
          </EmptyState>
        </div>
      )}

      {view !== null && (group !== null || athletes > 0) && (
        <>
          <Filters filter={filter} onChange={setFilter} />
          <div className="group-select">
            {groups.length > 0 && (
              <label className="field">
                <span className="field-label">Grupo</span>
                <select
                  className="select"
                  value={group ?? ""}
                  onChange={(e) => onGroup(e.target.value === "" ? null : Number(e.target.value))}
                >
                  <option value="">Todos los atletas</option>
                  {groups.map((g) => (
                    <option key={g.id} value={g.id}>
                      {g.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {selected !== null && <GroupDot color={selected.color} size={14} />}
            {group === null ? (
              <label className="check">
                <input
                  type="checkbox"
                  checked={view.include_self}
                  onChange={(e) => includeSelf(e.target.checked)}
                />
                Incluirme: mis carreras cuentan como un atleta más
              </label>
            ) : (
              <span className="small muted">Salen sus miembros; tú, si estás en el grupo.</span>
            )}
          </div>
          {group !== null && view.runners.length === 0 && (
            <Notice>
              Ningún miembro de este grupo tiene carreras con tramos con estos filtros.
            </Notice>
          )}

          {view.runners.length > 0 && (
            <>
              <section>
                <h3 className="section-title">Atletas</h3>
                <p className="small muted">
                  Cada uno con sus umbrales y solo con las carreras que ha compartido con sus
                  tramos. IR medio: 100 % es ir tan rápido como la referencia del recorrido.
                </p>
                <div className="card card-flush">
                  <div className="table-wrap">
                    <table className="table">
                      <thead>
                        <tr>
                          <th>Atleta</th>
                          <th className="num">Carreras</th>
                          <th className="num">IR medio</th>
                          <th className="num">Tasa de error</th>
                          <th className="num">Pérdida media</th>
                          <th>Error más común</th>
                          <th>Más errores en tramos de</th>
                          {SLOPE_CLASSES.map((c) => (
                            <th key={c} className="num">
                              IR {SLOPE_LABELS[c].toLowerCase()}
                            </th>
                          ))}
                        </tr>
                      </thead>
                      <tbody>
                        {view.runners.map((r, i) => {
                          const row = r.row;
                          return (
                            <tr
                              key={r.runner.runner_id}
                              className="clickable"
                              tabIndex={0}
                              title={
                                r.is_self ? "Ver mis carreras" : `Ver las carreras de ${names[i]}`
                              }
                              onClick={() => onOpenRunner(r.is_self ? null : r.runner.runner_id)}
                              onKeyDown={(e) => {
                                if (e.key === "Enter")
                                  onOpenRunner(r.is_self ? null : r.runner.runner_id);
                              }}
                            >
                              <td className="strong">{names[i]}</td>
                              {row === null ? (
                                <td colSpan={6 + SLOPE_CLASSES.length} className="muted">
                                  {r.problem ?? "Sin datos"}
                                </td>
                              ) : (
                                <>
                                  <td className="num">{row.stats.races}</td>
                                  <td className="num">{ir(row.stats.mean_performance)}</td>
                                  <td className="num">{rate(row.stats.error_rate)}</td>
                                  <td className="num">
                                    {row.stats.mean_loss_pct === null
                                      ? "—"
                                      : `${decimal(row.stats.mean_loss_pct, 1)} %`}
                                  </td>
                                  <td>
                                    {row.top_error === null
                                      ? "—"
                                      : `${typeName(row.top_error.error_type)} (${percent(row.top_error.share * 100)})`}
                                  </td>
                                  <td>
                                    {row.weakest_leg_length === null
                                      ? "—"
                                      : `${bucketLabel(row.weakest_leg_length)} (${rate(row.weakest_leg_length.error_rate)})`}
                                  </td>
                                  {row.slope.map((s) => (
                                    <td key={s.class} className="num">
                                      {s.legs === 0 ? "—" : ir(s.mean_performance)}
                                    </td>
                                  ))}
                                </>
                              )}
                            </tr>
                          );
                        })}
                      </tbody>
                      {view.runners.length > 1 && (
                        <tfoot>
                          <tr className="total-row">
                            <td className="strong">
                              {selected === null ? "Todos" : `Total de ${selected.name}`}
                            </td>
                            <td className="num">{view.total.races}</td>
                            <td className="num">{ir(view.total.mean_performance)}</td>
                            <td className="num">{rate(view.total.error_rate)}</td>
                            <td className="num">
                              {view.total.mean_loss_pct === null
                                ? "—"
                                : `${decimal(view.total.mean_loss_pct, 1)} %`}
                            </td>
                            <td colSpan={2 + SLOPE_CLASSES.length} />
                          </tr>
                        </tfoot>
                      )}
                    </table>
                  </div>
                </div>
                <p className="small muted">
                  {view.runners.length > 1 &&
                    "La última fila junta todas sus carreras y todos sus tramos: el IR medio de todas las carreras y la tasa de error y la pérdida media de todos los tramos. "}
                  Pérdida media: la de los errores, en % del tiempo esperado, repartida entre todos
                  los tramos. Error más común: el tipo que más ha etiquetado, sobre sus errores. Más
                  errores en tramos de: la duración con más tasa de error, si tiene al menos 10
                  tramos. IR por desnivel: solo con las carreras con track.
                </p>
              </section>

              {names.length > 1 && <HeadToHeadTable view={view} names={names} />}

              {shared > 0 && (
                <section>
                  <h3 className="section-title">Carreras compartidas</h3>
                  <div className="card card-flush">
                    <div className="table-wrap">
                      <table className="table">
                        <thead>
                          <tr>
                            <th>Fecha</th>
                            <th>Carrera</th>
                            <th>Por IR</th>
                          </tr>
                        </thead>
                        <tbody>
                          {view.comparison.shared_races.map((race) => (
                            <tr key={race.race_id}>
                              <td className="num muted">{race.date}</td>
                              <td>
                                <div className="strong">{race.name ?? "Sin nombre"}</div>
                                {race.format !== null && (
                                  <div className="meta">
                                    <span className="pill pill-accent">
                                      {FORMAT_LABELS[race.format]}
                                    </span>
                                  </div>
                                )}
                              </td>
                              <td>
                                <ol className="shared-results">
                                  {race.results.map((r) => (
                                    <li key={r.runner}>
                                      <span className="strong">{names[r.runner]}</span>{" "}
                                      <span className="num">{ir(r.stats.mean_performance)}</span>
                                      <span className="small muted">
                                        {" "}
                                        · {r.stats.errors}{" "}
                                        {r.stats.errors === 1 ? "error" : "errores"}
                                      </span>
                                    </li>
                                  ))}
                                </ol>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                </section>
              )}
            </>
          )}
        </>
      )}
    </>
  );
}

/** Todos contra todos: en cuántas carreras compartidas tuvo cada uno más IR que otro. */
function HeadToHeadTable({ view, names }: { view: GroupView; names: string[] }) {
  const pair = (runner: number, other: number) =>
    view.comparison.head_to_head.find((p) => p.runner === runner && p.other === other);
  return (
    <section>
      <h3 className="section-title">Cara a cara</h3>
      <p className="small muted">
        En las carreras que han corrido los dos (aunque sea en otra categoría): cuántas veces tuvo
        el de la fila más IR que el de la columna, cuántas menos y, debajo, la diferencia media de
        IR en puntos.
      </p>
      <div className="card card-flush">
        <div className="table-wrap">
          <table className="table head-to-head">
            <thead>
              <tr>
                <th>
                  <span className="visually-hidden">Atleta</span>
                </th>
                {names.map((name, j) => (
                  <th key={j} className="num">
                    {name}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {names.map((name, i) => (
                <tr key={i}>
                  <th scope="row">{name}</th>
                  {names.map((_, j) => {
                    if (i === j) {
                      return (
                        <td key={j} className="num muted">
                          ·
                        </td>
                      );
                    }
                    const p = pair(i, j);
                    return (
                      <td key={j} className="num">
                        {p === undefined ? (
                          <span className="muted">—</span>
                        ) : (
                          <>
                            <div
                              className={
                                p.better > p.worse
                                  ? "loss-good"
                                  : p.better < p.worse
                                    ? "loss-bad"
                                    : undefined
                              }
                            >
                              {p.better}–{p.worse}
                            </div>
                            <div className="small muted">{points(p.mean_difference)}</div>
                          </>
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}

export default GroupScreen;
