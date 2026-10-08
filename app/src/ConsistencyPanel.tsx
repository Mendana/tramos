// Consistencia (P10, docs/tiempo-perdido.md): la de cada carrera del histórico que la tiene, de
// la más antigua a la más reciente, con los mismos filtros. Los valores vienen del núcleo; aquí
// solo se dibujan.
import { FORMAT_LABELS, HistoryRaceRow, HistoryStats, spread } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { LineChart } from "./charts/LineChart";
import { racesLabel, tickPercent } from "./HistoryPanels";

/** `AAAA-MM-DD` → `DD/MM/AA`, para el eje. */
const shortDate = (date: string) => `${date.slice(8, 10)}/${date.slice(5, 7)}/${date.slice(2, 4)}`;

/** Sección «Consistencia» de la vista histórica (P10). */
export function ConsistencyPanel({
  races,
  total,
}: {
  /** Filas del histórico, de la más reciente a la más antigua. */
  races: HistoryRaceRow[];
  total: HistoryStats;
}) {
  const series = races
    .flatMap((race) => {
      const c = race.stats?.mean_consistency ?? null;
      return c === null ? [] : [{ race, consistency: c }];
    })
    .reverse();
  const name = (race: HistoryRaceRow) => race.name ?? "Sin nombre";
  return (
    <>
      <h3 className="section-title">Consistencia</h3>
      <ChartPanel
        id="consistency"
        title="Consistencia por carrera"
        description={`Cuánto varía tu IR de un tramo a otro en cada carrera (desviación típica, ponderada por la referencia), de la más antigua a la más reciente: más bajo es más consistente. Tu media con estos filtros: ${spread(total.mean_consistency)}.`}
        cases={racesLabel(series.length)}
        chart={
          <LineChart
            label="Consistencia de cada carrera a lo largo del tiempo"
            formatTick={tickPercent}
            labelSpacing={64}
            points={series.map(({ race, consistency }) => ({
              key: race.result_id,
              label: shortDate(race.date),
              value: consistency * 100,
              marker: true,
              tooltip: {
                value: spread(consistency),
                detail: `${race.date} · ${name(race)}`,
              },
            }))}
          />
        }
        table={
          <table className="table">
            <thead>
              <tr>
                <th className="num">Fecha</th>
                <th>Carrera</th>
                <th>Formato</th>
                <th className="num">Consistencia</th>
              </tr>
            </thead>
            <tbody>
              {series.map(({ race, consistency }) => (
                <tr key={race.result_id}>
                  <td className="num muted">{race.date}</td>
                  <td>{name(race)}</td>
                  <td>{race.format === null ? "Sin formato" : FORMAT_LABELS[race.format]}</td>
                  <td className="num">{spread(consistency)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        }
      />
    </>
  );
}
