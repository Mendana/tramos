// Después de fallar (P8, docs/historico.md): si un error trae otro, si acelerar tras un error
// sale caro y si las rachas limpias largas acaban en error. Los números vienen del núcleo; aquí
// solo se dibujan.
import { AfterError, HistoryStats, Rate } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { legsLabel, percent, tickPercent } from "./HistoryPanels";

const pct = (r: Rate) => (r.error_rate === null ? null : r.error_rate * 100);
const errorsLabel = (n: number) => `${n} ${n === 1 ? "error" : "errores"}`;

/** Una columna con la tasa de error de `rate` y su n debajo. */
function column(key: string, label: string, rate: Rate) {
  const v = pct(rate);
  return {
    key,
    label,
    sublabel: `n = ${rate.legs}`,
    value: v,
    tooltip: {
      value: v === null ? "Sin tramos" : percent(v),
      detail: `${label} · ${legsLabel(rate.legs)} · ${errorsLabel(rate.errors)}`,
    },
  };
}

function RateTable({ rows }: { rows: { key: string; label: string; rate: Rate }[] }) {
  return (
    <table className="table">
      <thead>
        <tr>
          <th>Tramo</th>
          <th className="num">Tramos (n)</th>
          <th className="num">Errores</th>
          <th className="num">Tasa de error</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => {
          const v = pct(r.rate);
          return (
            <tr key={r.key}>
              <td>{r.label}</td>
              <td className="num muted">{r.rate.legs}</td>
              <td className="num">{r.rate.errors}</td>
              <td className="num">{v === null ? "—" : percent(v)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

const streakLabel = (from: number, to: number | null) =>
  to === null ? `> ${from - 1}` : from === to ? String(from) : `${from}–${to}`;

/** Sección «Después de fallar» de la vista histórica (P8). */
export function AfterErrorPanel({ data, total }: { data: AfterError; total: HistoryStats }) {
  const reference =
    total.error_rate === null ? undefined : { value: total.error_rate * 100, label: "Tu media" };
  const chain = [
    { key: "clean", label: "Tras un tramo limpio", rate: data.after_clean },
    { key: "error", label: "Tras un error", rate: data.after_error },
    { key: "fast", label: "Tras un error, acelerando", rate: data.accelerated },
    { key: "slow", label: "Tras un error, sin acelerar", rate: data.not_accelerated },
  ];
  const withoutSpeed =
    data.after_error_without_speed > 0
      ? ` ${legsLabel(data.after_error_without_speed)} tras un error no tienen velocidad para comparar (sin FIT o con pocos tramos limpios) y solo cuentan en «Tras un error».`
      : "";
  const streaks = data.streaks.map((s) => ({
    key: String(s.from),
    label: streakLabel(s.from, s.to),
    rate: s.rate,
  }));
  return (
    <>
      <h3 className="section-title">Después de fallar</h3>
      <ChartPanel
        title="¿Un error trae otro?"
        description={`Tasa de error del tramo siguiente a uno limpio y a un error. Tras un error, separado según aceleraste (más de un 5 % más rápido que tu mediana en los tramos limpios de esa carrera) o no.${withoutSpeed}`}
        cases={legsLabel(data.after_clean.legs + data.after_error.legs)}
        chart={
          <ColumnChart
            label="Tasa de error del tramo siguiente a uno limpio, a un error, acelerando y sin acelerar"
            formatTick={tickPercent}
            reference={reference}
            columns={chain.map((c) => column(c.key, c.label, c.rate))}
          />
        }
        table={<RateTable rows={chain} />}
      />
      <ChartPanel
        title="Rachas limpias"
        description="Tasa de error de un tramo según cuántos tramos limpios seguidos llevabas justo antes (0 = venías de un error), frente a tu media. Si las rachas largas acaban en error, puede ser exceso de confianza."
        cases={legsLabel(streaks.reduce((sum, s) => sum + s.rate.legs, 0))}
        chart={
          <ColumnChart
            label="Tasa de error según los tramos limpios seguidos previos"
            formatTick={tickPercent}
            reference={reference}
            columns={streaks.map((s) => column(s.key, `${s.label} limpios`, s.rate))}
          />
        }
        table={
          <RateTable rows={streaks.map((s) => ({ ...s, label: `${s.label} limpios antes` }))} />
        }
      />
    </>
  );
}
