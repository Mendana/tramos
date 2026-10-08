import { Fragment, lazy, Suspense, useEffect, useState } from "react";
import {
  FORMAT_LABELS,
  LegReport,
  RaceDetail,
  RaceSharing,
  SHARE_LABELS,
  ShareChoice,
  clock,
  codeLabel,
  decimal,
  spread,
  raceDetail,
  raceSharing,
  RaceFormat,
  setRaceFormat,
  setRaceSharing,
  signed,
  statusLabel,
} from "./api";
import { RaceBreakdownPanel } from "./BreakdownPanel";
import { ClockOffset } from "./ClockOffset";
import { GroupComparison } from "./GroupComparison";
import { LegTagsState, TagControls, TagEditor, TagSummary, typeLabel, useLegTags } from "./LegTags";
import { CumulativeLossPanel, GainLossPanel, LossPanel, PerformancePanel } from "./RacePanels";
import { ChevronLeft, Notice, PageHeader, Stat } from "./ui";
import { useViewer } from "./viewer";

// MapLibre pesa: se carga solo al abrir una carrera.
const MapView = lazy(() => import("./MapView"));

/** Una carrera: totales y tabla de tramos (P1, `docs/app.md`). */
function RaceView({
  resultId,
  onBack,
  onChanged,
}: {
  resultId: number;
  onBack: () => void;
  /** La carrera ha cambiado (p. ej. su formato): la lista tiene que volver a cargarse. */
  onChanged: () => void;
}) {
  const [detail, setDetail] = useState<RaceDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const viewer = useViewer();

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
        <ChevronLeft size={16} />{" "}
        {viewer.runnerName === null ? "Tus carreras" : `Carreras de ${viewer.runnerName}`}
      </button>
      {error !== null && <Notice kind="error">{error}</Notice>}
      {detail === null ? (
        error === null && <p className="muted">Cargando…</p>
      ) : (
        <Detail
          detail={detail}
          onFormatChange={(format) => {
            setError(null);
            setRaceFormat(resultId, format)
              .then(() => {
                setDetail({ ...detail, format });
                onChanged();
              })
              .catch((err: unknown) => setError(String(err)));
          }}
        />
      )}
    </>
  );
}

function Detail({
  detail,
  onFormatChange,
}: {
  detail: RaceDetail;
  onFormatChange: (format: RaceFormat | null) => void;
}) {
  // Tramo seleccionado, compartido por el mapa y la tabla. Otro clic en el mismo lo quita.
  const [selectedLeg, setSelectedLeg] = useState<number | null>(null);
  const toggleLeg = (leg: number) => setSelectedLeg((s) => (s === leg ? null : leg));
  const tagging = useLegTags(detail.result_id);
  // Sube cada vez que cambia el desfase del reloj: el mapa y P2 lo vuelven a pedir.
  const [trackRevision, setTrackRevision] = useState(0);
  // Tramo con el formulario de etiqueta abierto; al abrirlo se selecciona en el mapa.
  const [editing, setEditing] = useState<number | null>(null);
  const toggleEditing = (leg: number) => {
    if (editing !== leg) setSelectedLeg(leg);
    setEditing((e) => (e === leg ? null : leg));
  };
  const { readOnly } = useViewer();
  const sharing = useRaceSharing(detail.result_id, !readOnly);
  const lost = detail.report.lost_time;
  const course = detail.report.course;
  const name = `${detail.given_name} ${detail.family_name}`.trim();
  const shared = course.classes.length > 1 ? course.classes.map((c) => c.name).join(", ") : null;
  const proposed = lost.legs.filter((leg) => leg.is_error);
  const confirmed = proposed.filter((leg) => tagging.tags.get(leg.index)?.tag.confirmation != null).length;
  return (
    <>
      <PageHeader
        title={detail.name ?? "Carrera sin nombre"}
        subtitle={
          <>
            <span className="num">{detail.date}</span>
            <span className="pill">{detail.class_name}</span>
            {readOnly && detail.format !== null && (
              <span className="pill pill-accent">{FORMAT_LABELS[detail.format]}</span>
            )}
            <span>
              {name} · {statusLabel(detail.status, detail.place)}
            </span>
          </>
        }
        actions={
          <>
            {sharing.view?.available && (
              <SharingPicker view={sharing.view} onChange={sharing.change} />
            )}
            {!readOnly && <FormatPicker format={detail.format} onChange={onFormatChange} />}
          </>
        }
      />

      {sharing.error !== null && <Notice kind="error">{sharing.error}</Notice>}
      {sharing.view?.available && sharing.view.problem !== null && (
        <Notice kind="warning">
          No se ha podido dejar en la carpeta compartida: {sharing.view.problem}
        </Notice>
      )}
      {sharing.view?.available &&
        (sharing.view.choice ?? sharing.view.default_choice) === "track" &&
        sharing.view.shared === "legs" && (
          <Notice>Esta carrera no tiene track: se comparten los tramos.</Notice>
        )}

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
          detail={`Consistencia ${spread(lost.consistency, 0)}`}
          hint="Consistencia: cuánto varía tu IR de un tramo a otro (desviación típica, ponderada por la referencia). Menor = más consistente."
        />
      </div>

      {course.weak_reference && (
        <Notice kind="warning">
          Referencia débil: solo {course.valid_runners} clasificados en el recorrido. Las pérdidas
          son poco fiables.
        </Notice>
      )}

      <div className="chart-panels">
        <h3 className="section-title">Gráficas</h3>
        <LossPanel legs={lost.legs} />
        <CumulativeLossPanel legs={lost.legs} />
        <GainLossPanel legs={lost.legs} streaks={lost.losing_streaks} />
        <PerformancePanel legs={lost.legs} />
        <RaceBreakdownPanel resultId={detail.result_id} revision={trackRevision} />
        <h3 className="section-title">Frente al grupo</h3>
        <GroupComparison resultId={detail.result_id} />
      </div>

      <ClockOffset resultId={detail.result_id} onChange={() => setTrackRevision((r) => r + 1)} />

      <Suspense fallback={<p className="muted">Cargando el mapa…</p>}>
        <MapView
          resultId={detail.result_id}
          revision={trackRevision}
          legs={lost.legs}
          selected={selectedLeg}
          onSelect={toggleLeg}
        />
      </Suspense>

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
        <div className="tag-intro">
          <p className="small muted">
            {readOnly
              ? "Las etiquetas son las que ha puesto el corredor: si fue error, el tipo y el contexto."
              : "¿Fue un error? Responde en cada tramo propuesto con un clic: Sí, No o Físico (perdiste tiempo sin fallar en la orientación). Con el lápiz añades el tipo de error y el contexto, también en cualquier otro tramo."}
          </p>
          <span className="small strong num">
            {proposed.length === 0
              ? "Ningún error propuesto"
              : `${confirmed} de ${proposed.length} propuestos revisados`}
          </span>
        </div>
        {tagging.error !== null && <Notice kind="error">{tagging.error}</Notice>}
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
                <Fragment key={leg.index}>
                  <LegRow
                    leg={leg}
                    selected={leg.index === selectedLeg}
                    onSelect={toggleLeg}
                    tagging={tagging}
                    editing={editing === leg.index}
                    onToggleEditing={() => toggleEditing(leg.index)}
                  />
                  {editing === leg.index && (
                    <tr className="tag-editor-row">
                      <td colSpan={9}>
                        <TagEditor leg={leg.index} state={tagging} onClose={() => setEditing(null)} />
                      </td>
                    </tr>
                  )}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      </div>

    </>
  );
}

