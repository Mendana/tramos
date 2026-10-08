// Pérdida según duración del tramo (P7, docs/historico.md): los tramos que cuentan del
// histórico, en cubos por su tiempo de referencia, con los mismos filtros. Los cubos y sus
// números vienen del núcleo; aquí solo se dibujan.
import { HistoryStats, LegLengthStats, decimal } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { legsLabel, percent, percent1, tickPercent } from "./HistoryPanels";

/** Nombre del cubo: «20–30 s», «1–2 min» o «≥ 8 min» (el inicio entra, el final no). */
export function bucketLabel(b: { from_s: number; to_s: number | null }): string {
  if (b.to_s === null) return b.from_s < 60 ? `≥ ${b.from_s} s` : `≥ ${b.from_s / 60} min`;
  return b.to_s <= 60 ? `${b.from_s}–${b.to_s} s` : `${b.from_s / 60}–${b.to_s / 60} min`;
}

const errorRate = (b: LegLengthStats) => (b.error_rate === null ? null : b.error_rate * 100);
const show = (v: number | null, format: (v: number) => string) => (v === null ? "—" : format(v));
const errorsLabel = (n: number) => `${n} ${n === 1 ? "error" : "errores"}`;

/** Sección «Por duración del tramo» de la vista histórica (P7). */
export function LegLengthPanel({
  buckets,
  total,
}: {
  buckets: LegLengthStats[];
  /** Total del histórico con los mismos filtros: su tasa de error es la línea de referencia. */
  total: HistoryStats;
}) {
  const legs = buckets.reduce((sum, b) => sum + b.legs, 0);
  const reference =
    total.error_rate === null ? undefined : { value: total.error_rate * 100, label: "Tu media" };
  return (
    <>
      <h3 className="section-title">Por duración del tramo</h3>
      <ChartPanel
        id="leg-length"
        title="Pérdida según duración del tramo"
        description="Tasa de error de tus tramos agrupados por su tiempo de referencia, de los más cortos a los más largos, frente a tu media. Debajo de cada cubo, sus tramos (n): un cubo con pocos tramos es poco fiable. En la tabla, también la pérdida media por tramo."
        cases={legsLabel(legs)}
        chart={
          <ColumnChart
            label="Tasa de error según el tiempo de referencia del tramo"
            formatTick={tickPercent}
            reference={reference}
            columns={buckets.map((b) => {
              const v = errorRate(b);
              const loss =
                b.mean_loss_pct === null ? "" : ` · pérdida ${percent1(b.mean_loss_pct)} por tramo`;
              return {
                key: b.from_s,
                label: bucketLabel(b),
                sublabel: `n = ${b.legs}`,
                value: v,
                tooltip: {
                  value: v === null ? "Sin tramos" : percent(v),
                  detail: `${bucketLabel(b)} · ${legsLabel(b.legs)} · ${errorsLabel(b.errors)}${loss}`,
                },
              };
            })}
          />
        }
        table={
          <table className="table">
            <thead>
              <tr>
                <th>Referencia del tramo</th>
                <th className="num">Tramos (n)</th>
                <th className="num">Errores</th>
                <th className="num">Tasa de error</th>
                <th className="num">Pérdida (%)</th>
                <th className="num">Pérdida (s)</th>
              </tr>
            </thead>
            <tbody>
              {buckets.map((b) => (
                <tr key={b.from_s}>
                  <td>{bucketLabel(b)}</td>
                  <td className="num muted">{b.legs}</td>
                  <td className="num">{b.errors}</td>
                  <td className="num">{show(errorRate(b), percent)}</td>
                  <td className="num">{show(b.mean_loss_pct, percent1)}</td>
                  <td className="num">{show(b.mean_loss_s, (s) => `${decimal(s, 1)} s`)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        }
      />
    </>
  );
}
