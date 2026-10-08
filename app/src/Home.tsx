// Inicio (#127, `docs/app.md`, "Inicio"): qué hay nuevo y qué queda por hacer. La última
// carrera, lo pendiente y el rendimiento de las últimas carreras; los análisis, en Estadísticas.
import { useEffect, useState } from "react";
import { FORMAT_LABELS, RaceRow, clock, getHistory, getSettings, statusLabel } from "./api";
import type { RaceTab } from "./RaceView";
import { LineChart } from "./charts/LineChart";
import { percent, tickPercent } from "./HistoryPanels";
import { TrackThumb } from "./TrackThumb";
import { ChevronRight, EmptyState, FileIcon, PageHeader, Stat, TagIcon, WatchIcon } from "./ui";

/** Carreras de la gráfica de rendimiento. */
const TREND_RACES = 10;
/** Carreras con errores por revisar que se listan; el resto, en Mis carreras. */
const PENDING_RACES = 5;

/** `AAAA-MM-DD` → `DD/MM/AA`, para el eje. */
const shortDate = (date: string) => `${date.slice(8, 10)}/${date.slice(5, 7)}/${date.slice(2, 4)}`;

const errorsLabel = (n: number) => `${n} ${n === 1 ? "error" : "errores"}`;

function Home({
  races,
  onOpen,
  onImport,
  onRaces,
  onHistory,
}: {
  /** De la más reciente a la más antigua. */
  races: RaceRow[] | null;
  onOpen: (resultId: number, tab?: RaceTab) => void;
  onImport: () => void;
  onRaces: () => void;
  onHistory: () => void;
}) {
  const [name, setName] = useState<string | null>(null);
  const [meanPerformance, setMeanPerformance] = useState<number | null>(null);
  const count = races?.length ?? 0;

  useEffect(() => {
    getSettings()
      .then((s) => setName(s.identity.full_name?.split(/\s+/)[0] ?? null))
      .catch(() => undefined);
  }, []);

  // El IR medio sale del histórico sin filtros (`docs/historico.md`): no se calcula aquí.
  useEffect(() => {
    if (count === 0) return;
    getHistory({ from: null, to: null, format: null })
      .then((h) => setMeanPerformance(h.history.total.mean_performance))
      .catch(() => undefined);
  }, [count]);

  const title = name === null ? "Inicio" : `Hola, ${name}`;
  if (races === null) {
    return (
      <>
        <PageHeader title={title} />
        <p className="muted">Cargando…</p>
      </>
    );
  }
  if (races.length === 0) {
    return (
      <>
        <PageHeader title={title} />
        <div className="card">
          <EmptyState icon={<FileIcon size={40} />} title="Empieza por tu primera carrera">
            <p>Importa el .spl de WinSplits de una carrera y, si lo tienes, el FIT de tu reloj.</p>
            <button type="button" className="btn btn-primary btn-lg" onClick={onImport}>
              Importar una carrera
            </button>
          </EmptyState>
        </div>
      </>
    );
  }

  const last = races[0];
  const pending = races.filter((r) => r.unreviewed_count > 0);
  const withoutTrack = races.filter((r) => !r.has_track).length;
  const trend = races
    .slice(0, TREND_RACES)
    .filter((r) => r.usual_performance !== null)
    .reverse();

  return (
    <>
      <PageHeader title={title} />

      <section className="card card-flush home-last">
        <div className="home-last-info">
          <span className="eyebrow">Tu última carrera</span>
          <div>
            <h2>{last.name ?? "Sin nombre"}</h2>
            <div className="meta">
              <span className="num">{last.date}</span>
              <span>{last.class_name}</span>
              {last.format !== null && (
                <span className="pill pill-accent">{FORMAT_LABELS[last.format]}</span>
              )}
            </div>
          </div>
          <div className="stats">
            <Stat
              label="Tiempo"
              value={clock(last.total_s)}
              detail={statusLabel(last.status, last.place)}
            />
            <Stat
              label="Tiempo perdido"
              value={clock(last.lost_time_s)}
              tone="error"
              detail={errorsLabel(last.error_count)}
            />
            <Stat
              label="Rendimiento"
              value={last.usual_performance === null ? "—" : percent(last.usual_performance * 100)}
              detail={
                meanPerformance === null ? undefined : `Tu media: ${percent(meanPerformance * 100)}`
              }
              hint="Lo cerca que fuiste de la referencia en un tramo normal (100 % = la referencia)."
            />
          </div>
          <div className="row">
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => onOpen(last.result_id)}
            >
              Ver carrera <ChevronRight size={16} />
            </button>
            {last.unreviewed_count > 0 && (
              <button type="button" className="btn" onClick={() => onOpen(last.result_id, "legs")}>
                <TagIcon size={16} /> Revisar {errorsLabel(last.unreviewed_count)}
              </button>
            )}
          </div>
        </div>
        {last.has_track && (
          <div className="home-last-map">
            <TrackThumb resultId={last.result_id} />
          </div>
        )}
      </section>

      <div className="home-grid">
        <section className="card">
          <div className="card-title">
            <h3>Pendiente</h3>
          </div>
          {pending.length === 0 && withoutTrack === 0 ? (
            <p className="muted">Nada pendiente: todos los errores están revisados.</p>
          ) : (
            <ul className="tasks">
              {pending.slice(0, PENDING_RACES).map((race) => (
                <li key={race.result_id} className="task">
                  <span className="task-icon task-icon-error">
                    <TagIcon />
                  </span>
                  <div className="task-text">
                    <span className="strong">{errorsLabel(race.unreviewed_count)} sin revisar</span>
                    <span className="small muted">
                      {race.name ?? "Sin nombre"} · {race.date}
                    </span>
                  </div>
                  <button
                    type="button"
                    className="btn"
                    onClick={() => onOpen(race.result_id, "legs")}
                  >
                    Revisar
                  </button>
                </li>
              ))}
              {pending.length > PENDING_RACES && (
                <li className="task">
                  <div className="task-text">
                    <span className="small muted">
                      Y {pending.length - PENDING_RACES} carreras más con errores sin revisar.
                    </span>
                  </div>
                  <button type="button" className="btn btn-ghost" onClick={onRaces}>
                    Ver carreras
                  </button>
                </li>
              )}
              {withoutTrack > 0 && (
                <li className="task">
                  <span className="task-icon">
                    <WatchIcon />
                  </span>
                  <div className="task-text">
                    <span className="strong">
                      {withoutTrack} {withoutTrack === 1 ? "carrera" : "carreras"} sin el FIT del
                      reloj
                    </span>
                    <span className="small muted">
                      Vuelve a importarlas con su FIT y se añade el track.
                    </span>
                  </div>
                  <button type="button" className="btn" onClick={onImport}>
                    Importar
                  </button>
                </li>
              )}
            </ul>
          )}
        </section>

        <section className="card">
          <div className="card-title">
            <h3>Tu rendimiento</h3>
            <button type="button" className="btn btn-ghost" onClick={onHistory}>
              Estadísticas <ChevronRight size={16} />
            </button>
          </div>
          <p className="small muted">
            En tus {trend.length} últimas carreras, de la más antigua a la más reciente. 100 % = la
            referencia de cada tramo.
          </p>
          {trend.length < 2 ? (
            <p className="muted">Con dos carreras o más verás aquí cómo evolucionas.</p>
          ) : (
            <LineChart
              label="Rendimiento de las últimas carreras"
              formatTick={tickPercent}
              labelSpacing={64}
              points={trend.map((race) => ({
                key: race.result_id,
                label: shortDate(race.date),
                value: (race.usual_performance ?? 0) * 100,
                marker: true,
                tooltip: {
                  value: percent((race.usual_performance ?? 0) * 100),
                  detail: `${race.date} · ${race.name ?? "Sin nombre"}`,
                },
              }))}
            />
          )}
        </section>
      </div>
    </>
  );
}

export default Home;
