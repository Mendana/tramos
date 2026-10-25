import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { ExportReport, exportAnalysis } from "./api";
import { Notice } from "./ui";

/**
 * Apartado de Ajustes «Exportar datos para análisis» (ver `docs/app.md`, "Exportar datos para
 * análisis"): escribe en una carpeta que elige el usuario los CSV de sus carreras y tramos, con su
 * esquema, para analizarlos fuera de la app. Dos casillas, desmarcadas, añaden la serie de cada
 * carrera (`samples.csv`) y las coordenadas de la serie. No pasa por «Guardar».
 */

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** Lo que dice el resumen tras exportar. */
export function exportSummary(report: ExportReport): string {
  const series =
    report.sample_rows > 0
      ? ` Con la serie de ${plural(report.sample_races, "carrera", "carreras")} (${report.sample_rows} filas).`
      : "";
  return `${plural(report.races, "carrera", "carreras")} y ${plural(report.legs, "tramo", "tramos")} exportados en ${report.folder}.${series}`;
}

export function ExportDataSection() {
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ExportReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [samples, setSamples] = useState(false);
  const [coordinates, setCoordinates] = useState(false);

  async function exportData() {
    setReport(null);
    setError(null);
    try {
      const folder = await open({ directory: true, multiple: false });
      if (typeof folder !== "string") return;
      setBusy(true);
      setReport(await exportAnalysis(folder, samples, samples && coordinates));
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
        <label className="check">
          <input type="checkbox" checked={samples} onChange={(e) => setSamples(e.target.checked)} />
          Incluir la serie de cada carrera (samples.csv)
        </label>
        <span className="field-hint">
          Una fila por segundo, de la salida a la meta, de cada carrera con FIT: distancia, altitud,
          velocidad, pulso, cadencia, zancada, tramo y estado. Es mucho más grande que las otras
          tablas (unas 3.600 filas por hora de carrera) y sirve para modelos de secuencias.
        </span>
        <label className="check">
          <input
            type="checkbox"
            checked={samples && coordinates}
            disabled={!samples}
            onChange={(e) => setCoordinates(e.target.checked)}
          />
          Incluir coordenadas
        </label>
        <span className="field-hint">
          Añade la latitud y la longitud de cada segundo a la serie. Atención: dicen por dónde has
          corrido y, con la fecha, dónde estabas. Compártelas con mucho cuidado.
        </span>
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
          No llevan nombres de nadie, ni los resultados de los demás corredores (solo la referencia
          de cada tramo), ni coordenadas salvo que marques la casilla de arriba. Aun así son tus
          datos, con tus fechas y tu pulso: compártelos con cuidado. Si en la carpeta ya hay
          ficheros con esos nombres, los sustituye; y si no marcas la serie, borra el{" "}
          <code>samples.csv</code> de una exportación anterior que hubiera en ella.
        </span>
      </div>
      {error !== null && <Notice kind="error">{error}</Notice>}
      {report !== null && (
        <span className="field-hint" role="status">
          {exportSummary(report)} Ficheros: {report.files.join(", ")}.
          {report.removed.length > 0 &&
            ` Se ha borrado ${report.removed.join(", ")} de una exportación anterior.`}
        </span>
      )}
    </>
  );
}
