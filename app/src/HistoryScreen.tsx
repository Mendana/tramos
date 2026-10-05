// Vista histórica (P6, docs/app.md): todas las carreras del usuario agregadas por formato, con
// filtros de fechas y formato. Los análisis que se apoyan en el histórico (P7, P10, P11, P13)
// añaden sus secciones de paneles al final, con los mismos filtros.
import { useEffect, useState } from "react";
import {
  FORMAT_LABELS,
  HistoryFilter,
  HistoryStats,
  HistoryView,
  RaceFormat,
  decimal,
  getHistory,
} from "./api";
import {
  FormatErrorRatePanel,
  FormatLossPanel,
  FormatPerformancePanel,
  groupLabel,
  percent,
  percent1,
  racesLabel,
} from "./HistoryPanels";
import { ChartIcon, EmptyState, Notice, PageHeader, Stat } from "./ui";

const NO_FILTER: HistoryFilter = { from: null, to: null, format: null };
const FORMATS: RaceFormat[] = ["sprint", "middle", "long"];

function HistoryScreen({ onImport }: { onImport: () => void }) {
  const [filter, setFilter] = useState<HistoryFilter>(NO_FILTER);
  const [view, setView] = useState<HistoryView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    setError(null);
    getHistory(filter)
      .then((v) => {
        if (current) setView(v);
      })
      .catch((err: unknown) => {
        if (current) setError(String(err));
      });
    // Una respuesta que llega tarde (otro filtro ya pedido) se descarta.
    return () => {
      current = false;
    };
  }, [filter]);

  const reversed = filter.from !== null && filter.to !== null && filter.from > filter.to;

  return (
    <>
      <PageHeader
        title="Histórico"
        subtitle={
          view === null
            ? "Cargando…"
            : view.all_races === 0
              ? undefined
              : `${racesLabel(view.all_races)} del ${view.first_date ?? "—"} al ${view.last_date ?? "—"}`
        }
      />
      {error !== null && <Notice kind="error">{error}</Notice>}

      {view !== null && view.all_races === 0 && (
        <div className="card">
          <EmptyState icon={<ChartIcon size={40} />} title="Aún no hay carreras">
            <p>El histórico junta todas tus carreras. Importa alguna para verlo.</p>
            <button type="button" className="btn btn-primary btn-lg" onClick={onImport}>
              Importar carrera
            </button>
          </EmptyState>
        </div>
      )}

      {view !== null && view.all_races > 0 && (
        <>
          <Filters filter={filter} onChange={setFilter} />
          {reversed && (
            <Notice kind="warning">La fecha «desde» es posterior a «hasta»: no entra ninguna carrera.</Notice>
          )}
          {view.history.total.races === 0 ? (
            !reversed && (
              <div className="card">
                <EmptyState icon={<ChartIcon size={40} />} title="Ninguna carrera con estos filtros">
                  <button type="button" className="btn" onClick={() => setFilter(NO_FILTER)}>
                    Quitar filtros
                  </button>
                </EmptyState>
              </div>
            )
          ) : (
            <Summary view={view} />
          )}
        </>
      )}
    </>
  );
}

/** Filtros: fechas (incluidas) y formato. */
function Filters({
  filter,
  onChange,
}: {
  filter: HistoryFilter;
  onChange: (filter: HistoryFilter) => void;
}) {
  const date = (value: string) => (value === "" ? null : value);
  const active = filter.from !== null || filter.to !== null || filter.format !== null;
  return (
    <div className="card history-filters">
      <label className="field">
        <span className="field-label">Desde</span>
        <input
          className="input num"
          type="date"
          value={filter.from ?? ""}
          onChange={(e) => onChange({ ...filter, from: date(e.target.value) })}
        />
      </label>
      <label className="field">
        <span className="field-label">Hasta</span>
        <input
          className="input num"
          type="date"
          value={filter.to ?? ""}
          onChange={(e) => onChange({ ...filter, to: date(e.target.value) })}
        />
      </label>
      <div className="field">
        <span className="field-label" id="history-format">
          Formato
        </span>
        <div className="segmented" role="radiogroup" aria-labelledby="history-format">
          {[null, ...FORMATS].map((format) => (
            <label key={format ?? "all"}>
              <input
                type="radio"
                name="history-format"
                checked={filter.format === format}
                onChange={() => onChange({ ...filter, format })}
              />
              {format === null ? "Todos" : FORMAT_LABELS[format]}
            </label>
          ))}
        </div>
      </div>
      {active && (
        <button
          type="button"
          className="btn btn-ghost"
          onClick={() => onChange({ from: null, to: null, format: null })}
        >
          Quitar filtros
        </button>
      )}
    </div>
  );
}

