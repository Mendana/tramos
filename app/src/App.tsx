import { ReactNode, useCallback, useEffect, useState } from "react";
import {
  AthleteNews,
  RaceRow,
  ReceiveReport,
  Role,
  RunnerViewInfo,
  SharingSettings,
  athleteNews,
  chooseRole,
  coreVersion,
  getSettings,
  listRaces,
  receivePackages,
  roleChosen,
  viewRunner,
  viewedRunner,
} from "./api";
import AthletesScreen from "./AthletesScreen";
import GroupScreen from "./GroupScreen";
import GroupsScreen from "./GroupsScreen";
import CompareGroupsScreen from "./CompareGroupsScreen";
import HelpScreen from "./help/HelpScreen";
import { HelpPageId, helpTitle } from "./help/pages";
import { helpFor } from "./help/views";
import HistoryScreen, { HistoryTab } from "./HistoryScreen";
import Home from "./Home";
import { PanelVisibilityProvider } from "./panels";
import ImportScreen from "./ImportScreen";
import RaceList, { RACE_LIST_START, RaceListState } from "./RaceList";
import RaceView, { RaceTab } from "./RaceView";
import SettingsView from "./SettingsView";
import Welcome from "./Welcome";
import { ViewerContext } from "./viewer";
import {
  AthletesIcon,
  ChartIcon,
  ChevronLeft,
  ControlFlag,
  EyeIcon,
  GroupIcon,
  HelpIcon,
  HomeIcon,
  ListIcon,
  Notice,
  SlidersIcon,
  TagIcon,
  UploadIcon,
  UserIcon,
} from "./ui";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/components.css";
import "./styles/charts.css";
import "./styles/map.css";

/** Cada cuánto busca quien entrena paquetes nuevos en la carpeta compartida. */
const RECEIVE_EVERY_MS = 60_000;

/**
 * Si entrena y hay carpeta, importa los paquetes nuevos de sus atletas al abrir la app y cada
 * minuto (`docs/paquete.md`, "Carpeta compartida"). Devuelve lo último que ha encontrado.
 */
function useReceivePackages(sharing: SharingSettings | null, onReceived: () => void) {
  const [report, setReport] = useState<ReceiveReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const active = sharing?.coach === true && sharing.folder !== null;
  useEffect(() => {
    setReport(null);
    setError(null);
    if (!active) return;
    const receive = () => {
      receivePackages()
        .then((r) => {
          setReport(r);
          setError(null);
          if (r.created + r.replaced > 0) onReceived();
        })
        .catch((err: unknown) => setError(String(err)));
    };
    receive();
    const timer = window.setInterval(receive, RECEIVE_EVERY_MS);
    return () => window.clearInterval(timer);
    // `onReceived` es estable (useCallback).
  }, [active, sharing?.folder, onReceived]);
  return { active, report, error };
}

function ReceiveStatus({ report, error }: { report: ReceiveReport | null; error: string | null }) {
  if (error !== null) return <div title={error}>Carpeta compartida: no se puede leer</div>;
  if (report === null) return <div>Carpeta compartida: buscando…</div>;
  const problems = report.problems.length;
  return (
    <div title={report.problems.join("\n") || undefined}>
      Recibidos: {report.packages} {report.packages === 1 ? "paquete" : "paquetes"} de{" "}
      {report.runners} {report.runners === 1 ? "atleta" : "atletas"}
      {problems > 0 && ` · ${problems} sin leer`}
    </div>
  );
}

/**
 * Pantalla abierta. Una carrera se abre desde la lista (o al acabar de importarla). Cada pantalla
 * (y cada pestaña) tiene su página de ayuda en `help/views.ts`: si añades una, dale página.
 */
export type Screen =
  | { kind: "home" }
  | { kind: "races" }
  | { kind: "race"; resultId: number; tab?: RaceTab }
  | { kind: "history" }
  | { kind: "athletes" }
  | { kind: "group"; groupId?: number }
  | { kind: "groups" }
  | { kind: "compare-groups" }
  | { kind: "import" }
  | { kind: "profile" }
  | { kind: "settings" }
  | { kind: "help"; page: HelpPageId };

/** Pantallas que se recuerdan para el botón de volver. */
const BACK_LIMIT = 30;

