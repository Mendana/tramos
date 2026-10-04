import { useEffect, useState } from "react";
import { getSettings, saveSettings } from "./api";

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
}

/** Ajustes: umbrales de error, zona horaria e identidad (ver `docs/app.md`). */
function SettingsView({ onBack, onSaved }: { onBack: () => void; onSaved: () => void }) {
  const [form, setForm] = useState<Form | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    getSettings()
      .then((s) =>
        setForm({
          thresholdS: String(s.error_threshold_s),
          thresholdPct: String(s.error_threshold_pct),
          timeZone: s.time_zone,
          siCard: s.identity.si_card === null ? "" : String(s.identity.si_card),
          fullName: s.identity.full_name ?? "",
        }),
      )
      .catch((err: unknown) => setError(String(err)));
  }, []);

  function update(field: keyof Form, value: string) {
    setForm((current) => (current === null ? current : { ...current, [field]: value }));
    setSaved(false);
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
    try {
      await saveSettings({
        error_threshold_s: thresholdS,
        error_threshold_pct: thresholdPct,
        time_zone: form.timeZone.trim(),
        identity: {
          si_card: card === "" ? null : Number(card),
          full_name: form.fullName.trim() === "" ? null : form.fullName.trim(),
        },
      });
      setSaved(true);
      onSaved();
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <section className="panel">
      <button type="button" onClick={onBack}>
        ← Tus carreras
      </button>
      <h2>Ajustes</h2>
      {form === null ? (
        error === null && <p className="muted">Cargando…</p>
      ) : (
        <form
          className="settings"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <fieldset>
            <legend>Tramo con error</legend>
            <p className="muted">
              Un tramo es error si pierdes más de los dos umbrales. Cambiarlos recalcula todas tus
              carreras.
            </p>
            <label>
              Pérdida mínima (segundos)
              <input
                inputMode="decimal"
                value={form.thresholdS}
                onChange={(e) => update("thresholdS", e.target.value)}
              />
            </label>
            <label>
              Pérdida mínima (% del tiempo esperado)
              <input
                inputMode="decimal"
                value={form.thresholdPct}
                onChange={(e) => update("thresholdPct", e.target.value)}
              />
            </label>
          </fieldset>

          <fieldset>
            <legend>Hora de las carreras</legend>
            <p className="muted">
              Zona horaria de las horas del .spl. Se aplica a las carreras que importes a partir de
              ahora.
            </p>
            <label>
              Zona horaria
              <input
                list="time-zones"
                value={form.timeZone}
                onChange={(e) => update("timeZone", e.target.value)}
              />
              <datalist id="time-zones">
                {COMMON_TIME_ZONES.map((zone) => (
                  <option key={zone} value={zone} />
                ))}
              </datalist>
            </label>
          </fieldset>

          <fieldset>
            <legend>Quién eres</legend>
            <p className="muted">Para encontrarte en cada carrera al importarla.</p>
            <label>
              Tarjeta SI
              <input
                inputMode="numeric"
                value={form.siCard}
                onChange={(e) => update("siCard", e.target.value)}
              />
            </label>
            <label>
              Nombre y apellidos
              <input value={form.fullName} onChange={(e) => update("fullName", e.target.value)} />
            </label>
          </fieldset>

          <button type="submit">Guardar</button>
          {saved && <span className="muted"> Guardado.</span>}
        </form>
      )}
      {error !== null && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
    </section>
  );
}

export default SettingsView;
