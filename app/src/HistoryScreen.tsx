// Estadísticas (vista histórica, P6, docs/app.md): todas las carreras del usuario agregadas por
// formato, con filtros de fechas y formato. Los análisis que se apoyan en el histórico (P7, P10,
// P11, P13…, y el físico, #214) van en pestañas por pregunta (#130), con los mismos filtros.
import { useEffect, useState } from "react";
import { useViewer } from "./viewer";
import { Subject, useSubject } from "./subject";
import {
  FORMAT_LABELS,
  EventKind,
  HistoryFilter,
  HistoryInsightTarget,
  HistoryRaceRow,
  HistoryStats,
  HistoryView,
  RaceFormat,
  decimal,
  getHistory,
  spread,
  NO_FILTER,
} from "./api";
import { ConsistencyPanel } from "./ConsistencyPanel";
import { RivalsPanel } from "./RivalsPanel";
import { TrendPanel } from "./TrendPanel";
import { DaysOffPanel } from "./DaysOffPanel";
import {
  FormatErrorRatePanel,
  FormatLossPanel,
  FormatPerformancePanel,
  groupLabel,
  percent,
  percent1,
  racesLabel,
} from "./HistoryPanels";
import { ChartIcon, EmptyState, Notice, PageHeader, SlidersIcon, Stat, TabItem, Tabs } from "./ui";
import { PanelsOpen } from "./charts/ChartPanel";
import { allHidden, usePanelVisibility } from "./panels";
import { LegLengthPanel } from "./LegLengthPanel";
import { SlopePanel } from "./SlopePanel";
import { HistoryBreakdownPanel } from "./BreakdownPanel";
import { AfterErrorPanel } from "./AfterErrorPanel";
import { CommonErrorsPanel } from "./CommonErrorsPanel";
import { FatiguePanel } from "./FatiguePanel";
import { PhysicalHistoryTab } from "./PhysicalHistoryTab";
import { InsightPlace, Insights, useInsightFocus } from "./Insights";

/** Pestañas de Estadísticas: una por pregunta (#130). */
export type HistoryTab = "summary" | "where" | "progress" | "body" | "physical";

/**
 * Las pestañas, con las preguntas y las explicaciones en la voz de quien se mira (#165): el propio
 * usuario, un atleta o un grupo (#145; la consistencia no sale en un grupo).
 */
export function historyTabs(who: Subject): (TabItem<HistoryTab> & { intro: string })[] {
  return [
    {
      id: "summary",
      label: "Resumen",
      intro: who.pick(
        "Cómo vas en general y por formato.",
        "Cómo va en general y por formato.",
        "Cómo va el grupo en general y por formato.",
      ),
    },
    {
      id: "where",
      label: who.pick("¿Dónde fallo?", "¿Dónde falla?"),
      intro: who.pick(
        "Qué tramos y qué terreno te cuestan más, y de qué tipo son tus errores.",
        "Qué tramos y qué terreno le cuestan más, y de qué tipo son sus errores.",
        "Qué tramos y qué terreno le cuestan más al grupo, y de qué tipo son sus errores.",
      ),
    },
    {
      id: "progress",
      label: who.pick("¿Cómo evoluciono?", "¿Cómo evoluciona?"),
      intro: who.pick(
        "Si estás mejorando, tu regularidad y cómo entras en mapa tras días sin competir.",
        "Si está mejorando, su regularidad y cómo entra en mapa tras días sin competir.",
        "Cómo entra en mapa el grupo tras días sin competir.",
      ),
    },
    {
      id: "body",
      label: "Cabeza y piernas",
      intro: "Qué pasa después de un error y si el cansancio lo anticipa.",
    },
    {
      id: "physical",
      label: "Físico",
      intro: who.pick(
        "Cómo evoluciona tu rendimiento físico, carrera a carrera: eficiencia, pulso, velocidad, y cómo corres en cada pendiente y si te caes al final.",
        "Cómo evoluciona su rendimiento físico, carrera a carrera: eficiencia, pulso, velocidad, y cómo corre en cada pendiente y si se cae al final.",
      ),
    },
  ];
}
const FORMATS: RaceFormat[] = ["sprint", "middle", "long"];

