// Comparar dos grupos de atletas (#121, docs/historico.md, "Comparar grupos"): IR medio, tasa
// de error y pérdida de un grupo frente a otro, y sus tipos de error, duración del tramo (P7) y
// desnivel (P13). Qué pasa con quien está en los dos y qué carreras entran son opciones. Los
// números, también las diferencias, vienen del núcleo; aquí solo se dibujan.
import { useEffect, useState } from "react";
import {
  CompareOptions,
  GroupInfo,
  GroupSide,
  GroupsComparison,
  HistoryFilter,
  SLOPE_LABELS,
  Taxonomy,
  athleteGroups,
  compareAthleteGroups,
  decimal,
  getTaxonomy,
} from "./api";
import { ChartPanel, PanelsOpen } from "./charts/ChartPanel";
import { Legend, SERIES_COLORS } from "./charts/common";
import { GroupedColumnChart } from "./charts/GroupedColumnChart";
import { GroupDot } from "./GroupsScreen";
import { Filters } from "./HistoryScreen";
import { percent, tickPercent } from "./HistoryPanels";
import { bucketLabel } from "./LegLengthPanel";
import { EmptyState, GroupIcon, Notice, PageHeader } from "./ui";

const NO_FILTER: HistoryFilter = { from: null, to: null, format: null };
const START: CompareOptions = { overlap: "count_in_both", races: "shared" };
/** Tipos de error que salen en la gráfica, los más comunes entre los dos grupos. */
const MAX_TYPES = 6;

const pct = (v: number | null) => (v === null ? "—" : percent(v * 100));
/** Diferencia en puntos: «+2,5 puntos» o «−1,0 puntos». */
const points = (v: number | null) =>
  v === null ? "—" : `${v > 0 ? "+" : v < 0 ? "−" : ""}${decimal(Math.abs(v) * 100, 1)} puntos`;
const lossPct = (v: number | null) => (v === null ? "—" : `${decimal(v, 1)} %`);

function CompareGroupsScreen() {
  const [groups, setGroups] = useState<GroupInfo[] | null>(null);
  const [a, setA] = useState<number | null>(null);
  const [b, setB] = useState<number | null>(null);
  const [options, setOptions] = useState<CompareOptions>(START);
  const [filter, setFilter] = useState<HistoryFilter>(NO_FILTER);
  const [view, setView] = useState<GroupsComparison | null>(null);
  const [taxonomy, setTaxonomy] = useState<Taxonomy | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    athleteGroups()
      .then((v) => {
        setGroups(v.groups);
        // De entrada, los dos primeros.
        setA(v.groups[0]?.id ?? null);
        setB(v.groups[1]?.id ?? null);
      })
      .catch((err: unknown) => setError(String(err)));
    getTaxonomy()
      .then(setTaxonomy)
      .catch(() => setTaxonomy(null));
  }, []);

  useEffect(() => {
    if (a === null || b === null) return;
    let current = true;
    setError(null);
    compareAthleteGroups(filter, a, b, options)
      .then((v) => {
        if (current) setView(v);
      })
      .catch((err: unknown) => {
        if (current) setError(String(err));
      });
    return () => {
      current = false;
    };
  }, [a, b, options, filter]);

  const groupA = groups?.find((g) => g.id === a) ?? null;
  const groupB = groups?.find((g) => g.id === b) ?? null;
  const names = [groupA?.name ?? "A", groupB?.name ?? "B"];
  const typeName = (key: string) => taxonomy?.types.find((t) => t.key === key)?.label ?? key;

  return (
    <>
      <PageHeader title="Comparar grupos" subtitle="Un grupo de atletas frente a otro." />
      {error !== null && <Notice kind="error">{error}</Notice>}

      {groups !== null && groups.length < 2 && (
        <div className="card">
          <EmptyState icon={<GroupIcon size={40} />} title="Hacen falta dos grupos">
            <p>Crea al menos dos grupos en Grupos para compararlos.</p>
          </EmptyState>
        </div>
      )}

      {groups !== null && groups.length >= 2 && (
        <>
          <div className="card compare-controls">
            <GroupPicker label="Grupo A" groups={groups} value={a} onChange={setA} />
            <GroupPicker label="Grupo B" groups={groups} value={b} onChange={setB} />
            <Choice
              label="Si alguien está en los dos"
              name="compare-overlap"
              value={options.overlap}
              choices={[
                ["count_in_both", "Cuenta en los dos"],
                ["exclude", "Se deja fuera"],
              ]}
              onChange={(overlap) => setOptions({ ...options, overlap })}
            />
            <Choice
              label="Carreras"
              name="compare-races"
              value={options.races}
              choices={[
                ["shared", "Las de los dos grupos"],
                ["all", "Todas"],
              ]}
              onChange={(races) => setOptions({ ...options, races })}
            />
          </div>
          <Filters filter={filter} onChange={setFilter} />

          {view !== null && (
            <>
              <Notice>
                {view.shared_races === null
                  ? "Cuentan todas las carreras de cada grupo con estos filtros."
                  : `Cuentan solo las carreras que han corrido un atleta de cada grupo: ${view.shared_races === 1 ? "1 carrera" : `${view.shared_races} carreras`}.`}{" "}
                {view.in_both > 0 &&
                  (view.options.overlap === "count_in_both"
                    ? `${view.in_both === 1 ? "1 atleta está" : `${view.in_both} atletas están`} en los dos grupos y cuenta${view.in_both === 1 ? "" : "n"} en los dos.`
                    : `${view.in_both === 1 ? "1 atleta está" : `${view.in_both} atletas están`} en los dos grupos y no cuenta${view.in_both === 1 ? "" : "n"} en ninguno.`)}
              </Notice>
              <SummaryTable view={view} groups={[groupA, groupB]} />
              <PanelsOpen.Provider value={true}>
                <div className="panel-grid">
                  <ErrorTypesPanel view={view} names={names} typeName={typeName} />
                  <LegLengthPanel view={view} names={names} />
                  <SlopePanel view={view} names={names} />
                </div>
              </PanelsOpen.Provider>
            </>
          )}
        </>
      )}
    </>
  );
}

