// Editor de las zonas de color del mapa (#96, docs/app.md, "Ajustes"): por cuantiles de cada
// carrera o zonas propias de ritmo o pulso con su color. Los avisos sobre los colores los calcula
// Rust (`check_zones`); aquí solo se editan los textos.
import { useEffect, useState } from "react";
import { MAX_ZONES, ZONE_COLORS, Zones, checkZones, clock } from "./api";
import { Notice } from "./ui";

export type ZoneMetric = "pace" | "heart_rate";

/** Zonas tal y como se editan: los límites, como los escribe el usuario. */
export interface ZoneDraft {
  limits: string[];
  colors: string[];
}

/** Zonas que se proponen al elegir «Mis zonas»: pulso en ppm y ritmo en s/km. */
const SUGGESTED: Record<ZoneMetric, number[]> = {
  heart_rate: [120, 140, 160, 175],
  pace: [270, 330, 390, 480],
};
/** Cuánto sube el límite de una zona añadida respecto a la anterior. */
const STEP: Record<ZoneMetric, number> = { heart_rate: 10, pace: 60 };

const NAMES: Record<ZoneMetric, string> = { pace: "ritmo", heart_rate: "pulso" };

function formatLimit(metric: ZoneMetric, value: number): string {
  return metric === "pace" ? clock(value) : String(value).replace(".", ",");
}

/** Un límite escrito: ritmo en «m:ss» (o minutos enteros) y pulso en ppm. `null` si no vale. */
function parseLimit(metric: ZoneMetric, text: string): number | null {
  const t = text.trim();
  if (metric === "pace") {
    const m = /^(\d{1,2})(?::([0-5]\d))?$/.exec(t);
    return m === null ? null : Number(m[1]) * 60 + Number(m[2] ?? 0);
  }
  const v = Number(t.replace(",", "."));
  return t === "" || !Number.isFinite(v) ? null : v;
}

export function toDraft(metric: ZoneMetric, zones: Zones | null): ZoneDraft | null {
  if (zones === null) return null;
  return { limits: zones.limits.map((v) => formatLimit(metric, v)), colors: [...zones.colors] };
}

/** Las zonas del borrador, o por qué no se entiende algún límite. */
export function fromDraft(metric: ZoneMetric, draft: ZoneDraft): Zones | string {
  const limits: number[] = [];
  for (const text of draft.limits) {
    const v = parseLimit(metric, text);
    if (v === null) {
      return metric === "pace"
        ? `«${text}» no es un ritmo: escríbelo como 5:30 (min/km).`
        : `«${text}» no es un número de pulsaciones.`;
    }
    limits.push(v);
  }
  return { limits, colors: draft.colors };
}

/** El texto de error de unas zonas, con su magnitud delante. */
export function zonesError(metric: ZoneMetric, message: string): string {
  return `Zonas de ${NAMES[metric]}: ${message}`;
}

function suggested(metric: ZoneMetric): ZoneDraft {
  const limits = SUGGESTED[metric];
  return {
    limits: limits.map((v) => formatLimit(metric, v)),
    colors: ZONE_COLORS.slice(0, limits.length + 1),
  };
}

function ZoneEditor({
  metric,
  draft,
  onChange,
}: {
  metric: ZoneMetric;
  draft: ZoneDraft | null;
  onChange: (draft: ZoneDraft | null) => void;
}) {
  const [warnings, setWarnings] = useState<string[]>([]);

  useEffect(() => {
    if (draft === null) {
      setWarnings([]);
      return;
    }
    const zones = fromDraft(metric, draft);
    if (typeof zones === "string") {
      setWarnings([zones]);
      return;
    }
    let current = true;
    const timer = window.setTimeout(() => {
      checkZones(zones)
        .then((w) => {
          if (current) setWarnings(w);
        })
        .catch(() => {
          if (current) setWarnings([]);
        });
    }, 250);
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [metric, draft]);

  const title = metric === "pace" ? "Ritmo" : "Pulso";
  const hint =
    draft === null
      ? "Cinco tonos de azul repartidos por el tiempo de cada carrera: sirven para comparar dentro de una carrera, no entre carreras."
      : metric === "pace"
        ? "En min/km, por ejemplo 5:30. La zona 1 es la más rápida."
        : "En pulsaciones por minuto.";

  function setLimit(k: number, text: string) {
    if (draft === null) return;
    const limits = [...draft.limits];
    limits[k] = text;
    onChange({ ...draft, limits });
  }

  function setColor(k: number, color: string) {
    if (draft === null) return;
    const colors = [...draft.colors];
    colors[k] = color;
    onChange({ ...draft, colors });
  }

  function remove(k: number) {
    if (draft === null) return;
    const limits = [...draft.limits];
    limits.splice(k === 0 ? 0 : k - 1, 1);
    onChange({ limits, colors: draft.colors.filter((_, i) => i !== k) });
  }

  function add() {
    if (draft === null) return;
    const last = draft.limits.length === 0 ? null : draft.limits[draft.limits.length - 1];
    const previous = last === null ? null : parseLimit(metric, last);
    const next = previous === null ? SUGGESTED[metric][0] : previous + STEP[metric];
    const color =
      ZONE_COLORS.find((c) => !draft.colors.includes(c)) ??
      ZONE_COLORS[draft.colors.length % ZONE_COLORS.length];
    onChange({
      limits: [...draft.limits, formatLimit(metric, next)],
      colors: [...draft.colors, color],
    });
  }

  return (
    <div className="zone-editor">
      <label className="field">
        <span className="field-label">{title}</span>
        <select
          className="select"
          value={draft === null ? "quantiles" : "zones"}
          onChange={(e) => onChange(e.target.value === "zones" ? suggested(metric) : null)}
        >
          <option value="quantiles">Por cuantiles de cada carrera</option>
          <option value="zones">Mis zonas</option>
        </select>
        <span className="field-hint">{hint}</span>
      </label>
      {draft !== null && (
        <>
          <ol className="zone-list">
            {draft.colors.map((color, k) => (
              <li key={k} className="zone-row">
                <input
                  type="color"
                  className="zone-color"
                  value={color}
                  aria-label={`Color de la zona ${k + 1}`}
                  onChange={(e) => setColor(k, e.target.value)}
                />
                <span className="zone-name">Zona {k + 1}</span>
                {k === 0 ? (
                  <span className="small muted">
                    {metric === "pace" ? "más rápido que la zona 2" : "por debajo de la zona 2"}
                  </span>
                ) : (
                  <label className="zone-from">
                    <span className="small muted">desde</span>
                    <input
                      className="input num zone-limit"
                      inputMode={metric === "pace" ? "text" : "decimal"}
                      value={draft.limits[k - 1]}
                      aria-label={`Zona ${k + 1}: desde`}
                      onChange={(e) => setLimit(k - 1, e.target.value)}
                    />
                    <span className="small muted">{metric === "pace" ? "min/km" : "ppm"}</span>
                  </label>
                )}
                <button
                  type="button"
                  className="btn btn-ghost"
                  disabled={draft.colors.length <= 2}
                  aria-label={`Quitar la zona ${k + 1}`}
                  onClick={() => remove(k)}
                >
                  Quitar
                </button>
              </li>
            ))}
          </ol>
          <button
            type="button"
            className="btn"
            disabled={draft.colors.length >= MAX_ZONES}
            onClick={add}
          >
            Añadir zona
          </button>
          {warnings.length > 0 && (
            <Notice kind="warning">
              <ul>
                {warnings.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
            </Notice>
          )}
        </>
      )}
    </div>
  );
}

export default ZoneEditor;
