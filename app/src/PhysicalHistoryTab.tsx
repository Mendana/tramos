// Pestaña Físico de Estadísticas (#214, `docs/rendimiento-fisico.md`, "Histórico"): cómo evoluciona
// el rendimiento físico de carrera en carrera, qué haces en cada pendiente y si te caes siempre al
// final. Los números salen del núcleo (`physical_history`); aquí solo se dibujan. Cada formato es
// una serie (un sprint y una larga no se comparan) y cada magnitud, su gráfica y su escala: nunca
// dos ejes en la misma.
import { ReactNode, useEffect, useState } from "react";
import { FORMAT_LABELS, HistoryFilter, RaceFormat, clock, decimal, signed } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { DateLineChart, DateSeries } from "./charts/DateLineChart";
import { GroupedColumnChart } from "./charts/GroupedColumnChart";
import { Legend } from "./charts/common";
import { LineChart } from "./charts/LineChart";
import { racesLabel } from "./HistoryPanels";
import { GRADE_LABELS, dash, kmh } from "./PhysicalTab";
import { PhysicalHistoryView, PhysicalRaceRow, RaceFigures, physicalHistory } from "./physicalApi";
import { Notice, TitleWithHelp } from "./ui";
import { useViewer } from "./viewer";

/** Orden y color de cada formato: fijos, para que un formato no cambie de color según los demás. */
const FORMATS: { format: RaceFormat | null; label: string; color: string }[] = [
  { format: "sprint", label: FORMAT_LABELS.sprint, color: "var(--chart-series-1)" },
  { format: "middle", label: FORMAT_LABELS.middle, color: "var(--chart-series-2)" },
  { format: "long", label: FORMAT_LABELS.long, color: "var(--chart-series-3)" },
  { format: null, label: "Sin formato", color: "var(--chart-series-4)" },
];

/** Una gráfica de evolución: una magnitud de `RaceFigures`. */
interface Metric {
  id: string;
  title: string;
  /** La explicación fija, que sale en el «?». */
  description: string;
  help: { page: "estadisticas-fisico"; section: string };
  value: (race: RaceFigures) => number | null;
  format: (value: number) => string;
  tick: (value: number) => string;
  /** Por qué falta en una carrera, para el aviso («2 carreras sin pulso»). */
  lacks: string;
  /** Mensaje cuando ninguna carrera la tiene. */
  empty: string;
  /** En las carreras sin altitud el dato es otro (velocidad real): punto hueco y aviso. */
  withoutAltitudeIsDifferent?: boolean;
}

