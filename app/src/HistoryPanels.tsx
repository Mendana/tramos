// Paneles de la vista histórica (docs/app.md, "Vista histórica"). Salen de los mismos grupos que
// la tabla por formato, así que sus valores coinciden con ella.
import { FORMAT_LABELS, FormatHistory, HistoryStats, decimal } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";

/** Nombre del grupo: el formato o «Sin formato». */
export function groupLabel(group: FormatHistory): string {
  return group.format === null ? "Sin formato" : FORMAT_LABELS[group.format];
}

export const percent = (v: number) => `${decimal(v, 0)} %`;
export const percent1 = (v: number) => `${decimal(v, 1)} %`;
/** Marca del eje en %, con decimales solo si los tiene (0,5 %). */
export const tickPercent = (v: number) => `${String(Number(v.toFixed(2))).replace(".", ",")} %`;

export function racesLabel(n: number): string {
  return `${n} ${n === 1 ? "carrera" : "carreras"}`;
}

export function legsLabel(n: number): string {
  return `${n} ${n === 1 ? "tramo" : "tramos"}`;
}

/** Una medida de los grupos en % (o `null` sin datos) y cómo se escribe. */
interface Measure {
  get: (stats: HistoryStats) => number | null;
  format: (value: number) => string;
}

/** Tabla accesible de un panel: grupo, la medida y su número de casos. */
function GroupTable({
  groups,
  heading,
  value,
  cases,
}: {
  groups: FormatHistory[];
  heading: string;
  value: (stats: HistoryStats) => string;
  cases: (stats: HistoryStats) => string;
}) {
  return (
    <table className="table">
      <thead>
        <tr>
          <th>Formato</th>
          <th className="num">{heading}</th>
          <th className="num">Casos</th>
        </tr>
      </thead>
      <tbody>
        {groups.map((g) => (
          <tr key={g.format ?? "none"}>
            <td>{groupLabel(g)}</td>
            <td className="num">{value(g.stats)}</td>
            <td className="num muted">{cases(g.stats)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Una columna por grupo con la medida `measure` (en %). */
function GroupColumns({
  groups,
  measure,
  cases,
  detail,
  label,
  reference,
}: {
  groups: FormatHistory[];
  measure: Measure;
  cases: (stats: HistoryStats) => string;
  /** Texto extra del tooltip, tras el formato y los casos. */
  detail?: (stats: HistoryStats) => string;
  label: string;
  reference?: { value: number; label: string };
}) {
  return (
    <ColumnChart
      label={label}
      formatTick={tickPercent}
      reference={reference}
      columns={groups.map((g) => {
        const v = measure.get(g.stats);
        return {
          key: g.format ?? "none",
          label: groupLabel(g),
          value: v,
          tooltip: {
            value: v === null ? "Sin datos" : measure.format(v),
            detail: `${groupLabel(g)} · ${cases(g.stats)}${detail === undefined ? "" : ` · ${detail(g.stats)}`}`,
          },
        };
      })}
    />
  );
}

const meanPerformance: Measure = {
  get: (s) => (s.mean_performance === null ? null : s.mean_performance * 100),
  format: percent,
};
const errorRate: Measure = {
  get: (s) => (s.error_rate === null ? null : s.error_rate * 100),
  format: percent,
};
const meanLossPct: Measure = { get: (s) => s.mean_loss_pct, format: percent1 };
const show = (m: Measure) => (s: HistoryStats) => {
  const v = m.get(s);
  return v === null ? "—" : m.format(v);
};
const raceCases = (s: HistoryStats) => racesLabel(s.races);
const legCases = (s: HistoryStats) => legsLabel(s.legs);

/** IR medio de cada formato frente al 100 % de la referencia (P6). */
export function FormatPerformancePanel({
  groups,
  total,
}: {
  groups: FormatHistory[];
  total: HistoryStats;
}) {
  return (
    <ChartPanel
      title="IR medio por formato"
      description="Media del rendimiento habitual de tus carreras de cada formato: 100 % es ir tan rápido como la referencia del recorrido; por debajo, más lento."
      cases={racesLabel(total.races)}
      defaultOpen
      chart={
        <GroupColumns
          groups={groups}
          measure={meanPerformance}
          cases={raceCases}
          label="IR medio de cada formato frente al 100 % de la referencia"
          reference={{ value: 100, label: "Referencia" }}
        />
      }
      table={
        <GroupTable
          groups={groups}
          heading="IR medio"
          value={show(meanPerformance)}
          cases={raceCases}
        />
      }
    />
  );
}

/** Tasa de error de cada formato (P6). */
export function FormatErrorRatePanel({
  groups,
  total,
}: {
  groups: FormatHistory[];
  total: HistoryStats;
}) {
  return (
    <ChartPanel
      title="Tasa de error por formato"
      description="Qué parte de tus tramos acaban en error en cada formato. Cuentan los tramos con pérdida, salvo el último y los de referencia corta."
      cases={legsLabel(total.legs)}
      chart={
        <GroupColumns
          groups={groups}
          measure={errorRate}
          cases={legCases}
          detail={(s) => `${s.errors} ${s.errors === 1 ? "error" : "errores"}`}
          label="Tasa de error de cada formato"
        />
      }
      table={
        <GroupTable groups={groups} heading="Tasa de error" value={show(errorRate)} cases={legCases} />
      }
    />
  );
}

/** Pérdida media por tramo de cada formato, en % del tiempo esperado (P6). */
export function FormatLossPanel({
  groups,
  total,
}: {
  groups: FormatHistory[];
  total: HistoryStats;
}) {
  const seconds = (s: HistoryStats) =>
    s.mean_loss_s === null ? "—" : `${decimal(s.mean_loss_s, 1)} s por tramo`;
  return (
    <ChartPanel
      title="Pérdida media por tramo"
      description="Tiempo perdido en errores repartido entre todos tus tramos, en % del tiempo esperado: así se comparan formatos con tramos de duraciones muy distintas. En la tabla, también en segundos."
      cases={legsLabel(total.legs)}
      chart={
        <GroupColumns
          groups={groups}
          measure={meanLossPct}
          cases={legCases}
          detail={seconds}
          label="Pérdida media por tramo de cada formato, en % del tiempo esperado"
        />
      }
      table={
        <table className="table">
          <thead>
            <tr>
              <th>Formato</th>
              <th className="num">Pérdida (%)</th>
              <th className="num">Pérdida (s)</th>
              <th className="num">Casos</th>
            </tr>
          </thead>
          <tbody>
            {groups.map((g) => (
              <tr key={g.format ?? "none"}>
                <td>{groupLabel(g)}</td>
                <td className="num">{show(meanLossPct)(g.stats)}</td>
                <td className="num">
                  {g.stats.mean_loss_s === null ? "—" : `${decimal(g.stats.mean_loss_s, 1)} s`}
                </td>
                <td className="num muted">{legCases(g.stats)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      }
    />
  );
}
