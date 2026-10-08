import { ReactNode, useCallback, useEffect, useState } from "react";
import {
  AppMode,
  CoachRunner,
  RaceRow,
  ReceiveReport,
  RunnerViewInfo,
  SharingSettings,
  chooseMode,
  coachRunners,
  coreVersion,
  getSettings,
  listRaces,
  modeChosen,
  receivePackages,
  viewRunner,
  viewedRunner,
} from "./api";
import GroupScreen from "./GroupScreen";
import HistoryScreen from "./HistoryScreen";
import Home from "./Home";
import ImportScreen from "./ImportScreen";
import RaceList from "./RaceList";
import RaceView from "./RaceView";
import SettingsView from "./SettingsView";
import Welcome from "./Welcome";
import { ViewerContext } from "./viewer";
import {
  ChartIcon,
  ChevronLeft,
  ControlFlag,
  GroupIcon,
  HomeIcon,
  ListIcon,
  Notice,
  SlidersIcon,
  UploadIcon,
  UserIcon,
} from "./ui";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/components.css";
import "./styles/charts.css";
import "./styles/map.css";

/** Cada cuánto busca la entrenadora paquetes nuevos en la carpeta compartida. */
const RECEIVE_EVERY_MS = 60_000;

/**
 * En modo entrenadora con carpeta, importa los paquetes nuevos al abrir la app y cada minuto
 * (`docs/paquete.md`, "Carpeta compartida"). Devuelve lo último que ha encontrado.
 */
function useReceivePackages(sharing: SharingSettings | null, onReceived: () => void) {
  const [report, setReport] = useState<ReceiveReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const active = sharing?.mode === "coach" && sharing.folder !== null;
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
      {report.runners} {report.runners === 1 ? "corredor" : "corredores"}
      {problems > 0 && ` · ${problems} sin leer`}
    </div>
  );
}

/** Pantalla abierta. Una carrera se abre desde la lista (o al acabar de importarla). */
type Screen =
  | { kind: "home" }
  | { kind: "races" }
  | { kind: "race"; resultId: number }
  | { kind: "history" }
  | { kind: "group" }
  | { kind: "import" }
  | { kind: "profile" }
  | { kind: "settings" };

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
      case "group":
        return [{ label: "Grupo" }];
      case "import":
        return [{ label: "Importar" }];
      case "profile":
        return [{ label: "Mi perfil" }];
      case "settings":
        return [{ label: "Ajustes" }];
    }
  })();
  // La entrenadora ve a un corredor: su nombre va delante, salvo en lo que es de todos.
  const general = screen.kind === "group" || screen.kind === "settings";
  return runnerName === null || general ? own : [{ label: runnerName }, ...own];
}

