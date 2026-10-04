import { useEffect, useState } from "react";
import {
  FORMAT_LABELS,
  RaceDetail,
  clock,
  codeLabel,
  decimal,
  raceDetail,
  signed,
  statusLabel,
} from "./api";

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
    <section className="panel">
      <button type="button" onClick={onBack}>
        ← Tus carreras
      </button>
      {error !== null && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {detail === null ? (
        error === null && <p className="muted">Cargando…</p>
      ) : (
        <Detail detail={detail} />
      )}
    </section>
  );
}

function Detail({ detail }: { detail: RaceDetail }) {
  const lost = detail.report.lost_time;
  const course = detail.report.course;
  const name = `${detail.given_name} ${detail.family_name}`.trim();
  return (
    <>
      <h2>
        {detail.name ?? "Carrera sin nombre"} · {detail.date}
      </h2>
      <p className="muted">
        {detail.class_name} · {name} · {statusLabel(detail.status, detail.place)}
        {detail.format !== null && ` · ${FORMAT_LABELS[detail.format]}`} ·{" "}
        {course.controls.length} balizas · {course.valid_runners} clasificados en el recorrido
        {course.classes.length > 1 &&
          ` (${course.classes.map((c) => c.name).join(", ")})`}
      </p>

      <dl className="totals">
        <div>
          <dt>Tiempo</dt>
          <dd>{clock(lost.total_s)}</dd>
        </div>
        <div>
          <dt>Tiempo perdido</dt>
          <dd>{clock(lost.lost_time_s)}</dd>
        </div>
        <div>
          <dt>Sin errores</dt>
          <dd>{clock(lost.time_without_errors_s)}</dd>
        </div>
        <div>
          <dt>Errores</dt>
          <dd>{lost.error_count}</dd>
        </div>
        <div>
          <dt>Rendimiento habitual</dt>
          <dd>
            {lost.usual_performance === null
              ? "—"
              : `${decimal(lost.usual_performance * 100, 1)} %`}
          </dd>
        </div>
      </dl>
      {course.weak_reference && (
        <p className="note">
          Referencia débil: solo {course.valid_runners} clasificados en el recorrido.
        </p>
      )}
      <p className="muted">
        Un tramo es error si pierdes más de {decimal(detail.config.error_threshold_s, 0)} s y más
        del {decimal(detail.config.error_threshold_pct, 0)} % de lo esperado.
      </p>

      <table className="legs">
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
          {lost.legs.map((leg) => {
            const notes = [
              leg.is_error ? "error" : null,
              leg.is_last ? "último" : null,
              leg.short_reference ? "ref. corta" : null,
            ].filter((n) => n !== null);
            return (
              <tr key={leg.index} className={leg.is_error ? "error-leg" : undefined}>
                <td className="num">{leg.index}</td>
                <td>
                  {codeLabel(leg.from)}→{codeLabel(leg.to)}
                </td>
                <td className="num">{clock(leg.split_s)}</td>
                <td className="num">{leg.place ?? "—"}</td>
                <td className="num">{clock(leg.reference_s)}</td>
                <td className="num">
                  {leg.performance_index === null
                    ? "—"
                    : `${decimal(leg.performance_index * 100, 1)} %`}
                </td>
                <td className="num">{leg.loss_s === null ? "—" : `${signed(leg.loss_s)} s`}</td>
                <td className="num">
                  {leg.loss_pct === null ? "—" : `${signed(leg.loss_pct)} %`}
                </td>
                <td>{notes.join(", ")}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </>
  );
}

export default RaceView;
