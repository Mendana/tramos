import { open } from "@tauri-apps/plugin-dialog";
import { ReactNode, useEffect, useState } from "react";
import {
  SHARE_HINTS,
  SHARE_LABELS,
  ShareChoice,
  getSettings,
  receivePackages,
  saveSettings,
  shareAll,
} from "./api";
import { Notice, PageHeader } from "./ui";
import { PANEL_GROUPS, usePanelVisibility } from "./panels";
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
  shareOwn: boolean;
  coach: boolean;
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

/** Un apartado del formulario: título, explicación y campos. */
interface Section {
  id: string;
  title: string;
  text: ReactNode;
  fields: ReactNode;
}

/**
 * Ajustes del usuario (ver `docs/app.md`, "Ajustes"), en dos pantallas que guardan el mismo
 * formulario: **Mi perfil** (quién eres y qué compartes) y **Ajustes** (umbrales de error, zona
 * horaria, colores del mapa, paneles y entrenar, con un índice a cada apartado).
 */
function SettingsView({ page, onSaved }: { page: "profile" | "settings"; onSaved: () => void }) {
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
          shareOwn: s.sharing.share_own,
          coach: s.sharing.coach,
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

  function toggle(field: "shareOwn" | "coach", value: boolean) {
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

  /** Tras guardar con carpeta: exporta las carreras propias si las comparte y busca las de sus
   * atletas si entrena. */
  async function syncFolder(shareOwn: boolean, coach: boolean): Promise<FolderResult | null> {
    const messages: string[] = [];
    const problems: string[] = [];
    if (shareOwn) {
      const r = await shareAll();
      const parts = [`${plural(r.written, "carrera exportada", "carreras exportadas")}`];
      if (r.unchanged > 0) parts.push(`${r.unchanged} sin cambios`);
      if (r.not_shared > 0) parts.push(`${r.not_shared} sin compartir`);
      messages.push(parts.join(", "));
      problems.push(...r.problems);
    }
    if (coach) {
      const r = await receivePackages();
      messages.push(
        `${plural(r.packages, "paquete", "paquetes")} de ${plural(r.runners, "atleta", "atletas")}`,
      );
      problems.push(...r.problems);
    }
    if (messages.length === 0) return null;
    return { message: `Carpeta compartida: ${messages.join("; ")}.`, problems };
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
          share_own: form.shareOwn,
          coach: form.coach,
          folder: folder === "" ? null : folder,
          default_choice: form.defaultChoice,
        },
        map: { pace_zones: paceZones, heart_rate_zones: heartRateZones },
      });
      setSaved(true);
      onSaved();
      if (folder !== "") setFolderResult(await syncFolder(form.shareOwn, form.coach));
    } catch (err) {
      setError(String(err));
    }
  }

  const visibility = usePanelVisibility();
  const hiddenCount = visibility?.hidden.size ?? 0;
  const panelsSection: Section = {
    id: "panels",
    title: "Paneles de análisis",
    text: "Los que has ocultado en Estadísticas y en las carreras. Se cambian al momento, sin guardar.",
    fields: (
      <div className="field">
        <span className="field-label">
          {hiddenCount === 0
            ? "Ves todos los paneles."
            : `Ocultos: ${hiddenCount} de ${PANEL_GROUPS.reduce((n, g) => n + g.panels.length, 0)}.`}
        </span>
        <div className="row">
          <button type="button" className="btn" onClick={() => visibility?.customize()}>
            Elegir paneles
          </button>
          {hiddenCount > 0 && (
            <button type="button" className="btn btn-ghost" onClick={() => visibility?.showAll()}>
              Enseñar todos
            </button>
          )}
        </div>
      </div>
    ),
  };

  const folderField = (view: "profile" | "settings") => (
    <div className="field">
      <span className="field-label">Carpeta compartida</span>
      <div className="row">
        <input
          className="input"
          aria-label="Carpeta compartida"
          placeholder="Sin carpeta: no se comparte nada"
          value={form?.folder ?? ""}
          onChange={(e) => update("folder", e.target.value)}
        />
        <button type="button" className="btn" onClick={() => void chooseFolder()}>
          Elegir…
        </button>
      </div>
      <span className="field-hint">
        {view === "profile"
          ? "Tu carpeta: la subcarpeta que quien te entrena ha compartido contigo (Drive, OneDrive, Dropbox…). Ahí se dejan tus carreras. No hace falta servidor."
          : "La carpeta madre, con una subcarpeta por atleta (Drive, OneDrive, Dropbox…). Se leen los paquetes de la carpeta y de sus subcarpetas, hasta tres niveles. No hace falta servidor."}
      </span>
    </div>
  );

  const sections = (form: Form): Section[] => {
    if (page === "profile") {
      return [
        {
          id: "who",
          title: "Quién eres",
          text: "Para encontrarte en cada carrera al importarla.",
          fields: (
            <>
              <label className="field">
                <span className="field-label">Nombre y apellidos</span>
                <input
                  className="input"
                  value={form.fullName}
                  onChange={(e) => update("fullName", e.target.value)}
                />
                <span className="field-hint">Tal y como sale en los resultados.</span>
              </label>
              <label className="field">
                <span className="field-label">Tarjeta SI</span>
                <input
                  className="input num"
                  inputMode="numeric"
                  value={form.siCard}
                  onChange={(e) => update("siCard", e.target.value)}
                />
              </label>
            </>
          ),
        },
        {
          id: "sharing",
          title: "Qué compartes",
          text: "Con quien te entrena, por la carpeta compartida. Tus carreras se exportan solas cuando cambian.",
          fields: (
            <>
              <Toggle
                label="Compartir mis carreras"
                hint="Se exportan a la carpeta compartida. Si lo quitas, las que ya están se quedan."
                checked={form.shareOwn}
                onChange={(v) => toggle("shareOwn", v)}
              />
              {folderField("profile")}
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
            </>
          ),
        },
      ];
    }
    const coach: Section = {
      id: "coach",
      title: "Entrenar",
      text: "Para ver las carreras de tus atletas. Tus carreras siguen igual.",
      fields: (
        <>
          <Toggle
            label="Entreno a otros atletas"
            hint="Añade la sección Atletas al menú. Cada minuto se importan de la carpeta compartida los paquetes nuevos de tus atletas, que se ven en solo lectura y con sus propios umbrales."
            checked={form.coach}
            onChange={(v) => toggle("coach", v)}
          />
          {form.coach && folderField("settings")}
        </>
      ),
    };
    return [
      {
        id: "errors",
        title: "Tramo con error",
        text: "Un tramo es error si pierdes más de los dos umbrales. Cambiarlos recalcula todas tus carreras.",
        fields: (
          <>
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
          </>
        ),
      },
      {
        id: "time",
        title: "Hora de las carreras",
        text: "Zona horaria de las horas del .spl. Se aplica a las carreras que importes a partir de ahora.",
        fields: (
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
            <span className="field-hint">Por ejemplo, Europe/Madrid o Atlantic/Canary.</span>
          </label>
        ),
      },
      {
        id: "map",
        title: "Colores del mapa",
        text: "Cómo se colorea el track por ritmo o por pulso. Con tus zonas, el mismo color significa lo mismo en todas las carreras.",
        fields: (
          <>
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
          </>
        ),
      },
      panelsSection,
      coach,
    ];
  };

  const list = form === null ? [] : sections(form);
  const body =
    form === null ? (
      error === null && <p className="muted">Cargando…</p>
    ) : (
      <form
        className="card"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        {list.map((section) => (
          <div className="form-section" key={section.id} id={`settings-${section.id}`}>
            <div className="form-section-text">
              <h3>{section.title}</h3>
              <p className="small muted">{section.text}</p>
            </div>
            <div className="form-fields">{section.fields}</div>
          </div>
        ))}

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
    );

  return (
    <>
      <PageHeader
        title={page === "profile" ? "Mi perfil" : "Ajustes"}
        subtitle="Se guardan en tu equipo."
      />
      {page === "settings" && list.length > 1 ? (
        <div className="with-index">
          <nav className="page-index" aria-label="Apartados">
            {list.map((section) => (
              <a key={section.id} className="page-index-link" href={`#settings-${section.id}`}>
                {section.title}
              </a>
            ))}
          </nav>
          <div className="with-index-body">{body}</div>
        </div>
      ) : (
        body
      )}
      {form === null && error !== null && <Notice kind="error">{error}</Notice>}
    </>
  );
}

/** Casilla con su explicación debajo. */
function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="check check-block">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>
        <span className="strong">{label}</span>
        <span className="field-hint">{hint}</span>
      </span>
    </label>
  );
}

export default SettingsView;
