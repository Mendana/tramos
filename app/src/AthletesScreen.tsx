// Mis atletas (#142, docs/app.md, "Atletas"): una tarjeta por atleta con lo principal de su
// histórico, las carreras nuevas y sus grupos, con búsqueda y filtro por grupo. Un clic en la
// tarjeta entra en el atleta, en solo lectura. Qué lleva cada tarjeta se elige en «Personalizar»
// (los mismos paneles ocultos de `panels.tsx`).
// Los números vienen del núcleo (los de su fila en Comparar atletas); aquí solo se dibujan.
import { useEffect, useState } from "react";
import {
  AthleteCard,
  GroupInfo,
  Taxonomy,
  TrendPoint,
  athleteCards,
  athleteGroups,
  getTaxonomy,
} from "./api";
import { GroupDot } from "./GroupsScreen";
import { percent } from "./HistoryPanels";
import { usePanelVisibility } from "./panels";
import { EmptyState, GroupIcon, Notice, PageHeader } from "./ui";

const ir = (v: number | null | undefined) =>
  v === null || v === undefined ? "—" : percent(v * 100);

/** Para buscar sin mayúsculas ni tildes: «Íñigo» lo encuentra «inigo». */
const fold = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLocaleLowerCase("es");

export const newRacesLabel = (n: number) => `${n} ${n === 1 ? "nueva" : "nuevas"}`;

/** Línea con el rendimiento de las últimas carreras, sin ejes: la forma, no los números. */
function Sparkline({ points }: { points: TrendPoint[] }) {
  const width = 120;
  const height = 36;
  const pad = 4;
  const values = points.map((p) => p.performance);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const x = (i: number) =>
    points.length === 1 ? width / 2 : pad + ((width - 2 * pad) * i) / (points.length - 1);
  const y = (v: number) =>
    max === min ? height / 2 : pad + (height - 2 * pad) * (1 - (v - min) / (max - min));
  const first = values[0];
  const last = values[values.length - 1];
  const label =
    points.length === 1
      ? `Rendimiento de su única carrera: ${ir(last)}`
      : `Rendimiento de sus ${points.length} últimas carreras: de ${ir(first)} a ${ir(last)}`;
  return (
    <svg
      className="spark"
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      role="img"
      aria-label={label}
    >
      <title>{label}</title>
      {points.length > 1 && (
        <polyline
          className="chart-line spark-line"
          points={values.map((v, i) => `${x(i)},${y(v)}`).join(" ")}
        />
      )}
      {points.map((p, i) => (
        <circle
          key={i}
          className={i === points.length - 1 ? "spark-marker spark-last" : "spark-marker"}
          cx={x(i)}
          cy={y(p.performance)}
          r={i === points.length - 1 ? 3.5 : 2}
        >
          <title>{`${p.date} · ${p.name ?? "Sin nombre"}: ${ir(p.performance)}`}</title>
        </circle>
      ))}
    </svg>
  );
}