const METRICS: Metric[] = [
  {
    id: "history-physical-efficiency",
    title: "Eficiencia",
    description:
      "Metros por latido en el traslado de cada carrera (en llano, si hay altitud). Si sube con el tiempo, corres más rápido con el mismo pulso: estás en mejor forma. Una línea por formato, porque un sprint y una larga no se comparan.",
    help: { page: "estadisticas-fisico", section: "Eficiencia" },
    value: (r) => r.efficiency_m_per_beat,
    format: (v) => `${decimal(v, 2)} m/latido`,
    tick: (v) => decimal(v, 2),
    lacks: "sin pulso",
    empty: "Ninguna carrera tiene pulso: sin él no hay eficiencia.",
    withoutAltitudeIsDifferent: true,
  },
  {
    id: "history-physical-decoupling",
    title: "Desacople",
    description:
      "Cuánto cae la eficiencia de la primera mitad del traslado a la segunda, en %. Positivo: en la segunda mitad cuesta más pulso ir igual de rápido (te cansas). Cerca de 0 es un día fresco. Una línea por formato.",
    help: { page: "estadisticas-fisico", section: "Desacople" },
    value: (r) => r.decoupling_pct,
    format: (v) => `${signed(v)} %`,
    tick: (v) => `${decimal(v, 0)} %`,
    lacks: "sin pulso o con menos de 2 min de traslado en cada mitad",
    empty: "Ninguna carrera tiene pulso y traslado para comparar sus dos mitades.",
  },
  {
    id: "history-physical-speed",
    title: "Velocidad de traslado",
    description:
      "Km/h en el traslado de cada carrera: sin contar paradas ni los últimos 50 m de cada baliza. Con altitud es la velocidad en llano, que descuenta las subidas y bajadas para comparar un día de monte con uno de parque. Una línea por formato.",
    help: { page: "estadisticas-fisico", section: "Velocidad de traslado" },
    value: (r) => (r.transit_speed_mps === null ? null : kmh(r.transit_speed_mps)),
    format: (v) => `${decimal(v, 1)} km/h`,
    tick: (v) => decimal(v, 1),
    lacks: "sin traslado",
    empty: "Ninguna carrera tiene tiempo de traslado.",
    withoutAltitudeIsDifferent: true,
  },
  {
    id: "history-physical-heart-rate",
    title: "Pulso medio",
    description:
      "Pulso medio de la salida a la meta de cada carrera, parado incluido. Depende de lo fuerte que corras: míralo junto a la velocidad. El pulso de muñeca llega con retraso y da saltos. Una línea por formato.",
    help: { page: "estadisticas-fisico", section: "Pulso medio" },
    value: (r) => r.heart_rate_bpm,
    format: (v) => `${decimal(v, 0)} ppm`,
    tick: (v) => decimal(v, 0),
    lacks: "sin pulso",
    empty: "Ninguna carrera tiene pulso.",
  },
  {
    id: "history-physical-vam",
    title: "VAM",
    description:
      "Metros que subes por hora en los trozos con un 5 % de pendiente o más. Solo sale si subiste al menos un minuto, así que una carrera plana no tiene punto. Una línea por formato.",
    help: { page: "estadisticas-fisico", section: "VAM" },
    value: (r) => r.vam_m_per_h,
    format: (v) => `${decimal(v, 0)} m/h`,
    tick: (v) => decimal(v, 0),
    lacks: "sin altitud o con menos de 1 min de subida",
    empty: "Ninguna carrera tiene altitud y al menos un minuto de subida.",
  },
  {
    id: "history-physical-cadence",
    title: "Cadencia",
    description:
      "Pasos por minuto en el traslado de cada carrera. Cambia con el terreno y el ritmo: compárala dentro de un mismo formato. Una línea por formato.",
    help: { page: "estadisticas-fisico", section: "Cadencia" },
    value: (r) => r.cadence_spm,
    format: (v) => `${decimal(v, 0)} pasos/min`,
    tick: (v) => decimal(v, 0),
    lacks: "sin cadencia",
    empty: "Ninguna carrera tiene cadencia.",
  },
  {
    id: "history-physical-stride",
    title: "Zancada",
    description:
      "Metros por paso en el traslado de cada carrera. Si se acorta con el tiempo a igual velocidad, algo cambia en tu técnica o tu fuerza. Una línea por formato.",
    help: { page: "estadisticas-fisico", section: "Zancada" },
    value: (r) => r.stride_m,
    format: (v) => `${decimal(v, 2)} m`,
    tick: (v) => decimal(v, 2),
    lacks: "sin cadencia",
    empty: "Ninguna carrera tiene cadencia.",
  },
  {
    id: "history-physical-load",
    title: "Carga",
    description:
      "Minutos en cada zona de pulso por el número de la zona (1, 2, 3…), sumados, con las zonas de Ajustes. Crece con la duración y la intensidad: compárala dentro de un formato y con las mismas zonas. Una línea por formato.",
    help: { page: "estadisticas-fisico", section: "Carga" },
    value: (r) => r.load,
    format: (v) => decimal(v, 1),
    tick: (v) => decimal(v, 0),
    lacks: "sin pulso",
    empty: "Ninguna carrera tiene zonas y pulso.",
  },
];

const shortDate = (date: string) => `${date.slice(8, 10)}/${date.slice(5, 7)}/${date.slice(2, 4)}`;
const raceName = (race: PhysicalRaceRow) => race.name ?? "Sin nombre";

/** Pestaña Físico del histórico, con los filtros de Estadísticas. */
export function PhysicalHistoryTab({
  filter,
  onOpen,
}: {
  filter: HistoryFilter;
  /** Abre una carrera de la tabla. */
  onOpen: (resultId: number) => void;
}) {
  const [view, setView] = useState<PhysicalHistoryView | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const { readOnly } = useViewer();

  useEffect(() => {
    let current = true;
    setError(null);
    physicalHistory(filter)
      .then((v) => {
        if (current) setView(v);
      })
      .catch((err: unknown) => {
        if (current) setError(String(err));
      });
    // Una respuesta que llega tarde (otro filtro ya pedido) se descarta.
    return () => {
      current = false;
    };
  }, [filter]);

  if (error !== null) return <Notice kind="error">{error}</Notice>;
  if (view === undefined) return <p className="muted">Cargando…</p>;
  if (view.races.length === 0) {
    return (
      <Notice>
        {readOnly
          ? "Ninguna de las carreras compartidas con estos filtros trae el FIT del reloj, y el rendimiento físico sale de él."
          : "Ninguna de las carreras con estos filtros tiene el FIT del reloj, y el rendimiento físico sale de él. Importa tus carreras con el FIT para ver aquí cómo evolucionas."}
      </Notice>
    );
  }
  return <Ready view={view} readOnly={readOnly} onOpen={onOpen} />;
}

