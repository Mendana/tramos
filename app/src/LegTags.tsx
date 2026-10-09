// Etiquetado de errores en la vista de carrera (docs/taxonomia.md). Tres niveles, ninguno
// obligatorio: confirmar con un clic los tramos propuestos (nivel 1), y en «Detalles» tipo y
// subtipo (nivel 2) y contexto (nivel 3). La taxonomía y la validación vienen del núcleo.
import { ReactNode, useEffect, useState } from "react";
import {
  CONFIRMATION_LABELS,
  Confirmation,
  EMPTY_TAG,
  LEG_PART_LABELS,
  LegPart,
  LegTag,
  TagView,
  Taxonomy,
  getTaxonomy,
  legTags,
  saveLegTag,
} from "./api";
import { PencilIcon } from "./ui";

const CONFIRMATIONS = Object.keys(CONFIRMATION_LABELS) as Confirmation[];
const LEG_PARTS = Object.keys(LEG_PART_LABELS) as LegPart[];

const CONFIRMATION_HINTS: Record<Confirmation, string> = {
  error: "Fue un error de orientación",
  no_error: "No hubo error",
  physical: "Perdiste tiempo por el físico, no por orientarte mal",
};

/** Tono de cada respuesta del nivel 1, para que se distingan de un vistazo. */
const CONFIRMATION_TONES: Record<Confirmation, string> = {
  error: "error",
  no_error: "success",
  physical: "warning",
};

export interface LegTagsState {
  taxonomy: Taxonomy | null;
  /** Etiquetas por tramo. */
  tags: Map<number, TagView>;
  /** Tramo que se está guardando. */
  saving: number | null;
  error: string | null;
  save: (leg: number, tag: LegTag) => Promise<boolean>;
}

/**
 * Taxonomía y etiquetas de un resultado, con la función para guardar una. `onSaved` avisa tras
 * cada guardado (p. ej. para el contador de errores sin revisar de la barra lateral).
 */
export function useLegTags(resultId: number, onSaved?: () => void): LegTagsState {
  const [taxonomy, setTaxonomy] = useState<Taxonomy | null>(null);
  const [tags, setTags] = useState<Map<number, TagView>>(new Map());
  const [saving, setSaving] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    setTags(new Map());
    setError(null);
    Promise.all([getTaxonomy(), legTags(resultId)])
      .then(([t, list]) => {
        if (!current) return;
        setTaxonomy(t);
        setTags(new Map(list.map((v) => [v.leg_index, v])));
      })
      .catch((err: unknown) => {
        if (current) setError(String(err));
      });
    return () => {
      current = false;
    };
  }, [resultId]);

  const save = async (leg: number, tag: LegTag) => {
    setSaving(leg);
    setError(null);
    try {
      const saved = await saveLegTag(resultId, leg, tag);
      setTags((prev) => {
        const next = new Map(prev);
        if (saved === null) next.delete(leg);
        else next.set(leg, saved);
        return next;
      });
      onSaved?.();
      return true;
    } catch (err: unknown) {
      setError(String(err));
      return false;
    } finally {
      setSaving(null);
    }
  };

  return { taxonomy, tags, saving, error, save };
}

/** «Navegación · Paralelo», o la clave tal cual si ya no está en la taxonomía. */
export function typeLabel(taxonomy: Taxonomy | null, tag: LegTag): string | null {
  if (tag.error_type === null) return null;
  const type = taxonomy?.types.find((t) => t.key === tag.error_type);
  const name = type?.label ?? tag.error_type;
  if (tag.error_subtype === null) return name;
  const sub = type?.subtypes.find((s) => s.key === tag.error_subtype)?.label ?? tag.error_subtype;
  return `${name} · ${sub}`;
}