/** Pestaña y panel donde está el análisis que justifica cada frase del resumen (#126, #144). */
const INSIGHT_PLACES: Record<HistoryInsightTarget, InsightPlace<HistoryTab>> = {
  formats: { tab: "summary", panels: ["format-error-rate"] },
  leg_length: { tab: "where", panels: ["leg-length"] },
  common_errors: { tab: "where", panels: ["common-errors"] },
  slope: { tab: "where", panels: ["slope-performance"] },
  loss_breakdown: { tab: "where", panels: ["breakdown"] },
  after_error: { tab: "body", panels: ["after-error"] },
  fatigue: { tab: "body", panels: ["fatigue-heart-rate"] },
  days_off: { tab: "progress", panels: ["days-off-entry"] },
  consistency: { tab: "progress", panels: ["consistency"] },
  trend: { tab: "progress", panels: ["trend-performance", "trend-error-rate"] },
};

function HistoryScreen({
  tab,
  onTab,
  onImport,
  onOpen,
}: {
  tab: HistoryTab;
  onTab: (tab: HistoryTab) => void;
  onImport: () => void;
  /** Abre una carrera de la lista. */
  onOpen: (resultId: number) => void;
}) {
  const [filter, setFilter] = useState<HistoryFilter>(NO_FILTER);
  const [view, setView] = useState<HistoryView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const { readOnly, runnerName } = useViewer();
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
        title="Estadísticas"
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
          {readOnly ? (
            <EmptyState icon={<ChartIcon size={40} />} title="Aún no hay carreras">
              <p>
                {runnerName === null
                  ? "Elige un corredor en la barra lateral."
                  : "El histórico junta las carreras compartidas con sus tramos."}
              </p>
            </EmptyState>
          ) : (
            <EmptyState icon={<ChartIcon size={40} />} title="Aún no hay carreras">
              <p>El histórico junta todas tus carreras. Importa alguna para verlo.</p>
              <button type="button" className="btn btn-primary btn-lg" onClick={onImport}>
                Importar carrera
              </button>
            </EmptyState>
          )}
        </div>
      )}

      {view !== null && view.all_races > 0 && (
        <>
          <Filters filter={filter} onChange={setFilter} />
          {reversed && (
            <Notice kind="warning">
              La fecha «desde» es posterior a «hasta»: no entra ninguna carrera.
            </Notice>
          )}
          {view.history.total.races === 0 ? (
            !reversed && (
              <div className="card">
                <EmptyState
                  icon={<ChartIcon size={40} />}
                  title="Ninguna carrera con estos filtros"
                >
                  <button type="button" className="btn" onClick={() => setFilter(NO_FILTER)}>
                    Quitar filtros
                  </button>
                </EmptyState>
              </div>
            )
          ) : (
            <Summary view={view} tab={tab} onTab={onTab} onOpen={onOpen} />
          )}
        </>
      )}
    </>
  );
}

/** Opciones del filtro de tipo (#195): `null` = las dos. */
const KIND_FILTERS: { kind: EventKind | null; label: string }[] = [
  { kind: null, label: "Todo" },
  { kind: "race", label: "Carreras" },
  { kind: "training", label: "Entrenamientos" },
];

/** Filtros: fechas (incluidas), formato y tipo. */
export function Filters({
  filter,
  onChange,
}: {
  filter: HistoryFilter;
  onChange: (filter: HistoryFilter) => void;
}) {
  const date = (value: string) => (value === "" ? null : value);
  const active =
    filter.from !== null || filter.to !== null || filter.format !== null || filter.kind !== null;
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
      <div className="field">
        <span className="field-label" id="history-kind">
          Tipo
        </span>
        <div className="segmented" role="radiogroup" aria-labelledby="history-kind">
          {KIND_FILTERS.map(({ kind, label }) => (
            <label key={kind ?? "all"}>
              <input
                type="radio"
                name="history-kind"
                checked={filter.kind === kind}
                onChange={() => onChange({ ...filter, kind })}
              />
              {label}
            </label>
          ))}
        </div>
      </div>
      {active && (
        <button type="button" className="btn btn-ghost" onClick={() => onChange(NO_FILTER)}>
          Quitar filtros
        </button>
      )}
    </div>
  );
}

