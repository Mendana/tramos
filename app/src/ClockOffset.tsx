// Desfase entre el reloj y el cronometraje de una carrera (#68, docs/app.md, "Vista de carrera").
// Enseña el calculado y su confianza y deja fijarlo a mano, aplicar el desplazamiento de horas
// que sugiere la alineación o volver al automático. Rust comprueba y aplica el desfase; aquí solo
// se pide y se enseña.
import { FormEvent, useEffect, useState } from "react";
import { OffsetView, decimal, raceOffset, setRaceOffset, signed } from "./api";
import { Notice, Stat } from "./ui";
import { useViewer } from "./viewer";

const INTRO =
  "Tu reloj y el cronometraje no marcan exactamente la misma hora. Tramos calcula la diferencia (el desfase) para saber dónde estabas al picar cada baliza. Cámbialo solo si en el mapa las balizas no caen donde estaban.";

/** Horas enteras más cercanas y lo que sobra, si está a menos de un minuto de ellas. */
function wholeHours(seconds: number): { hours: number; rest: number } | null {
  const hours = Math.round(seconds / 3600);
  const rest = seconds - hours * 3600;
  return hours !== 0 && Math.abs(rest) < 60 ? { hours, rest } : null;
}

/** «+7,1 s», o cerca de horas enteras: «-1 h +0,2 s». */
function offsetLabel(seconds: number): string {
  const h = wholeHours(seconds);
  if (h === null) return `${signed(seconds)} s`;
  const hours = `${h.hours > 0 ? "+" : "-"}${Math.abs(h.hours)} h`;
  return Math.abs(h.rest) < 0.05 ? hours : `${hours} ${signed(h.rest)} s`;
}

/** Qué quiere decir el signo, en palabras: «tu reloj va 1 h menos 0,2 s atrasado». */
function meaning(seconds: number): string {
  const abs = Math.abs(seconds);
  if (abs < 0.05) return "el reloj y el cronometraje van a la par";
  const h = wholeHours(abs);
  const amount =
    h === null
      ? `${decimal(abs, 1)} s`
      : `${h.hours} h` +
        (Math.abs(h.rest) < 0.05
          ? ""
          : ` ${h.rest > 0 ? "y" : "menos"} ${decimal(Math.abs(h.rest), 1)} s`);
  return `tu reloj va ${amount} ${seconds > 0 ? "adelantado" : "atrasado"}`;
}

/** Desfase de la carrera; no se enseña si no hay track. `onChange` avisa de que ha cambiado. */
export function ClockOffset({ resultId, onChange }: { resultId: number; onChange: () => void }) {
  const { readOnly } = useViewer();
  const [view, setView] = useState<OffsetView | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);

  // El campo empieza con el desfase en uso (el fijado o el calculado).
  const fill = (v: OffsetView | null) => {
    const current = v?.manual_offset_s ?? v?.automatic_offset_s ?? null;
    setText(current === null ? "" : decimal(current, 1));
  };

  useEffect(() => {
    let current = true;
    setView(undefined);
    setError(null);
    raceOffset(resultId)
      .then((v) => {
        if (!current) return;
        setView(v);
        fill(v);
      })
      .catch((err: unknown) => {
        if (current) setError(String(err));
      });
    return () => {
      current = false;
    };
  }, [resultId]);

  const save = (offset: number | null) => {
    setSaving(true);
    setError(null);
    setRaceOffset(resultId, offset)
      .then((v) => {
        setView(v);
        fill(v);
        onChange();
      })
      .catch((err: unknown) => setError(String(err)))
      .finally(() => setSaving(false));
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const value = Number(text.trim().replace(",", ".").replace("−", "-").replace(/^\+/, ""));
    if (text.trim() === "" || !Number.isFinite(value)) {
      setError("Escribe el desfase en segundos, por ejemplo 7 o -3,5.");
      return;
    }
    save(value);
  };

  if (view === null) return null;
  if (view === undefined) {
    return error === null ? null : <Notice kind="error">{error}</Notice>;
  }

  const manual = view.manual_offset_s;
  const automatic = view.automatic_offset_s;
  const inUse = manual ?? automatic;
  const confidence = view.confidence === null ? null : Math.round(view.confidence * 100);
  return (
    <div className="card offset-card">
      <div className="card-title">
        <h3>Reloj y cronometraje</h3>
        <span className={manual === null ? "pill" : "pill pill-accent"}>
          {manual === null ? "Automático" : "Fijado a mano"}
        </span>
      </div>
      <p className="small muted">{INTRO}</p>

      <div className="stats">
        <Stat
          label="Desfase calculado"
          value={automatic === null ? "—" : offsetLabel(automatic)}
          detail={
            automatic === null
              ? "No se ha podido calcular"
              : !view.automatic_estimated
                ? "Pocas balizas útiles: se toma 0"
                : `Confianza ${confidence ?? 0} %`
          }
          hint="El que calcula Tramos comparando las picadas con tus giros y paradas en el track."
        />
        <Stat
          label="En uso"
          value={inUse === null ? "—" : offsetLabel(inUse)}
          detail={inUse === null ? "El track no se puede situar" : meaning(inUse)}
        />
      </div>

      {view.automatic_error !== null && (
        <Notice kind="warning">
          {view.automatic_error}
          {view.suggested_shift_s !== null &&
            " Suele pasar con el cambio de horario o con una zona horaria mal elegida al importar."}
        </Notice>
      )}
      {view.automatic_error === null && view.low_confidence && (
        <Notice kind="warning">
          Confianza baja en el desfase calculado: mira en el mapa si las balizas caen donde estaban
          y, si no, fíjalo a mano.
        </Notice>
      )}
      {manual === null &&
        view.automatic_warnings.map((w) => (
          <p key={w} className="small muted">
            {w}
          </p>
        ))}

      {!readOnly && view.suggested_offset_s !== null && view.suggested_shift_s !== null && (
        <div className="row">
          <button
            type="button"
            className="btn btn-primary"
            disabled={saving || manual === view.suggested_offset_s}
            onClick={() => save(view.suggested_offset_s)}
          >
            Aplicar {offsetLabel(view.suggested_shift_s)}
          </button>
          <span className="small muted">
            {manual === view.suggested_offset_s
              ? "Aplicado."
              : `Queda un desfase de ${offsetLabel(view.suggested_offset_s)}, con el ajuste fino ya calculado.`}
          </span>
        </div>
      )}

      {!readOnly && (
        <>
          <form className="row" onSubmit={submit}>
            <label className="field offset-field">
              <span className="field-label">Desfase a mano (segundos)</span>
              <input
                className="input num"
                inputMode="decimal"
                value={text}
                onChange={(e) => setText(e.target.value)}
                aria-describedby="offset-hint"
              />
            </label>
            <button type="submit" className="btn" disabled={saving}>
              Aplicar
            </button>
            {manual !== null && (
              <button
                type="button"
                className="btn btn-ghost"
                disabled={saving}
                onClick={() => save(null)}
              >
                Volver al automático
              </button>
            )}
          </form>
          <p id="offset-hint" className="field-hint">
            Positivo si tu reloj va adelantado respecto al cronometraje; negativo si va atrasado. Al
            aplicarlo se recolocan las balizas en el mapa y se recalculan las métricas de los
            tramos.
          </p>
        </>
      )}
      {error !== null && <Notice kind="error">{error}</Notice>}
    </div>
  );
}
