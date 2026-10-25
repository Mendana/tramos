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
import { racesLabel, tickNumber } from "./HistoryPanels";
import { useSubject } from "./subject";

/** Las tres partes, en orden fijo, con su color de serie. */
const PARTS = [
  { key: "detour_s", label: "Desvío", color: "var(--chart-series-1)" },
  { key: "stopped_s", label: "Paradas", color: "var(--chart-series-2)" },
  { key: "pace_s", label: "Ritmo", color: "var(--chart-series-3)" },
] as const;

const HOW =
  "Desvío: los metros que corriste de más respecto a lo habitual en la carrera, al ritmo al que ibas. Paradas: el tiempo parado (sin los 5 s tras picar). Ritmo: el resto. Es una estimación: una parte puede salir negativa.";

const secs = (v: number) => `${signed(v)} s`;
const tickSeconds = (v: number) => String(Number(v.toFixed(1))).replace(".", ",");
const errorsLabel = (n: number) => `${n} ${n === 1 ? "error" : "errores"}`;

const ATTACK_HOW =
  "Ataque: el tiempo de más en los últimos 50 m antes de picar, frente a lo que tardas normalmente en esa carrera (mediana de tus tramos sin error). Separa perderse por el camino de no encontrar la baliza al llegar.";

/** «En el ataque se fueron 60 s de 150 s (40 %); en 1 de 2 errores, más de la mitad.» */
function attackSummary(t: BreakdownTotals): string {
  if (t.attack_legs === 0 || t.attack_legs_loss_s <= 0) return "";
  const pct = decimal((t.attack_loss_s / t.attack_legs_loss_s) * 100, 0);
  const of = t.attack_legs === 1 ? "del error" : `de ${errorsLabel(t.attack_legs)}`;
  return ` En el ataque se fueron ${decimal(t.attack_loss_s, 0)} s de los ${decimal(t.attack_legs_loss_s, 0)} s ${of} con tiempo de ataque (${pct} %); en ${t.attack_dominant} de ${t.attack_legs}, más de la mitad.`;
}

/** «desvío 120 s (80 %), paradas 20 s (13 %) y ritmo 10 s (7 %)». */
function summary(t: BreakdownTotals): string {
  const share = (v: number) => (t.loss_s > 0 ? ` (${decimal((v / t.loss_s) * 100, 0)} %)` : "");
  const [d, s, p] = PARTS.map(
    (part) => `${part.label.toLowerCase()} ${decimal(t[part.key], 0)} s${share(t[part.key])}`,
  );
  return `${d}, ${s} y ${p}`;
}

/**
 * Panel de la vista de carrera: cada error, repartido. `revision` cambia cuando cambian los tramos
 * del track (el desfase del reloj): entonces se vuelve a pedir.
 */
export function RaceBreakdownPanel({
  resultId,
  revision = 0,
}: {
  resultId: number;
  revision?: number;
}) {
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
  }, [resultId, revision]);

  const title = "¿Lento o desorientado?";
  if (error !== null || data === undefined || data === null || data.usual_ratio === null) {
    const message =
      error ??
      (data === undefined
        ? "Cargando…"
        : data === null
          ? "Necesita el FIT del reloj situado en la carrera: impórtala con él o, si ya lo tiene, corrige el desfase en «Reloj y cronometraje»."
          : `Hacen falta al menos 3 tramos sin error con track para saber cuánto rodeas normalmente (hay ${data.clean_legs}).`);
    return (
      <ChartPanel
        id="race-breakdown"
        help={{ page: "carrera-analisis", section: "¿Lento o desorientado?" }}
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
  const note =
    errors.length === 0
      ? "Ningún error que repartir."
      : `De los ${decimal(t.loss_s, 0)} s que perdiste en ${errorsLabel(t.legs)}: ${summary(t)}.${attackSummary(t)}`;
  return (
    <ChartPanel
      id="race-breakdown"
      help={{ page: "carrera-analisis", section: "¿Lento o desorientado?" }}
      title={title}
      description={`${HOW} ${ATTACK_HOW}`}
      note={note}
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
              yUnit="s"
              xUnit="tramo con error"
            />
          </>
        )
      }
      table={
        <LegsTable
          legs={data.legs}
          usualRatio={data.usual_ratio}
          usualAttack={data.usual_attack_s}
        />
      }
    />
  );
}