function TopBar({
  crumbs,
  onBack,
  onNavigate,
}: {
  crumbs: Crumb[];
  onBack: (() => void) | null;
  onNavigate: (screen: Screen) => void;
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
  const [chosen, setChosen] = useState<boolean | null>(null);
  const [sharing, setSharing] = useState<SharingSettings | null>(null);
  // Modo entrenadora: corredores con paquetes y el que se está viendo.
  const [runners, setRunners] = useState<CoachRunner[]>([]);
  const [runner, setRunner] = useState<RunnerViewInfo | null>(null);
  const coach = sharing?.mode === "coach";

  const refresh = useCallback(() => {
    listRaces()
      .then(setRaces)
      .catch((err: unknown) => setError(String(err)));
    getSettings()
      .then((s) => setSharing(s.sharing))
      .catch((err: unknown) => setError(String(err)));
  }, []);

  // Ha llegado algo por la carpeta: el corredor que se ve ya está al día en el núcleo.
  const onReceived = useCallback(() => {
    coachRunners()
      .then(setRunners)
      .catch((err: unknown) => setError(String(err)));
    viewedRunner()
      .then(setRunner)
      .catch((err: unknown) => setError(String(err)));
    refresh();
  }, [refresh]);
  const receiving = useReceivePackages(sharing, onReceived);

  useEffect(() => {
    coreVersion()
      .then(setVersion)
      .catch((err: unknown) => setError(String(err)));
    modeChosen()
      .then(setChosen)
      .catch((err: unknown) => setError(String(err)));
    refresh();
  }, [refresh]);

  const selectRunner = useCallback(
    (runnerId: string | null) => {
      viewRunner(runnerId)
        .then((info) => {
          setRunner(info);
          // Las pantallas anteriores eran de otro corredor.
          setPrevious([]);
          setScreen((s) => (s.kind === "race" ? { kind: "races" } : s));
          refresh();
        })
        .catch((err: unknown) => setError(String(err)));
    },
    [refresh],
  );

  // La entrenadora no tiene Inicio, perfil ni importar: empieza en las carreras del corredor.
  useEffect(() => {
    if (!coach) return;
    setPrevious([]);
    setScreen((s) =>
      s.kind === "home" || s.kind === "profile" || s.kind === "import" ? { kind: "races" } : s,
    );
  }, [coach]);

  // Al entrar en modo entrenadora, sus corredores; al salir, ya no se ve a nadie.
  useEffect(() => {
    if (!coach) {
      setRunners([]);
      setRunner(null);
      return;
    }
    coachRunners()
      .then(setRunners)
      .catch((err: unknown) => setError(String(err)));
  }, [coach]);

  // Si no se ve a nadie y hay corredores, el primero.
  useEffect(() => {
    if (coach && runner === null && runners.length > 0) selectRunner(runners[0].runner_id);
  }, [coach, runner, runners, selectRunner]);

  const choose = (mode: AppMode) => {
    chooseMode(mode)
      .then(() => {
        setChosen(true);
        // La entrenadora necesita la carpeta para recibir nada.
        if (mode === "coach") navigate({ kind: "settings" });
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
  const showRaces = () => navigate({ kind: "races" });
  const showImport = () => navigate({ kind: "import" });
  const openRace = (resultId: number) => navigate({ kind: "race", resultId });

  if (chosen === false) {
    return (
      <>
        {error !== null && <Notice kind="error">{error}</Notice>}
        <Welcome onChoose={choose} />
      </>
    );
  }

  const viewer = {
    readOnly: coach,
    runnerName: coach ? (runner?.runner.display_name ?? null) : null,
  };
  const unreviewed = coach ? 0 : (races ?? []).reduce((n, r) => n + r.unreviewed_count, 0);
  const at = (...kinds: Screen["kind"][]) => kinds.includes(screen.kind);
  const goTo = (kind: Exclude<Screen["kind"], "race">) => navigate({ kind });

  return (
    <ViewerContext.Provider value={viewer}>
      <div className="shell">
        <aside className="sidebar">
          <div className="brand">
            <ControlFlag />
            Tramos
          </div>
          <nav className="nav" aria-label="Secciones">
            {!coach && (
              <NavItem
                icon={<HomeIcon />}
                label="Inicio"
                current={at("home")}
                onClick={() => goTo("home")}
              />
            )}
            {coach ? (
              <NavSection label="Corredor">
                <RunnerPicker
                  runners={runners}
                  current={runner?.runner.runner_id ?? null}
                  onChange={selectRunner}
                />
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
              </NavSection>
            ) : (
              <NavSection label="Lo mío">
                <NavItem
                  icon={<ListIcon />}
                  label="Mis carreras"
                  current={at("races", "race")}
                  count={unreviewed}
                  countLabel={`${unreviewed} ${unreviewed === 1 ? "error" : "errores"} por revisar`}
                  onClick={showRaces}
                />
                <NavItem
                  icon={<ChartIcon />}
                  label="Estadísticas"
                  current={at("history")}
                  onClick={() => goTo("history")}
                />
                <NavItem
                  icon={<UploadIcon />}
                  label="Importar"
                  current={at("import")}
                  onClick={showImport}
                />
              </NavSection>
            )}
            {coach && (
              <NavSection label="Todos">
                <NavItem
                  icon={<GroupIcon />}
                  label="Grupo"
                  current={at("group")}
                  onClick={() => goTo("group")}
                />
              </NavSection>
            )}
            <NavSection label="Cuenta">
              {!coach && (
                <NavItem
                  icon={<UserIcon />}
                  label="Mi perfil"
                  current={at("profile")}
                  onClick={() => goTo("profile")}
                />
              )}
              <NavItem
                icon={<SlidersIcon />}
                label="Ajustes"
                current={at("settings")}
                onClick={() => goTo("settings")}
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
          />
          {/* Otro corredor, otras pantallas: no se arrastra nada del anterior. */}
          <div className="page" key={runner?.runner.runner_id ?? "self"}>
            {error !== null && (
              <Notice kind="error">No se pudo consultar el núcleo: {error}</Notice>
            )}
            {coach && runner !== null && runner.problems.length > 0 && (
              <Notice kind="warning">
                Algunos paquetes de {runner.runner.display_name} no se han podido leer:{" "}
                {runner.problems.join("; ")}
              </Notice>
            )}
            {screen.kind === "home" && !coach && (
              <Home
                races={races}
                onOpen={openRace}
                onImport={showImport}
                onRaces={showRaces}
                onHistory={() => goTo("history")}
              />
            )}
            {screen.kind === "races" && (
              <RaceList
                races={races}
                onOpen={openRace}
                onImport={showImport}
                summaryOnly={coach ? (runner?.summary_only ?? []) : []}
              />
            )}
            {screen.kind === "race" && <RaceView resultId={screen.resultId} onChanged={refresh} />}
            {screen.kind === "history" && <HistoryScreen onImport={showImport} onOpen={openRace} />}
            {screen.kind === "group" && coach && (
              <GroupScreen
                onOpenRunner={(runnerId) => {
                  selectRunner(runnerId);
                  navigate({ kind: "races" });
                }}
              />
            )}
            {screen.kind === "import" && !coach && (
              <ImportScreen
                onImported={refresh}
                onOpen={openRace}
                onSettings={() => goTo("profile")}
              />
            )}
            {screen.kind === "profile" && !coach && (
              <SettingsView page="profile" onSaved={refresh} />
            )}
            {screen.kind === "settings" && <SettingsView page="settings" onSaved={refresh} />}
          </div>
        </main>
      </div>
    </ViewerContext.Provider>
  );
}

/** Modo entrenadora: de qué corredor se ven las carreras. */
function RunnerPicker({
  runners,
  current,
  onChange,
}: {
  runners: CoachRunner[];
  current: string | null;
  onChange: (runnerId: string) => void;
}) {
  if (runners.length === 0) {
    return <p className="small muted runner-picker">Aún no ha llegado nada de los corredores.</p>;
  }
  return (
    <label className="field runner-picker">
      <span className="field-label">Corredor</span>
      <select className="select" value={current ?? ""} onChange={(e) => onChange(e.target.value)}>
        {current === null && <option value="">Elige…</option>}
        {runners.map((r) => (
          <option key={r.runner_id} value={r.runner_id}>
            {r.display_name === "" ? "Sin nombre" : r.display_name} ({r.races})
          </option>
        ))}
      </select>
    </label>
  );
}

export default App;