function NavItem({
  icon,
  label,
  current,
  count,
  countLabel,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  current: boolean;
  /** Contador a la derecha (p. ej. errores por revisar); no sale si es 0. */
  count?: number;
  countLabel?: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="nav-item"
      aria-current={current ? "page" : undefined}
      onClick={onClick}
    >
      {icon}
      <span className="nav-label">{label}</span>
      {count !== undefined && count > 0 && (
        <span className="nav-count" title={countLabel} aria-label={countLabel}>
          {count}
        </span>
      )}
    </button>
  );
}

function NavSection({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="nav-section" role="group" aria-label={label}>
      <span className="nav-section-label">{label}</span>
      {children}
    </div>
  );
}

/** Migas de pan: dónde se está. Las anteriores a la última llevan a su pantalla. */
interface Crumb {
  label: string;
  to?: Screen;
}

function crumbsFor(screen: Screen, races: RaceRow[] | null, runnerName: string | null): Crumb[] {
  const racesLabel = runnerName === null ? "Mis carreras" : "Carreras";
  const own: Crumb[] = (() => {
    switch (screen.kind) {
      case "home":
        return [{ label: "Inicio" }];
      case "races":
        return [{ label: racesLabel }];
      case "race": {
        const race = races?.find((r) => r.result_id === screen.resultId);
        return [{ label: racesLabel, to: { kind: "races" } }, { label: race?.name ?? "Carrera" }];
      }
      case "history":
        return [{ label: "Estadísticas" }];
      case "athletes":
        return [{ label: "Mis atletas" }];
      case "group":
        return [{ label: "Comparar atletas" }];
      case "groups":
        return [{ label: "Grupos" }];
      case "compare-groups":
        return [{ label: "Grupos", to: { kind: "groups" } }, { label: "Comparar grupos" }];
      case "import":
        return [{ label: "Importar" }];
      case "profile":
        return [{ label: "Mi perfil" }];
      case "settings":
        return [{ label: "Ajustes" }];
      case "help":
        return screen.page === "indice"
          ? [{ label: "Ayuda" }]
          : [
              { label: "Ayuda", to: { kind: "help", page: "indice" } },
              { label: helpTitle(screen.page) },
            ];
    }
  })();
  // Se ve a un atleta: su nombre va delante, salvo en lo que es de todos.
  const general =
    screen.kind === "athletes" ||
    screen.kind === "group" ||
    screen.kind === "groups" ||
    screen.kind === "compare-groups" ||
    screen.kind === "settings" ||
    screen.kind === "help";
  return runnerName === null || general ? own : [{ label: runnerName }, ...own];
}

function TopBar({
  crumbs,
  onBack,
  onNavigate,
  onHelp,
}: {
  crumbs: Crumb[];
  onBack: (() => void) | null;
  onNavigate: (screen: Screen) => void;
  /** Abre la ayuda de esta pantalla; `null` en la propia ayuda. */
  onHelp: (() => void) | null;
}) {
  return (
    <header className="topbar">
      {onBack !== null && (
        <button
          type="button"
          className="btn btn-ghost btn-icon"
          onClick={onBack}
          aria-label="Volver"
          title="Volver"
        >
          <ChevronLeft />
        </button>
      )}
      <nav className="crumbs" aria-label="Estás en">
        {crumbs.map((crumb, i) => {
          const last = i === crumbs.length - 1;
          const to = crumb.to;
          return (
            <span key={`${i}-${crumb.label}`} className="crumb">
              {i > 0 && (
                <span className="crumb-sep" aria-hidden="true">
                  /
                </span>
              )}
              {last || to === undefined ? (
                <span
                  className={last ? "crumb-here" : undefined}
                  aria-current={last ? "page" : undefined}
                >
                  {crumb.label}
                </span>
              ) : (
                <button type="button" className="crumb-link" onClick={() => onNavigate(to)}>
                  {crumb.label}
                </button>
              )}
            </span>
          );
        })}
      </nav>
      {onHelp !== null && (
        <button
          type="button"
          className="btn btn-ghost btn-icon topbar-help"
          onClick={onHelp}
          aria-label="Ayuda de esta pantalla"
          title="Ayuda de esta pantalla"
        >
          <HelpIcon />
        </button>
      )}
    </header>
  );
}

