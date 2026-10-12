// Rivales habituales (#193, docs/historico.md): quién coincide en el mismo recorrido en tres
// carreras o más y cómo te va frente a cada uno. Los números vienen del núcleo; aquí solo se
// enseñan. Abrir un rival despliega sus carreras juntas.
import { Fragment, useEffect, useState } from "react";
import { HistoryFilter, LegGap, Rival, clock, decimal, getRivals } from "./api";
import { ChevronRight, Notice } from "./ui";
import { useSubject } from "./subject";

/** «−0:20» / «+1:05»: tu tiempo menos el suyo. */
const gap = (s: number | null) =>
  s === null ? "—" : `${s > 0 ? "+" : s < 0 ? "−" : ""}${clock(Math.abs(s))}`;
const legGap = (g: LegGap) => (g.legs === 0 ? "—" : `${gap(g.median_gap_s)} (${g.legs})`);

export function RivalsPanel({ filter, revision }: { filter: HistoryFilter; revision: unknown }) {
  const who = useSubject();
  const [rivals, setRivals] = useState<Rival[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    setError(null);
    getRivals(filter)
      .then((r) => {
        if (current) setRivals(r);
      })
      .catch((err: unknown) => {
        if (current) setError(String(err));
      });
    return () => {
      current = false;
    };
    // `revision` cambia con cada histórico nuevo (otro filtro, otro atleta, una carrera más).
  }, [filter, revision]);

  const title = who.pick("Tus rivales", "Sus rivales");
  return (
    <div className="card card-flush">
      <div className="card-title card-head">
        <h3>{title}</h3>
        <span className="small muted">
          {who.pick(
            "Quien coincide contigo en el mismo recorrido en 3 carreras o más.",
            "Quien coincide con este atleta en el mismo recorrido en 3 carreras o más.",
          )}{" "}
          Diferencias: {who.pick("tu", "su")} tiempo menos el del rival (− ={" "}
          {who.pick("le ganas", "le gana")}).
        </span>
      </div>
      {error !== null && <Notice kind="error">{error}</Notice>}
      {rivals === null ? (
        error === null && <p className="muted card-head">Cargando…</p>
      ) : rivals.length === 0 ? (
        <p className="muted card-head">
          Nadie coincide {who.pick("contigo", "con este atleta")} en el mismo recorrido en 3
          carreras o más con estos filtros.
        </p>
      ) : (
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th>Rival</th>
                <th className="num">Carreras</th>
                <th className="num">{who.pick("Le ganas", "Le gana")}</th>
                <th className="num">Diferencia</th>
                <th className="num">Tramos ganados</th>
                <th className="num">Por tramo, sin fallar</th>
                <th className="num">Por tramo, fallando</th>
                <th aria-label="Abrir" />
              </tr>
            </thead>
            <tbody>
              {rivals.map((r) => {
                const key = `${r.name}-${r.club ?? ""}`;
                const isOpen = open === key;
                const toggle = () => setOpen(isOpen ? null : key);
                return (
                  <Fragment key={key}>
                    <tr
                      className="clickable"
                      tabIndex={0}
                      aria-expanded={isOpen}
                      onClick={toggle}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.preventDefault();
                          toggle();
                        }
                      }}
                    >
                      <td>
                        <div className="strong">{r.name}</div>
                        {r.club !== null && <div className="meta">{r.club}</div>}
                      </td>
                      <td className="num">{r.races}</td>
                      <td className="num">
                        {r.races_compared === 0 ? "—" : `${r.races_ahead} de ${r.races_compared}`}
                      </td>
                      <td className="num">{gap(r.median_gap_s)}</td>
                      <td className="num">
                        {r.legs_compared === 0
                          ? "—"
                          : `${decimal((r.legs_ahead / r.legs_compared) * 100, 0)} %`}
                      </td>
                      <td className="num">{legGap(r.clean_legs)}</td>
                      <td className="num">{legGap(r.error_legs)}</td>
                      <td>
                        <ChevronRight />
                      </td>
                    </tr>
                    {isOpen &&
                      r.meetings.map((m) => (
                        <tr key={`${key}-${m.date}-${m.race_name ?? ""}`}>
                          <td className="indent muted">
                            <span className="num">{m.date}</span> · {m.race_name ?? "Sin nombre"}
                          </td>
                          <td />
                          <td className="num muted">
                            {clock(m.own_total_s)} / {clock(m.rival_total_s)}
                          </td>
                          <td className="num">
                            {m.own_total_s === null || m.rival_total_s === null
                              ? "—"
                              : gap(m.own_total_s - m.rival_total_s)}
                          </td>
                          <td colSpan={4} />
                        </tr>
                      ))}
                  </Fragment>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
