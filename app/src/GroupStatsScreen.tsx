// Estadísticas de un grupo de atletas (#145, docs/historico.md, "Estadísticas de un grupo"): las
// pestañas y los paneles de Estadísticas con todos los miembros juntos, cada uno con sus umbrales.
// Solo salen los análisis que se pueden juntar sin aproximar: la consistencia no. Los números
// vienen del núcleo; aquí solo se dibujan, con los mismos paneles que Estadísticas.
import { useEffect, useState } from "react";
import { GroupStatsMember, GroupStatsView, HistoryFilter, athleteGroupStats } from "./api";
import { PanelsOpen } from "./charts/ChartPanel";
import { allHidden, usePanelVisibility } from "./panels";
import { Filters, HistoryTab, StatsRow, errorRate, lossS, performance } from "./HistoryScreen";
import {
  FormatErrorRatePanel,
  FormatLossPanel,
  FormatPerformancePanel,
  groupLabel,
  racesLabel,
} from "./HistoryPanels";
import { LegLengthPanel } from "./LegLengthPanel";
import { CommonErrorsPanel } from "./CommonErrorsPanel";
import { SlopePanel } from "./SlopePanel";
import { HistoryBreakdownPanel } from "./BreakdownPanel";
import { DaysOffPanel } from "./DaysOffPanel";
import { AfterErrorPanel } from "./AfterErrorPanel";
import { FatiguePanel } from "./FatiguePanel";
import { GroupDot } from "./GroupsScreen";
import { ChartIcon, EmptyState, Notice, PageHeader, SlidersIcon, Stat, TabItem, Tabs } from "./ui";

const NO_FILTER: HistoryFilter = { from: null, to: null, format: null };

/** Las pestañas de Estadísticas, contadas para el grupo. */
const TABS: (TabItem<HistoryTab> & { intro: string })[] = [
  { id: "summary", label: "Resumen", intro: "Cómo va el grupo en general y por formato." },
  {
    id: "where",
    label: "¿Dónde falla?",
    intro: "Qué tramos y qué terreno le cuestan más al grupo, y de qué tipo son sus errores.",
  },
  {
    id: "progress",
    label: "¿Cómo evoluciona?",
    intro: "Cómo entra en mapa el grupo tras días sin competir.",
  },
  {
    id: "body",
    label: "Cabeza y piernas",
    intro: "Qué pasa después de un error y si el cansancio lo anticipa.",
  },
];

const memberName = (m: GroupStatsMember) =>
  m.is_self
    ? m.runner.display_name === ""
      ? "Tú"
      : `${m.runner.display_name} (tú)`
    : m.runner.display_name || "Sin nombre";

function GroupStatsScreen({ group }: { group: number }) {
  const [filter, setFilter] = useState<HistoryFilter>(NO_FILTER);
  const [view, setView] = useState<GroupStatsView | null>(null);
  const [tab, setTab] = useState<HistoryTab>("summary");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    setError(null);
    athleteGroupStats(filter, group)
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
  }, [filter, group]);

  const athletes = view?.members.length ?? 0;
  return (
    <>
      <PageHeader
        title={view === null ? "Estadísticas del grupo" : view.name}
        subtitle={
          view === null
            ? "Cargando…"
            : `Estadísticas del grupo · ${athletes} ${athletes === 1 ? "atleta" : "atletas"}`
        }
      />
      {error !== null && <Notice kind="error">{error}</Notice>}
      {view !== null && view.description !== "" && <p className="muted">{view.description}</p>}

      {view !== null && athletes === 0 && (
        <div className="card">
          <EmptyState icon={<ChartIcon size={40} />} title="Aún no hay carreras">
            <p>Ningún miembro del grupo tiene carreras compartidas con sus tramos.</p>
          </EmptyState>
        </div>
      )}

      {view !== null && athletes > 0 && (
        <>
          <Filters filter={filter} onChange={setFilter} />
          <Members view={view} />
          {view.stats.history.total.races === 0 ? (
            <div className="card">
              <EmptyState icon={<ChartIcon size={40} />} title="Ninguna carrera con estos filtros">
                <button type="button" className="btn" onClick={() => setFilter(NO_FILTER)}>
                  Quitar filtros
                </button>
              </EmptyState>
            </div>
          ) : (
            <Summary view={view} tab={tab} onTab={setTab} />
          )}
        </>
      )}
    </>
  );
}