function GroupPicker({
  label,
  groups,
  value,
  onChange,
}: {
  label: string;
  groups: GroupInfo[];
  value: number | null;
  onChange: (id: number) => void;
}) {
  return (
    <label className="field">
      <span className="field-label">{label}</span>
      <select
        className="select"
        value={value ?? ""}
        onChange={(e) => onChange(Number(e.target.value))}
      >
        {groups.map((g) => (
          <option key={g.id} value={g.id}>
            {g.name}
          </option>
        ))}
      </select>
    </label>
  );
}

function Choice<T extends string>({
  label,
  name,
  value,
  choices,
  onChange,
}: {
  label: string;
  name: string;
  value: T;
  choices: [T, string][];
  onChange: (value: T) => void;
}) {
  return (
    <div className="field">
      <span className="field-label" id={`${name}-label`}>
        {label}
      </span>
      <div className="segmented" role="radiogroup" aria-labelledby={`${name}-label`}>
        {choices.map(([key, text]) => (
          <label key={key}>
            <input
              type="radio"
              name={name}
              checked={value === key}
              onChange={() => onChange(key)}
            />
            {text}
          </label>
        ))}
      </div>
    </div>
  );
}

/** Cifras de los dos grupos y su diferencia (A − B). */
function SummaryTable({
  view,
  groups,
}: {
  view: GroupsComparison;
  groups: [GroupInfo | null, GroupInfo | null];
}) {
  const sides: [GroupSide, GroupSide] = [view.a, view.b];
  const rows: { label: string; value: (s: GroupSide) => string; difference?: string }[] = [
    { label: "Atletas", value: (s) => String(s.runners) },
    { label: "Carreras", value: (s) => String(s.stats.races) },
    { label: "Tramos", value: (s) => String(s.stats.legs) },
    {
      label: "IR medio",
      value: (s) => pct(s.stats.mean_performance),
      difference: points(view.performance_difference),
    },
    {
      label: "Tasa de error",
      value: (s) => pct(s.stats.error_rate),
      difference: points(view.error_rate_difference),
    },
    {
      label: "Pérdida media por tramo",
      value: (s) => lossPct(s.stats.mean_loss_pct),
      difference: points(view.loss_pct_difference === null ? null : view.loss_pct_difference / 100),
    },
  ];
  return (
    <div className="card card-flush">
      <div className="table-wrap">
        <table className="table">
          <thead>
            <tr>
              <th />
              {groups.map((g, i) => (
                <th key={i} className="num">
                  <span className="compare-group-name">
                    {g !== null && <GroupDot color={g.color} />}
                    {g?.name ?? (i === 0 ? "A" : "B")}
                  </span>
                </th>
              ))}
              <th className="num">Diferencia (A − B)</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.label}>
                <td>{row.label}</td>
                {sides.map((s, i) => (
                  <td key={i} className="num">
                    {row.value(s)}
                  </td>
                ))}
                <td className="num muted">{row.difference ?? ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="small muted compare-note">
        Cada grupo junta todas las carreras y todos los tramos de sus atletas, cada uno con sus
        umbrales. IR medio: 100 % es ir tan rápido como la referencia del recorrido.
      </p>
    </div>
  );
}

