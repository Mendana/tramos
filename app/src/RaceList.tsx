import { FORMAT_LABELS, RaceRow, clock, statusLabel } from "./api";

/** Carreras del usuario, de la más reciente a la más antigua. Una fila abre su carrera. */
function RaceList({ races, onOpen }: { races: RaceRow[]; onOpen: (resultId: number) => void }) {
  if (races.length === 0) {
    return <p className="muted">Aún no has importado ninguna carrera.</p>;
  }
  return (
    <table className="races">
      <thead>
        <tr>
          <th>Fecha</th>
          <th>Carrera</th>
          <th>Categoría</th>
          <th>Resultado</th>
          <th>Formato</th>
          <th className="num">Tiempo</th>
          <th className="num">Perdido</th>
          <th>Reloj</th>
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
            <td>{race.date}</td>
            <td>{race.name ?? "Sin nombre"}</td>
            <td>{race.class_name}</td>
            <td>{statusLabel(race.status, race.place)}</td>
            <td>{race.format === null ? "—" : FORMAT_LABELS[race.format]}</td>
            <td className="num">{clock(race.total_s)}</td>
            <td className="num">
              {clock(race.lost_time_s)}
              {race.error_count > 0 && (
                <span className="muted">
                  {" "}
                  ({race.error_count} {race.error_count === 1 ? "error" : "errores"})
                </span>
              )}
            </td>
            <td>{race.has_track ? "Sí" : "No"}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export default RaceList;