function AthletesScreen({ onOpenRunner }: { onOpenRunner: (runnerId: string) => void }) {
  const [cards, setCards] = useState<AthleteCard[] | null>(null);
  const [groups, setGroups] = useState<GroupInfo[]>([]);
  const [taxonomy, setTaxonomy] = useState<Taxonomy | null>(null);
  const [query, setQuery] = useState("");
  // Grupo por el que se filtra; `null` = todos.
  const [group, setGroup] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const visibility = usePanelVisibility();
  const shows = (id: string) => visibility === null || !visibility.hidden.has(id);

  useEffect(() => {
    athleteCards()
      .then(setCards)
      .catch((err: unknown) => setError(String(err)));
    athleteGroups()
      .then((v) => setGroups(v.groups))
      .catch((err: unknown) => setError(String(err)));
    getTaxonomy()
      .then(setTaxonomy)
      .catch(() => setTaxonomy(null));
  }, []);

  const typeName = (key: string) => taxonomy?.types.find((t) => t.key === key)?.label ?? key;
  const found = (cards ?? []).filter(
    (c) =>
      fold(c.runner.display_name).includes(fold(query.trim())) &&
      (group === null || c.groups.includes(group)),
  );
  const count = cards?.length ?? 0;
  const showStats = ["athlete-races", "athlete-performance", "athlete-error-rate"].some(shows);

  return (
    <>
      <PageHeader
        title="Mis atletas"
        subtitle={
          cards === null
            ? "Cargando…"
            : `${count} ${count === 1 ? "atleta te comparte" : "atletas te comparten"} sus carreras por la carpeta compartida.`
        }
        actions={
          visibility !== null && (
            <button type="button" className="btn btn-ghost" onClick={visibility.customize}>
              Personalizar
            </button>
          )
        }
      />
      {error !== null && <Notice kind="error">{error}</Notice>}

      {cards !== null && count === 0 && (
        <div className="card">
          <EmptyState icon={<GroupIcon size={40} />} title="Aún no hay atletas">
            <p>Aparecerán cuando dejen sus carreras en la carpeta compartida.</p>
          </EmptyState>
        </div>
      )}

      {count > 0 && (
        <>
          <div className="athletes-toolbar">
            <input
              className="input list-search"
              type="search"
              placeholder="Buscar atleta"
              aria-label="Buscar atleta"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            {groups.length > 0 && (
              <div className="chips" role="group" aria-label="Grupo">
                <button
                  type="button"
                  className="chip"
                  aria-pressed={group === null}
                  onClick={() => setGroup(null)}
                >
                  Todos
                </button>
                {groups.map((g) => (
                  <button
                    key={g.id}
                    type="button"
                    className="chip athletes-chip"
                    aria-pressed={group === g.id}
                    onClick={() => setGroup(g.id)}
                  >
                    <GroupDot color={g.color} size={10} />
                    {g.name}
                  </button>
                ))}
              </div>
            )}
          </div>

          {found.length === 0 && <p className="muted">Ningún atleta con esa búsqueda.</p>}

          <div className="athlete-cards">
            {found.map((card) => {
              const name = card.runner.display_name || "Sin nombre";
              const stats = card.row?.stats;
              const top = card.row?.top_error ?? null;
              const memberOf = groups.filter((g) => card.groups.includes(g.id));
              return (
                <button
                  key={card.runner.runner_id}
                  type="button"
                  className="card athlete-card"
                  title={`Ver las carreras de ${name}`}
                  onClick={() => onOpenRunner(card.runner.runner_id)}
                >
                  <span className="athlete-card-head">
                    <span className="athlete-card-name">
                      <span className="strong">
                        {name}{" "}
                        {card.new_races > 0 && (
                          <span className="pill pill-accent">{newRacesLabel(card.new_races)}</span>
                        )}
                      </span>
                      <span className="small muted">
                        {card.last_race === null
                          ? "Sin carreras"
                          : `Última carrera: ${card.last_race}`}
                      </span>
                    </span>
                    {shows("athlete-trend") && card.trend.length > 0 && (
                      <Sparkline points={card.trend} />
                    )}
                  </span>

                  {card.problem !== null && (
                    <span className="small muted">
                      No se han podido calcular sus números: {card.problem}
                    </span>
                  )}

                  {stats !== undefined && showStats && (
                    <span className="athlete-card-stats">
                      {shows("athlete-races") && (
                        <span className="athlete-card-stat">
                          <span className="athlete-card-value num">{stats.races}</span>
                          <span className="small muted">
                            {stats.races === 1 ? "carrera" : "carreras"}
                          </span>
                        </span>
                      )}
                      {shows("athlete-performance") && (
                        <span className="athlete-card-stat">
                          <span className="athlete-card-value num">
                            {ir(stats.mean_performance)}
                          </span>
                          <span className="small muted">rendimiento</span>
                        </span>
                      )}
                      {shows("athlete-error-rate") && (
                        <span className="athlete-card-stat">
                          <span className="athlete-card-value num">{ir(stats.error_rate)}</span>
                          <span className="small muted">tasa de error</span>
                        </span>
                      )}
                    </span>
                  )}

                  {card.row !== null && shows("athlete-top-error") && (
                    <span className="small">
                      <span className="muted">Error más común: </span>
                      {top === null
                        ? "—"
                        : `${typeName(top.error_type)} (${percent(top.share * 100)} de sus errores)`}
                    </span>
                  )}

                  {shows("athlete-groups") && memberOf.length > 0 && (
                    <span className="chips">
                      {memberOf.map((g) => (
                        <span key={g.id} className="pill">
                          <GroupDot color={g.color} size={8} />
                          {g.name}
                        </span>
                      ))}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
          <p className="small muted">
            Cifras de todas sus carreras con tramos, como su fila en Comparar atletas. Rendimiento:
            el IR medio (100 % es ir tan rápido como la referencia). Error más común: el tipo que
            más ha etiquetado. La línea, su rendimiento en las 10 últimas carreras. Nuevas: las
            recibidas desde la última vez que entraste en el atleta.
          </p>
        </>
      )}
    </>
  );
}

export default AthletesScreen;
