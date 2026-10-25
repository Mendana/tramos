import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { ExportReport, exportAnalysis } from "./api";
import { Notice } from "./ui";

/**
 * Apartado de Ajustes «Exportar datos para análisis» (ver `docs/app.md`, "Exportar datos para
 * análisis"): escribe en una carpeta que elige el usuario los CSV de sus carreras y tramos, con su
 * esquema, para analizarlos fuera de la app. No pasa por «Guardar».
 */

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** Lo que dice el resumen tras exportar. */
export function exportSummary(report: ExportReport): string {
  return `${plural(report.races, "carrera", "carreras")} y ${plural(report.legs, "tramo", "tramos")} exportados en ${report.folder}.`;
}

export function ExportDataSection() {
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ExportReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function exportData() {
    setReport(null);
    setError(null);
    try {
      const folder = await open({ directory: true, multiple: false });
      if (typeof folder !== "string") return;
      setBusy(true);
      setReport(await exportAnalysis(folder));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="field">
        <span className="field-label">Exportar datos</span>
        <div className="row">
          <button type="button" className="btn" disabled={busy} onClick={() => void exportData()}>
            Exportar datos…
          </button>
        </div>
        <span className="field-hint">
          Escribe en la carpeta que elijas <code>races.csv</code> (una fila por carrera),{" "}
          <code>legs.csv</code> (una fila por tramo), <code>schema.json</code> y{" "}
          <code>LEEME.md</code> con lo que significa cada columna. Salen todas tus carreras, con los
          números que ves en la app: el tiempo perdido, tus errores revisados y, si tienen FIT, las
          cifras físicas.
        </span>
        <span className="field-hint">
          No llevan nombres de nadie ni coordenadas, ni los resultados de los demás corredores (solo
          la referencia de cada tramo). Aun así son tus datos, con tus fechas y tu pulso:
          compártelos con cuidado. Si en la carpeta ya hay ficheros con esos nombres, los sustituye.
        </span>
      </div>
      {error !== null && <Notice kind="error">{error}</Notice>}
      {report !== null && (
        <span className="field-hint" role="status">
          {exportSummary(report)} Ficheros: {report.files.join(", ")}.
        </span>
      )}
    </>
  );
}
