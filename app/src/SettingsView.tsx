import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import {
  AppMode,
  SHARE_HINTS,
  SHARE_LABELS,
  ShareChoice,
  getSettings,
  receivePackages,
  saveSettings,
  shareAll,
} from "./api";
import { Notice, PageHeader } from "./ui";
import ZoneEditor, { ZoneDraft, ZoneMetric, fromDraft, toDraft, zonesError } from "./ZoneEditor";

/** Zonas horarias que se ofrecen en la lista; se puede escribir cualquier otra IANA. */
const COMMON_TIME_ZONES = [
  "Europe/Madrid",
  "Atlantic/Canary",
  "Europe/Lisbon",
  "Europe/Paris",
  "Europe/London",
  "UTC",
];

interface Form {
  thresholdS: string;
  thresholdPct: string;
  timeZone: string;
  siCard: string;
  fullName: string;
  mode: AppMode;
  folder: string;
  defaultChoice: ShareChoice;
  paceZones: ZoneDraft | null;
  heartRateZones: ZoneDraft | null;
}

/** Qué ha pasado con la carpeta compartida al guardar. */
interface FolderResult {
  message: string;
  problems: string[];
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** Ajustes: umbrales de error, zona horaria, identidad y carpeta compartida (ver `docs/app.md`). */
function SettingsView({ onSaved }: { onSaved: () => void }) {
  const [form, setForm] = useState<Form | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [folderResult, setFolderResult] = useState<FolderResult | null>(null);

  useEffect(() => {
    getSettings()
      .then((s) =>
        setForm({
          thresholdS: String(s.error_threshold_s),
          thresholdPct: String(s.error_threshold_pct),
          timeZone: s.time_zone,
          siCard: s.identity.si_card === null ? "" : String(s.identity.si_card),
          fullName: s.identity.full_name ?? "",
          mode: s.sharing.mode,
          folder: s.sharing.folder ?? "",
          defaultChoice: s.sharing.default_choice,
          paceZones: toDraft("pace", s.map.pace_zones),
          heartRateZones: toDraft("heart_rate", s.map.heart_rate_zones),
        }),
      )
      .catch((err: unknown) => setError(String(err)));
  }, []);

  function update(field: keyof Form, value: string) {
    setForm((current) => (current === null ? current : { ...current, [field]: value }));
    setSaved(false);
    setFolderResult(null);
  }

  function updateZones(field: "paceZones" | "heartRateZones", draft: ZoneDraft | null) {
    setForm((current) => (current === null ? current : { ...current, [field]: draft }));
    setSaved(false);
  }

  async function chooseFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") update("folder", selected);
  }

  /** Tras guardar con carpeta: el corredor exporta todas sus carreras y la entrenadora busca. */
  async function syncFolder(mode: AppMode): Promise<FolderResult> {
    if (mode === "runner") {
      const r = await shareAll();
      const parts = [`${plural(r.written, "carrera exportada", "carreras exportadas")}`];
      if (r.unchanged > 0) parts.push(`${r.unchanged} sin cambios`);
      if (r.not_shared > 0) parts.push(`${r.not_shared} sin compartir`);
      return { message: `Carpeta compartida: ${parts.join(", ")}.`, problems: r.problems };
    }
    const r = await receivePackages();
    return {
      message: `Carpeta compartida: ${plural(r.packages, "paquete", "paquetes")} de ${plural(r.runners, "corredor", "corredores")}.`,
      problems: r.problems,
    };
  }

