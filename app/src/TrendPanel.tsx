// ¿Estoy mejorando? (#192, docs/historico.md): el IR y la tasa de error de cada carrera con la
// recta de tendencia y su banda al 90 %. Los números vienen del núcleo; aquí solo se dibujan.
import { Trend, decimal } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { MultiLineChart } from "./charts/MultiLineChart";
import { racesLabel, tickNumber } from "./HistoryPanels";
import { useSubject } from "./subject";

/** `AAAA-MM-DD` → `DD/MM/AA`, para el eje. */
const shortDate = (date: string) => `${date.slice(8, 10)}/${date.slice(5, 7)}/${date.slice(2, 4)}`;
const points = (v: number) => `${decimal(Math.abs(v) * 100, 0)} puntos`;
const percent0 = (v: number | null) => (v === null ? "—" : `${decimal(v * 100, 0)} %`);

/** Una gráfica de tendencia: el IR o la tasa de error. */
function TrendChart({
  id,
  title,
  measure,
  yUnit,
  trend,
  higherIsBetter,
}: {
  id: string;
  title: string;
  /** «tu IR», «tu tasa de error»… */
  measure: string;
  /** Unidad del eje Y. */
  yUnit: string;
  trend: Trend;
  higherIsBetter: boolean;
}) {
  const who = useSubject();
  const p = trend.points;
  const verdict = (() => {
    const slope = trend.slope_per_quarter;
    const range =
      trend.slope_low === null || trend.slope_high === null
        ? ""
        : ` (entre ${decimal(trend.slope_low * 100, 0)} y ${decimal(trend.slope_high * 100, 0)} puntos, al 90 %)`;
    switch (trend.verdict) {
      case "not_enough_data": {
        const missing = [
          trend.missing_races > 0 ? racesLabel(trend.missing_races) : null,
          trend.missing_days > 0 ? `${trend.missing_days} días más de historia` : null,
        ].filter((m) => m !== null);
        return `Todavía no se puede decir: hacen falta al menos 8 carreras en 3 meses (faltan ${missing.join(" y ")}).`;
      }
      case "no_change":
        return `Sin cambio claro: ${measure} se mueve ${slope === null ? "poco" : `unos ${points(slope)} cada 3 meses`}${range}, pero el margen incluye no cambiar nada.`;
      case "improving":
      case "worsening": {
        const up = (slope ?? 0) > 0;
        const better = trend.verdict === "improving";
        return `${better ? who.pick("Mejoras", "Mejora", "Mejoran") : who.pick("Empeoras", "Empeora", "Empeoran")}: ${measure} ${up ? "sube" : "baja"} unos ${points(slope ?? 0)} cada 3 meses${range}.`;
      }
    }
  })();
  const how = `Cada punto es una carrera, de la más antigua a la más reciente. La recta es la tendencia y las líneas finas, su margen al 90 %. Solo se habla de mejora o de empeora si el margen entero queda de un lado${higherIsBetter ? "" : " (en la tasa de error, bajar es mejorar)"}.`;
  return (
    <ChartPanel
      id={id}
      help={{ page: "estadisticas-evolucion", section: "¿Estoy mejorando?" }}
      title={title}
      description={how}
      note={verdict}
      cases={racesLabel(p.length)}
      chart={
        p.length === 0 ? (
          <p className="muted">Sin carreras con números.</p>
        ) : (
          <MultiLineChart
            label={`${title}: cada carrera, la tendencia y su margen`}
            xLabels={p.map((point) => shortDate(point.date))}
            formatValue={(v) => (v === null ? "—" : `${decimal(v, 0)} %`)}
            formatTick={tickNumber}
            yUnit={yUnit}
            xUnit="fecha (dd/mm/aa)"
            tooltipTitle={(i) => ({
              value: p[i] === undefined ? "" : percent0(p[i].value),
              detail: p[i]?.date ?? "",
            })}
            series={[
              {
                key: "value",
                label: "Carrera",
                color: "var(--chart-series-1)",
                values: p.map((point) => point.value * 100),
              },
              {
                key: "fitted",
                label: "Tendencia",
                color: "var(--chart-reference)",
                emphasis: true,
                values: p.map((point) => (point.fitted === null ? null : point.fitted * 100)),
              },
              {
                key: "high",
                label: "Margen",
                color: "var(--chart-muted)",
                values: p.map((point) => (point.high === null ? null : point.high * 100)),
              },
              {
                key: "low",
                label: "Margen",
                color: "var(--chart-muted)",
                values: p.map((point) => (point.low === null ? null : point.low * 100)),
              },
            ]}
          />
        )
      }
      table={
        <table className="table">
          <thead>
            <tr>
              <th className="num">Fecha</th>
              <th className="num">Carrera</th>
              <th className="num">Tendencia</th>
              <th className="num">Margen al 90 %</th>
            </tr>
          </thead>
          <tbody>
            {p.map((point) => (
              <tr key={point.date + String(point.value)}>
                <td className="num muted">{point.date}</td>
                <td className="num">{percent0(point.value)}</td>
                <td className="num">{percent0(point.fitted)}</td>
                <td className="num">
                  {point.low === null || point.high === null
                    ? "—"
                    : `${percent0(point.low)} – ${percent0(point.high)}`}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      }
    />
  );
}

/** Sección «¿Estoy mejorando?» de la pestaña ¿Cómo evoluciono? */
export function TrendPanel({ performance, errorRate }: { performance: Trend; errorRate: Trend }) {
  const who = useSubject();
  return (
    <>
      <h3 className="section-title">
        {who.pick("¿Estoy mejorando?", "¿Está mejorando?", "¿Está mejorando el grupo?")}
      </h3>
      <TrendChart
        id="trend-performance"
        title="Tendencia del IR"
        measure={`${who.yourOne} IR`}
        yUnit="IR (%)"
        trend={performance}
        higherIsBetter
      />
      <TrendChart
        id="trend-error-rate"
        title="Tendencia de la tasa de error"
        measure={`${who.yourOne} tasa de error`}
        yUnit="% de tramos con error"
        trend={errorRate}
        higherIsBetter={false}
      />
    </>
  );
}