/** Cifras del total y, en pestañas, la tabla por formato, las carreras y los paneles. */
function Summary({
  view,
  tab,
  onTab,
  onOpen,
}: {
  view: HistoryView;
  tab: HistoryTab;
  onTab: (tab: HistoryTab) => void;
  onOpen: (resultId: number) => void;
}) {
  const { total, by_format: groups, races_without_data: withoutData } = view.history;
  const config = view.config;
  const who = useSubject();
  const tabs = historyTabs(who);
  const visibility = usePanelVisibility();
  // El título de un apartado sale si queda alguno de sus paneles.
  const shown = (...ids: string[]) => !allHidden(visibility, ids);
  const intro = tabs.find((t) => t.id === tab)?.intro;
  const focus = useInsightFocus(INSIGHT_PLACES, tab, onTab);
  return (
    <>
      <Insights insights={view.insights} focus={focus} />

      <div className="stats">
        <Stat label="Carreras" value={total.races} />
        <Stat
          label="IR medio"
          value={performance(total)}
          detail={`Consistencia ${spread(total.mean_consistency, 0)}`}
          hint={`Consistencia media: la de cada carrera es cuánto varía ${who.yourOne} IR de un tramo a otro. Menor = más consistente.`}
        />
        <Stat label="Tasa de error" value={errorRate(total)} />
        <Stat label="Pérdida por tramo" value={lossS(total)} />
      </div>

      {withoutData > 0 && (
        <Notice>
          {withoutData === 1 ? "Una carrera no cuenta" : `${withoutData} carreras no cuentan`}: sin
          ningún tramo con split y referencia (por ejemplo, no presentado).
        </Notice>
      )}

      <div className="tabs-row">
        <Tabs tabs={tabs} current={tab} onChange={onTab} label="Preguntas" />
        {visibility !== null && (
          <button type="button" className="btn btn-ghost" onClick={visibility.customize}>
            <SlidersIcon size={16} /> Personalizar
          </button>
        )}
      </div>
      {intro !== undefined && <p className="small muted">{intro}</p>}

      <div className="tab-panel" role="tabpanel">
        <PanelsOpen.Provider value={true}>
          {tab === "summary" && (
            <>
              <div className="card card-flush">
                <div className="card-title card-head">
                  <h3>Por formato</h3>
                  <span className="small muted">
                    Cuentan los tramos con pérdida, salvo el último y los de referencia menor de 20
                    s · error si {who.pick("pierdes", "pierde")} más de{" "}
                    {decimal(config.error_threshold_s, 0)} s y del{" "}
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
                        <th className="num">Consistencia</th>
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
              <div className="panel-grid">
                <FormatPerformancePanel groups={groups} total={total} />
                <FormatErrorRatePanel groups={groups} total={total} />
                <FormatLossPanel groups={groups} total={total} />
              </div>
              <RaceRows races={view.races} onOpen={onOpen} />
              <RivalsPanel filter={view.history.filter} revision={view} />
            </>
          )}

          {tab === "where" && (
            <div className="panel-grid">
              {shown("leg-length") && <LegLengthPanel buckets={view.by_leg_length} total={total} />}
              {shown("common-errors") && <CommonErrorsPanel data={view.common_errors} />}
              {shown("slope-performance", "slope-error-rate") && (
                <SlopePanel slope={view.by_slope} />
              )}
              {shown("breakdown", "breakdown-attack") && (
                <HistoryBreakdownPanel breakdown={view.loss_breakdown} />
              )}
            </div>
          )}

          {tab === "progress" && (
            <div className="panel-grid">
              {shown("trend-performance", "trend-error-rate") && (
                <TrendPanel
                  performance={view.trends.performance}
                  errorRate={view.trends.error_rate}
                />
              )}
              {shown("consistency") && <ConsistencyPanel races={view.races} total={total} />}
              {shown("days-off-entry", "days-off-start") && (
                <DaysOffPanel
                  buckets={view.days_off.buckets}
                  withoutPrevious={view.days_off.without_previous}
                  total={total}
                />
              )}
            </div>
          )}

          {tab === "body" && (
            <div className="panel-grid">
              {shown("after-error", "clean-streaks") && (
                <AfterErrorPanel data={view.after_error} total={total} />
              )}
              {shown("fatigue-drift", "fatigue-heart-rate", "fatigue-effort") && (
                <FatiguePanel data={view.fatigue} />
              )}
            </div>
          )}

          {tab === "physical" && (
            <PhysicalHistoryTab filter={view.history.filter} onOpen={onOpen} />
          )}
        </PanelsOpen.Provider>
        {visibility !== null && visibility.hidden.size > 0 && (
          <p className="small muted">
            Hay paneles ocultos.{" "}
            <button type="button" className="btn-link" onClick={visibility.customize}>
              Elegir qué paneles ves
            </button>
          </p>
        )}
      </div>
    </>
  );
}

