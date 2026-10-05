import { useState } from "react";
import BatchImportPanel from "./BatchImportPanel";
import ImportPanel from "./ImportPanel";
import { PageHeader } from "./ui";

type Mode = "single" | "folder";

const MODES: { mode: Mode; label: string; title: string; subtitle: string }[] = [
  {
    mode: "single",
    label: "Una carrera",
    title: "Importar una carrera",
    subtitle: "El .spl de WinSplits y, si lo tienes, el FIT de tu reloj.",
  },
  {
    mode: "folder",
    label: "Una carpeta",
    title: "Importar una carpeta",
    subtitle: "Todas las carreras de una carpeta, cada una con el FIT de su reloj.",
  },
];

/** Pantalla Importar: una carrera o una carpeta entera (`docs/app.md`). */
function ImportScreen({
  onImported,
  onOpen,
  onSettings,
}: {
  onImported: () => void;
  onOpen: (resultId: number) => void;
  onSettings: () => void;
}) {
  const [mode, setMode] = useState<Mode>("single");
  const current = MODES.find((m) => m.mode === mode) ?? MODES[0];
  return (
    <>
      <PageHeader
        title={current.title}
        subtitle={current.subtitle}
        actions={
          <div className="segmented" role="radiogroup" aria-label="Qué importar">
            {MODES.map((m) => (
              <label key={m.mode}>
                <input
                  type="radio"
                  name="import-mode"
                  checked={mode === m.mode}
                  onChange={() => setMode(m.mode)}
                />
                {m.label}
              </label>
            ))}
          </div>
        }
      />
      {mode === "single" ? (
        <ImportPanel onImported={onImported} onOpen={onOpen} />
      ) : (
        <BatchImportPanel onImported={onImported} onOpen={onOpen} onSettings={onSettings} />
      )}
    </>
  );
}

export default ImportScreen;