function Ready({
  view,
  readOnly,
  onOpen,
}: {
  view: PhysicalHistoryView;
  readOnly: boolean;
  onOpen: (resultId: number) => void;
}) {
  const formats = new Set(view.races.map((r) => r.format));
  return (
    <>
      {view.races_without_track > 0 && (
        <Notice>
          {view.races_without_track === 1
            ? "Una carrera con estos filtros no sale: no tiene FIT, o no se pudo situar en su track."
            : `${view.races_without_track} carreras con estos filtros no salen: no tienen FIT, o no se pudieron situar en su track.`}
        </Notice>
      )}
      {view.races.length < 3 && (
        <Notice>
          Hay {racesLabel(view.races.length)} con FIT: pocas para hablar de evolución. Las gráficas
          mejoran con más.
        </Notice>
      )}
      <div className="panel-grid">
        {METRICS.map((metric) => (
          <EvolutionPanel
            key={metric.id}
            metric={metric}
            races={view.races}
            noZones={metric.id === "history-physical-load" && !view.has_zones}
            readOnly={readOnly}
          />
        ))}
        <GradePanels view={view} formats={formats.size} />
        <DriftPanel view={view} formats={formats.size} />
      </div>
      <RacesTable races={view.races} onOpen={onOpen} />
    </>
  );
}

/** Aviso en lugar de una gráfica que no se puede dibujar, en el panel y en su tabla. */
function noData(message: ReactNode) {
  return { chart: <p className="muted">{message}</p>, table: <p className="muted">{message}</p> };
}

