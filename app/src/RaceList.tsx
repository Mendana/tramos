import { FORMAT_LABELS, RaceRow, SummaryRace, clock, statusLabel } from "./api";
import { ChevronRight, EmptyState, FileIcon, PageHeader, WatchIcon } from "./ui";
import { useViewer } from "./viewer";

/** Carreras del usuario, de la más reciente a la más antigua. Una fila abre su carrera. */
function RaceList({
  races,
  onOpen,
  onImport,
  summaryOnly = [],
}: {
  races: RaceRow[] | null;
  onOpen: (resultId: number) => void;
  onImport: () => void;
  /** En modo entrenadora, las carreras compartidas solo con el resumen. */
  summaryOnly?: SummaryRace[];
}) {
  const { readOnly, runnerName } = useViewer();
  const count = races?.length ?? 0;
  if (readOnly && runnerName === null) {
    return (
      <>
        <PageHeader title="Carreras" />
        <div className="card">
          <EmptyState icon={<FileIcon size={40} />} title="Elige un corredor">
            <p>
              Sus carreras llegan por la carpeta compartida. Elige de quién verlas en la barra
              lateral.
            </p>
          </EmptyState>
        </div>
      </>
    );
  }
  return (
    <>
      <PageHeader
        title={runnerName === null ? "Tus carreras" : `Carreras de ${runnerName}`}
        subtitle={
          races === null
            ? "Cargando…"
            : count === 0
              ? undefined
              : `${count} ${count === 1 ? (readOnly ? "carrera compartida" : "carrera importada") : readOnly ? "carreras compartidas" : "carreras importadas"}`
        }
        actions={
          !readOnly &&
          count > 0 && (
            <button type="button" className="btn btn-primary" onClick={onImport}>
              Importar carrera
            </button>
          )
        }
      />

      {readOnly && races !== null && races.length === 0 && summaryOnly.length === 0 && (
        <div className="card">
          <EmptyState icon={<FileIcon size={40} />} title="Aún no ha compartido carreras">
            <p>Aparecerán aquí cuando las deje en la carpeta compartida.</p>
          </EmptyState>
        </div>
      )}

      {!readOnly && races !== null && races.length === 0 && (
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

      {summaryOnly.length > 0 && <SummaryOnlyRaces races={summaryOnly} />}
    </>
  );
}

/** Carreras compartidas solo con el resumen: sin tramos, no se pueden abrir. */
function SummaryOnlyRaces({ races }: { races: SummaryRace[] }) {
  return (
    <section>
      <h3 className="section-title">Solo con el resumen</h3>
      <p className="small muted">
        De estas carreras solo ha compartido el resumen: no se pueden abrir ni entran en el
        histórico.
      </p>
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
              </tr>
            </thead>
            <tbody>
              {races.map((race) => (
                <tr key={`${race.date}-${race.name ?? ""}-${race.summary.class_name}`}>
                  <td className="num muted">{race.date}</td>
                  <td>
                    <div className="strong">{race.name ?? "Sin nombre"}</div>
                    {race.format !== null && (
                      <div className="meta">
                        <span className="pill pill-accent">{FORMAT_LABELS[race.format]}</span>
                      </div>
                    )}
                  </td>
                  <td>{race.summary.class_name}</td>
                  <td>{statusLabel(race.summary.status, race.summary.place)}</td>
                  <td className="num">{clock(race.summary.total_s)}</td>
                  <td className="num">
                    <span className={race.summary.error_count > 0 ? "loss-bad" : undefined}>
                      {clock(race.summary.lost_time_s)}
                    </span>
                    {race.summary.error_count > 0 && (
                      <div className="small muted">
                        {race.summary.error_count}{" "}
                        {race.summary.error_count === 1 ? "error" : "errores"}
                      </div>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}

export default RaceList;