/** Cifras del total, tabla por formato y paneles. */
function Summary({ view }: { view: HistoryView }) {
  const { total, by_format: groups, races_without_data: withoutData } = view.history;
  const config = view.config;
  return (
    <>
      <div className="stats">
        <Stat label="Carreras" value={total.races} />
        <Stat label="IR medio" value={performance(total)} />
        <Stat label="Tasa de error" value={errorRate(total)} />
        <Stat label="Pérdida por tramo" value={lossS(total)} />
      </div>

      {withoutData > 0 && (
        <Notice>
          {withoutData === 1 ? "Una carrera no cuenta" : `${withoutData} carreras no cuentan`}: sin
          ningún tramo con split y referencia (por ejemplo, no presentado).
        </Notice>
      )}

      <div className="card card-flush">
        <div className="card-title card-head">
          <h3>Por formato</h3>
          <span className="small muted">
            Cuentan los tramos con pérdida, salvo el último y los de referencia menor de 20 s ·
            error si pierdes más de {decimal(config.error_threshold_s, 0)} s y del{" "}
            {decimal(config.error_threshold_pct, 0)} %
          </span>
        </div>
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th>Formato</th>
                <th className="num">Carreras</th>
                <th className="num">Tramos</th>
                <th className="num">Errores</th>
                <th className="num">IR medio</th>
                <th className="num">Tasa de error</th>
                <th className="num">Pérdida por tramo</th>
              </tr>
            </thead>
            <tbody>
              {groups.map((g) => (
                <StatsRow key={g.format ?? "none"} label={groupLabel(g)} stats={g.stats} />
              ))}
              {groups.length > 1 && <StatsRow label="Total" stats={total} strong />}
            </tbody>
          </table>
        </div>
      </div>

      <div className="chart-panels">
        <h3 className="section-title">Gráficas por formato</h3>
        <FormatPerformancePanel groups={groups} total={total} />
        <FormatErrorRatePanel groups={groups} total={total} />
        <FormatLossPanel groups={groups} total={total} />
        {/* P7, P10, P11 y P13: sus secciones de paneles van aquí, con los mismos filtros. */}
      </div>
    </>
  );
}

const performance = (s: HistoryStats) =>
  s.mean_performance === null ? "—" : percent(s.mean_performance * 100);
const errorRate = (s: HistoryStats) => (s.error_rate === null ? "—" : percent(s.error_rate * 100));
const lossS = (s: HistoryStats) => (s.mean_loss_s === null ? "—" : `${decimal(s.mean_loss_s, 1)} s`);

function StatsRow({ label, stats, strong }: { label: string; stats: HistoryStats; strong?: boolean }) {
  return (
    <tr className={strong ? "total-row" : undefined}>
      <td className={strong ? "strong" : undefined}>{label}</td>
      <td className="num">{stats.races}</td>
      <td className="num muted">{stats.legs}</td>
      <td className="num">{stats.errors}</td>
      <td className="num">{performance(stats)}</td>
      <td className="num">{errorRate(stats)}</td>
      <td className="num">
        {lossS(stats)}
        {stats.mean_loss_pct !== null && (
          <span className="muted"> · {percent1(stats.mean_loss_pct)}</span>
        )}
      </td>
    </tr>
  );
}

export default HistoryScreen;
