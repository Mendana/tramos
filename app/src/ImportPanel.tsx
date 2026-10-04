import { useEffect, useMemo, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FORMAT_LABELS,
  ImportOutcome,
  ImportPreview,
  RaceFormat,
  RunnerChoice,
  RunnerIdentity,
  importRace,
  previewImport,
  statusLabel,
  storedIdentity,
} from "./api";

/** Con más candidatos que esto, la lista pide filtrar. */
const MAX_LISTED = 50;

interface Files {
  spl: string | null;
  fit: string | null;
}

/** Separa los ficheros soltados o elegidos en el .spl y el FIT, por su extensión. */
function classify(paths: string[], current: Files): Files {
  const next = { ...current };
  for (const path of paths) {
    const lower = path.toLowerCase();
    if (lower.endsWith(".spl")) next.spl = path;
    else if (lower.endsWith(".fit")) next.fit = path;
  }
  return next;
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function runnerLabel(c: RunnerChoice): string {
  const name = `${c.given_name} ${c.family_name}`.trim() || "Sin nombre";
  const card = c.si_card === null ? "" : ` · tarjeta ${c.si_card}`;
  const club = c.club === null ? "" : ` · ${c.club}`;
  return `${c.class_name} · ${name}${club}${card} · ${statusLabel(c.status, c.place)}`;
}

function sameResult(a: RunnerChoice, b: RunnerChoice | null): boolean {
  return (
    b !== null &&
    a.result.class_index === b.result.class_index &&
    a.result.result_index === b.result.result_index
  );
}

/** Importar una carrera: ficheros, identidad, corredor y formato (ver `docs/app.md`). */
function ImportPanel({ onImported }: { onImported: () => void }) {
  const [files, setFiles] = useState<Files>({ spl: null, fit: null });
  const [dragging, setDragging] = useState(false);
  const [siCard, setSiCard] = useState("");
  const [fullName, setFullName] = useState("");
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [chosen, setChosen] = useState<RunnerChoice | null>(null);
  const [format, setFormat] = useState<RaceFormat | null>(null);
  const [filter, setFilter] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<ImportOutcome | null>(null);

  useEffect(() => {
    storedIdentity()
      .then((identity) => {
        setSiCard(identity.si_card === null ? "" : String(identity.si_card));
        setFullName(identity.full_name ?? "");
      })
      .catch((err: unknown) => setError(String(err)));
  }, []);

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over") {
        setDragging(true);
      } else if (payload.type === "leave") {
        setDragging(false);
      } else {
        setDragging(false);
        addFiles(payload.paths);
      }
    });
    return () => {
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, []);

  function addFiles(paths: string[]) {
    setFiles((current) => classify(paths, current));
    setPreview(null);
    setOutcome(null);
    setError(null);
  }

  async function chooseFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "Splits (.spl) y reloj (.fit)", extensions: ["spl", "fit", "SPL", "FIT"] }],
    });
    if (selected !== null) addFiles(selected);
  }

  function identity(): RunnerIdentity {
    const card = Number.parseInt(siCard.trim(), 10);
    return {
      si_card: Number.isFinite(card) && card > 0 ? card : null,
      full_name: fullName.trim() === "" ? null : fullName.trim(),
    };
  }

  async function review() {
    if (files.spl === null) return;
    setBusy(true);
    setError(null);
    setOutcome(null);
    try {
      const p = await previewImport(files.spl, files.fit, identity());
      setPreview(p);
      setFormat(p.suggested_format);
      setFilter("");
      const single = p.matched === "unique" || p.matched === "unique_name_mismatch";
      setChosen(single ? p.candidates[0] : null);
    } catch (err) {
      setPreview(null);
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function confirm() {
    if (files.spl === null || preview === null || chosen === null) return;
    setBusy(true);
    setError(null);
    try {
      const result = await importRace({
        spl_path: files.spl,
        fit_path: files.fit,
        result: chosen.result,
        format,
        identity: identity(),
      });
      setOutcome(result);
      setPreview(null);
      setFiles({ spl: null, fit: null });
      onImported();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  const listed = useMemo(() => {
    if (preview === null) return [];
    const words = filter.toLowerCase().split(/\s+/).filter((w) => w !== "");
    return preview.candidates.filter((c) => {
      const text = runnerLabel(c).toLowerCase();
      return words.every((w) => text.includes(w));
    });
  }, [preview, filter]);

  return (
    <section className="panel">
      <h2>Importar una carrera</h2>

      <div className={dragging ? "dropzone dragging" : "dropzone"}>
        <p>Arrastra aquí el .spl de WinSplits y, si lo tienes, el .fit de tu reloj.</p>
        <button type="button" onClick={chooseFiles} disabled={busy}>
          Elegir ficheros…
        </button>
        <ul className="files">
          <li>Splits: {files.spl === null ? "—" : fileName(files.spl)}</li>
          <li>Reloj: {files.fit === null ? "— (opcional)" : fileName(files.fit)}</li>
        </ul>
      </div>

      <div className="identity">
        <label>
          Tu tarjeta SI
          <input
            inputMode="numeric"
            value={siCard}
            onChange={(e) => setSiCard(e.target.value)}
          />
        </label>
        <label>
          Tu nombre y apellidos
          <input value={fullName} onChange={(e) => setFullName(e.target.value)} />
        </label>
        <button type="button" onClick={review} disabled={busy || files.spl === null}>
          Revisar
        </button>
      </div>

      {error !== null && <p role="alert" className="error">{error}</p>}

      {preview !== null && (
        <div className="preview">
          <h3>
            {preview.event_name ?? "Carrera sin nombre"} · {preview.event_date}
          </h3>
          {preview.already_imported && (
            <p className="note">
              Esta carrera ya está importada: no se duplicará, solo se actualizará.
            </p>
          )}
          {preview.fit_points !== null && (
            <p className="muted">Reloj: {preview.fit_points} puntos con posición.</p>
          )}

          <fieldset>
            <legend>Tu resultado</legend>
            {preview.matched === "unique_name_mismatch" && (
              <p className="note">
                La tarjeta coincide, pero el nombre no: comprueba que es tu resultado.
              </p>
            )}
            {preview.matched === "ambiguous" && (
              <p className="note">Hay varios resultados posibles: elige el tuyo.</p>
            )}
            {preview.matched === "not_found" && (
              <>
                <p className="note">
                  No te he encontrado por tarjeta ni por nombre: búscate en la lista.
                </p>
                <input
                  placeholder="Filtrar por nombre, club, categoría…"
                  value={filter}
                  onChange={(e) => setFilter(e.target.value)}
                />
              </>
            )}
            {listed.length > MAX_LISTED ? (
              <p className="muted">
                {listed.length} resultados: escribe algo en el filtro para acotar.
              </p>
            ) : (
              <ul className="candidates">
                {listed.map((c) => (
                  <li key={`${c.result.class_index}-${c.result.result_index}`}>
                    <label>
                      <input
                        type="radio"
                        name="candidate"
                        checked={sameResult(c, chosen)}
                        onChange={() => setChosen(c)}
                      />
                      {runnerLabel(c)}
                    </label>
                  </li>
                ))}
              </ul>
            )}
          </fieldset>

          <label>
            Formato
            <select
              value={format ?? ""}
              onChange={(e) =>
                setFormat(e.target.value === "" ? null : (e.target.value as RaceFormat))
              }
            >
              <option value="">Sin decidir</option>
              {(Object.keys(FORMAT_LABELS) as RaceFormat[]).map((f) => (
                <option key={f} value={f}>
                  {FORMAT_LABELS[f]}
                </option>
              ))}
            </select>
          </label>
          {preview.median_winner_s !== null && (
            <p className="muted">
              Sugerido por el tiempo de los ganadores (mediana de las categorías:{" "}
              {Math.round(preview.median_winner_s / 60)} min).
            </p>
          )}

          <button type="button" onClick={confirm} disabled={busy || chosen === null}>
            Importar
          </button>
        </div>
      )}

      {outcome !== null && (
        <div className="outcome">
          <p>
            {outcome.already_imported
              ? "Carrera actualizada (ya estaba importada)."
              : "Carrera importada."}
          </p>
          {outcome.alignment !== null && (
            <p>
              {outcome.alignment.track_saved
                ? `Reloj alineado: desfase ${outcome.alignment.offset_s?.toFixed(1)} s, confianza ${Math.round((outcome.alignment.confidence ?? 0) * 100)} %.`
                : "El reloj no se ha guardado."}
            </p>
          )}
          {[...outcome.warnings, ...(outcome.alignment?.messages ?? [])].map((m) => (
            <p key={m} className="note">
              {m}
            </p>
          ))}
        </div>
      )}
    </section>
  );
}

export default ImportPanel;
