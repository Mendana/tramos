import { useEffect, useState } from "react";
import { getSettings, saveSettings } from "./api";
import { Notice, PageHeader } from "./ui";

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
function SettingsView({ onSaved }: { onSaved: () => void }) {
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
          <div className="form-section">
            <div className="form-section-text">
              <h3>Tramo con error</h3>
              <p className="small muted">
                Un tramo es error si pierdes más de los dos umbrales. Cambiarlos recalcula todas tus
                carreras.
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
                Zona horaria de las horas del .spl. Se aplica a las carreras que importes a partir de
                ahora.
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
                <span className="field-hint">Por ejemplo, Europe/Madrid o Atlantic/Canary.</span>
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

          {error !== null && <Notice kind="error">{error}</Notice>}
          <div className="row">
            <button type="submit" className="btn btn-primary">
              Guardar
            </button>
            {saved && <span className="small muted">Guardado.</span>}
          </div>
        </form>
      )}
      {form === null && error !== null && <Notice kind="error">{error}</Notice>}
    </>
  );
}

export default SettingsView;
