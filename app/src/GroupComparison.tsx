// Frente al grupo (P4, `docs/app.md`, "Gráficas"): el corredor junto a compañeros elegidos de su
// mismo recorrido, en la diferencia acumulada respecto al tiempo ideal y en la pérdida por tramo.
// Los números salen del núcleo (`race_comparison`); aquí solo se eligen y se dibujan.
import { useEffect, useState } from "react";
import {
  ComparedRunner,
  CourseComparison,
  clock,
  codeLabel,
  raceComparison,
  signed,
} from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { GroupedColumnChart } from "./charts/GroupedColumnChart";
import { MultiLineChart } from "./charts/MultiLineChart";
import { SERIES_COLORS } from "./charts/common";
import { CloseIcon, Notice } from "./ui";

/** Compañeros a la vez, además del corredor: una serie por color de la paleta. */
const MAX_OTHERS = SERIES_COLORS.length - 1;

const keyOf = (r: ComparedRunner) => `${r.result.class_index}-${r.result.result_index}`;
const fullName = (r: ComparedRunner) => `${r.given_name} ${r.family_name}`.trim();
const shortName = (r: ComparedRunner) =>
  r.is_self ? "Tú" : `${r.given_name} ${r.family_name.charAt(0)}.`.trim();

/** Diferencia con el ideal: `+1:23` por detrás, `-0:30` por delante. */
const behind = (v: number | null) => (v === null ? "—" : `${v > 0 ? "+" : ""}${clock(v)}`);
const loss = (v: number | null) => (v === null ? "—" : `${signed(v)} s`);

/** Por defecto, el ganador del recorrido (o el segundo, si el ganador es el corredor). */
function defaultOthers(runners: ComparedRunner[]): string[] {
  const first = runners.find((r) => !r.is_self && r.course_place !== null);
  return first === undefined ? [] : [keyOf(first)];
}

