import { useCallback, useEffect, useState } from "react";
import { RaceRow, coreVersion, listRaces } from "./api";
import ImportPanel from "./ImportPanel";
import RaceList from "./RaceList";
import RaceView from "./RaceView";
import SettingsView from "./SettingsView";
import "./App.css";

function App() {
  const [version, setVersion] = useState<string | null>(null);
  const [races, setRaces] = useState<RaceRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  // Pantalla: la principal (importar y lista), una carrera o los ajustes.
  const [screen, setScreen] = useState<
    { kind: "home" } | { kind: "race"; resultId: number } | { kind: "settings" }
  >({ kind: "home" });
  const home = () => setScreen({ kind: "home" });

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

  return (
    <main>
      <header>
        <h1>Tramos</h1>
        <p className="muted">Núcleo tramos-core {version === null ? "…" : `v${version}`}</p>
        <button
          type="button"
          className="header-action"
          onClick={() => setScreen({ kind: "settings" })}
        >
          Ajustes
        </button>
      </header>
      {error !== null && (
        <p role="alert" className="error">
          No se pudo consultar el núcleo: {error}
        </p>
      )}
      {screen.kind === "race" && <RaceView resultId={screen.resultId} onBack={home} />}
      {screen.kind === "settings" && <SettingsView onBack={home} onSaved={refresh} />}
      {screen.kind === "home" && (
        <>
          <ImportPanel onImported={refresh} />
          <section className="panel">
            <h2>Tus carreras</h2>
            <RaceList
              races={races}
              onOpen={(resultId) => setScreen({ kind: "race", resultId })}
            />
          </section>
        </>
      )}
    </main>
  );
}

export default App;
