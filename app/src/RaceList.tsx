import { FORMAT_LABELS, RaceRow, statusLabel } from "./api";

/** Carreras del usuario, de la más reciente a la más antigua. */
function RaceList({ races }: { races: RaceRow[] }) {
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
          <th>Reloj</th>
        </tr>
      </thead>
      <tbody>
        {races.map((race) => (
          <tr key={race.result_id}>
            <td>{race.date}</td>
            <td>{race.name ?? "Sin nombre"}</td>
            <td>{race.class_name}</td>
            <td>{statusLabel(race.status, race.place)}</td>
            <td>{race.format === null ? "—" : FORMAT_LABELS[race.format]}</td>
            <td>{race.has_track ? "Sí" : "No"}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export default RaceList;