/** Las dos series de una gráfica, con los colores fijos de A y B. */
const series = (names: string[], values: [(number | null)[], (number | null)[]]) =>
  names.map((label, i) => ({
    key: i === 0 ? "a" : "b",
    label,
    color: SERIES_COLORS[i],
    values: values[i],
  }));

const legend = (names: string[]) => (
  <Legend items={names.map((label, i) => ({ label, color: SERIES_COLORS[i] }))} />
);

function ErrorTypesPanel({
  view,
  names,
  typeName,
}: {
  view: GroupsComparison;
  names: string[];
  typeName: (key: string) => string;
}) {
  // Los tipos más comunes entre los dos, por errores sumados.
  const totals = new Map<string, number>();
  for (const t of [...view.a.by_type, ...view.b.by_type]) {
    totals.set(t.error_type, (totals.get(t.error_type) ?? 0) + t.errors);
  }
  const keys = [...totals.entries()]
    .sort((x, y) => y[1] - x[1])
    .slice(0, MAX_TYPES)
    .map(([key]) => key);
  const share = (side: GroupSide, key: string) =>
    side.by_type.find((t) => t.error_type === key) ?? null;
  const value = (side: GroupSide) =>
    keys.map((k) => {
      const t = share(side, k);
      return side.orientation_errors === 0 ? null : (t?.share ?? 0) * 100;
    });
  const cell = (side: GroupSide, key: string) => {
    const t = share(side, key);
    return side.orientation_errors === 0
      ? "—"
      : `${t?.errors ?? 0} (${percent((t?.share ?? 0) * 100)})`;
  };
  return (
    <ChartPanel
      id="groups-error-types"
      title="Tipos de error de cada grupo"
      description="Qué parte de los errores de orientación de cada grupo es de cada tipo (los sin tipo cuentan en el total)."
      cases={`${view.a.orientation_errors} y ${view.b.orientation_errors} errores`}
      chart={
        keys.length === 0 ? (
          <p className="muted">Ningún error con tipo en estos grupos.</p>
        ) : (
          <>
            {legend(names)}
            <GroupedColumnChart
              label="Parte de los errores de cada tipo, en cada grupo"
              xLabels={keys.map(typeName)}
              series={series(names, [value(view.a), value(view.b)])}
              tooltipTitle={(i) => ({ value: typeName(keys[i] ?? ""), detail: "De sus errores" })}
              formatValue={(v) => (v === null ? "—" : percent(v))}
              formatTick={tickPercent}
            />
          </>
        )
      }
      table={
        <table className="table">
          <thead>
            <tr>
              <th>Tipo</th>
              <th className="num">{names[0]}</th>
              <th className="num">{names[1]}</th>
            </tr>
          </thead>
          <tbody>
            {keys.map((k) => (
              <tr key={k}>
                <td>{typeName(k)}</td>
                <td className="num">{cell(view.a, k)}</td>
                <td className="num">{cell(view.b, k)}</td>
              </tr>
            ))}
            <tr>
              <td className="muted">Sin tipo</td>
              <td className="num muted">{view.a.untyped}</td>
              <td className="num muted">{view.b.untyped}</td>
            </tr>
          </tbody>
        </table>
      }
    />
  );
}