function App() {
  const [version, setVersion] = useState<string | null>(null);
  const [races, setRaces] = useState<RaceRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [screen, setScreen] = useState<Screen>({ kind: "home" });
  // Pantallas anteriores, para volver.
  const [previous, setPrevious] = useState<Screen[]>([]);
  // Filtros y página de la lista de carreras: siguen ahí al volver de una carrera.
  const [raceList, setRaceList] = useState<RaceListState>(RACE_LIST_START);
  // Pestaña de Estadísticas: la última que se miró.
  const [historyTab, setHistoryTab] = useState<HistoryTab>("summary");
  const [chosen, setChosen] = useState<boolean | null>(null);
  const [sharing, setSharing] = useState<SharingSettings | null>(null);
  // Si entrena: lo nuevo de cada atleta y el que se está viendo (`null` = lo propio, editable).
  const [news, setNews] = useState<AthleteNews[]>([]);
  const [runner, setRunner] = useState<RunnerViewInfo | null>(null);
  // Errores por revisar de lo propio: `races` es de quien se ve.
  const [unreviewed, setUnreviewed] = useState(0);
  const coach = sharing?.coach === true;
  const viewing = runner !== null;

  const refresh = useCallback(() => {
    listRaces()
      .then(setRaces)
      .catch((err: unknown) => setError(String(err)));
    getSettings()
      .then((s) => setSharing(s.sharing))
      .catch((err: unknown) => setError(String(err)));
  }, []);

  const loadNews = useCallback(() => {
    athleteNews()
      .then(setNews)
      .catch((err: unknown) => setError(String(err)));
  }, []);

  // Ha llegado algo por la carpeta: el atleta que se ve ya está al día en el núcleo.
  const onReceived = useCallback(() => {
    loadNews();
    viewedRunner()
      .then(setRunner)
      .catch((err: unknown) => setError(String(err)));
    refresh();
  }, [refresh, loadNews]);
  const receiving = useReceivePackages(sharing, onReceived);

  useEffect(() => {
    coreVersion()
      .then(setVersion)
      .catch((err: unknown) => setError(String(err)));
    roleChosen()
      .then(setChosen)
      .catch((err: unknown) => setError(String(err)));
    refresh();
  }, [refresh]);

  useEffect(() => {
    if (!viewing && races !== null) {
      setUnreviewed(races.reduce((n, r) => n + r.unreviewed_count, 0));
    }
  }, [viewing, races]);

  /**
   * Pasa a ver a un atleta o, con `null`, vuelve a lo propio, y abre `next` (o, si no, la misma
   * pantalla, salvo una carrera, que era de la otra base).
   */
  const selectRunner = useCallback(
    (runnerId: string | null, next?: Screen) => {
      viewRunner(runnerId)
        .then((info) => {
          setRunner(info);
          // Al entrar en un atleta, sus novedades se quedan a cero.
          if (runnerId !== null) loadNews();
          // Las pantallas anteriores y los filtros eran de otra base.
          setPrevious([]);
          setRaceList(RACE_LIST_START);
          setScreen((s) => next ?? (s.kind === "race" ? { kind: "races" } : s));
          refresh();
        })
        .catch((err: unknown) => setError(String(err)));
    },
    [refresh, loadNews],
  );

  // Si entrena, lo nuevo de sus atletas; si deja de entrenar, ya no se ve a nadie.
  useEffect(() => {
    if (!coach) {
      setNews([]);
      setRunner(null);
      setScreen((s) =>
        s.kind === "athletes" ||
        s.kind === "group" ||
        s.kind === "groups" ||
        s.kind === "compare-groups"
          ? { kind: "home" }
          : s,
      );
      return;
    }
    loadNews();
  }, [coach, loadNews]);

  const choose = (role: Role) => {
    chooseRole(role)
      .then(() => {
        setChosen(true);
        // Para recibir lo de los atletas hace falta la carpeta.
        if (role !== "runner") navigate({ kind: "settings" });
        refresh();
      })
      .catch((err: unknown) => setError(String(err)));
  };

  const navigate = useCallback(
    (next: Screen) => {
      setPrevious((stack) => [...stack, screen].slice(-BACK_LIMIT));
      setScreen(next);
    },
    [screen],
  );
  const goBack = () => {
    const last = previous[previous.length - 1];
    if (last === undefined) return;
    setPrevious(previous.slice(0, -1));
    setScreen(last);
  };
  /** Una pantalla de lo propio: si se está viendo a un atleta, primero se vuelve a lo propio. */
  const goOwn = (next: Screen) => (viewing ? selectRunner(null, next) : navigate(next));
  const showImport = () => goOwn({ kind: "import" });
  // Las carreras y las estadísticas son de quien se ve; la lista, la de esa misma base.
  const showRaces = () => navigate({ kind: "races" });
  const openRace = (resultId: number, tab?: RaceTab) => navigate({ kind: "race", resultId, tab });

  if (chosen === false) {
    return (
      <>
        {error !== null && <Notice kind="error">{error}</Notice>}
        <Welcome onChoose={choose} />
      </>
    );
  }

  const viewer = {
    readOnly: viewing,
    runnerName: runner?.runner.display_name ?? null,
  };
  const at = (...kinds: Screen["kind"][]) => kinds.includes(screen.kind);
  const goTo = (kind: Exclude<Screen["kind"], "race" | "help">) => navigate({ kind });
  const openHelp = (page: HelpPageId) => navigate({ kind: "help", page });
  // Lo que se ve de un atleta (y lleva la franja de solo lectura).
  const athleteScreen = viewing && at("races", "race", "history");
  const newRaces = news.reduce((n, a) => n + a.new_races.length, 0);

  return (
    <ViewerContext.Provider value={viewer}>
      <PanelVisibilityProvider>
        <div className="shell">
          <aside className="sidebar">
            <div className="brand">
              <ControlFlag />
              Tramos
            </div>
            <nav className="nav" aria-label="Secciones">
              <NavItem
                icon={<HomeIcon />}
                label="Inicio"
                current={!viewing && at("home")}
                onClick={() => goOwn({ kind: "home" })}
              />
              <NavSection label="Lo mío">
                <NavItem
                  icon={<ListIcon />}
                  label="Mis carreras"
                  current={!viewing && at("races", "race")}
                  count={unreviewed}
                  countLabel={`${unreviewed} ${unreviewed === 1 ? "error" : "errores"} por revisar`}
                  onClick={() => goOwn({ kind: "races" })}
                />
                <NavItem
                  icon={<ChartIcon />}
                  label="Estadísticas"
                  current={!viewing && at("history")}
                  onClick={() => goOwn({ kind: "history" })}
                />
                <NavItem
                  icon={<UploadIcon />}
                  label="Importar"
                  current={at("import")}
                  onClick={showImport}
                />
              </NavSection>
              {coach && (
                <NavSection label="Atletas">
                  <NavItem
                    icon={<AthletesIcon />}
                    label="Mis atletas"
                    current={at("athletes")}
                    count={newRaces}
                    countLabel={`${newRaces} ${newRaces === 1 ? "carrera nueva" : "carreras nuevas"}`}
                    onClick={() => goTo("athletes")}
                  />
                  {viewing && runner !== null && (
                    <>
                      <span className="nav-runner small" title="El atleta que estás viendo">
                        {runner.runner.display_name || "Sin nombre"}
                      </span>
                      <NavItem
                        icon={<ListIcon />}
                        label="Carreras"
                        current={at("races", "race")}
                        onClick={showRaces}
                      />
                      <NavItem
                        icon={<ChartIcon />}
                        label="Estadísticas"
                        current={at("history")}
                        onClick={() => goTo("history")}
                      />
                    </>
                  )}
                  <NavItem
                    icon={<GroupIcon />}
                    label="Comparar atletas"
                    current={at("group")}
                    onClick={() => goTo("group")}
                  />
                  <NavItem
                    icon={<TagIcon />}
                    label="Grupos"
                    current={at("groups", "compare-groups")}
                    onClick={() => goTo("groups")}
                  />
                </NavSection>
              )}
              <NavSection label="Cuenta">
                <NavItem
                  icon={<UserIcon />}
                  label="Mi perfil"
                  current={at("profile")}
                  onClick={() => goOwn({ kind: "profile" })}
                />
                <NavItem
                  icon={<SlidersIcon />}
                  label="Ajustes"
                  current={at("settings")}
                  onClick={() => goTo("settings")}
                />
                <NavItem
                  icon={<HelpIcon />}
                  label="Ayuda"
                  current={at("help")}
                  onClick={() => openHelp("indice")}
                />
              </NavSection>
            </nav>
            <div className="sidebar-footer">
              {receiving.active && (
                <ReceiveStatus report={receiving.report} error={receiving.error} />
              )}
              <div>Núcleo {version === null ? "…" : `v${version}`}</div>
            </div>
          </aside>

          <main className="content">
            <TopBar
              crumbs={crumbsFor(screen, races, viewer.runnerName)}
              onBack={previous.length > 0 ? goBack : null}
              onNavigate={navigate}
              onHelp={screen.kind === "help" ? null : () => openHelp(helpFor(screen, historyTab))}
            />
            {/* Otro corredor, otras pantallas: no se arrastra nada del anterior. */}
            <div className="page" key={runner?.runner.runner_id ?? "self"}>
              {athleteScreen && runner !== null && (
                <div className="viewing-banner" role="status">
                  <EyeIcon />
                  <span className="viewing-banner-text">
                    Estás viendo a <strong>{runner.runner.display_name || "un atleta"}</strong>.
                    Solo lectura: no se puede etiquetar ni cambiar nada.
                  </span>
                  <button
                    type="button"
                    className="btn"
                    onClick={() => selectRunner(null, { kind: "home" })}
                  >
                    <ChevronLeft size={16} />
                    Volver a lo mío
                  </button>
                </div>
              )}
              {error !== null && (
                <Notice kind="error">No se pudo consultar el núcleo: {error}</Notice>
              )}
              {athleteScreen && runner !== null && runner.problems.length > 0 && (
                <Notice kind="warning">
                  Algunos paquetes de {runner.runner.display_name} no se han podido leer:{" "}
                  {runner.problems.join("; ")}
                </Notice>
              )}
              {screen.kind === "home" && !viewing && (
                <Home
                  races={races}
                  news={coach ? news : null}
                  onOpen={openRace}
                  onImport={showImport}
                  onRaces={showRaces}
                  onHistory={() => goTo("history")}
                  onAthletes={() => goTo("athletes")}
                  onOpenRunner={(runnerId) => selectRunner(runnerId, { kind: "races" })}
                />
              )}
              {screen.kind === "races" && (
                <RaceList
                  races={races}
                  state={raceList}
                  onStateChange={setRaceList}
                  onOpen={openRace}
                  onImport={showImport}
                  summaryOnly={runner?.summary_only ?? []}
                />
              )}
              {screen.kind === "race" && (
                <RaceView
                  resultId={screen.resultId}
                  tab={screen.tab ?? "summary"}
                  // Cambiar de pestaña no es otra pantalla: no entra en «volver».
                  onTab={(tab) => setScreen({ ...screen, tab })}
                  onChanged={refresh}
                />
              )}
              {screen.kind === "history" && (
                <HistoryScreen
                  tab={historyTab}
                  onTab={setHistoryTab}
                  onImport={showImport}
                  onOpen={openRace}
                />
              )}
              {screen.kind === "athletes" && coach && (
                <AthletesScreen
                  onOpenRunner={(runnerId) => selectRunner(runnerId, { kind: "races" })}
                />
              )}
              {screen.kind === "group" && coach && (
                <GroupScreen
                  group={screen.groupId ?? null}
                  // Cambiar de grupo no es otra pantalla: no entra en «volver».
                  onGroup={(groupId) => setScreen({ kind: "group", groupId: groupId ?? undefined })}
                  onOpenRunner={(runnerId) => selectRunner(runnerId, { kind: "races" })}
                />
              )}
              {screen.kind === "groups" && coach && (
                <GroupsScreen
                  onStats={(groupId) => navigate({ kind: "group", groupId })}
                  onCompare={() => goTo("compare-groups")}
                />
              )}
              {screen.kind === "compare-groups" && coach && <CompareGroupsScreen />}
              {screen.kind === "import" && !viewing && (
                <ImportScreen
                  onImported={refresh}
                  onOpen={openRace}
                  onSettings={() => goOwn({ kind: "profile" })}
                />
              )}
              {screen.kind === "profile" && <SettingsView page="profile" onSaved={refresh} />}
              {screen.kind === "settings" && <SettingsView page="settings" onSaved={refresh} />}
              {screen.kind === "help" && <HelpScreen page={screen.page} onOpen={openHelp} />}
            </div>
          </main>
        </div>
      </PanelVisibilityProvider>
    </ViewerContext.Provider>
  );
}

export default App;
