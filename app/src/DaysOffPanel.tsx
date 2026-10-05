// Días sin competir (P11, docs/historico.md): las carreras del histórico en cubos por los días
// desde la anterior, con los mismos filtros. Dos paneles, uno por medida (IR al entrar en mapa y
// errores al principio): no comparten escala. Los números vienen del núcleo; aquí solo se dibujan.
import { DaysOffStats, HistoryStats } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { legsLabel, percent, racesLabel, tickPercent } from "./HistoryPanels";

/** Nombre del cubo: «≤ 7 días», «8–14 días» o «> 30 días». */
export function daysLabel(b: DaysOffStats): string {
  if (b.to_days === null) return `> ${b.from_days - 1} días`;
  return b.from_days <= 1 ? `≤ ${b.to_days} días` : `${b.from_days}–${b.to_days} días`;
}

const pct = (v: number | null) => (v === null ? null : v * 100);
const show = (v: number | null) => (v === null ? "—" : percent(v));

/** Tabla común a los dos paneles: todas las cifras de cada cubo. */
function DaysOffTable({ buckets }: { buckets: DaysOffStats[] }) {
  return (
    <table className="table">
      <thead>
        <tr>
          <th>Días desde la anterior</th>
          <th className="num">Carreras</th>
          <th className="num">IR 3 primeros</th>
          <th className="num">Tramos (n)</th>
          <th className="num">Error 1.er tercio</th>
          <th className="num">Tramos (n)</th>
        </tr>
      </thead>
      <tbody>
        {buckets.map((b) => (
          <tr key={b.from_days}>
            <td>{daysLabel(b)}</td>
            <td className="num">{b.races}</td>
            <td className="num">{show(pct(b.first_legs_performance))}</td>
            <td className="num muted">{b.first_legs}</td>
            <td className="num">{show(pct(b.first_third_error_rate))}</td>
            <td className="num muted">{b.first_third_legs}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Sección «Días sin competir» de la vista histórica (P11). */
export function DaysOffPanel({
  buckets,
  withoutPrevious,
  total,
}: {
  buckets: DaysOffStats[];
  /** Carreras sin anterior: no entran en ningún cubo. */
  withoutPrevious: number;
  /** Total del histórico con los mismos filtros: sus medias son las líneas de referencia. */
  total: HistoryStats;
}) {
  const races = buckets.reduce((sum, b) => sum + b.races, 0);
  const cases =
    racesLabel(races) + (withoutPrevious > 0 ? ` · ${withoutPrevious} sin anterior` : "");
  const sublabel = (b: DaysOffStats) => `${b.races} ${b.races === 1 ? "carrera" : "carreras"}`;
  const firstLegs = legsLabel(buckets.reduce((sum, b) => sum + b.first_legs, 0));
  return (
    <>
      <h3 className="section-title">Días sin competir</h3>
      <ChartPanel
        title="IR al entrar en mapa"
        description={`IR medio de los tres primeros tramos de tus carreras, según los días desde la carrera anterior (cualquiera, pase o no los filtros), frente a tu IR medio. Si entras peor en mapa tras días sin competir, las columnas de la derecha quedan más bajas. ${firstLegs}.`}
        cases={cases}
        chart={
          <ColumnChart
            label="IR medio de los tres primeros tramos según los días desde la carrera anterior"
            formatTick={tickPercent}
            reference={
              total.mean_performance === null
                ? undefined
                : { value: total.mean_performance * 100, label: "Tu IR medio" }
            }
            columns={buckets.map((b) => {
              const v = pct(b.first_legs_performance);
              return {
                key: b.from_days,
                label: daysLabel(b),
                sublabel: sublabel(b),
                value: v,
                tooltip: {
                  value: v === null ? "Sin carreras" : percent(v),
                  detail: `${daysLabel(b)} · ${racesLabel(b.races)} · ${legsLabel(b.first_legs)}`,
                },
              };
            })}
          />
        }
        table={<DaysOffTable buckets={buckets} />}
      />
      <ChartPanel
        title="Errores al principio de la carrera"
        description="Tasa de error del primer tercio de tus carreras (por número de tramos), según los días desde la carrera anterior, frente a tu tasa de error con estos filtros."
        cases={cases}
        chart={
          <ColumnChart
            label="Tasa de error del primer tercio según los días desde la carrera anterior"
            formatTick={tickPercent}
            reference={
              total.error_rate === null
                ? undefined
                : { value: total.error_rate * 100, label: "Tu media" }
            }
            columns={buckets.map((b) => {
              const v = pct(b.first_third_error_rate);
              const errors = `${b.first_third_errors} ${b.first_third_errors === 1 ? "error" : "errores"}`;
              return {
                key: b.from_days,
                label: daysLabel(b),
                sublabel: sublabel(b),
                value: v,
                tooltip: {
                  value: v === null ? "Sin carreras" : percent(v),
                  detail: `${daysLabel(b)} · ${legsLabel(b.first_third_legs)} · ${errors}`,
                },
              };
            })}
          />
        }
        table={<DaysOffTable buckets={buckets} />}
      />
    </>
  );
}
