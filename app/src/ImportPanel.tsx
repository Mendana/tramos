import { ReactNode, useEffect, useMemo, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import {
  FORMAT_LABELS,
  ImportOutcome,
  ImportPreview,
  RaceFormat,
  RunnerChoice,
  RunnerIdentity,
  decimal,
  fileName,
  getSettings,
  importRace,
  previewImport,
  statusLabel,
} from "./api";
import { FileIcon, Notice, UploadIcon, WatchIcon } from "./ui";

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

function runnerName(c: RunnerChoice): string {
  return `${c.given_name} ${c.family_name}`.trim() || "Sin nombre";
}

function runnerDetails(c: RunnerChoice): string {
  return [
    c.class_name,
    c.club,
    c.si_card === null ? null : `tarjeta ${c.si_card}`,
    statusLabel(c.status, c.place),
  ]
    .filter((part) => part !== null)
    .join(" · ");
}

function sameResult(a: RunnerChoice, b: RunnerChoice | null): boolean {
  return (
    b !== null &&
    a.result.class_index === b.result.class_index &&
    a.result.result_index === b.result.result_index
  );
}

/** Importar una carrera: ficheros, identidad, corredor y formato (ver `docs/app.md`). La cabecera
 * la pone `ImportScreen`. */
function ImportPanel({
  onImported,
  onOpen,
}: {
  onImported: () => void;
  onOpen: (resultId: number) => void;
}) {
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
    getSettings()
      .then(({ identity }) => {
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
      const text = `${runnerName(c)} ${runnerDetails(c)}`.toLowerCase();
      return words.every((w) => text.includes(w));
    });
  }, [preview, filter]);

  return (
    <>
      {outcome !== null && <Outcome outcome={outcome} onOpen={onOpen} />}

      <section className="card">
        <h3 className="section-title">1 · Ficheros</h3>
        <div className={dragging ? "dropzone is-dragging" : "dropzone"}>
          <span className="dropzone-icon">
            <UploadIcon size={32} />
          </span>
          <p>
            <strong>Arrastra aquí los ficheros</strong>
            <br />
            <span className="small muted">o elígelos desde tu equipo</span>
          </p>
          <button type="button" className="btn" onClick={chooseFiles} disabled={busy}>
            Elegir ficheros…
          </button>
          <div className="files">
            <FileSlot icon={<FileIcon />} label="Splits (.spl)" path={files.spl} />
            <FileSlot icon={<WatchIcon />} label="Reloj (.fit), opcional" path={files.fit} />
          </div>
        </div>
      </section>

      <section className="card">
        <h3 className="section-title">2 · Quién eres</h3>
        <div className="row">
          <label className="field">
            <span className="field-label">Tarjeta SI</span>
            <input
              className="input num"
              inputMode="numeric"
              value={siCard}
              onChange={(e) => setSiCard(e.target.value)}
            />
          </label>
          <label className="field field-grow">
            <span className="field-label">Nombre y apellidos</span>
            <input className="input" value={fullName} onChange={(e) => setFullName(e.target.value)} />
          </label>
          <button
            type="button"
            className="btn btn-primary"
            onClick={review}
            disabled={busy || files.spl === null}
          >
            Revisar
          </button>
        </div>
        {files.spl === null && (
          <p className="small muted">Añade primero el .spl para revisar la carrera.</p>
        )}
      </section>

      {error !== null && <Notice kind="error">{error}</Notice>}

      {preview !== null && (
        <section className="card">
          <h3 className="section-title">3 · Confirmar</h3>
          <div className="page-header-text">
            <h2>{preview.event_name ?? "Carrera sin nombre"}</h2>
            <div className="meta">
              <span className="num">{preview.event_date}</span>
              {preview.fit_points !== null && (
                <span className="pill">
                  <WatchIcon size={14} /> {preview.fit_points} puntos del reloj
                </span>
              )}
            </div>
          </div>
          {preview.already_imported && (
            <Notice>Esta carrera ya está importada: no se duplicará, solo se actualizará.</Notice>
          )}

          <div className="stack">
            <div className="field">
              <span className="field-label">Tu resultado</span>
              {preview.matched === "unique_name_mismatch" && (
                <Notice kind="warning">
                  La tarjeta coincide, pero el nombre no: comprueba que es tu resultado.
                </Notice>
              )}
              {preview.matched === "ambiguous" && (
                <Notice kind="warning">Hay varios resultados posibles: elige el tuyo.</Notice>
              )}
              {preview.matched === "not_found" && (
                <>
                  <Notice kind="warning">
                    No te he encontrado por tarjeta ni por nombre: búscate en la lista.
                  </Notice>
                  <input
                    className="input"
                    placeholder="Filtrar por nombre, club, categoría…"
                    value={filter}
                    onChange={(e) => setFilter(e.target.value)}
                  />
                </>
              )}
              {listed.length > MAX_LISTED ? (
                <p className="small muted">
                  {listed.length} resultados: escribe algo en el filtro para acotar.
                </p>
              ) : listed.length === 0 ? (
                <p className="small muted">Ningún resultado casa con el filtro.</p>
              ) : (
                <div className="choices" role="radiogroup" aria-label="Tu resultado">
                  {listed.map((c) => (
                    <label className="choice" key={`${c.result.class_index}-${c.result.result_index}`}>
                      <input
                        type="radio"
                        name="candidate"
                        checked={sameResult(c, chosen)}
                        onChange={() => setChosen(c)}
                      />
                      <span className="choice-main">
                        <span className="strong">{runnerName(c)}</span>
                        <span className="small muted">{runnerDetails(c)}</span>
                      </span>
                    </label>
                  ))}
                </div>
              )}
            </div>

            <div className="field">
              <span className="field-label">Formato</span>
              <div className="segmented" role="radiogroup" aria-label="Formato">
                {(Object.keys(FORMAT_LABELS) as RaceFormat[]).map((f) => (
                  <label key={f}>
                    <input
                      type="radio"
                      name="format"
                      checked={format === f}
                      onChange={() => setFormat(f)}
                    />
                    {FORMAT_LABELS[f]}
                  </label>
                ))}
              </div>
              {preview.median_winner_s !== null && (
                <span className="field-hint">
                  Sugerido por el tiempo de los ganadores: la mediana de las categorías es de{" "}
                  {Math.round(preview.median_winner_s / 60)} min.
                </span>
              )}
            </div>
          </div>

          <div className="row">
            <button
              type="button"
              className="btn btn-primary btn-lg"
              onClick={confirm}
              disabled={busy || chosen === null}
            >
              Importar
            </button>
            {chosen === null && <span className="small muted">Elige tu resultado.</span>}
          </div>
        </section>
      )}
    </>
  );
}

