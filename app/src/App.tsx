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
import HistoryScreen from "./HistoryScreen";
import ImportScreen from "./ImportScreen";
import RaceList from "./RaceList";
import RaceView from "./RaceView";
import SettingsView from "./SettingsView";
import Welcome from "./Welcome";
import { ViewerContext } from "./viewer";
import { ChartIcon, ControlFlag, ListIcon, Notice, SlidersIcon, UploadIcon } from "./ui";
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
  | { kind: "races" }
  | { kind: "race"; resultId: number }
  | { kind: "history" }
  | { kind: "import" }
  | { kind: "settings" };

function NavItem({
  icon,
  label,
  current,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  current: boolean;
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
      {label}
    </button>
  );
}

function App() {
  const [version, setVersion] = useState<string | null>(null);
  const [races, setRaces] = useState<RaceRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [screen, setScreen] = useState<Screen>({ kind: "races" });
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
          setScreen((s) => (s.kind === "race" ? { kind: "races" } : s));
          refresh();
        })
        .catch((err: unknown) => setError(String(err)));
    },
    [refresh],
  );

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
        if (mode === "coach") setScreen({ kind: "settings" });
        refresh();
      })
      .catch((err: unknown) => setError(String(err)));
  };

  const showRaces = () => setScreen({ kind: "races" });
  const showImport = () => setScreen({ kind: "import" });
  const openRace = (resultId: number) => setScreen({ kind: "race", resultId });

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

  return (
    <ViewerContext.Provider value={viewer}>
      <div className="shell">
        <aside className="sidebar">
          <div className="brand">
            <ControlFlag />
            Tramos
          </div>
          {coach && (
            <RunnerPicker
              runners={runners}
              current={runner?.runner.runner_id ?? null}
              onChange={selectRunner}
            />
          )}
          <nav className="nav" aria-label="Secciones">
            <NavItem
              icon={<ListIcon />}
              label="Carreras"
              current={screen.kind === "races" || screen.kind === "race"}
              onClick={showRaces}
            />
            <NavItem
              icon={<ChartIcon />}
              label="Histórico"
              current={screen.kind === "history"}
              onClick={() => setScreen({ kind: "history" })}
            />
            {!coach && (
              <NavItem
                icon={<UploadIcon />}
                label="Importar"
                current={screen.kind === "import"}
                onClick={showImport}
              />
            )}
            <NavItem
              icon={<SlidersIcon />}
              label="Ajustes"
              current={screen.kind === "settings"}
              onClick={() => setScreen({ kind: "settings" })}
            />
          </nav>
          <div className="sidebar-footer">
            {receiving.active && (
              <ReceiveStatus report={receiving.report} error={receiving.error} />
            )}
            <div>Núcleo {version === null ? "…" : `v${version}`}</div>
          </div>
        </aside>

        <main className="content">
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
            {screen.kind === "races" && (
              <RaceList
                races={races}
                onOpen={openRace}
                onImport={showImport}
                summaryOnly={coach ? (runner?.summary_only ?? []) : []}
              />
            )}
            {screen.kind === "race" && (
              <RaceView resultId={screen.resultId} onBack={showRaces} onChanged={refresh} />
            )}
            {screen.kind === "history" && <HistoryScreen onImport={showImport} onOpen={openRace} />}
            {screen.kind === "import" && !coach && (
              <ImportScreen
                onImported={refresh}
                onOpen={openRace}
                onSettings={() => setScreen({ kind: "settings" })}
              />
            )}
            {screen.kind === "settings" && <SettingsView onSaved={refresh} />}
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