/** Botón que se queda pulsado. */
function Chip({
  pressed,
  onClick,
  disabled,
  tone,
  small,
  title,
  children,
}: {
  pressed: boolean;
  onClick: () => void;
  disabled?: boolean;
  tone?: string;
  small?: boolean;
  title?: string;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={`chip${small ? " chip-sm" : ""}`}
      data-tone={tone}
      aria-pressed={pressed}
      disabled={disabled}
      title={title}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

/** Respuesta del nivel 1 en solo lectura. */
const CONFIRMATION_SUMMARIES: Record<Confirmation, string> = {
  error: "Error",
  no_error: "Sin error",
  physical: "Físico",
};

/**
 * Etiqueta de un tramo en solo lectura (al ver a un atleta): la respuesta del nivel 1 y el
 * contexto. El tipo ya sale en la fila.
 */
export function TagSummary({ leg, state }: { leg: number; state: LegTagsState }) {
  const tag = state.tags.get(leg)?.tag;
  if (tag === undefined) return null;
  const causes = tag.causes.map(
    (key) => state.taxonomy?.causes.find((c) => c.key === key)?.label ?? key,
  );
  const context = [
    tag.leg_part === null ? null : LEG_PART_LABELS[tag.leg_part],
    ...causes,
    tag.perceived_loss_s === null ? null : `${tag.perceived_loss_s} s percibidos`,
    tag.effort === null ? null : `esfuerzo ${tag.effort}/10`,
  ].filter((part): part is string => part !== null);
  return (
    <div className="tag-summary">
      {tag.confirmation !== null && (
        <span className="pill" data-tone={CONFIRMATION_TONES[tag.confirmation]}>
          {CONFIRMATION_SUMMARIES[tag.confirmation]}
        </span>
      )}
      {context.length > 0 && <span className="small muted">{context.join(" · ")}</span>}
      {tag.note !== null && <span className="small">«{tag.note}»</span>}
    </div>
  );
}

/** Nivel 1 en una fila: Error / No / Físico. Otro clic en la respuesta marcada la quita. */
function ConfirmChips({
  value,
  disabled,
  small,
  onChange,
}: {
  value: Confirmation | null;
  disabled?: boolean;
  small?: boolean;
  onChange: (c: Confirmation | null) => void;
}) {
  return (
    <div className="chips" role="group" aria-label="¿Hubo error?">
      {CONFIRMATIONS.map((c) => (
        <Chip
          key={c}
          small={small}
          tone={CONFIRMATION_TONES[c]}
          pressed={value === c}
          disabled={disabled}
          title={CONFIRMATION_HINTS[c]}
          onClick={() => onChange(value === c ? null : c)}
        >
          {CONFIRMATION_LABELS[c]}
        </Chip>
      ))}
    </div>
  );
}

/**
 * Etiqueta de un tramo en su fila. Los propuestos (o ya etiquetados) se confirman aquí mismo con
 * un clic; el lápiz abre el formulario completo, también para marcar un error en otro tramo.
 */
export function TagControls({
  leg,
  proposed,
  state,
  open,
  onToggleOpen,
}: {
  leg: number;
  proposed: boolean;
  state: LegTagsState;
  open: boolean;
  onToggleOpen: () => void;
}) {
  const tag = state.tags.get(leg)?.tag ?? null;
  const busy = state.saving !== null;
  const showConfirm = proposed || tag !== null;
  return (
    // Los clics y las teclas de los controles no seleccionan el tramo.
    <div className="tag-controls" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
        {showConfirm && (
          <ConfirmChips
            small
            value={tag?.confirmation ?? null}
            disabled={busy || state.taxonomy === null}
            onChange={(confirmation) => void state.save(leg, { ...(tag ?? EMPTY_TAG), confirmation })}
          />
        )}
        <button
          type="button"
          className="btn btn-ghost btn-icon"
          aria-expanded={open}
          aria-label={`Etiqueta del tramo ${leg}: tipo y contexto`}
          title="Tipo de error y contexto"
          disabled={state.taxonomy === null}
          onClick={onToggleOpen}
        >
          <PencilIcon size={16} />
        </button>
    </div>
  );
}

/** Formulario con los tres niveles de un tramo. Se guarda con «Guardar». */
export function TagEditor({
  leg,
  state,
  onClose,
}: {
  leg: number;
  state: LegTagsState;
  onClose: () => void;
}) {
  const stored = state.tags.get(leg)?.tag ?? null;
  const [draft, setDraft] = useState<LegTag>(stored ?? EMPTY_TAG);
  // Texto de los campos numéricos tal cual se escribe; se lee al guardar.
  const [loss, setLoss] = useState(stored?.perceived_loss_s?.toString() ?? "");
  const [effort, setEffort] = useState(stored?.effort?.toString() ?? "");
  const [invalid, setInvalid] = useState<string | null>(null);
  const taxonomy = state.taxonomy;
  if (taxonomy === null) return null;

  const set = (patch: Partial<LegTag>) => setDraft((d) => ({ ...d, ...patch }));
  const type = taxonomy.types.find((t) => t.key === draft.error_type) ?? null;
  const busy = state.saving !== null;

  const submit = async () => {
    const lossValue = loss.trim() === "" ? null : Number(loss.replace(",", "."));
    if (lossValue !== null && (!Number.isFinite(lossValue) || lossValue < 0)) {
      setInvalid("Los segundos perdidos tienen que ser un número positivo.");
      return;
    }
    const effortValue = effort === "" ? null : Number(effort);
    setInvalid(null);
    // Si no hubo error, el tipo no tiene sentido.
    const noError = draft.confirmation === "no_error";
    const ok = await state.save(leg, {
      ...draft,
      error_type: noError ? null : draft.error_type,
      error_subtype: noError ? null : draft.error_subtype,
      perceived_loss_s: lossValue,
      effort: effortValue,
    });
    if (ok) onClose();
  };

  return (
    <div className="tag-editor">
      <div className="tag-editor-head">
        <h4>Etiqueta del tramo {leg}</h4>
        <span className="small muted">Ningún apartado es obligatorio.</span>
      </div>

      <section className="tag-level">
        <h5>¿Hubo error?</h5>
        <ConfirmChips value={draft.confirmation} onChange={(confirmation) => set({ confirmation })} />
      </section>

      {draft.confirmation !== "no_error" && (
        <section className="tag-level">
          <h5>Tipo de error</h5>
          <div className="chips">
            {taxonomy.types.map((t) => (
              <Chip
                key={t.key}
                pressed={draft.error_type === t.key}
                title={t.description}
                onClick={() =>
                  set(
                    draft.error_type === t.key
                      ? { error_type: null, error_subtype: null }
                      : { error_type: t.key, error_subtype: null },
                  )
                }
              >
                {t.label}
              </Chip>
            ))}
          </div>
          {type !== null && (
            <div className="chips">
              {type.subtypes.map((s) => (
                <Chip
                  key={s.key}
                  small
                  pressed={draft.error_subtype === s.key}
                  onClick={() => set({ error_subtype: draft.error_subtype === s.key ? null : s.key })}
                >
                  {s.label}
                </Chip>
              ))}
            </div>
          )}
        </section>
      )}

      <section className="tag-level">
        <h5>Contexto</h5>
        <div className="tag-context">
          <div className="field">
            <span className="field-label">Por qué crees que fue</span>
            <div className="chips">
              {taxonomy.causes.map((c) => {
                const on = draft.causes.includes(c.key);
                return (
                  <Chip
                    key={c.key}
                    small
                    pressed={on}
                    onClick={() =>
                      set({
                        causes: on ? draft.causes.filter((k) => k !== c.key) : [...draft.causes, c.key],
                      })
                    }
                  >
                    {c.label}
                  </Chip>
                );
              })}
            </div>
          </div>
          <div className="field">
            <span className="field-label">En qué parte del tramo</span>
            <div className="chips">
              {LEG_PARTS.map((p) => (
                <Chip
                  key={p}
                  small
                  pressed={draft.leg_part === p}
                  onClick={() => set({ leg_part: draft.leg_part === p ? null : p })}
                >
                  {LEG_PART_LABELS[p]}
                </Chip>
              ))}
            </div>
          </div>
          <div className="tag-fields">
            <label className="field">
              <span className="field-label">Segundos que crees haber perdido</span>
              <input
                className="input"
                inputMode="decimal"
                value={loss}
                onChange={(e) => setLoss(e.target.value)}
              />
            </label>
            <label className="field">
              <span className="field-label">Esfuerzo (1 = suave, 10 = a tope)</span>
              <select className="select" value={effort} onChange={(e) => setEffort(e.target.value)}>
                <option value="">Sin indicar</option>
                {Array.from({ length: 10 }, (_, i) => i + 1).map((n) => (
                  <option key={n} value={String(n)}>
                    {n}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <label className="field">
            <span className="field-label">Nota</span>
            <textarea
              className="input textarea"
              rows={2}
              value={draft.note ?? ""}
              onChange={(e) => set({ note: e.target.value })}
            />
          </label>
        </div>
      </section>

      {invalid !== null && <p className="small loss-bad">{invalid}</p>}
      <div className="tag-actions">
        {stored !== null && (
          <button
            type="button"
            className="btn btn-ghost"
            disabled={busy}
            onClick={() => void state.save(leg, EMPTY_TAG).then((ok) => ok && onClose())}
          >
            Quitar etiqueta
          </button>
        )}
        <button type="button" className="btn" disabled={busy} onClick={onClose}>
          Cancelar
        </button>
        <button type="button" className="btn btn-primary" disabled={busy} onClick={() => void submit()}>
          Guardar
        </button>
      </div>
    </div>
  );
}
