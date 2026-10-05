// ¿Lento o desorientado? (P2, docs/tiempo-perdido.md): la pérdida de los errores repartida en
// desvío, paradas y ritmo, con las métricas del FIT. Un panel en la vista de carrera y otro en el
// histórico. Los números vienen del núcleo; aquí solo se dibujan.
import { useEffect, useState } from "react";
import {
  BreakdownHistory,
  BreakdownTotals,
  LegBreakdown,
  RaceBreakdown,
  decimal,
  raceBreakdown,
  signed,
} from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { Legend } from "./charts/common";
import { GroupedColumnChart } from "./charts/GroupedColumnChart";
import { racesLabel, tickPercent } from "./HistoryPanels";

/** Las tres partes, en orden fijo, con su color de serie. */
const PARTS = [
  { key: "detour_s", label: "Desvío", color: "var(--chart-series-1)" },
  { key: "stopped_s", label: "Paradas", color: "var(--chart-series-2)" },
  { key: "pace_s", label: "Ritmo", color: "var(--chart-series-3)" },
] as const;

const HOW =
  "Desvío: los metros que corriste de más respecto a lo habitual en la carrera, al ritmo al que ibas. Paradas: el tiempo parado (sin los 5 s tras picar). Ritmo: el resto. Es una estimación: una parte puede salir negativa.";

const secs = (v: number) => `${signed(v)} s`;
const tickSeconds = (v: number) => `${String(Number(v.toFixed(1))).replace(".", ",")} s`;
const errorsLabel = (n: number) => `${n} ${n === 1 ? "error" : "errores"}`;

/** «desvío 120 s (80 %), paradas 20 s (13 %) y ritmo 10 s (7 %)». */
function summary(t: BreakdownTotals): string {
  const share = (v: number) => (t.loss_s > 0 ? ` (${decimal((v / t.loss_s) * 100, 0)} %)` : "");
  const [d, s, p] = PARTS.map((part) => `${part.label.toLowerCase()} ${decimal(t[part.key], 0)} s${share(t[part.key])}`);
  return `${d}, ${s} y ${p}`;
}

/** Panel de la vista de carrera: cada error, repartido. */
export function RaceBreakdownPanel({ resultId }: { resultId: number }) {
  const [data, setData] = useState<RaceBreakdown | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    setData(undefined);
    setError(null);
    raceBreakdown(resultId)
      .then((b) => {
        if (current) setData(b);
      })
      .catch((err: unknown) => {
        if (current) setError(String(err));
      });
    return () => {
      current = false;
    };
  }, [resultId]);

  const title = "¿Lento o desorientado?";
  if (error !== null || data === undefined || data === null || data.usual_ratio === null) {
    const message =
      error ??
      (data === undefined
        ? "Cargando…"
        : data === null
          ? "Necesita el FIT del reloj: importa la carrera con él para verlo."
          : `Hacen falta al menos 3 tramos sin error con track para saber cuánto rodeas normalmente (hay ${data.clean_legs}).`);
    return (
      <ChartPanel
        title={title}
        description={HOW}
        cases="sin datos"
        chart={<p className="muted">{message}</p>}
        table={<p className="muted">{message}</p>}
      />
    );
  }

  const errors = data.legs.filter((l) => l.is_error);
  const t = data.errors;
  const description =
    errors.length === 0
      ? `Ningún error que repartir. ${HOW}`
      : `De los ${decimal(t.loss_s, 0)} s que perdiste en ${errorsLabel(t.legs)}: ${summary(t)}. ${HOW}`;
  return (
    <ChartPanel
      title={title}
      description={description}
      cases={errorsLabel(errors.length)}
      chart={
        errors.length === 0 ? (
          <p className="muted">Ningún error en los tramos que cuentan.</p>
        ) : (
          <>
            <Legend items={PARTS.map((p) => ({ label: p.label, color: p.color }))} />
            <GroupedColumnChart
              label="Pérdida de cada error repartida en desvío, paradas y ritmo"
              xLabels={errors.map((l) => String(l.index))}
              series={PARTS.map((p) => ({
                key: p.key,
                label: p.label,
                color: p.color,
                values: errors.map((l) => l[p.key]),
              }))}
              tooltipTitle={(i) => {
                const leg = errors[i];
                return {
                  value: leg === undefined ? "" : `${secs(leg.loss_s)} perdidos`,
                  detail: leg === undefined ? "" : `Tramo ${leg.index}`,
                };
              }}
              formatValue={(v) => (v === null ? "—" : secs(v))}
              formatTick={tickSeconds}
            />
          </>
        )
      }
      table={<LegsTable legs={data.legs} usualRatio={data.usual_ratio} />}
    />
  );
}