export const performance = (s: HistoryStats) =>
  s.mean_performance === null ? "—" : percent(s.mean_performance * 100);
export const errorRate = (s: HistoryStats) =>
  s.error_rate === null ? "—" : percent(s.error_rate * 100);
export const lossS = (s: HistoryStats) =>
  s.mean_loss_s === null ? "—" : `${decimal(s.mean_loss_s, 1)} s`;

/** Fila de la tabla por formato. Sin `consistency`, sin su columna (grupos, #145). */
export function StatsRow({
  label,
  stats,
  strong,
  consistency = true,
}: {
  label: string;
  stats: HistoryStats;
  strong?: boolean;
  consistency?: boolean;
}) {
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
      {consistency && <td className="num">{spread(stats.mean_consistency)}</td>}
    </tr>
  );
}

/** Las carreras que entran con estos filtros, cada una con sus números (#98). */
function RaceRows({
  races,
  onOpen,
}: {
  races: HistoryRaceRow[];
  onOpen: (resultId: number) => void;
}) {
  return (
    <div className="card card-flush">
      <div className="card-title card-head">
        <h3>Carreras</h3>
        <span className="small muted">Las que entran con estos filtros. Abre una para verla.</span>
      </div>
      <div className="table-wrap">
        <table className="table">
          <thead>
            <tr>
              <th className="num">Fecha</th>
              <th>Carrera</th>
              <th>Categoría</th>
              <th className="num">IR</th>
              <th className="num">Tramos</th>
              <th className="num">Errores</th>
              <th className="num">Tasa de error</th>
              <th className="num">Pérdida por tramo</th>
            </tr>
          </thead>
          <tbody>
            {races.map((race) => (
              <tr
                key={race.result_id}
                className="clickable"
                tabIndex={0}
                onClick={() => onOpen(race.result_id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") onOpen(race.result_id);
                }}
              >
                <td className="num muted">{race.date}</td>
                <td>
                  <div className="strong">{race.name ?? "Sin nombre"}</div>
                  <div className="meta">
                    <span className={race.format === null ? "pill" : "pill pill-accent"}>
                      {race.format === null ? "Sin formato" : FORMAT_LABELS[race.format]}
                    </span>
                    {race.kind === "training" && <span className="pill">Entrenamiento</span>}
                  </div>
                </td>
                <td>{race.class_name}</td>
                {race.stats === null ? (
                  <td className="muted" colSpan={5}>
                    No cuenta: sin tramos con split y referencia
                  </td>
                ) : (
                  <>
                    <td className="num">{performance(race.stats)}</td>
                    <td className="num muted">{race.stats.legs}</td>
                    <td className="num">{race.stats.errors}</td>
                    <td className="num">{errorRate(race.stats)}</td>
                    <td className="num">
                      {lossS(race.stats)}
                      {race.stats.mean_loss_pct !== null && (
                        <span className="muted"> · {percent1(race.stats.mean_loss_pct)}</span>
                      )}
                    </td>
                  </>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export default HistoryScreen;
