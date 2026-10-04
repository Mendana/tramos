import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

function App() {
  const [version, setVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("core_version")
      .then(setVersion)
      .catch((err: unknown) => setError(String(err)));
  }, []);

  return (
    <main>
      <h1>Tramos</h1>
      {error !== null ? (
        <p role="alert">No se pudo consultar el núcleo: {error}</p>
      ) : (
        <p>Núcleo tramos-core {version === null ? "…" : `v${version}`}</p>
      )}
    </main>
  );
}

export default App;