function LegsTable({ legs, usualRatio }: { legs: LegBreakdown[]; usualRatio: number }) {
  return (
    <>
      <p className="small muted">
        Recorres normalmente {decimal(usualRatio, 2)} veces la línea recta (mediana de tus tramos
        sin error).
      </p>
      <table className="table">
        <thead>
          <tr>
            <th className="num">Tramo</th>
            <th className="num">Pérdida</th>
            <th className="num">Desvío</th>
            <th className="num">Paradas</th>
            <th className="num">Ritmo</th>
            <th>Notas</th>
          </tr>
        </thead>
        <tbody>
          {legs.map((l) => (
            <tr key={l.index}>
              <td className="num">{l.index}</td>
              <td className="num">{secs(l.loss_s)}</td>
              <td className="num">{secs(l.detour_s)}</td>
              <td className="num">{decimal(l.stopped_s, 0)} s</td>
              <td className="num">{secs(l.pace_s)}</td>
              <td>{l.is_error ? "Error" : ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

/** Sección «¿Lento o desorientado?» del histórico: todos los errores juntos. */
export function HistoryBreakdownPanel({ breakdown }: { breakdown: BreakdownHistory }) {
  const t = breakdown.errors;
  const pct = (v: number) => (t.loss_s > 0 ? (v / t.loss_s) * 100 : null);
  const without =
    breakdown.errors_without_breakdown > 0
      ? ` ${errorsLabel(breakdown.errors_without_breakdown)} no se pueden repartir (sin FIT o sin datos suficientes).`
      : "";
  return (
    <>
      <h3 className="section-title">¿Lento o desorientado?</h3>
      <ChartPanel
        title="De qué está hecha la pérdida de tus errores"
        description={
          (t.legs === 0
            ? "Ningún error con FIT que repartir."
            : `De los ${decimal(t.loss_s, 0)} s perdidos en ${errorsLabel(t.legs)}: ${summary(t)}.`) +
          without +
          ` ${HOW}`
        }
        cases={`${errorsLabel(t.legs)} · ${racesLabel(breakdown.races_with_track)} con FIT`}
        chart={
          <ColumnChart
            label="Parte de la pérdida de los errores que es desvío, paradas y ritmo"
            formatTick={tickPercent}
            columns={PARTS.map((p) => {
              const v = pct(t[p.key]);
              return {
                key: p.key,
                label: p.label,
                value: v,
                color: p.color,
                tooltip: {
                  value: v === null ? "Sin datos" : `${decimal(v, 0)} %`,
                  detail: `${p.label} · ${secs(t[p.key])} de ${decimal(t.loss_s, 0)} s`,
                },
              };
            })}
          />
        }
        table={
          <table className="table">
            <thead>
              <tr>
                <th>Parte</th>
                <th className="num">Segundos</th>
                <th className="num">% de la pérdida</th>
              </tr>
            </thead>
            <tbody>
              {PARTS.map((p) => {
                const v = pct(t[p.key]);
                return (
                  <tr key={p.key}>
                    <td>{p.label}</td>
                    <td className="num">{secs(t[p.key])}</td>
                    <td className="num">{v === null ? "—" : `${decimal(v, 0)} %`}</td>
                  </tr>
                );
              })}
              <tr className="total-row">
                <td className="strong">Pérdida en errores</td>
                <td className="num">{secs(t.loss_s)}</td>
                <td className="num">{t.loss_s > 0 ? "100 %" : "—"}</td>
              </tr>
            </tbody>
          </table>
        }
      />
    </>
  );
}
