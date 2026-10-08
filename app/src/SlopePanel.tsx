// Pérdida según desnivel (P13, docs/historico.md): los tramos que cuentan del histórico con
// track, en subida, llano y bajada, con los mismos filtros. Las clases y sus números vienen del
// núcleo; aquí solo se dibujan.
import { SLOPE_LABELS, SlopeHistory, SlopeStats, decimal } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { legsLabel, percent, racesLabel, tickPercent } from "./HistoryPanels";

const performance = (s: SlopeStats) =>
  s.mean_performance === null ? null : s.mean_performance * 100;
const errorRate = (s: SlopeStats) => (s.error_rate === null ? null : s.error_rate * 100);
const show = (v: number | null) => (v === null ? "—" : percent(v));
const errorsLabel = (n: number) => `${n} ${n === 1 ? "error" : "errores"}`;

/** Una columna por clase con la medida `value` (en %), y `n` debajo de la etiqueta. */
function ClassColumns({
  classes,
  value,
  detail,
  label,
  reference,
}: {
  classes: SlopeStats[];
  value: (s: SlopeStats) => number | null;
  /** Texto del tooltip tras el valor. */
  detail: (s: SlopeStats) => string;
  label: string;
  reference?: { value: number; label: string };
}) {
  return (
    <ColumnChart
      label={label}
      formatTick={tickPercent}
      reference={reference}
      columns={classes.map((s) => {
        const v = value(s);
        return {
          key: s.class,
          label: SLOPE_LABELS[s.class],
          sublabel: `n = ${s.legs}`,
          value: v,
          tooltip: {
            value: v === null ? "Sin tramos" : percent(v),
            detail: `${SLOPE_LABELS[s.class]} · ${detail(s)}`,
          },
        };
      })}
    />
  );
}

/** Tabla de las clases: tramos, errores, IR medio y tasa de error. */
function ClassTable({ classes }: { classes: SlopeStats[] }) {
  return (
    <table className="table">
      <thead>
        <tr>
          <th>Desnivel</th>
          <th className="num">Tramos (n)</th>
          <th className="num">Errores</th>
          <th className="num">IR medio</th>
          <th className="num">Tasa de error</th>
        </tr>
      </thead>
      <tbody>
        {classes.map((s) => (
          <tr key={s.class}>
            <td>{SLOPE_LABELS[s.class]}</td>
            <td className="num muted">{s.legs}</td>
            <td className="num">{s.errors}</td>
            <td className="num">{show(performance(s))}</td>
            <td className="num">{show(errorRate(s))}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Sección «Por desnivel» de la vista histórica (P13). */
export function SlopePanel({ slope }: { slope: SlopeHistory }) {
  const classes = slope.by_class;
  const classified = classes.reduce((sum, s) => sum + s.legs, 0);
  const races = slope.races_with_track + slope.races_without_track;
  const threshold = decimal(slope.config.threshold_m_per_100m, 0);
  const rule = `Subida si sube al menos ${threshold} m por cada 100 m recorridos; bajada si baja al menos eso; llano en otro caso (si cumple las dos, la mayor).`;
  return (
    <>
      <h3 className="section-title">Por desnivel</h3>
      <p className="small muted">
        Solo aportan tramos las carreras con FIT: {slope.races_with_track} de {racesLabel(races)}
        {" · "}
        {legsLabel(classified)} clasificados
        {slope.unclassified_legs > 0 &&
          ` (${legsLabel(slope.unclassified_legs)} sin clasificar: sin track del tramo, sin altitud o muy cortos)`}
        {slope.legs_without_track > 0 &&
          ` · ${legsLabel(slope.legs_without_track)} de carreras sin FIT no cuentan`}
        .
      </p>
      <ChartPanel
        id="slope-performance"
        title="IR medio según desnivel"
        description={`IR medio de tus tramos en subida, llano y bajada (ponderado por la referencia de cada tramo): 100 % es ir tan rápido como la referencia. ${rule} Debajo de cada clase, sus tramos (n).`}
        cases={legsLabel(classified)}
        chart={
          <ClassColumns
            classes={classes}
            value={performance}
            detail={(s) => `${legsLabel(s.legs)} · tasa de error ${show(errorRate(s))}`}
            label="IR medio de los tramos en subida, llano y bajada frente al 100 % de la referencia"
            reference={{ value: 100, label: "Referencia" }}
          />
        }
        table={<ClassTable classes={classes} />}
      />
      <ChartPanel
        id="slope-error-rate"
        title="Tasa de error según desnivel"
        description={`Qué parte de tus tramos en subida, llano y bajada acaban en error. ${rule} Debajo de cada clase, sus tramos (n): una clase con pocos tramos es poco fiable.`}
        cases={legsLabel(classified)}
        chart={
          <ClassColumns
            classes={classes}
            value={errorRate}
            detail={(s) => `${legsLabel(s.legs)} · ${errorsLabel(s.errors)}`}
            label="Tasa de error de los tramos en subida, llano y bajada"
          />
        }
        table={<ClassTable classes={classes} />}
      />
    </>
  );
}