/** Formato de la carrera, que se puede corregir después de importarla (cuenta en el histórico). */
/** Qué se comparte de la carrera con la entrenadora; pedirlo también la exporta si hace falta. */
function useRaceSharing(resultId: number, enabled: boolean) {
  const [view, setView] = useState<RaceSharing | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!enabled) return;
    raceSharing(resultId)
      .then(setView)
      .catch((err: unknown) => setError(String(err)));
  }, [resultId, enabled]);
  const change = (choice: ShareChoice | null) => {
    setError(null);
    setRaceSharing(resultId, choice)
      .then(setView)
      .catch((err: unknown) => setError(String(err)));
  };
  return { view, error, change };
}

function SharingPicker({
  view,
  onChange,
}: {
  view: RaceSharing;
  onChange: (choice: ShareChoice | null) => void;
}) {
  return (
    <label className="field format-picker">
      <span className="field-label">Compartir</span>
      <select
        className="select"
        value={view.choice ?? ""}
        onChange={(e) => onChange(e.target.value === "" ? null : (e.target.value as ShareChoice))}
      >
        <option value="">Por defecto ({SHARE_LABELS[view.default_choice].toLowerCase()})</option>
        {(Object.keys(SHARE_LABELS) as ShareChoice[]).map((choice) => (
          <option key={choice} value={choice}>
            {SHARE_LABELS[choice]}
          </option>
        ))}
      </select>
    </label>
  );
}

function FormatPicker({
  format,
  onChange,
}: {
  format: RaceFormat | null;
  onChange: (format: RaceFormat | null) => void;
}) {
  return (
    <label className="field format-picker">
      <span className="field-label">Formato</span>
      <select
        className="select"
        value={format ?? ""}
        onChange={(e) => onChange(e.target.value === "" ? null : (e.target.value as RaceFormat))}
      >
        {(Object.keys(FORMAT_LABELS) as RaceFormat[]).map((f) => (
          <option key={f} value={f}>
            {FORMAT_LABELS[f]}
          </option>
        ))}
        <option value="">Sin formato</option>
      </select>
    </label>
  );
}

function LegRow({
  leg,
  selected,
  onSelect,
  tagging,
  editing,
  onToggleEditing,
}: {
  leg: LegReport;
  selected: boolean;
  onSelect: (leg: number) => void;
  tagging: LegTagsState;
  editing: boolean;
  onToggleEditing: () => void;
}) {
  const { readOnly } = useViewer();
  const tag = tagging.tags.get(leg.index)?.tag ?? null;
  const type = tag === null ? null : typeLabel(tagging.taxonomy, tag);
  const lossClass =
    leg.loss_s === null ? undefined : leg.is_error ? "loss-bad" : leg.loss_s < 0 ? "loss-good" : undefined;
  return (
    <tr
      className={`clickable${leg.is_error ? " is-error" : ""}${selected ? " is-selected" : ""}`}
      tabIndex={0}
      aria-current={selected ? "true" : undefined}
      onClick={() => onSelect(leg.index)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(leg.index);
        }
      }}
    >
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
        {/* Los errores propuestos ya se ven en la fila; sus botones de confirmar hacen de aviso. */}
        <div className="notes-cell">
          <div className="meta">
            {leg.is_last && <span className="pill">Último</span>}
            {leg.short_reference && <span className="pill">Ref. corta</span>}
            {type !== null && <span className="pill pill-accent">{type}</span>}
          </div>
          {readOnly ? (
            <TagSummary leg={leg.index} state={tagging} />
          ) : (
            <TagControls
              leg={leg.index}
              proposed={leg.is_error}
              state={tagging}
              open={editing}
              onToggleOpen={onToggleEditing}
            />
          )}
        </div>
      </td>
    </tr>
  );
}

export default RaceView;
