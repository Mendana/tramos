import { useCallback, useEffect, useState } from "react";
import { RaceRow, coreVersion, listRaces } from "./api";
import ImportPanel from "./ImportPanel";
import RaceList from "./RaceList";
import "./App.css";

function App() {
  const [version, setVersion] = useState<string | null>(null);
  const [races, setRaces] = useState<RaceRow[]>([]);
  const [error, setError] = useState<string | null>(null);

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
      </header>
      {error !== null && (
        <p role="alert" className="error">
          No se pudo consultar el núcleo: {error}
        </p>
      )}
      <ImportPanel onImported={refresh} />
      <section className="panel">
        <h2>Tus carreras</h2>
        <RaceList races={races} />
      </section>
    </main>
  );
}

export default App;