/** Quién cuenta: cada miembro con sus carreras, y los que no. */
function Members({ view }: { view: GroupStatsView }) {
  const problems = view.members.filter((m) => m.problem !== null);
  return (
    <div className="card">
      <div className="meta">
        <GroupDot color={view.color} />
        {view.members.map((m) => (
          <span
            key={m.runner.runner_id}
            className={m.races > 0 ? "pill pill-accent" : "pill"}
            title={m.problem ?? undefined}
          >
            {memberName(m)} · {racesLabel(m.races)}
          </span>
        ))}
      </div>
      <p className="small muted">
        Cada atleta cuenta con sus carreras y sus umbrales de error, como en sus Estadísticas.
        {view.missing > 0 &&
          (view.missing === 1
            ? " Un miembro no cuenta: ya no hay carreras suyas."
            : ` ${view.missing} miembros no cuentan: ya no hay carreras suyas.`)}
      </p>
      {problems.map((m) => (
        <Notice key={m.runner.runner_id} kind="warning">
          {memberName(m)} no cuenta: {m.problem}
        </Notice>
      ))}
    </div>
  );
}

/** Cifras del grupo y, en pestañas, la tabla por formato y los paneles de Estadísticas. */
function Summary({
  view,
  tab,
  onTab,
}: {
  view: GroupStatsView;
  tab: HistoryTab;
  onTab: (tab: HistoryTab) => void;
}) {
  const stats = view.stats;
  const { total, by_format: groups, races_without_data: withoutData } = stats.history;
  const visibility = usePanelVisibility();
  // El título de un apartado sale si queda alguno de sus paneles.
  const shown = (...ids: string[]) => !allHidden(visibility, ids);
  const intro = TABS.find((t) => t.id === tab)?.intro;
  return (
    <>
      <div className="stats">
        <Stat label="Carreras" value={total.races} detail={`de ${stats.runners} atletas`} />
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

      <div className="tabs-row">
        <Tabs tabs={TABS} current={tab} onChange={onTab} label="Preguntas" />
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
                    Todas las carreras y todos los tramos del grupo juntos. Cuentan los tramos con
                    pérdida, salvo el último y los de referencia menor de 20 s
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
                        <StatsRow
                          key={g.format ?? "none"}
                          label={groupLabel(g)}
                          stats={g.stats}
                          consistency={false}
                        />
                      ))}
                      {groups.length > 1 && (
                        <StatsRow label="Total" stats={total} strong consistency={false} />
                      )}
                    </tbody>
                  </table>
                </div>
              </div>
              <div className="panel-grid">
                <FormatPerformancePanel groups={groups} total={total} />
                <FormatErrorRatePanel groups={groups} total={total} />
                <FormatLossPanel groups={groups} total={total} />
              </div>
            </>
          )}

          {tab === "where" && (
            <div className="panel-grid">
              {shown("leg-length") && (
                <LegLengthPanel buckets={stats.by_leg_length} total={total} />
              )}
              {shown("common-errors") && <CommonErrorsPanel data={stats.common_errors} />}
              {shown("slope-performance", "slope-error-rate") && (
                <SlopePanel slope={stats.by_slope} />
              )}
              {shown("breakdown") && <HistoryBreakdownPanel breakdown={stats.loss_breakdown} />}
            </div>
          )}

          {tab === "progress" && (
            <>
              <p className="small muted">
                La consistencia no sale en un grupo: la de cada atleta ya es una media de sus
                carreras y no se puede juntar sin aproximar. Está en las Estadísticas de cada uno.
              </p>
              <div className="panel-grid">
                {shown("days-off-entry", "days-off-start") && (
                  <DaysOffPanel
                    buckets={stats.days_off.buckets}
                    withoutPrevious={stats.days_off.without_previous}
                    total={total}
                  />
                )}
              </div>
            </>
          )}

          {tab === "body" && (
            <div className="panel-grid">
              {shown("after-error", "clean-streaks") && (
                <AfterErrorPanel data={stats.after_error} total={total} />
              )}
              {shown("fatigue-drift", "fatigue-heart-rate", "fatigue-effort") && (
                <FatiguePanel data={stats.fatigue} />
              )}
            </div>
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

export default GroupStatsScreen;
