import { ReactNode, useCallback, useEffect, useState } from "react";
import {
  RaceRow,
  ReceiveReport,
  SharingSettings,
  coreVersion,
  getSettings,
  listRaces,
  receivePackages,
} from "./api";
import HistoryScreen from "./HistoryScreen";
import ImportScreen from "./ImportScreen";
import RaceList from "./RaceList";
import RaceView from "./RaceView";
import SettingsView from "./SettingsView";
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
function useReceivePackages(sharing: SharingSettings | null) {
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
        })
        .catch((err: unknown) => setError(String(err)));
    };
    receive();
    const timer = window.setInterval(receive, RECEIVE_EVERY_MS);
    return () => window.clearInterval(timer);
  }, [active, sharing?.folder]);
  return { active, report, error };
}

function ReceiveStatus({
  report,
  error,
}: {
  report: ReceiveReport | null;
  error: string | null;
}) {
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
  const [sharing, setSharing] = useState<SharingSettings | null>(null);
  const receiving = useReceivePackages(sharing);

  const refresh = useCallback(() => {
    listRaces()
      .then(setRaces)
      .catch((err: unknown) => setError(String(err)));
    getSettings()
      .then((s) => setSharing(s.sharing))
      .catch((err: unknown) => setError(String(err)));
  }, []);

  useEffect(() => {
    coreVersion()
      .then(setVersion)
      .catch((err: unknown) => setError(String(err)));
    refresh();
  }, [refresh]);

  const showRaces = () => setScreen({ kind: "races" });
  const showImport = () => setScreen({ kind: "import" });
  const openRace = (resultId: number) => setScreen({ kind: "race", resultId });

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <ControlFlag />
          Tramos
        </div>
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
          <NavItem
            icon={<UploadIcon />}
            label="Importar"
            current={screen.kind === "import"}
            onClick={showImport}
          />
          <NavItem
            icon={<SlidersIcon />}
            label="Ajustes"
            current={screen.kind === "settings"}
            onClick={() => setScreen({ kind: "settings" })}
          />
        </nav>
        <div className="sidebar-footer">
          {receiving.active && <ReceiveStatus report={receiving.report} error={receiving.error} />}
          <div>Núcleo {version === null ? "…" : `v${version}`}</div>
        </div>
      </aside>

      <main className="content">
        <div className="page">
          {error !== null && <Notice kind="error">No se pudo consultar el núcleo: {error}</Notice>}
          {screen.kind === "races" && (
            <RaceList races={races} onOpen={openRace} onImport={showImport} />
          )}
          {screen.kind === "race" && (
            <RaceView resultId={screen.resultId} onBack={showRaces} onChanged={refresh} />
          )}
          {screen.kind === "history" && (
            <HistoryScreen onImport={showImport} onOpen={openRace} />
          )}
          {screen.kind === "import" && (
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
  );
}

export default App;