function LegLengthPanel({ view, names }: { view: GroupsComparison; names: string[] }) {
  // Los cubos son los mismos en los dos; los de uno que falten en el otro, sin columna.
  const buckets = view.a.by_leg_length.length > 0 ? view.a.by_leg_length : view.b.by_leg_length;
  const rate = (side: GroupSide) =>
    buckets.map((b) => {
      const s = side.by_leg_length.find((x) => x.from_s === b.from_s);
      return s?.error_rate == null ? null : s.error_rate * 100;
    });
  const legs = (side: GroupSide, from: number) =>
    side.by_leg_length.find((x) => x.from_s === from)?.legs ?? 0;
  return (
    <ChartPanel
      id="groups-leg-length"
      title="Tasa de error según duración del tramo"
      description="Errores entre tramos de cada duración de referencia (P7), en cada grupo."
      cases={`${view.a.stats.legs} y ${view.b.stats.legs} tramos`}
      chart={
        <>
          {legend(names)}
          <GroupedColumnChart
            label="Tasa de error de cada grupo por duración del tramo"
            xLabels={buckets.map(bucketLabel)}
            series={series(names, [rate(view.a), rate(view.b)])}
            tooltipTitle={(i) => ({
              value: buckets[i] === undefined ? "" : bucketLabel(buckets[i]),
              detail: "Tasa de error",
            })}
            formatValue={(v) => (v === null ? "Sin tramos" : percent(v))}
            formatTick={tickPercent}
          />
        </>
      }
      table={
        <table className="table">
          <thead>
            <tr>
              <th>Duración</th>
              <th className="num">{names[0]}</th>
              <th className="num">{names[1]}</th>
            </tr>
          </thead>
          <tbody>
            {buckets.map((b, i) => {
              const [ra, rb] = [rate(view.a)[i], rate(view.b)[i]];
              return (
                <tr key={b.from_s}>
                  <td>{bucketLabel(b)}</td>
                  <td className="num">
                    {ra === null ? "—" : percent(ra)}{" "}
                    <span className="muted">(n = {legs(view.a, b.from_s)})</span>
                  </td>
                  <td className="num">
                    {rb === null ? "—" : percent(rb)}{" "}
                    <span className="muted">(n = {legs(view.b, b.from_s)})</span>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      }
    />
  );
}

function SlopePanel({ view, names }: { view: GroupsComparison; names: string[] }) {
  const classes = view.a.by_slope.length > 0 ? view.a.by_slope : view.b.by_slope;
  const ir = (side: GroupSide) =>
    classes.map((c) => {
      const s = side.by_slope.find((x) => x.class === c.class);
      return s?.mean_performance == null ? null : s.mean_performance * 100;
    });
  const stats = (side: GroupSide, i: number) =>
    side.by_slope.find((x) => x.class === classes[i]?.class);
  return (
    <ChartPanel
      id="groups-slope"
      title="IR medio según desnivel"
      description="IR medio de los tramos en subida, llano y bajada (P13), en cada grupo. Solo las carreras con track."
      cases={`${classes.length} clases`}
      chart={
        classes.length === 0 ? (
          <p className="muted">Ninguna carrera con track en estos grupos.</p>
        ) : (
          <>
            {legend(names)}
            <GroupedColumnChart
              label="IR medio de cada grupo en subida, llano y bajada"
              xLabels={classes.map((c) => SLOPE_LABELS[c.class])}
              series={series(names, [ir(view.a), ir(view.b)])}
              tooltipTitle={(i) => ({
                value: classes[i] === undefined ? "" : SLOPE_LABELS[classes[i].class],
                detail: "IR medio",
              })}
              formatValue={(v) => (v === null ? "Sin tramos" : percent(v))}
              formatTick={tickPercent}
            />
          </>
        )
      }
      table={
        <table className="table">
          <thead>
            <tr>
              <th>Desnivel</th>
              <th className="num">{names[0]}</th>
              <th className="num">{names[1]}</th>
            </tr>
          </thead>
          <tbody>
            {classes.map((c, i) => (
              <tr key={c.class}>
                <td>{SLOPE_LABELS[c.class]}</td>
                {[view.a, view.b].map((side, j) => {
                  const s = stats(side, i);
                  return (
                    <td key={j} className="num">
                      IR {pct(s?.mean_performance ?? null)} · error {pct(s?.error_rate ?? null)}{" "}
                      <span className="muted">(n = {s?.legs ?? 0})</span>
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      }
    />
  );
}

export default CompareGroupsScreen;
