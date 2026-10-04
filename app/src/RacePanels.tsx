// Paneles de gráficas de la vista de carrera (docs/app.md, "Gráficas"). Todos salen de los mismos
// tramos que la tabla, así que sus valores coinciden con ella.
import { LegReport, clock, codeLabel, decimal, signed } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { Legend } from "./charts/common";
import { LineChart } from "./charts/LineChart";

function legName(leg: LegReport): string {
  return `Tramo ${leg.index} · ${codeLabel(leg.from)} → ${codeLabel(leg.to)}`;
}

function cases(n: number): string {
  return `${n} ${n === 1 ? "tramo" : "tramos"}`;
}

const seconds = (v: number) => `${decimal(v, 0)} s`;

/** IR de cada tramo frente al 100 % de la referencia. */
export function PerformancePanel({ legs }: { legs: LegReport[] }) {
  const withIr = legs.filter((leg) => leg.performance_index !== null).length;
  const percent = (v: number) => `${decimal(v, 0)} %`;
  return (
    <ChartPanel
      title="Rendimiento por tramo"
      description="IR de cada tramo: 100 % es ir tan rápido como la referencia del recorrido; por debajo, más lento."
      cases={cases(withIr)}
      chart={
        <ColumnChart
          label="Rendimiento (IR) de cada tramo frente al 100 % de la referencia"
          formatTick={percent}
          reference={{ value: 100, label: "Referencia" }}
          columns={legs.map((leg) => ({
            key: leg.index,
            label: String(leg.index),
            value: leg.performance_index === null ? null : leg.performance_index * 100,
            tooltip: {
              value:
                leg.performance_index === null ? "Sin dato" : percent(leg.performance_index * 100),
              detail: legName(leg),
            },
          }))}
        />
      }
      table={
        <table className="table">
          <thead>
            <tr>
              <th className="num">Tramo</th>
              <th>Balizas</th>
              <th className="num">IR</th>
            </tr>
          </thead>
          <tbody>
            {legs.map((leg) => (
              <tr key={leg.index}>
                <td className="num">{leg.index}</td>
                <td className="num">
                  {codeLabel(leg.from)} → {codeLabel(leg.to)}
                </td>
                <td className="num">
                  {leg.performance_index === null ? "—" : percent(leg.performance_index * 100)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      }
    />
  );
}

/** Pérdida de cada tramo frente a lo esperado (P1), con los errores resaltados. */
export function LossPanel({ legs }: { legs: LegReport[] }) {
  const withLoss = legs.filter((leg) => leg.loss_s !== null).length;
  return (
    <ChartPanel
      title="Pérdida por tramo"
      description="Segundos perdidos (arriba) o ganados (abajo) en cada tramo frente a lo esperado con tu rendimiento habitual."
      cases={cases(withLoss)}
      defaultOpen
      chart={
        <>
          <Legend
            items={[
              { label: "Tramo con error", color: "var(--chart-error)" },
              { label: "Resto", color: "var(--chart-muted)" },
            ]}
          />
          <ColumnChart
            label="Pérdida en segundos de cada tramo; los tramos con error, en naranja"
            formatTick={seconds}
            columns={legs.map((leg) => ({
              key: leg.index,
              label: String(leg.index),
              value: leg.loss_s,
              color: leg.is_error ? "var(--chart-error)" : "var(--chart-muted)",
              tooltip: {
                value: leg.loss_s === null ? "Sin dato" : `${signed(leg.loss_s)} s`,
                detail: `${legName(leg)}${leg.is_error ? " · error" : ""}`,
              },
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
              <th className="num">Pérdida</th>
              <th className="num">%</th>
              <th>Error</th>
            </tr>
          </thead>
          <tbody>
            {legs.map((leg) => (
              <tr key={leg.index}>
                <td className="num">{leg.index}</td>
                <td className="num">
                  {codeLabel(leg.from)} → {codeLabel(leg.to)}
                </td>
                <td className="num">{leg.loss_s === null ? "—" : `${signed(leg.loss_s)} s`}</td>
                <td className="num">{leg.loss_pct === null ? "—" : `${signed(leg.loss_pct)} %`}</td>
                <td>{leg.is_error ? "Sí" : ""}</td>
              </tr>
            ))}
          </tbody>
        </table>
      }
    />
  );
}

/**
 * Tiempo perdido acumulado (P3): la suma, tramo a tramo, de la pérdida de los tramos con error.
 * Acaba en el tiempo perdido total de la carrera.
 */
export function cumulativeLoss(legs: LegReport[]): number[] {
  let total = 0;
  return legs.map((leg) => {
    if (leg.is_error && leg.loss_s !== null) total += leg.loss_s;
    return total;
  });
}

export function CumulativeLossPanel({ legs }: { legs: LegReport[] }) {
  const cumulative = cumulativeLoss(legs);
  const errors = legs.filter((leg) => leg.is_error).length;
  return (
    <ChartPanel
      title="Pérdida acumulada"
      description="Tiempo perdido sumado tramo a tramo: solo suben los tramos con error (los puntos). Acaba en el tiempo perdido de la carrera."
      cases={`${errors} ${errors === 1 ? "error" : "errores"} en ${cases(legs.length)}`}
      chart={
        <LineChart
          label="Tiempo perdido acumulado tras cada tramo"
          formatTick={seconds}
          markerColor="var(--chart-error)"
          points={[
            {
              key: "salida",
              label: "S",
              value: 0,
              tooltip: { value: "0:00", detail: "Salida" },
            },
            ...legs.map((leg, i) => ({
              key: leg.index,
              label: String(leg.index),
              value: cumulative[i],
              marker: leg.is_error,
              tooltip: {
                value: `${clock(cumulative[i])} perdidos`,
                detail: `Tras el ${legName(leg).charAt(0).toLowerCase()}${legName(leg).slice(1)}${
                  leg.is_error && leg.loss_s !== null ? ` · error de ${signed(leg.loss_s)} s` : ""
                }`,
              },
            })),
          ]}
        />
      }
      table={
        <table className="table">
          <thead>
            <tr>
              <th className="num">Tramo</th>
              <th>Balizas</th>
              <th className="num">Pérdida por error</th>
              <th className="num">Acumulada</th>
            </tr>
          </thead>
          <tbody>
            {legs.map((leg, i) => (
              <tr key={leg.index} className={leg.is_error ? "is-error" : undefined}>
                <td className="num">{leg.index}</td>
                <td className="num">
                  {codeLabel(leg.from)} → {codeLabel(leg.to)}
                </td>
                <td className="num">
                  {leg.is_error && leg.loss_s !== null ? `${signed(leg.loss_s)} s` : ""}
                </td>
                <td className="num">{clock(cumulative[i])}</td>
              </tr>
            ))}
          </tbody>
        </table>
      }
    />
  );
}
