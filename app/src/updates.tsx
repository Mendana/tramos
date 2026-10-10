import { useEffect, useState, useSyncExternalStore } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

/**
 * Actualizaciones de la app (ver `docs/distribucion.md`, "Actualizaciones"). La app busca versión
 * nueva al arrancar, en segundo plano, y con el botón de Ajustes; si la hay, avisa y solo la
 * descarga e instala cuando el usuario lo pide. El estado es uno para toda la app: el aviso de
 * arriba y el apartado de Ajustes enseñan lo mismo.
 */
export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "latest" }
  | { kind: "available"; update: Update }
  | { kind: "installing"; update: Update; done: number; total: number | null }
  | { kind: "failed"; message: string; update: Update | null };

/** `dismissed`: «Más tarde» en el aviso; no vuelve a salir hasta el próximo arranque. */
interface Snapshot {
  state: UpdateState;
  dismissed: boolean;
}

let snapshot: Snapshot = { state: { kind: "idle" }, dismissed: false };
const listeners = new Set<() => void>();

function publish(next: Partial<Snapshot>) {
  snapshot = { ...snapshot, ...next };
  listeners.forEach((l) => l());
}

function set(state: UpdateState) {
  publish({ state });
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** El estado de las actualizaciones; se vuelve a pintar cuando cambia. */
export function useUpdateState(): Snapshot {
  return useSyncExternalStore(subscribe, () => snapshot);
}

/**
 * Busca versión nueva. En `silent` (al arrancar) un fallo no se enseña: sin red, o en desarrollo,
 * la app sigue igual.
 */
export async function checkForUpdate(silent: boolean): Promise<void> {
  const { kind } = snapshot.state;
  if (kind === "checking" || kind === "installing") return;
  set({ kind: "checking" });
  try {
    const update = await check();
    set(update === null ? { kind: "latest" } : { kind: "available", update });
  } catch (err: unknown) {
    set(silent ? { kind: "idle" } : { kind: "failed", message: String(err), update: null });
  }
}

/** Descarga e instala la versión nueva y reinicia la app con ella. */
export async function installUpdate(update: Update): Promise<void> {
  set({ kind: "installing", update, done: 0, total: null });
  let done = 0;
  let total: number | null = null;
  try {
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") total = event.data.contentLength ?? null;
      if (event.event === "Progress") done += event.data.chunkLength;
      set({ kind: "installing", update, done, total });
    });
    // En Windows el instalador cierra la app; en el resto, se reinicia aquí.
    await relaunch();
  } catch (err: unknown) {
    set({ kind: "failed", message: String(err), update });
  }
}

export function dismissUpdate() {
  publish({ dismissed: true });
}

function progress(done: number, total: number | null): string {
  const mb = (bytes: number) => (bytes / 1_000_000).toFixed(1);
  return total === null || total === 0
    ? `${mb(done)} MB`
    : `${Math.round((done / total) * 100)} % de ${mb(total)} MB`;
}

/** Botón «Instalar y reiniciar», o el progreso mientras se descarga. */
function InstallButton({ state }: { state: UpdateState }) {
  if (state.kind === "installing") {
    return (
      <span className="small muted" role="status">
        Descargando… {progress(state.done, state.total)}
      </span>
    );
  }
  const update = state.kind === "available" || state.kind === "failed" ? state.update : null;
  if (update === null) return null;
  return (
    <button type="button" className="btn btn-primary" onClick={() => void installUpdate(update)}>
      {state.kind === "failed" ? "Reintentar" : "Instalar y reiniciar"}
    </button>
  );
}

/** Aviso de arriba cuando hay versión nueva. */
export function UpdateBanner() {
  const { state, dismissed } = useUpdateState();
  const update =
    state.kind === "available" || state.kind === "installing"
      ? state.update
      : state.kind === "failed"
        ? state.update
        : null;
  if (update === null || (dismissed && state.kind === "available")) return null;
  return (
    <div className="viewing-banner" role="status">
      <span className="viewing-banner-text">
        {state.kind === "failed" ? (
          <>
            No se ha podido instalar la versión {update.version}: {state.message}
          </>
        ) : (
          <>
            Hay una versión nueva de Tramos, la <strong>{update.version}</strong>. Tus carreras y
            ajustes se quedan como están.
          </>
        )}
      </span>
      <InstallButton state={state} />
      {state.kind === "available" && (
        <button type="button" className="btn btn-ghost" onClick={dismissUpdate}>
          Más tarde
        </button>
      )}
    </div>
  );
}

/**
 * El apartado de Ajustes: la versión instalada, «Buscar actualizaciones», qué pasó al buscar y, si
 * hay versión nueva, el botón de instalarla.
 */
export function UpdateStatus() {
  const { state } = useUpdateState();
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(null));
  }, []);
  const text = (() => {
    switch (state.kind) {
      case "idle":
        return null;
      case "checking":
        return "Buscando…";
      case "latest":
        return "Tienes la última versión.";
      case "available":
        return `Hay una versión nueva, la ${state.update.version}.`;
      case "installing":
        return `Instalando la ${state.update.version}…`;
      case "failed":
        return state.update === null
          ? `No se ha podido comprobar: ${state.message}`
          : `No se ha podido instalar: ${state.message}`;
    }
  })();
  return (
    <div className="field">
      <span className="field-label">Tienes la versión {version ?? "…"}.</span>
      <div className="row">
        <button
          type="button"
          className="btn"
          disabled={state.kind === "checking" || state.kind === "installing"}
          onClick={() => void checkForUpdate(false)}
        >
          Buscar actualizaciones
        </button>
        <InstallButton state={state} />
      </div>
      {text !== null && (
        <span className="field-hint" role="status">
          {text}
        </span>
      )}
    </div>
  );
}