function LegsTable({
  legs,
  usualRatio,
  usualAttack,
}: {
  legs: LegBreakdown[];
  usualRatio: number;
  usualAttack: number | null;
}) {
  return (
    <>
      <p className="small muted">
        Recorres normalmente {decimal(usualRatio, 2)} veces la línea recta (mediana de tus tramos
        sin error).{" "}
        {usualAttack === null
          ? "Sin ataque habitual: hacen falta 3 tramos sin error de más de 100 m."
          : `Tardas normalmente ${decimal(usualAttack, 0)} s en los últimos 50 m.`}
      </p>
      <table className="table">
        <thead>
          <tr>
            <th className="num">Tramo</th>
            <th className="num">Pérdida</th>
            <th className="num">Desvío</th>
            <th className="num">Paradas</th>
            <th className="num">Ritmo</th>
            <th className="num">Ataque</th>
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
              <td
                className="num"
                title={
                  l.attack_s === null ? undefined : `${decimal(l.attack_s, 0)} s cerca de la baliza`
                }
              >
                {l.attack_loss_s === null ? "—" : `${decimal(l.attack_loss_s, 0)} s`}
              </td>
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
  const who = useSubject();
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
        id="breakdown"
        help={{ page: "estadisticas-donde", section: "¿Lento o desorientado?" }}
        title={`De qué está hecha la pérdida de ${who.your} errores`}
        description={HOW}
        note={
          (t.legs === 0
            ? "Ningún error con FIT que repartir."
            : `De los ${decimal(t.loss_s, 0)} s perdidos en ${errorsLabel(t.legs)}: ${summary(t)}.`) +
          without
        }
        cases={`${errorsLabel(t.legs)} · ${racesLabel(breakdown.races_with_track)} con FIT`}
        chart={
          <ColumnChart
            label="Parte de la pérdida de los errores que es desvío, paradas y ritmo"
            formatTick={tickNumber}
            yUnit="% de la pérdida"
            xUnit="causa"
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
      <AttackPanel totals={t} />
    </>
  );
}

/** ¿Por el camino o al llegar? (#189): la pérdida de los errores con tiempo de ataque, en el
 * ataque y en el resto del tramo. */
function AttackPanel({ totals: t }: { totals: BreakdownTotals }) {
  const who = useSubject();
  const base = t.attack_legs_loss_s;
  const parts = [
    { key: "attack", label: "Al llegar a la baliza", seconds: t.attack_loss_s },
    { key: "route", label: "Por el camino", seconds: base - t.attack_loss_s },
  ];
  const pct = (v: number) => (base > 0 ? (v / base) * 100 : null);
  const how = `Ataque: el tiempo de más en los últimos 50 m antes de picar, frente a lo que ${who.pick("tardas", "tarda", "tardan")} normalmente en esa carrera (mediana de los tramos sin error). El resto de la pérdida del error es por el camino. Solo los errores de tramos de más de 100 m en línea recta, con FIT.`;
  return (
    <ChartPanel
      id="breakdown-attack"
      help={{ page: "estadisticas-donde", section: "¿Por el camino o al llegar?" }}
      title="¿Por el camino o al llegar?"
      description={how}
      note={
        t.attack_legs === 0
          ? "Ningún error con tiempo de ataque."
          : `De los ${decimal(base, 0)} s perdidos en ${errorsLabel(t.attack_legs)}, ${decimal(t.attack_loss_s, 0)} s fueron al llegar a la baliza. En ${t.attack_dominant} de ${errorsLabel(t.attack_legs)}, más de la mitad.`
      }
      cases={errorsLabel(t.attack_legs)}
      chart={
        <ColumnChart
          label="Parte de la pérdida de los errores que es al llegar a la baliza y por el camino"
          formatTick={tickNumber}
          yUnit="% de la pérdida"
          xUnit="dónde se pierde"
          columns={parts.map((p, i) => {
            const v = pct(p.seconds);
            return {
              key: p.key,
              label: p.label,
              value: v,
              color: `var(--chart-series-${i + 1})`,
              tooltip: {
                value: v === null ? "Sin datos" : `${decimal(v, 0)} %`,
                detail: `${p.label} · ${decimal(p.seconds, 0)} s de ${decimal(base, 0)} s`,
              },
            };
          })}
        />
      }
      table={
        <table className="table">
          <thead>
            <tr>
              <th>Dónde</th>
              <th className="num">Segundos</th>
              <th className="num">% de la pérdida</th>
            </tr>
          </thead>
          <tbody>
            {parts.map((p) => {
              const v = pct(p.seconds);
              return (
                <tr key={p.key}>
                  <td>{p.label}</td>
                  <td className="num">{decimal(p.seconds, 0)} s</td>
                  <td className="num">{v === null ? "—" : `${decimal(v, 0)} %`}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      }
    />
  );
}