/** Una magnitud en el tiempo, una serie por formato. */
function EvolutionPanel({
  metric,
  races,
  noZones,
  readOnly,
}: {
  metric: Metric;
  races: PhysicalRaceRow[];
  /** Es la carga y no hay zonas de pulso en Ajustes. */
  noZones: boolean;
  readOnly: boolean;
}) {
  const withData = races.filter((r) => metric.value(r) !== null);
  const hollow = metric.withoutAltitudeIsDifferent
    ? withData.filter((r) => r.speed === "moving").length
    : 0;
  const notes = [
    races.length > withData.length
      ? `${racesLabel(races.length - withData.length)} ${metric.lacks}`
      : null,
    hollow > 0
      ? `${hollow === 1 ? "Un punto hueco" : `${hollow} puntos huecos`}: sin altitud, con la velocidad real, que no se compara bien con las demás`
      : null,
  ].filter((n) => n !== null);
  const note = notes.length === 0 ? undefined : `${notes.join(" · ")}.`;

  const series: DateSeries[] = FORMATS.flatMap(({ format, label, color }) => {
    // Una carrera sin el dato no se dibuja y la línea une las que sí lo tienen.
    const mine = races.filter((r) => r.format === format && metric.value(r) !== null);
    if (mine.length === 0) return [];
    return [
      {
        key: format ?? "none",
        label,
        color,
        points: mine.map((r) => {
          const v = metric.value(r);
          const isHollow = metric.withoutAltitudeIsDifferent === true && r.speed === "moving";
          return {
            date: r.date,
            value: v,
            hollow: isHollow,
            tooltip: {
              value: v === null ? "Sin dato" : metric.format(v),
              detail: `${r.date} · ${label} · ${raceName(r)}${isHollow ? " · sin altitud" : ""}`,
            },
          };
        }),
      },
    ];
  });

  const message: ReactNode = noZones
    ? readOnly
      ? "En la vista de un atleta no hay zonas de pulso: su paquete no las lleva."
      : "No tienes zonas de pulso. Defínelas en Ajustes."
    : withData.length === 0
      ? metric.empty
      : null;
  const empty = message === null ? null : noData(message);
  return (
    <ChartPanel
      id={metric.id}
      title={metric.title}
      description={metric.description}
      note={message === null ? note : undefined}
      help={metric.help}
      cases={message === null ? racesLabel(withData.length) : "sin datos"}
      chart={
        empty !== null ? (
          empty.chart
        ) : (
          <>
            {series.length > 1 && (
              <Legend items={series.map((s) => ({ label: s.label, color: s.color }))} />
            )}
            <DateLineChart
              label={`${metric.title} de cada carrera, por fecha y formato`}
              formatTick={metric.tick}
              series={series}
            />
          </>
        )
      }
      table={
        empty !== null ? (
          empty.table
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th className="num">Fecha</th>
                <th>Carrera</th>
                <th>Formato</th>
                <th className="num">{metric.title}</th>
              </tr>
            </thead>
            <tbody>
              {withData.map((r) => (
                <tr key={r.result_id}>
                  <td className="num muted">{r.date}</td>
                  <td>{raceName(r)}</td>
                  <td>{r.format === null ? "Sin formato" : FORMAT_LABELS[r.format]}</td>
                  <td className="num">{dash(metric.value(r), metric.format)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )
      }
    />
  );
}

/** Aviso de que lo agregado junta formatos, con el filtro de formato como salida. */
const mixedFormats = (formats: number) =>
  formats > 1 ? "Junta carreras de varios formatos: filtra por formato para separarlas." : null;

/** Cómo corre en cada clase de pendiente con los trozos de todas las carreras: velocidad y pulso. */
function GradePanels({ view, formats }: { view: PhysicalHistoryView; formats: number }) {
  const { by_grade: classes, graded_races: graded, races: total } = view.history;
  const ids = ["history-physical-grade-speed", "history-physical-grade-heart-rate"];
  const titles = ["Velocidad por pendiente", "Pulso por pendiente"];
  const helps = [
    { page: "estadisticas-fisico", section: "Velocidad por pendiente" },
    { page: "estadisticas-fisico", section: "Pulso por pendiente" },
  ] as const;
  const descriptions = [
    "Km/h reales y equivalentes en llano en cada clase de pendiente, sumando los trozos de todas las carreras (cada una pesa por su tiempo en la clase). Si subes más despacio de lo que tu ritmo en llano haría esperar, ahí pierdes. En bajada fuerte el ajuste exagera lo que se ahorra: mira la real.",
    "Pulso medio del traslado en cada clase, sumando los trozos de todas las carreras (cada una pesa por su tiempo con pulso). El pulso de muñeca tarda en reaccionar: en las subidas cortas se queda corto.",
  ];
  if (graded === 0) {
    const message = "Ninguna carrera tiene altitud: sin ella no hay pendiente.";
    return (
      <>
        {ids.map((id, k) => (
          <ChartPanel
            key={id}
            id={id}
            title={titles[k]}
            description={descriptions[k]}
            help={helps[k]}
            cases="sin datos"
            {...noData(message)}
          />
        ))}
      </>
    );
  }
  const note =
    [
      total > graded
        ? `${racesLabel(total - graded)} sin altitud no cuenta${total - graded === 1 ? "" : "n"} aquí`
        : null,
      mixedFormats(formats),
    ]
      .filter((n) => n !== null)
      .join(" · ") || undefined;
  const labels = classes.map((c) => GRADE_LABELS[c.class].short);
  const sublabels = classes.map((c) => GRADE_LABELS[c.class].range);
  const used = classes.filter((c) => c.transit_s > 0).length;
  const cases = `${graded} ${graded === 1 ? "carrera" : "carreras"} · ${used} ${used === 1 ? "clase" : "clases"}`;
  const where = (i: number) => ({
    value: GRADE_LABELS[classes[i].class].name,
    detail: `Pendiente ${GRADE_LABELS[classes[i].class].range} · ${clock(classes[i].transit_s)} de traslado en ${racesLabel(classes[i].races)}`,
  });
  const rowsTable = (columns: ReactNode, cells: (c: (typeof classes)[number]) => ReactNode) => (
    <table className="table">
      <thead>
        <tr>
          <th>Pendiente</th>
          <th className="num">Carreras</th>
          <th className="num">Traslado</th>
          {columns}
        </tr>
      </thead>
      <tbody>
        {classes.map((c) => (
          <tr key={c.class}>
            <td>
              {GRADE_LABELS[c.class].name} ({GRADE_LABELS[c.class].range})
            </td>
            <td className="num">{c.races}</td>
            <td className="num">{clock(c.transit_s)}</td>
            {cells(c)}
          </tr>
        ))}
      </tbody>
    </table>
  );
  return (
    <>
      <ChartPanel
        id={ids[0]}
        title={titles[0]}
        description={descriptions[0]}
        note={note}
        help={helps[0]}
        cases={cases}
        chart={
          <>
            <Legend
              items={[
                { label: "Real", color: "var(--chart-series-1)" },
                { label: "En llano", color: "var(--chart-series-2)" },
              ]}
            />
            <GroupedColumnChart
              label="Velocidad real y en llano por clase de pendiente, con todas las carreras"
              xLabels={labels}
              xSublabels={sublabels}
              tooltipTitle={where}
              formatValue={(v) => dash(v, (x) => `${decimal(x, 1)} km/h`)}
              formatTick={(v) => decimal(v, 0)}
              series={[
                {
                  key: "real",
                  label: "Real",
                  color: "var(--chart-series-1)",
                  values: classes.map((c) => (c.speed_mps === null ? null : kmh(c.speed_mps))),
                },
                {
                  key: "flat",
                  label: "En llano",
                  color: "var(--chart-series-2)",
                  values: classes.map((c) =>
                    c.flat_speed_mps === null ? null : kmh(c.flat_speed_mps),
                  ),
                },
              ]}
            />
          </>
        }
        table={rowsTable(
          <>
            <th className="num">Real</th>
            <th className="num">En llano</th>
          </>,
          (c) => (
            <>
              <td className="num">{dash(c.speed_mps, (v) => `${decimal(kmh(v), 1)} km/h`)}</td>
              <td className="num">{dash(c.flat_speed_mps, (v) => `${decimal(kmh(v), 1)} km/h`)}</td>
            </>
          ),
        )}
      />
      <ChartPanel
        id={ids[1]}
        title={titles[1]}
        description={descriptions[1]}
        note={note}
        help={helps[1]}
        cases={cases}
        chart={
          <LineChart
            label="Pulso medio por clase de pendiente, con todas las carreras"
            color="var(--chart-series-3)"
            zeroBased={false}
            formatTick={(v) => decimal(v, 0)}
            labelSpacing={60}
            points={classes.map((c, i) => ({
              key: c.class,
              label: labels[i],
              value: c.heart_rate_bpm,
              marker: c.heart_rate_bpm !== null,
              tooltip: {
                ...where(i),
                value:
                  c.heart_rate_bpm === null ? "Sin dato" : `${decimal(c.heart_rate_bpm, 0)} ppm`,
              },
            }))}
          />
        }
        table={rowsTable(<th className="num">Pulso</th>, (c) => (
          <td className="num">{dash(c.heart_rate_bpm, (v) => `${decimal(v, 0)} ppm`)}</td>
        ))}
      />
    </>
  );
}

/** Eficiencia de cada décima frente a la media de su carrera, promediada entre carreras. */
function DriftPanel({ view, formats }: { view: PhysicalHistoryView; formats: number }) {
  const drift = view.history.drift;
  const measured = Math.max(...drift.map((d) => d.races));
  const title = "Deriva media por décimas";
  const description =
    "La eficiencia de cada décima del traslado frente a la de su carrera entera (0 % = igual que su media), promediada entre todas las carreras. Si las últimas décimas salen siempre por debajo, te caes al final. Mira cuántas carreras hay en cada décima.";
  const where = (d: (typeof drift)[number]) =>
    `Décima ${d.index + 1} de 10 · ${racesLabel(d.races)}`;
  const relative = (v: number) => (v - 1) * 100;
  const none =
    measured <= 0 ? noData("Ninguna carrera tiene pulso y traslado para medir la deriva.") : null;
  return (
    <ChartPanel
      id="history-physical-drift"
      title={title}
      description={description}
      note={mixedFormats(formats) ?? undefined}
      help={{ page: "estadisticas-fisico", section: "Deriva media por décimas" }}
      cases={none === null ? racesLabel(measured) : "sin datos"}
      chart={
        none !== null ? (
          none.chart
        ) : (
          <ColumnChart
            label="Eficiencia de cada décima del traslado frente a la media de su carrera, con todas las carreras"
            formatTick={(v) => `${decimal(v, 0)} %`}
            columns={drift.map((d) => ({
              key: d.index,
              label: String(d.index + 1),
              sublabel: d.races > 0 ? `n=${d.races}` : undefined,
              value: d.relative_efficiency === null ? null : relative(d.relative_efficiency),
              color: "var(--chart-series-1)",
              tooltip: {
                value:
                  d.relative_efficiency === null
                    ? "Sin dato"
                    : `${signed(relative(d.relative_efficiency))} %`,
                detail: where(d),
              },
            }))}
          />
        )
      }
      table={
        none !== null ? (
          none.table
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th className="num">Décima</th>
                <th className="num">Carreras</th>
                <th className="num">Frente a su media</th>
              </tr>
            </thead>
            <tbody>
              {drift.map((d) => (
                <tr key={d.index}>
                  <td className="num">{d.index + 1}</td>
                  <td className="num">{d.races}</td>
                  <td className="num">
                    {dash(d.relative_efficiency, (v) => `${signed(relative(v))} %`)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )
      }
    />
  );
}

/** Las carreras con FIT y sus cifras, de la más reciente a la más antigua. */
function RacesTable({
  races,
  onOpen,
}: {
  races: PhysicalRaceRow[];
  onOpen: (resultId: number) => void;
}) {
  const newestFirst = [...races].reverse();
  return (
    <div className="card card-flush">
      <div className="card-title card-head">
        <TitleWithHelp
          title="Carreras con FIT"
          text="Las cifras físicas de cada carrera con FIT que entra con los filtros. Abre una para ver su pestaña Físico."
          help={{ page: "estadisticas-fisico", section: "Carreras con FIT" }}
        />
        <span className="small muted">{shortDateRange(races)}</span>
      </div>
      <div className="table-wrap">
        <table className="table">
          <thead>
            <tr>
              <th className="num">Fecha</th>
              <th>Carrera</th>
              <th className="num">Traslado</th>
              <th className="num">Eficiencia</th>
              <th className="num">Desacople</th>
              <th className="num">Velocidad</th>
              <th className="num">Pulso</th>
              <th className="num">VAM</th>
              <th className="num">Cadencia</th>
              <th className="num">Zancada</th>
              <th className="num">Carga</th>
            </tr>
          </thead>
          <tbody>
            {newestFirst.map((r) => (
              <tr
                key={r.result_id}
                className="clickable"
                tabIndex={0}
                onClick={() => onOpen(r.result_id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") onOpen(r.result_id);
                }}
              >
                <td className="num muted">{r.date}</td>
                <td>
                  <div className="strong">{raceName(r)}</div>
                  <div className="meta">
                    <span className={r.format === null ? "pill" : "pill pill-accent"}>
                      {r.format === null ? "Sin formato" : FORMAT_LABELS[r.format]}
                    </span>
                    {r.speed === "moving" && <span className="pill">Sin altitud</span>}
                  </div>
                </td>
                <td className="num">{r.transit_s > 0 ? clock(r.transit_s) : "—"}</td>
                <td className="num">
                  {dash(r.efficiency_m_per_beat, (v) => `${decimal(v, 2)} m/latido`)}
                </td>
                <td className="num">{dash(r.decoupling_pct, (v) => `${signed(v)} %`)}</td>
                <td className="num">
                  {dash(r.transit_speed_mps, (v) => `${decimal(kmh(v), 1)} km/h`)}
                </td>
                <td className="num">{dash(r.heart_rate_bpm, (v) => `${decimal(v, 0)} ppm`)}</td>
                <td className="num">{dash(r.vam_m_per_h, (v) => `${decimal(v, 0)} m/h`)}</td>
                <td className="num">{dash(r.cadence_spm, (v) => decimal(v, 0))}</td>
                <td className="num">{dash(r.stride_m, (v) => `${decimal(v, 2)} m`)}</td>
                <td className="num">{dash(r.load, (v) => decimal(v, 1))}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

/** «Del 03/10/26 al 21/11/26», de la primera a la última carrera. */
function shortDateRange(races: PhysicalRaceRow[]): string {
  const first = races[0];
  const last = races[races.length - 1];
  return `${racesLabel(races.length)}, del ${shortDate(first.date)} al ${shortDate(last.date)}. Abre una para verla.`;
}
