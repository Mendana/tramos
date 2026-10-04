import { ReactNode, useCallback, useEffect, useState } from "react";
import { RaceRow, coreVersion, listRaces } from "./api";
import ImportPanel from "./ImportPanel";
import RaceList from "./RaceList";
import RaceView from "./RaceView";
import SettingsView from "./SettingsView";
import { ControlFlag, ListIcon, Notice, SlidersIcon, UploadIcon } from "./ui";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/components.css";
import "./styles/charts.css";
import "./styles/map.css";

/** Pantalla abierta. Una carrera se abre desde la lista (o al acabar de importarla). */
type Screen =
  | { kind: "races" }
  | { kind: "race"; resultId: number }
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

  const refresh = useCallback(() => {
    listRaces()
      .then(setRaces)
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
        <div className="sidebar-footer">Núcleo {version === null ? "…" : `v${version}`}</div>
      </aside>

      <main className="content">
        <div className="page">
          {error !== null && <Notice kind="error">No se pudo consultar el núcleo: {error}</Notice>}
          {screen.kind === "races" && (
            <RaceList races={races} onOpen={openRace} onImport={showImport} />
          )}
          {screen.kind === "race" && <RaceView resultId={screen.resultId} onBack={showRaces} />}
          {screen.kind === "import" && <ImportPanel onImported={refresh} onOpen={openRace} />}
          {screen.kind === "settings" && <SettingsView onSaved={refresh} />}
        </div>
      </main>
    </div>
  );
}

export default App;
