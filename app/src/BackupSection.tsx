import { open, save } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { useState } from "react";
import { BackupSummary, backupExport, backupRestore, backupSummary } from "./api";
import { Notice } from "./ui";

/**
 * Apartado de Ajustes «Copia de seguridad» (ver `docs/app.md`, "Copia de seguridad"): guardar toda
 * la base en un fichero y restaurarla desde otro. Restaurar sustituye todo lo que hay, así que
 * antes enseña qué trae la copia y pide confirmar. No pasa por «Guardar».
 */
const FILTERS = [{ name: "Copia de Tramos", extensions: ["tramos"] }];
const ANY_FILE = [{ name: "Todos los ficheros", extensions: ["*"] }];

/** `tramos-copia-AAAA-MM-DD.tramos`, con la fecha local de hoy. */
export function backupFileName(today: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `tramos-copia-${today.getFullYear()}-${pad(today.getMonth() + 1)}-${pad(today.getDate())}.tramos`;
}

/** Fecha y hora de la copia, en la hora del equipo; sin dato, «fecha desconocida». */
export function formatCreated(createdAt: string | null): string {
  if (createdAt === null) return "de fecha desconocida";
  const date = new Date(createdAt);
  if (Number.isNaN(date.getTime())) return "de fecha desconocida";
  return `del ${date.toLocaleString("es-ES", { dateStyle: "long", timeStyle: "short" })}`;
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** Una copia elegida para restaurar, a la espera de que se confirme. */
interface Pending {
  path: string;
  summary: BackupSummary;
}

export function BackupSection() {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);

  async function exportCopy() {
    setMessage(null);
    setError(null);
    setPending(null);
    try {
      const path = await save({ defaultPath: backupFileName(new Date()), filters: FILTERS });
      if (path === null) return;
      setBusy(true);
      await backupExport(path);
      setMessage(
        `Copia guardada en ${path}. Lleva todos tus datos, también los ficheros originales: no la compartas.`,
      );
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function chooseCopy() {
    setMessage(null);
    setError(null);
    setPending(null);
    try {
      const path = await open({
        multiple: false,
        directory: false,
        filters: [...FILTERS, ...ANY_FILE],
      });
      if (typeof path !== "string") return;
      setBusy(true);
      setPending({ path, summary: await backupSummary(path) });
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function restore(copy: Pending) {
    setError(null);
    setBusy(true);
    try {
      const previous = await backupRestore(copy.path);
      setPending(null);
      setMessage(`Copia restaurada. Lo de antes queda en ${previous}. Reiniciando Tramos…`);
      try {
        await relaunch();
      } catch {
        setMessage(
          `Copia restaurada. Cierra Tramos y vuelve a abrirlo para verla. Lo de antes queda en ${previous}.`,
        );
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="field">
        <span className="field-label">Exportar copia</span>
        <div className="row">
          <button type="button" className="btn" disabled={busy} onClick={() => void exportCopy()}>
            Exportar copia…
          </button>
        </div>
        <span className="field-hint">
          Guarda en un fichero todas tus carreras, etiquetas y ajustes, con los ficheros originales
          (.spl y FIT) que has importado. Puedes hacerla con la app abierta. Esos ficheros llevan
          nombres de corredores, fechas de nacimiento y rutas de GPS: guarda la copia en un sitio
          tuyo y no la compartas.
        </span>
      </div>
      <div className="field">
        <span className="field-label">Restaurar copia</span>
        <div className="row">
          <button type="button" className="btn" disabled={busy} onClick={() => void chooseCopy()}>
            Restaurar copia…
          </button>
        </div>
        <span className="field-hint">
          Sustituye todo lo que tienes por lo que trae la copia. Antes te enseña qué trae y te pide
          confirmar, y guarda lo de ahora junto a la base de datos por si acaso. La app se reinicia
          sola.
        </span>
      </div>
      {pending !== null && (
        <Notice kind="warning">
          <p>
            Copia {formatCreated(pending.summary.created_at)}:{" "}
            {plural(pending.summary.events, "carrera", "carreras")} y{" "}
            {plural(pending.summary.tags, "etiqueta", "etiquetas")}.
            {pending.summary.migrates &&
              " Es de una versión anterior de Tramos y se actualizará al restaurarla."}
          </p>
          <p>
            Al restaurar se sustituyen todas las carreras, etiquetas y ajustes que tienes ahora. Lo
            de ahora se guarda junto a la base de datos, con el sufijo «.antes-de-restaurar».
          </p>
          <div className="row">
            <button
              type="button"
              className="btn btn-danger"
              disabled={busy}
              onClick={() => void restore(pending)}
            >
              Restaurar y reiniciar
            </button>
            <button
              type="button"
              className="btn btn-ghost"
              disabled={busy}
              onClick={() => setPending(null)}
            >
              Cancelar
            </button>
          </div>
        </Notice>
      )}
      {error !== null && <Notice kind="error">{error}</Notice>}
      {message !== null && (
        <span className="field-hint" role="status">
          {message}
        </span>
      )}
    </>
  );
}