  async function save() {
    if (form === null) return;
    setError(null);
    const number = (text: string) =>
      text.trim() === "" ? Number.NaN : Number(text.trim().replace(",", "."));
    const thresholdS = number(form.thresholdS);
    const thresholdPct = number(form.thresholdPct);
    if (!Number.isFinite(thresholdS) || !Number.isFinite(thresholdPct)) {
      setError("Los umbrales tienen que ser números.");
      return;
    }
    const card = form.siCard.trim();
    if (card !== "" && !/^\d+$/.test(card)) {
      setError("La tarjeta SI tiene que ser un número.");
      return;
    }
    const folder = form.folder.trim();
    const zones = (metric: ZoneMetric, draft: ZoneDraft | null) =>
      draft === null ? null : fromDraft(metric, draft);
    const paceZones = zones("pace", form.paceZones);
    const heartRateZones = zones("heart_rate", form.heartRateZones);
    if (typeof paceZones === "string") {
      setError(zonesError("pace", paceZones));
      return;
    }
    if (typeof heartRateZones === "string") {
      setError(zonesError("heart_rate", heartRateZones));
      return;
    }
    try {
      await saveSettings({
        error_threshold_s: thresholdS,
        error_threshold_pct: thresholdPct,
        time_zone: form.timeZone.trim(),
        identity: {
          si_card: card === "" ? null : Number(card),
          full_name: form.fullName.trim() === "" ? null : form.fullName.trim(),
        },
        sharing: {
          mode: form.mode,
          folder: folder === "" ? null : folder,
          default_choice: form.defaultChoice,
        },
        map: { pace_zones: paceZones, heart_rate_zones: heartRateZones },
      });
      setSaved(true);
      onSaved();
      if (folder !== "") setFolderResult(await syncFolder(form.mode));
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <>
      <PageHeader title="Ajustes" subtitle="Se guardan en tu equipo." />
      {form === null ? (
        error === null && <p className="muted">Cargando…</p>
      ) : (
        <form
          className="card"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          {/* La entrenadora ve a cada corredor con sus umbrales: estos ajustes son de corredor. */}
          {form.mode === "runner" && (
            <>
              <div className="form-section">
                <div className="form-section-text">
                  <h3>Tramo con error</h3>
                  <p className="small muted">
                    Un tramo es error si pierdes más de los dos umbrales. Cambiarlos recalcula todas
                    tus carreras.
                  </p>
                </div>
                <div className="form-fields">
                  <label className="field">
                    <span className="field-label">Pérdida mínima</span>
                    <input
                      className="input num"
                      inputMode="decimal"
                      value={form.thresholdS}
                      onChange={(e) => update("thresholdS", e.target.value)}
                    />
                    <span className="field-hint">En segundos. Por defecto, 15.</span>
                  </label>
                  <label className="field">
                    <span className="field-label">Pérdida mínima relativa</span>
                    <input
                      className="input num"
                      inputMode="decimal"
                      value={form.thresholdPct}
                      onChange={(e) => update("thresholdPct", e.target.value)}
                    />
                    <span className="field-hint">En % del tiempo esperado. Por defecto, 10.</span>
                  </label>
                </div>
              </div>

              <div className="form-section">
                <div className="form-section-text">
                  <h3>Hora de las carreras</h3>
                  <p className="small muted">
                    Zona horaria de las horas del .spl. Se aplica a las carreras que importes a
                    partir de ahora.
                  </p>
                </div>
                <div className="form-fields">
                  <label className="field">
                    <span className="field-label">Zona horaria</span>
                    <input
                      className="input"
                      list="time-zones"
                      value={form.timeZone}
                      onChange={(e) => update("timeZone", e.target.value)}
                    />
                    <datalist id="time-zones">
                      {COMMON_TIME_ZONES.map((zone) => (
                        <option key={zone} value={zone} />
                      ))}
                    </datalist>
                    <span className="field-hint">
                      Por ejemplo, Europe/Madrid o Atlantic/Canary.
                    </span>
                  </label>
                </div>
              </div>

              <div className="form-section">
                <div className="form-section-text">
                  <h3>Quién eres</h3>
                  <p className="small muted">Para encontrarte en cada carrera al importarla.</p>
                </div>
                <div className="form-fields">
                  <label className="field">
                    <span className="field-label">Tarjeta SI</span>
                    <input
                      className="input num"
                      inputMode="numeric"
                      value={form.siCard}
                      onChange={(e) => update("siCard", e.target.value)}
                    />
                  </label>
                  <label className="field">
                    <span className="field-label">Nombre y apellidos</span>
                    <input
                      className="input"
                      value={form.fullName}
                      onChange={(e) => update("fullName", e.target.value)}
                    />
                  </label>
                </div>
              </div>

              <div className="form-section">
                <div className="form-section-text">
                  <h3>Colores del mapa</h3>
                  <p className="small muted">
                    Cómo se colorea el track por ritmo o por pulso. Con tus zonas, el mismo color
                    significa lo mismo en todas las carreras.
                  </p>
                </div>
                <div className="form-fields">
                  <ZoneEditor
                    metric="pace"
                    draft={form.paceZones}
                    onChange={(d) => updateZones("paceZones", d)}
                  />
                  <ZoneEditor
                    metric="heart_rate"
                    draft={form.heartRateZones}
                    onChange={(d) => updateZones("heartRateZones", d)}
                  />
                </div>
              </div>
            </>
          )}
          <div className="form-section">
            <div className="form-section-text">
              <h3>Compartir con la entrenadora</h3>
              <p className="small muted">
                Por una carpeta sincronizada (Drive, OneDrive, Dropbox…) que compartís. No hace
                falta servidor.
              </p>
            </div>
            <div className="form-fields">
              <label className="field">
                <span className="field-label">Uso la app como</span>
                <select
                  className="select"
                  value={form.mode}
                  onChange={(e) => update("mode", e.target.value)}
                >
                  <option value="runner">Corredor</option>
                  <option value="coach">Entrenadora</option>
                </select>
                <span className="field-hint">
                  {form.mode === "runner"
                    ? "Tus carreras se exportan solas a la carpeta cuando cambian."
                    : "Cada minuto se importan los paquetes nuevos que dejen los corredores. Sus carreras se ven en solo lectura y con sus propios umbrales."}
                </span>
              </label>
              <div className="field">
                <span className="field-label">Carpeta compartida</span>
                <div className="row">
                  <input
                    className="input"
                    aria-label="Carpeta compartida"
                    placeholder="Sin carpeta: no se comparte nada"
                    value={form.folder}
                    onChange={(e) => update("folder", e.target.value)}
                  />
                  <button type="button" className="btn" onClick={() => void chooseFolder()}>
                    Elegir…
                  </button>
                </div>
              </div>
              {form.mode === "runner" && (
                <label className="field">
                  <span className="field-label">Qué compartes de cada carrera</span>
                  <select
                    className="select"
                    value={form.defaultChoice}
                    onChange={(e) => update("defaultChoice", e.target.value)}
                  >
                    {(Object.keys(SHARE_LABELS) as ShareChoice[]).map((choice) => (
                      <option key={choice} value={choice}>
                        {SHARE_LABELS[choice]}
                      </option>
                    ))}
                  </select>
                  <span className="field-hint">
                    {SHARE_HINTS[form.defaultChoice]} Puedes cambiarlo en cada carrera.
                  </span>
                </label>
              )}
            </div>
          </div>

          {error !== null && <Notice kind="error">{error}</Notice>}
          {folderResult !== null && folderResult.problems.length > 0 && (
            <Notice kind="warning">
              No se ha podido con todo:
              <ul>
                {folderResult.problems.map((p) => (
                  <li key={p}>{p}</li>
                ))}
              </ul>
            </Notice>
          )}
          <div className="row">
            <button type="submit" className="btn btn-primary">
              Guardar
            </button>
            {saved && (
              <span className="small muted">
                Guardado.{folderResult !== null && ` ${folderResult.message}`}
              </span>
            )}
          </div>
        </form>
      )}
      {form === null && error !== null && <Notice kind="error">{error}</Notice>}
    </>
  );
}

export default SettingsView;
