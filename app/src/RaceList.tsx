import { FORMAT_LABELS, RaceRow, clock, statusLabel } from "./api";
import { ChevronRight, EmptyState, FileIcon, PageHeader, WatchIcon } from "./ui";

/** Carreras del usuario, de la más reciente a la más antigua. Una fila abre su carrera. */
function RaceList({
  races,
  onOpen,
  onImport,
}: {
  races: RaceRow[] | null;
  onOpen: (resultId: number) => void;
  onImport: () => void;
}) {
  const count = races?.length ?? 0;
  return (
    <>
      <PageHeader
        title="Tus carreras"
        subtitle={
          races === null
            ? "Cargando…"
            : count === 0
              ? undefined
              : `${count} ${count === 1 ? "carrera importada" : "carreras importadas"}`
        }
        actions={
          count > 0 && (
            <button type="button" className="btn btn-primary" onClick={onImport}>
              Importar carrera
            </button>
          )
        }
      />

      {races !== null && races.length === 0 && (
        <div className="card">
          <EmptyState icon={<FileIcon size={40} />} title="Aún no hay carreras">
            <p>Importa el .spl de WinSplits de una carrera y, si lo tienes, el FIT de tu reloj.</p>
            <button type="button" className="btn btn-primary btn-lg" onClick={onImport}>
              Importar tu primera carrera
            </button>
          </EmptyState>
        </div>
      )}

      {races !== null && races.length > 0 && (
        <div className="card card-flush">
          <div className="table-wrap">
            <table className="table">
              <thead>
                <tr>
                  <th>Fecha</th>
                  <th>Carrera</th>
                  <th>Categoría</th>
                  <th>Resultado</th>
                  <th className="num">Tiempo</th>
                  <th className="num">Perdido</th>
                  <th>
                    <span className="visually-hidden">Abrir</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {races.map((race) => (
                  <tr
                    key={race.result_id}
                    className="clickable"
                    tabIndex={0}
                    onClick={() => onOpen(race.result_id)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") onOpen(race.result_id);
                    }}
                  >
                    <td className="num muted">{race.date}</td>
                    <td>
                      <div className="strong">{race.name ?? "Sin nombre"}</div>
                      <div className="meta">
                        {race.format !== null && (
                          <span className="pill pill-accent">{FORMAT_LABELS[race.format]}</span>
                        )}
                        {race.has_track && (
                          <span className="pill" title="Con el FIT del reloj">
                            <WatchIcon size={14} /> Reloj
                          </span>
                        )}
                      </div>
                    </td>
                    <td>{race.class_name}</td>
                    <td>{statusLabel(race.status, race.place)}</td>
                    <td className="num">{clock(race.total_s)}</td>
                    <td className="num">
                      <span className={race.error_count > 0 ? "loss-bad" : undefined}>
                        {clock(race.lost_time_s)}
                      </span>
                      {race.error_count > 0 && (
                        <div className="small muted">
                          {race.error_count} {race.error_count === 1 ? "error" : "errores"}
                        </div>
                      )}
                    </td>
                    <td className="chevron">
                      <ChevronRight />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </>
  );
}

export default RaceList;