export function GroupComparison({ resultId }: { resultId: number }) {
  const [comparison, setComparison] = useState<CourseComparison | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [others, setOthers] = useState<string[]>([]);

  useEffect(() => {
    setComparison(null);
    setError(null);
    raceComparison(resultId)
      .then((c) => {
        setComparison(c);
        setOthers(defaultOthers(c.runners));
      })
      .catch((err: unknown) => setError(String(err)));
  }, [resultId]);

  if (error !== null) return <Notice kind="error">{error}</Notice>;
  if (comparison === null) return null;

  const self = comparison.runners.find((r) => r.is_self);
  if (self === undefined) return null;
  const chosen = others.flatMap((k) => comparison.runners.filter((r) => keyOf(r) === k));
  const shown = [self, ...chosen].map((r, i) => ({ runner: r, color: SERIES_COLORS[i] }));
  const candidates = comparison.runners.filter((r) => !r.is_self && !others.includes(keyOf(r)));
  const legName = (i: number) => {
    const leg = comparison.legs[i];
    return `Tramo ${leg.index} · ${codeLabel(leg.from)} → ${codeLabel(leg.to)}`;
  };

  const picker = (
    <div className="compare-picker">
      {/* Las fichas hacen de leyenda: el color de cada corredor en las dos gráficas. */}
      <span className="compare-chip is-self">
        <span className="legend-swatch" style={{ background: SERIES_COLORS[0] }} />
        Tú
      </span>
      {chosen.map((r, i) => (
        <span className="compare-chip" key={keyOf(r)}>
          <span className="legend-swatch" style={{ background: SERIES_COLORS[i + 1] }} />
          {fullName(r)}
          <button
            type="button"
            aria-label={`Quitar a ${fullName(r)}`}
            onClick={() => setOthers(others.filter((k) => k !== keyOf(r)))}
          >
            <CloseIcon size={14} />
          </button>
        </span>
      ))}
      <select
        className="select"
        aria-label="Añadir un compañero del recorrido"
        value=""
        disabled={others.length >= MAX_OTHERS || candidates.length === 0}
        onChange={(e) => {
          if (e.target.value !== "") setOthers([...others, e.target.value]);
        }}
      >
        <option value="">
          {others.length >= MAX_OTHERS ? `Máximo ${MAX_OTHERS} compañeros` : "Añadir compañero…"}
        </option>
        {candidates.map((r) => (
          <option key={keyOf(r)} value={keyOf(r)}>
            {r.course_place === null ? "—" : `${r.course_place}.`} {fullName(r)} · {r.class_name} ·{" "}
            {clock(r.total_s)}
          </option>
        ))}
      </select>
    </div>
  );

  const xLabels = ["S", ...comparison.legs.map((l) => String(l.index))];
  const runnerCount = `${shown.length} ${shown.length === 1 ? "corredor" : "corredores"}`;

  return (
    <>
      <ChartPanel
        title="Diferencia con el tiempo ideal"
        description="Tiempo por detrás del ideal del recorrido al acabar cada tramo: cuanto más abajo, más lejos del ideal; una línea que baja de golpe es un tramo malo."
        cases={runnerCount}
        chart={
          <>
            {picker}
            <MultiLineChart
              label="Diferencia acumulada respecto al tiempo ideal, por tramo, de cada corredor"
              xLabels={xLabels}
              tooltipTitle={(i) =>
                i === 0
                  ? { value: "Salida", detail: "Todos empiezan en 0" }
                  : { value: legName(i - 1), detail: "Por detrás del ideal" }
              }
              // Hacia abajo es por detrás, como en la gráfica clásica de WinSplits.
              formatTick={(v) => `${Math.round(-v) === 0 ? 0 : Math.round(-v)} s`}
              formatValue={(v) => behind(v === null ? null : -v)}
              series={shown.map(({ runner, color }) => ({
                key: keyOf(runner),
                label: shortName(runner),
                color,
                emphasis: runner.is_self,
                values: [0, ...runner.behind_ideal_s.map((v) => (v === null ? null : -v))],
              }))}
            />
          </>
        }
        table={
          <table className="table">
            <thead>
              <tr>
                <th className="num">Tramo</th>
                <th>Balizas</th>
                {shown.map(({ runner }) => (
                  <th className="num" key={keyOf(runner)}>
                    {shortName(runner)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {comparison.legs.map((leg, i) => (
                <tr key={leg.index}>
                  <td className="num">{leg.index}</td>
                  <td className="num">
                    {codeLabel(leg.from)} → {codeLabel(leg.to)}
                  </td>
                  {shown.map(({ runner }) => (
                    <td className="num" key={keyOf(runner)}>
                      {behind(runner.behind_ideal_s[i])}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        }
      />
      <ChartPanel
        title="Pérdida por tramo comparada"
        description="Segundos perdidos (arriba) o ganados (abajo) en cada tramo frente a lo esperado con el rendimiento habitual de cada uno."
        cases={runnerCount}
        chart={
          <>
            {picker}
            <GroupedColumnChart
              label="Pérdida en segundos de cada tramo, por corredor"
              xLabels={comparison.legs.map((l) => String(l.index))}
              tooltipTitle={(i) => ({ value: legName(i), detail: "Pérdida frente a lo esperado" })}
              formatTick={(v) => `${Math.round(v)} s`}
              formatValue={loss}
              series={shown.map(({ runner, color }) => ({
                key: keyOf(runner),
                label: shortName(runner),
                color,
                values: runner.loss_s,
              }))}
            />
          </>
        }
        table={
          <table className="table">
            <thead>
              <tr>
                <th className="num">Tramo</th>
                <th>Balizas</th>
                {shown.map(({ runner }) => (
                  <th className="num" key={keyOf(runner)}>
                    {shortName(runner)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {comparison.legs.map((leg, i) => (
                <tr key={leg.index}>
                  <td className="num">{leg.index}</td>
                  <td className="num">
                    {codeLabel(leg.from)} → {codeLabel(leg.to)}
                  </td>
                  {shown.map(({ runner }) => (
                    <td className="num" key={keyOf(runner)}>
                      {loss(runner.loss_s[i])}
                      {runner.is_error[i] ? " · error" : ""}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        }
      />
    </>
  );
}
