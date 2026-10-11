// Carreras de los atletas (#163, docs/app.md, "Atletas"): una fila por carrera recibida, con
// los atletas que la corrieron; las de varios atletas, destacadas. Un clic abre la carrera con
// todos juntos (`AthleteRaceScreen`). Todo viene de Rust (`athlete_races`).
import { useEffect, useState } from "react";
import { AthleteRaceRow, FORMAT_LABELS, RaceRunner, athleteRaces } from "./api";
import { PAGE_SIZE } from "./raceFilter";
import { Pager } from "./RaceList";
import { ChevronRight, EmptyState, FileIcon, Notice, PageHeader } from "./ui";

/** «Ana», o «Ana (tú)» si es quien usa la app. */
export const athleteName = (a: RaceRunner) => {
  const name = a.runner.display_name || "Sin nombre";
  return a.is_self ? `${name} (tú)` : name;
};

function AthleteRacesScreen({ onOpen }: { onOpen: (raceId: string, name: string | null) => void }) {
  const [races, setRaces] = useState<AthleteRaceRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);

  useEffect(() => {
    athleteRaces()
      .then(setRaces)
      .catch((err: unknown) => setError(String(err)));
  }, []);

  const total = races?.length ?? 0;
  const shared = (races ?? []).filter((r) => r.athletes.length > 1).length;
  const pages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const rows = (races ?? []).slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE);

  return (
    <>
      <PageHeader
        title="Carreras"
        subtitle={
          races === null
            ? "Cargando…"
            : `${total} ${total === 1 ? "carrera recibida" : "carreras recibidas"}; ${shared} ${
                shared === 1 ? "la corrieron" : "las corrieron"
              } varios atletas.`
        }
      />
      {error !== null && <Notice kind="error">{error}</Notice>}

      {races !== null && total === 0 && (
        <div className="card">
          <EmptyState icon={<FileIcon size={40} />} title="Aún no hay carreras">
            <p>Aparecerán cuando tus atletas dejen sus carreras en la carpeta compartida.</p>
          </EmptyState>
        </div>
      )}

      {total > 0 && (
        <div className="card card-flush">
          <div className="table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>Fecha</th>
                  <th>Carrera</th>
                  <th>Atletas</th>
                  <th>
                    <span className="visually-hidden">Abrir</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {rows.map((race) => {
                  const many = race.athletes.length > 1;
                  return (
                    <tr
                      key={race.race_id}
                      className={many ? "clickable is-shared" : "clickable"}
                      tabIndex={0}
                      onClick={() => onOpen(race.race_id, race.name)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") onOpen(race.race_id, race.name);
                      }}
                    >
                      <td className="num muted">{race.date}</td>
                      <td>
                        <div className="strong">{race.name ?? "Sin nombre"}</div>
                        {race.format !== null && (
                          <div className="meta">
                            <span className="pill pill-accent">{FORMAT_LABELS[race.format]}</span>
                          </div>
                        )}
                      </td>
                      <td>
                        <div>
                          <span className={many ? "pill pill-accent" : "pill"}>
                            {race.athletes.length}{" "}
                            {race.athletes.length === 1 ? "atleta" : "atletas"}
                          </span>
                        </div>
                        <div className="small muted">
                          {race.athletes.map(athleteName).join(", ")}
                        </div>
                      </td>
                      <td className="chevron">
                        <ChevronRight />
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <Pager page={page} pages={pages} total={total} onPage={setPage} />
        </div>
      )}
    </>
  );
}

export default AthleteRacesScreen;