function FileSlot({
  icon,
  label,
  path,
}: {
  icon: ReactNode;
  label: string;
  path: string | null;
}) {
  return (
    <div className={path === null ? "file is-empty" : "file"}>
      {icon}
      <div className="choice-main">
        <span className="small muted">{label}</span>
        <span className="file-name" title={path ?? undefined}>
          {path === null ? "Sin fichero" : fileName(path)}
        </span>
      </div>
    </div>
  );
}

function Outcome({
  outcome,
  onOpen,
}: {
  outcome: ImportOutcome;
  onOpen: (resultId: number) => void;
}) {
  const alignment = outcome.alignment;
  const messages = [...outcome.warnings, ...(alignment?.messages ?? [])];
  return (
    <div className="card">
      <Notice
        kind={alignment !== null && (!alignment.track_saved || alignment.offset_s === null) ? "warning" : "success"}
      >
        <strong>
          {outcome.already_imported
            ? "Carrera actualizada (ya estaba importada)."
            : "Carrera importada."}
        </strong>{" "}
        {alignment !== null &&
          (!alignment.track_saved
            ? "El reloj no se ha guardado."
            : alignment.offset_s === null
              ? "El reloj se ha guardado, pero sin situar en la carrera: corrige el desfase en la vista de la carrera."
              : `Reloj alineado: desfase ${decimal(alignment.offset_s, 1)} s, confianza ${Math.round((alignment.confidence ?? 0) * 100)} %.`)}
      </Notice>
      {messages.map((m) => (
        <Notice key={m} kind="warning">
          {m}
        </Notice>
      ))}
      <div>
        <button type="button" className="btn btn-primary" onClick={() => onOpen(outcome.result_id)}>
          Ver la carrera
        </button>
      </div>
    </div>
  );
}

export default ImportPanel;
