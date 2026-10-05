import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  BatchRace,
  BatchSummary,
  RunnerIdentity,
  fileName,
  getSettings,
  importFolder,
} from "./api";
import { ChevronRight, FolderIcon, Notice, Stat, WatchIcon } from "./ui";

/** Tarjeta y nombre de los ajustes, en una frase. `null` si no hay ninguno. */
function identityText(identity: RunnerIdentity): string | null {
  const parts = [
    identity.si_card === null ? null : `tu tarjeta ${identity.si_card}`,
    identity.full_name === null ? null : `tu nombre, «${identity.full_name}»`,
  ].filter((p) => p !== null);
  return parts.length === 0 ? null : parts.join(" y ");
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

function raceTitle(race: BatchRace): string {
  return race.event_name ?? fileName(race.spl_path);
}

/** Importar todas las carreras de una carpeta (#41, `docs/app.md`, "Importar una carpeta"). */
function BatchImportPanel({
  onImported,
  onOpen,
  onSettings,
}: {
  onImported: () => void;
  onOpen: (resultId: number) => void;
  onSettings: () => void;
}) {
  const [folder, setFolder] = useState<string | null>(null);
  const [identity, setIdentity] = useState<RunnerIdentity | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [summary, setSummary] = useState<BatchSummary | null>(null);

  useEffect(() => {
    getSettings()
      .then((s) => setIdentity(s.identity))
      .catch((err: unknown) => setError(String(err)));
  }, []);

  async function chooseFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setFolder(selected);
      setSummary(null);
      setError(null);
    }
  }

  async function run() {
    if (folder === null) return;
    setBusy(true);
    setError(null);
    setSummary(null);
    try {
      setSummary(await importFolder(folder));
      onImported();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  const who = identity === null ? null : identityText(identity);

  return (
    <>
      <section className="card">
        <h3 className="section-title">1 · Carpeta</h3>
        <p className="small muted">
          La carpeta donde guardas los .spl de WinSplits y los FIT de tu reloj (también se mira en
          sus subcarpetas). Cada FIT se empareja con su carrera por la fecha y la hora.
        </p>
        <div className="row">
          <button type="button" className="btn" onClick={chooseFolder} disabled={busy}>
            Elegir carpeta…
          </button>
          <div className={folder === null ? "file batch-folder is-empty" : "file batch-folder"}>
            <FolderIcon />
            <span className="file-name" title={folder ?? undefined}>
              {folder ?? "Sin carpeta"}
            </span>
          </div>
        </div>
      </section>

      <section className="card">
        <h3 className="section-title">2 · Quién eres</h3>
        {identity !== null && who === null && (
          <Notice kind="warning">
            Para importar una carpeta, escribe antes en Ajustes tu tarjeta SI o tu nombre: con
            ellos te busco en cada carrera.
          </Notice>
        )}
        {who !== null && (
          <p>
            Te busco en cada carrera con {who}. Si en alguna no te encuentro con seguridad, no la
            importo: la verás en el resumen para importarla sola.
          </p>
        )}
        <div className="row">
          <button
            type="button"
            className="btn btn-primary btn-lg"
            onClick={run}
            disabled={busy || folder === null || who === null}
          >
            {busy ? "Importando…" : "Importar la carpeta"}
          </button>
          <button type="button" className="btn btn-ghost" onClick={onSettings} disabled={busy}>
            Cambiar en Ajustes
          </button>
        </div>
        {busy && (
          <p className="small muted" role="status">
            Con muchas carreras tarda un poco: hay que alinear cada FIT.
          </p>
        )}
      </section>

      {error !== null && <Notice kind="error">{error}</Notice>}
      {summary !== null && <Summary summary={summary} onOpen={onOpen} />}
    </>
  );
}

/** Resumen al acabar: importadas, sin pareja y con avisos. */
function Summary({ summary, onOpen }: { summary: BatchSummary; onOpen: (id: number) => void }) {
  const imported = summary.races.filter((r) => r.status === "imported");
  const withTrack = summary.races.filter(
    (r) => r.status !== "not_imported" && r.fit?.track_saved === true,
  );
  const racesWithoutFit = imported.filter((r) => r.fit === null);
  const fitsWithoutRace = summary.unpaired_fits.filter((f) => f.reason === "no_race");
  const racesWithNotes = summary.races.filter((r) => r.messages.length > 0);
  const badFits = summary.unpaired_fits.filter((f) => f.reason !== "no_race");
  const unpaired = racesWithoutFit.length + fitsWithoutRace.length;
  const notes = racesWithNotes.length + badFits.length + summary.warnings.length;
  const nothing = summary.races.length === 0 && summary.unpaired_fits.length === 0;

  return (
    <section className="card">
      <h3 className="section-title">3 · Resumen</h3>
      {nothing ? (
        <Notice kind="warning">No hay ningún .spl ni .fit en esa carpeta.</Notice>
      ) : (
        <Notice kind={notes > 0 ? "warning" : "success"}>
          <strong>
            {imported.length === 0
              ? "No se ha importado ninguna carrera nueva."
              : `${plural(imported.length, "carrera importada", "carreras importadas")}.`}
          </strong>{" "}
          {notes > 0 && "Revisa los avisos de abajo."}
        </Notice>
      )}

      <div className="stats">
        <Stat label="Importadas" value={imported.length} />
        <Stat
          label="Con reloj"
          value={withTrack.length}
          hint="Carreras de la carpeta con el FIT alineado y guardado."
        />
        <Stat label="Sin pareja" value={unpaired} hint="Carreras sin FIT y FIT sin carrera." />
        <Stat label="Con avisos" value={notes} />
      </div>

      {imported.length > 0 && (
        <div className="batch-block">
          <h4>Importadas</h4>
          <div className="table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>Fecha</th>
                  <th>Carrera</th>
                  <th>Reloj</th>
                  <th>
                    <span className="visually-hidden">Abrir</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {imported.map((race) => {
                  const id = race.result_id;
                  const openIt = () => {
                    if (id !== null) onOpen(id);
                  };
                  return (
                    <tr
                      key={race.spl_path}
                      className="clickable"
                      tabIndex={0}
                      onClick={openIt}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") openIt();
                      }}
                    >
                      <td className="num muted">{race.event_date}</td>
                      <td>
                        <div className="strong">{raceTitle(race)}</div>
                        <div className="small muted">{fileName(race.spl_path)}</div>
                      </td>
                      <td>
                        {race.fit === null ? (
                          <span className="muted">Sin FIT</span>
                        ) : race.fit.track_saved ? (
                          <span className="pill" title={race.fit.path}>
                            <WatchIcon size={14} /> {fileName(race.fit.path)}
                          </span>
                        ) : (
                          <span className="loss-bad">No se ha guardado</span>
                        )}
                      </td>
                      <td className="chevron">
                        <ChevronRight />
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {unpaired > 0 && (
        <div className="batch-block">
          <h4>Sin pareja</h4>
          {racesWithoutFit.length > 0 && (
            <>
              <p className="small muted">
                Importadas sin reloj: no había ningún FIT de su fecha y hora. Puedes añadírselo
                importándolas solas con su FIT.
              </p>
              <ul className="batch-list">
                {racesWithoutFit.map((race) => (
                  <li key={race.spl_path}>
                    <span className="strong">{raceTitle(race)}</span>{" "}
                    <span className="muted num">{race.event_date}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
          {fitsWithoutRace.length > 0 && (
            <>
              <p className="small muted">FIT que no se han usado:</p>
              <ul className="batch-list">
                {fitsWithoutRace.map((fit) => (
                  <li key={fit.path}>
                    <span className="strong" title={fit.path}>
                      {fileName(fit.path)}
                    </span>
                    <span className="small muted">{fit.message}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </div>
      )}

      {notes > 0 && (
        <div className="batch-block">
          <h4>Con avisos</h4>
          <ul className="batch-list">
            {summary.warnings.map((w) => (
              <li key={w}>
                <span className="small">{w}</span>
              </li>
            ))}
            {racesWithNotes.map((race) => (
              <li key={race.spl_path}>
                <span>
                  <span className="strong" title={race.spl_path}>
                    {raceTitle(race)}
                  </span>{" "}
                  {race.event_date !== null && (
                    <span className="muted num">{race.event_date}</span>
                  )}{" "}
                  {race.status === "not_imported" && (
                    <span className="pill pill-error">No importada</span>
                  )}
                </span>
                {race.messages.map((m) => (
                  <span key={m} className="small muted">
                    {m}
                  </span>
                ))}
              </li>
            ))}
            {badFits.map((fit) => (
              <li key={fit.path}>
                <span className="strong" title={fit.path}>
                  {fileName(fit.path)}
                </span>
                <span className="small muted">{fit.message}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

export default BatchImportPanel;
