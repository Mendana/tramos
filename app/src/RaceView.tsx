import { useEffect, useState } from "react";
import {
  FORMAT_LABELS,
  LegReport,
  RaceDetail,
  clock,
  codeLabel,
  decimal,
  raceDetail,
  signed,
  statusLabel,
} from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { ChevronLeft, Notice, PageHeader, Stat } from "./ui";

/** Una carrera: totales y tabla de tramos (P1, `docs/app.md`). */
function RaceView({ resultId, onBack }: { resultId: number; onBack: () => void }) {
  const [detail, setDetail] = useState<RaceDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setDetail(null);
    setError(null);
    raceDetail(resultId)
      .then(setDetail)
      .catch((err: unknown) => setError(String(err)));
  }, [resultId]);

  return (
    <>
      <button type="button" className="btn btn-ghost back" onClick={onBack}>
        <ChevronLeft size={16} /> Tus carreras
      </button>
      {error !== null && <Notice kind="error">{error}</Notice>}
      {detail === null ? (
        error === null && <p className="muted">Cargando…</p>
      ) : (
        <Detail detail={detail} />
      )}
    </>
  );
}

function Detail({ detail }: { detail: RaceDetail }) {
  const lost = detail.report.lost_time;
  const course = detail.report.course;
  const name = `${detail.given_name} ${detail.family_name}`.trim();
  const shared = course.classes.length > 1 ? course.classes.map((c) => c.name).join(", ") : null;
  return (
    <>
      <PageHeader
        title={detail.name ?? "Carrera sin nombre"}
        subtitle={
          <>
            <span className="num">{detail.date}</span>
            {detail.format !== null && (
              <span className="pill pill-accent">{FORMAT_LABELS[detail.format]}</span>
            )}
            <span className="pill">{detail.class_name}</span>
            <span>
              {name} · {statusLabel(detail.status, detail.place)}
            </span>
          </>
        }
      />

      <div className="stats">
        <Stat label="Tiempo" value={clock(lost.total_s)} />
        <Stat
          label="Tiempo perdido"
          value={clock(lost.lost_time_s)}
          tone={lost.error_count > 0 ? "error" : undefined}
        />
        <Stat label="Sin errores" value={clock(lost.time_without_errors_s)} />
        <Stat label="Errores" value={lost.error_count} tone={lost.error_count > 0 ? "error" : undefined} />
        <Stat
          label="Rendimiento"
          value={
            lost.usual_performance === null ? "—" : `${decimal(lost.usual_performance * 100, 0)} %`
          }
        />
      </div>

      {course.weak_reference && (
        <Notice kind="warning">
          Referencia débil: solo {course.valid_runners} clasificados en el recorrido. Las pérdidas
          son poco fiables.
        </Notice>
      )}

      <div className="card card-flush">
        <div className="card-title card-head">
          <h3>Tramos</h3>
          <span className="small muted">
            {course.controls.length} balizas · {course.valid_runners} clasificados
            {shared !== null && ` (${shared})`} · error si pierdes más de{" "}
            {decimal(detail.config.error_threshold_s, 0)} s y del{" "}
            {decimal(detail.config.error_threshold_pct, 0)} %
          </span>
        </div>
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th className="num">Tramo</th>
                <th>Balizas</th>
                <th className="num">Split</th>
                <th className="num">Puesto</th>
                <th className="num">Referencia</th>
                <th className="num">IR</th>
                <th className="num">Pérdida</th>
                <th className="num">%</th>
                <th>Notas</th>
              </tr>
            </thead>
            <tbody>
              {lost.legs.map((leg) => (
                <LegRow key={leg.index} leg={leg} />
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <div className="chart-panels">
        <h3 className="section-title">Gráficas</h3>
        <PerformancePanel legs={lost.legs} />
      </div>
    </>
  );
}

/** IR de cada tramo frente al 100 % de la referencia (panel de ejemplo de #21). */
function PerformancePanel({ legs }: { legs: LegReport[] }) {
  const withIr = legs.filter((leg) => leg.performance_index !== null).length;
  const percent = (v: number) => `${decimal(v, 0)} %`;
  const legName = (leg: LegReport) =>
    `Tramo ${leg.index} · ${codeLabel(leg.from)} → ${codeLabel(leg.to)}`;
  return (
    <ChartPanel
      title="Rendimiento por tramo"
      description="IR de cada tramo: 100 % es ir tan rápido como la referencia del recorrido; por debajo, más lento."
      cases={`${withIr} ${withIr === 1 ? "tramo" : "tramos"}`}
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

function LegRow({ leg }: { leg: LegReport }) {
  const lossClass =
    leg.loss_s === null ? undefined : leg.is_error ? "loss-bad" : leg.loss_s < 0 ? "loss-good" : undefined;
  return (
    <tr className={leg.is_error ? "is-error" : undefined}>
      <td className="num strong">{leg.index}</td>
      <td className="num">
        {codeLabel(leg.from)} → {codeLabel(leg.to)}
      </td>
      <td className="num strong">{clock(leg.split_s)}</td>
      <td className="num">{leg.place ?? "—"}</td>
      <td className="num muted">{clock(leg.reference_s)}</td>
      <td className="num">
        {leg.performance_index === null ? "—" : `${decimal(leg.performance_index * 100, 0)} %`}
      </td>
      <td className={`num ${lossClass ?? ""}`}>
        {leg.loss_s === null ? "—" : `${signed(leg.loss_s)} s`}
      </td>
      <td className={`num ${lossClass ?? ""}`}>
        {leg.loss_pct === null ? "—" : `${signed(leg.loss_pct)} %`}
      </td>
      <td>
        <div className="meta">
          {leg.is_error && <span className="pill pill-error">Error</span>}
          {leg.is_last && <span className="pill">Último</span>}
          {leg.short_reference && <span className="pill">Ref. corta</span>}
        </div>
      </td>
    </tr>
  );
}

export default RaceView;
