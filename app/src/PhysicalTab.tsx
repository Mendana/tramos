// Pestaña Físico de la vista de carrera (#213, `docs/rendimiento-fisico.md`): el rendimiento
// físico del track del reloj, sin mezclarlo con la orientación salvo que se pida (capas). Los
// números salen del núcleo (`race_physical`); aquí solo se dibujan.
import { ReactNode, useEffect, useMemo, useState } from "react";
import { LegReport, clock, codeLabel, decimal, signed, zoneLabel } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { GroupedColumnChart } from "./charts/GroupedColumnChart";
import { Legend } from "./charts/common";
import { LineChart } from "./charts/LineChart";
import { StackBand, StackRow, StackedCharts } from "./charts/StackedCharts";
import {
  GradeClass,
  PhysicalReady,
  PhysicalView,
  SeriesSample,
  SeriesState,
  racePhysical,
} from "./physicalApi";
import { Notice, Stat } from "./ui";
import { useViewer } from "./viewer";

/** Nombre (entero y corto, para el eje) y rango de cada clase de pendiente (`GRADE_LIMITS` del
 *  núcleo). */
export const GRADE_LABELS: Record<GradeClass, { name: string; short: string; range: string }> = {
  steep_down: { name: "Bajada fuerte", short: "Baj. fuerte", range: "< −8 %" },
  down: { name: "Bajada", short: "Bajada", range: "−8 a −3 %" },
  flat: { name: "Llano", short: "Llano", range: "−3 a 3 %" },
  up: { name: "Subida", short: "Subida", range: "3 a 8 %" },
  steep_up: { name: "Subida fuerte", short: "Sub. fuerte", range: "≥ 8 %" },
};

const STATE_LABELS: Record<SeriesState, string> = {
  transit: "Traslado",
  attack: "Ataque",
  stopped: "Parado",
  gap: "Sin datos",
};

/** Pasos «redondos» de las marcas del eje X: segundos y metros. */
const TIME_STEPS = [30, 60, 120, 300, 600, 900, 1200, 1800, 3600, 7200];
const DISTANCE_STEPS = [100, 200, 500, 1000, 2000, 5000, 10000];
/** Pasos del eje del ritmo (s/km): 10, 15 y 30 s, 1 y 2 min. */
const PACE_STEPS = [10, 15, 30, 60, 120];

/** Una fila de la tabla del panel de la carrera por cada tanto tiempo. */
const TABLE_EVERY_S = 60;

export const kmh = (mps: number) => mps * 3.6;
export const dash = (v: number | null, f: (v: number) => string) => (v === null ? "—" : f(v));

/** Ritmo en s/km → `m:ss`. */
function paceLabel(secondsPerKm: number): string {
  const total = Math.round(secondsPerKm);
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}

/** Ritmo (`m:ss`) de una velocidad en m/s. */
const paceOf = (mps: number) => paceLabel(1000 / mps);

/** Cambio con signo y un decimal, o la raya. */
const change = (v: number | null, unit: string) => (v === null ? "—" : `${signed(v)} ${unit}`);

/** Aviso en lugar de una gráfica que no se puede dibujar, en el panel y en su tabla. */
function noData(message: string) {
  return { chart: <p className="muted">{message}</p>, table: <p className="muted">{message}</p> };
}

/** Pestaña Físico de un resultado. `revision` cambia con el desfase del reloj. */
export default function PhysicalTab({
  resultId,
  revision,
  legs,
  onSettings,
}: {
  resultId: number;
  revision: number;
  /** Los tramos de la carrera: el estado revisado (`is_error`) decide cuáles son error. */
  legs: LegReport[];
  /** Abre Ajustes (donde se definen las zonas de pulso). */
  onSettings: () => void;
}) {
  const [view, setView] = useState<PhysicalView | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    setView(undefined);
    setError(null);
    racePhysical(resultId)
      .then((v) => {
        if (current) setView(v);
      })
      .catch((err: unknown) => {
        if (current) setError(String(err));
      });
    return () => {
      current = false;
    };
  }, [resultId, revision]);

  if (error !== null) return <Notice kind="error">{error}</Notice>;
  if (view === undefined) return <p className="muted">Cargando…</p>;
  if (view.status === "no_track") {
    return (
      <Notice>
        Esta carrera se importó sin el FIT del reloj, y el rendimiento físico sale de él. Vuelve a
        importarla con el FIT para ver aquí el ritmo, el pulso y la cadencia.
      </Notice>
    );
  }
  if (view.status === "not_aligned") {
    return (
      <Notice kind="warning">
        {view.message} Corrige el desfase en la pestaña Mapa, en «Reloj y cronometraje».
      </Notice>
    );
  }
  return <Ready data={view} legs={legs} onSettings={onSettings} />;
}

type Axis = "time" | "distance";
type Cadence = "cadence" | "stride";

function Ready({
  data,
  legs,
  onSettings,
}: {
  data: PhysicalReady;
  legs: LegReport[];
  onSettings: () => void;
}) {
  const { physical: p } = data;
  const hasHeartRate = p.heart_rate_bpm !== null;
  const hasCadence = p.transit.cadence_spm !== null;
  const hasAltitude = p.ascent_m !== null;

  return (
    <>
      {p.transit.transit_s <= 0 && (
        <Notice kind="warning">
          En esta carrera no hay tiempo de traslado (todo es ataque, parada o huecos del track): las
          cifras físicas no se pueden calcular.
        </Notice>
      )}
      {!hasHeartRate && (
        <Notice>
          El FIT no trae pulso (o lo trae en menos de la mitad de la carrera): no hay cifras de
          pulso, eficiencia, desacople ni zonas.
        </Notice>
      )}
      {!hasCadence && (
        <Notice>
          El FIT no trae cadencia (o la trae en menos de la mitad del traslado): no hay cifras de
          cadencia ni de zancada.
        </Notice>
      )}
      {!hasAltitude && (
        <Notice>
          El FIT no trae altitud: no hay pendiente, desnivel, VAM ni velocidad en llano. Las cifras
          usan la velocidad real.
        </Notice>
      )}
      <Summary data={data} />
      <RaceCharts data={data} legs={legs} />
      <div className="panel-grid">
        <DriftPanels data={data} />
        <GradePanels data={data} />
        <ZonesPanel data={data} onSettings={onSettings} />
      </div>
    </>
  );
}

/** Las cifras de la carrera (`docs/rendimiento-fisico.md`, «Carrera»). */
function Summary({ data }: { data: PhysicalReady }) {
  const { physical: p } = data;
  const flat = p.speed === "flat";
  const t = p.transit;
  const noHeartRate = "Sin pulso";
  return (
    <div className="stats">
      <Stat
        label="Traslado"
        value={clock(t.transit_s)}
        detail={`de ${clock(p.duration_s)} · ataque ${clock(p.attack_s)} · parado ${clock(p.stopped_s)}`}
        hint="Tiempo corriendo, fuera de la zona de ataque de cada baliza y de las paradas. Las demás cifras físicas se calculan solo sobre él."
      />
      <Stat
        label="Velocidad de traslado"
        value={t.speed_mps === null ? "—" : `${paceOf(t.speed_mps)} min/km`}
        detail={
          t.speed_mps === null
            ? undefined
            : flat && t.flat_speed_mps !== null
              ? `En llano ${paceOf(t.flat_speed_mps)} min/km`
              : `${decimal(kmh(t.speed_mps), 1)} km/h · sin altitud, la real`
        }
        hint="Ritmo medio del traslado. «En llano» ajusta cada trozo por su pendiente con el coste de correr de Minetti: lo que habrías corrido sin subidas ni bajadas."
      />
      <Stat
        label="Eficiencia"
        value={dash(t.efficiency_m_per_beat, (v) => `${decimal(v, 2)} m/latido`)}
        detail={
          t.efficiency_m_per_beat === null
            ? noHeartRate
            : flat
              ? "Metros en llano"
              : "Metros reales"
        }
        hint="Metros por latido en el traslado. Sube si corres más rápido con el mismo pulso."
      />
      <Stat
        label="Desacople"
        value={change(p.decoupling_pct, "%")}
        detail={
          p.decoupling_pct === null
            ? p.halves === null
              ? "Hacen falta 2 min de traslado en cada mitad"
              : noHeartRate
            : `Pulso ${change(p.heart_rate_change_bpm, "ppm")} · velocidad ${change(p.speed_change_pct, "%")}`
        }
        hint="Cuánto cae la eficiencia de la primera mitad del traslado a la segunda. Positivo: en la segunda mitad cuesta más pulso ir igual de rápido (deriva cardiaca)."
      />
      <Stat
        label="Pulso medio"
        value={dash(p.heart_rate_bpm, (v) => `${decimal(v, 0)} ppm`)}
        detail={
          p.heart_rate_max_bpm === null
            ? noHeartRate
            : `máximo ${decimal(p.heart_rate_max_bpm, 0)} ppm`
        }
        hint="Media de la salida a la meta, parado incluido. El máximo es el mayor pulso suavizado unos segundos: un pico del sensor de muñeca no cuenta."
      />
      <Stat
        label="VAM"
        value={dash(p.vam_m_per_h, (v) => `${decimal(v, 0)} m/h`)}
        detail={
          p.ascent_m === null
            ? "Sin altitud"
            : p.vam_m_per_h === null
              ? "Menos de 1 min subiendo"
              : `${clock(p.climb_s)} subiendo`
        }
        hint="Metros subidos por hora en los trozos con un 5 % de pendiente o más. Solo con al menos un minuto de subida."
      />
      <Stat
        label="Cadencia"
        value={dash(t.cadence_spm, (v) => `${decimal(v, 0)} pasos/min`)}
        detail={t.cadence_spm === null ? "Sin cadencia" : undefined}
        hint="Pasos por minuto en el traslado."
      />
      <Stat
        label="Zancada"
        value={dash(t.stride_m, (v) => `${decimal(v, 2)} m`)}
        detail={
          t.stride_m === null ? "Sin cadencia" : `${change(p.stride_change_pct, "%")} entre mitades`
        }
        hint="Metros por paso en el traslado. Si se acorta al final, es una señal de cansancio."
      />
      <Stat
        label="Desnivel"
        value={dash(p.ascent_m, (v) => `+${decimal(v, 0)} m`)}
        detail={p.descent_m === null ? "Sin altitud" : `−${decimal(p.descent_m, 0)} m`}
        hint="Subida y bajada acumuladas con la altitud suavizada."
      />
      <Stat
        label="Carga"
        value={dash(p.heart_rate_zones?.load ?? null, (v) => decimal(v, 1))}
        detail={
          p.heart_rate_zones === null
            ? data.zones === null
              ? "Sin zonas de pulso"
              : noHeartRate
            : "Zonas de Ajustes"
        }
        hint="Minutos en cada zona de pulso por el número de la zona (1, 2, 3…), sumados. Solo sirve para comparar carreras con las mismas zonas."
      />
    </div>
  );
}

/** Instantes de `samples` seguidos en el estado `state`, como franjas entre los puntos medios. */
function stateBands(
  samples: SeriesSample[],
  xs: number[],
  state: SeriesState,
  className: string,
): StackBand[] {
  const bands: StackBand[] = [];
  let start: number | null = null;
  const close = (end: number) => {
    if (start === null) return;
    const from = start === 0 ? xs[0] : (xs[start - 1] + xs[start]) / 2;
    const to = end === xs.length - 1 ? xs[end] : (xs[end] + xs[end + 1]) / 2;
    bands.push({ from, to, className });
    start = null;
  };
  samples.forEach((s, i) => {
    if (s.state === state) start ??= i;
    else close(i - 1);
  });
  close(samples.length - 1);
  return bands;
}

/** La carrera de la salida a la meta: altitud, ritmo, pulso y cadencia (o zancada) apilados. */
function RaceCharts({ data, legs }: { data: PhysicalReady; legs: LegReport[] }) {
  const { series, physical: p } = data;
  const [axis, setAxis] = useState<Axis>("time");
  const [cadence, setCadence] = useState<Cadence>("cadence");
  const [showControls, setShowControls] = useState(false);
  const [showErrors, setShowErrors] = useState(false);
  const [showEffort, setShowEffort] = useState(false);

  const samples = series.samples;
  const xs = useMemo(
    () => samples.map((s) => (axis === "time" ? s.t_s : s.distance_m)),
    [samples, axis],
  );
  const errors = useMemo(() => new Set(legs.filter((l) => l.is_error).map((l) => l.index)), [legs]);
  const legOf = (index: number) => series.legs.find((l) => l.index === index);

  const column = (value: (s: SeriesSample) => number | null) => samples.map(value);
  const pace = (mps: number | null, s: SeriesSample) =>
    mps !== null && mps > 0 && (s.state === "transit" || s.state === "attack") ? 1000 / mps : null;

  const rows: StackRow[] = [
    {
      key: "altitude",
      title: "Altitud (m)",
      name: "Altitud",
      area: true,
      series: [
        {
          key: "altitude",
          label: "Altitud",
          color: "var(--chart-line-neutral)",
          values: column((s) => s.altitude_m),
        },
      ],
      formatTick: (v) => decimal(v, 0),
      formatValue: (v) => dash(v, (x) => `${decimal(x, 0)} m`),
      missing: "El FIT no trae altitud",
    },
    {
      key: "pace",
      title: "Ritmo (min/km, más rápido arriba)",
      name: "Ritmo",
      tickSteps: PACE_STEPS,
      inverted: true,
      robust: true,
      series: [
        {
          key: "pace",
          label: "Real",
          color: "var(--chart-series-1)",
          values: column((s) => pace(s.speed_mps, s)),
        },
        ...(p.ascent_m === null
          ? []
          : [
              {
                key: "flat",
                label: "En llano",
                color: "var(--chart-series-2)",
                values: column((s) => pace(s.flat_speed_mps, s)),
              },
            ]),
      ],
      formatTick: paceLabel,
      formatValue: (v) => dash(v, paceLabel),
      missing: "Sin velocidad",
    },
    {
      key: "heart",
      title: "Pulso (ppm)",
      name: "Pulso",
      series: [
        {
          key: "heart",
          label: "Pulso",
          color: "var(--chart-series-3)",
          values: column((s) => s.heart_rate_bpm),
        },
      ],
      formatTick: (v) => decimal(v, 0),
      formatValue: (v) => dash(v, (x) => `${decimal(x, 0)} ppm`),
      missing: "El FIT no trae pulso",
    },
    cadence === "cadence"
      ? {
          key: "cadence",
          title: "Cadencia (pasos/min)",
          name: "Cadencia",
          series: [
            {
              key: "cadence",
              label: "Cadencia",
              color: "var(--chart-series-4)",
              values: column((s) => s.cadence_spm),
            },
          ],
          formatTick: (v) => decimal(v, 0),
          formatValue: (v) => dash(v, (x) => `${decimal(x, 0)} pasos/min`),
          missing: "El FIT no trae cadencia",
        }
      : {
          key: "stride",
          title: "Zancada (m por paso)",
          name: "Zancada",
          series: [
            {
              key: "stride",
              label: "Zancada",
              color: "var(--chart-series-4)",
              values: column((s) => s.stride_m),
            },
          ],
          formatTick: (v) => decimal(v, 2),
          formatValue: (v) => dash(v, (x) => `${decimal(x, 2)} m`),
          missing: "El FIT no trae cadencia",
        },
  ];

  const bands: StackBand[] = [
    ...(showErrors
      ? series.legs
          .filter((l) => errors.has(l.index))
          .map((l) => ({
            from: axis === "time" ? l.from_s : l.from_m,
            to: axis === "time" ? l.to_s : l.to_m,
            className: "stack-band-error",
          }))
      : []),
    ...(showEffort
      ? [
          ...stateBands(samples, xs, "attack", "stack-band-attack"),
          ...stateBands(samples, xs, "stopped", "stack-band-stopped"),
        ]
      : []),
  ];
  const markers = showControls
    ? series.controls.map((c) => ({
        x: axis === "time" ? c.t_s : c.distance_m,
        label: codeLabel(c.code),
      }))
    : [];

  const title = (i: number) => {
    const s = samples[i];
    const leg = s.leg === null ? undefined : legOf(s.leg);
    const where =
      s.leg === null
        ? ""
        : `Tramo ${s.leg}${leg === undefined ? "" : ` · ${codeLabel(leg.from)} → ${codeLabel(leg.to)}`}${errors.has(s.leg) ? " · error" : ""}`;
    const state = s.state === "transit" ? "" : STATE_LABELS[s.state];
    return {
      value: `${clock(s.t_s)} · ${decimal(s.distance_m / 1000, 2)} km`,
      detail: [where, state].filter((x) => x !== "").join(" · "),
    };
  };

  const everyN = Math.max(1, Math.round(TABLE_EVERY_S / series.step_s));
  const tableRows = samples.filter((_, i) => i % everyN === 0);
  const layerLegend = [
    ...(showErrors
      ? [{ label: "Tramo con error", color: "var(--chart-loss)", shape: "band" as const }]
      : []),
    ...(showEffort
      ? [
          { label: "Ataque", color: "var(--chart-gain)", shape: "band" as const },
          { label: "Parado", color: "var(--chart-muted)", shape: "band" as const },
        ]
      : []),
    ...(showControls
      ? [
          {
            label: "Picada (número de baliza)",
            color: "var(--chart-reference)",
            shape: "line" as const,
          },
        ]
      : []),
  ];

  const controls = (
    <div className="phys-controls">
      <div className="phys-controls-group">
        <span className="phys-controls-label">Eje</span>
        <div className="segmented" role="radiogroup" aria-label="Eje horizontal">
          <label>
            <input
              type="radio"
              name="phys-axis"
              checked={axis === "time"}
              onChange={() => setAxis("time")}
            />
            Tiempo
          </label>
          <label>
            <input
              type="radio"
              name="phys-axis"
              checked={axis === "distance"}
              onChange={() => setAxis("distance")}
            />
            Distancia
          </label>
        </div>
      </div>
      <div className="phys-controls-group">
        <span className="phys-controls-label">Ver</span>
        <div className="segmented" role="radiogroup" aria-label="Cadencia o zancada">
          <label>
            <input
              type="radio"
              name="phys-cadence"
              checked={cadence === "cadence"}
              onChange={() => setCadence("cadence")}
            />
            Cadencia
          </label>
          <label>
            <input
              type="radio"
              name="phys-cadence"
              checked={cadence === "stride"}
              onChange={() => setCadence("stride")}
            />
            Zancada
          </label>
        </div>
      </div>
      <div className="phys-controls-group">
        <span className="phys-controls-label">Capas</span>
        <label className="check">
          <input
            type="checkbox"
            checked={showControls}
            onChange={(e) => setShowControls(e.target.checked)}
          />
          Picadas
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={showErrors}
            onChange={(e) => setShowErrors(e.target.checked)}
          />
          Tramos con error
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={showEffort}
            onChange={(e) => setShowEffort(e.target.checked)}
          />
          Ataque y paradas
        </label>
      </div>
    </div>
  );

  return (
    <>
      {controls}
      <ChartPanel
        id="race-physical-series"
        title="La carrera, de la salida a la meta"
        help={{ page: "carrera-fisico", section: "La carrera, de la salida a la meta" }}
        description="Una gráfica por magnitud, con el mismo eje horizontal: al pasar el ratón, la línea vertical y el valor salen en todas a la vez. El ritmo en llano ajusta cada trozo por su pendiente. Donde no hay línea, el track no tiene datos o estabas parado."
        cases={`${clock(series.duration_s)} · ${decimal(p.distance_m / 1000, 1)} km`}
        defaultOpen
        chart={
          <>
            {layerLegend.length > 0 && <Legend items={layerLegend} />}
            <StackedCharts
              xs={xs}
              formatX={axis === "time" ? (v) => clock(v) : (v) => `${decimal(v / 1000, 1)} km`}
              xSteps={axis === "time" ? TIME_STEPS : DISTANCE_STEPS}
              rows={rows}
              bands={bands}
              markers={markers}
              tooltipTitle={title}
              label="Altitud, ritmo, pulso y cadencia de la carrera, de la salida a la meta"
            />
          </>
        }
        table={
          <table className="table">
            <thead>
              <tr>
                <th className="num">Tiempo</th>
                <th className="num">Distancia</th>
                <th className="num">Altitud</th>
                <th className="num">Ritmo</th>
                <th className="num">Ritmo en llano</th>
                <th className="num">Pulso</th>
                <th className="num">Cadencia</th>
                <th className="num">Zancada</th>
                <th>Estado</th>
              </tr>
            </thead>
            <tbody>
              {tableRows.map((s) => (
                <tr key={s.t_s}>
                  <td className="num">{clock(s.t_s)}</td>
                  <td className="num">{decimal(s.distance_m / 1000, 2)} km</td>
                  <td className="num">{dash(s.altitude_m, (v) => `${decimal(v, 0)} m`)}</td>
                  <td className="num">{dash(pace(s.speed_mps, s), paceLabel)}</td>
                  <td className="num">{dash(pace(s.flat_speed_mps, s), paceLabel)}</td>
                  <td className="num">{dash(s.heart_rate_bpm, (v) => decimal(v, 0))}</td>
                  <td className="num">{dash(s.cadence_spm, (v) => decimal(v, 0))}</td>
                  <td className="num">{dash(s.stride_m, (v) => `${decimal(v, 2)} m`)}</td>
                  <td>{STATE_LABELS[s.state]}</td>
                </tr>
              ))}
            </tbody>
          </table>
        }
      />
    </>
  );
}

/** Las décimas del traslado: eficiencia, pulso y velocidad en llano, cada una en su gráfica. */
function DriftPanels({ data }: { data: PhysicalReady }) {
  const { physical: p } = data;
  const flat = p.speed === "flat";
  const blocks = p.blocks;
  const cases = `${blocks.length} ${blocks.length === 1 ? "décima" : "décimas"}`;
  const how =
    "Cada punto es una décima del tiempo de traslado (la décima 1 es el principio de la carrera). ";
  const empty =
    "Hacen falta al menos 2 minutos de traslado en cada mitad de la carrera para medir cómo cambia.";

  const panel = (
    id: string,
    title: string,
    description: string,
    color: string,
    value: (b: (typeof blocks)[number]) => number | null,
    format: (v: number) => string,
    tick: (v: number) => string,
    missing: string,
    note?: string,
  ) => {
    const withData = blocks.some((b) => value(b) !== null);
    const message = blocks.length === 0 ? empty : withData ? null : missing;
    const where = (b: (typeof blocks)[number]) =>
      `Décima ${b.index + 1} de 10 · minutos ${Math.floor(b.from_s / 60)}–${Math.ceil(b.to_s / 60)}`;
    return (
      <ChartPanel
        id={id}
        title={title}
        description={how + description}
        note={note}
        help={{ page: "carrera-fisico", section: "Deriva" }}
        cases={message === null ? cases : "sin datos"}
        chart={
          message === null ? (
            <LineChart
              label={`${title} por décimas del traslado`}
              color={color}
              zeroBased={false}
              formatTick={tick}
              labelSpacing={20}
              points={blocks.map((b) => {
                const v = value(b);
                return {
                  key: b.index,
                  label: String(b.index + 1),
                  value: v,
                  tooltip: { value: v === null ? "Sin dato" : format(v), detail: where(b) },
                };
              })}
            />
          ) : (
            noData(message).chart
          )
        }
        table={
          message === null ? (
            <table className="table">
              <thead>
                <tr>
                  <th className="num">Décima</th>
                  <th>Minutos</th>
                  <th className="num">{title}</th>
                </tr>
              </thead>
              <tbody>
                {blocks.map((b) => {
                  const v = value(b);
                  return (
                    <tr key={b.index}>
                      <td className="num">{b.index + 1}</td>
                      <td className="num">
                        {Math.floor(b.from_s / 60)}–{Math.ceil(b.to_s / 60)}
                      </td>
                      <td className="num">{v === null ? "—" : format(v)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          ) : (
            noData(message).table
          )
        }
      />
    );
  };

  return (
    <>
      {panel(
        "race-physical-efficiency",
        "Eficiencia por décimas",
        `Metros ${flat ? "en llano " : ""}por latido. Si baja hacia el final, cuesta más pulso ir igual de rápido.`,
        "var(--chart-series-1)",
        (b) => b.efficiency_m_per_beat,
        (v) => `${decimal(v, 2)} m/latido`,
        (v) => decimal(v, 2),
        "Sin pulso no hay eficiencia.",
      )}
      {panel(
        "race-physical-heart-rate",
        "Pulso por décimas",
        "Pulso medio del traslado de cada décima. El pulso de muñeca llega con retraso: las diferencias pequeñas no son fiables.",
        "var(--chart-series-3)",
        (b) => b.heart_rate_bpm,
        (v) => `${decimal(v, 0)} ppm`,
        (v) => decimal(v, 0),
        "El FIT no trae pulso.",
      )}
      {panel(
        "race-physical-speed",
        flat ? "Velocidad en llano por décimas" : "Velocidad por décimas",
        flat
          ? "Km/h que habrías corrido sin subidas ni bajadas, para que el terreno no tape el cansancio."
          : "Km/h reales en el traslado de cada décima.",
        "var(--chart-series-2)",
        (b) => (flat ? b.flat_speed_mps : b.speed_mps) ?? null,
        (v) => `${decimal(kmh(v), 1)} km/h`,
        (v) => decimal(kmh(v), 1),
        "Sin velocidad en el traslado.",
        flat
          ? undefined
          : "El FIT no trae altitud: es la velocidad real, sin ajustar por pendiente.",
      )}
    </>
  );
}

/** Cómo corre en cada clase de pendiente: velocidad real y en llano, y pulso. */
function GradePanels({ data }: { data: PhysicalReady }) {
  const { by_grade: classes, speed } = data.physical;
  if (classes.length === 0 || speed !== "flat") {
    const message = "Sin altitud no hay pendiente: el FIT no la trae.";
    return (
      <>
        {["race-physical-grade-speed", "race-physical-grade-heart-rate"].map((id, k) => (
          <ChartPanel
            key={id}
            id={id}
            title={k === 0 ? "Velocidad por pendiente" : "Pulso por pendiente"}
            description="Cómo cambia por clase de pendiente."
            help={{ page: "carrera-fisico", section: "Por pendiente" }}
            cases="sin datos"
            {...noData(message)}
          />
        ))}
      </>
    );
  }
  const labels = classes.map((c) => GRADE_LABELS[c.class].short);
  const sublabels = classes.map((c) => GRADE_LABELS[c.class].range);
  const total = classes.reduce((sum, c) => sum + c.transit_s, 0);
  const used = classes.filter((c) => c.transit_s > 0).length;
  const where = (i: number) => ({
    value: GRADE_LABELS[classes[i].class].name,
    detail: `Pendiente ${GRADE_LABELS[classes[i].class].range} · ${clock(classes[i].transit_s)} de traslado`,
  });
  return (
    <>
      <ChartPanel
        id="race-physical-grade-speed"
        title="Velocidad por pendiente"
        help={{ page: "carrera-fisico", section: "Por pendiente" }}
        description="Km/h reales y equivalentes en llano en cada clase de pendiente. Si subes más despacio de lo que tu ritmo en llano haría esperar, ahí pierdes. En bajada fuerte, el ajuste exagera lo que se ahorra: mira la real."
        cases={`${used} ${used === 1 ? "clase" : "clases"}`}
        chart={
          <>
            <Legend
              items={[
                { label: "Real", color: "var(--chart-series-1)" },
                { label: "En llano", color: "var(--chart-series-2)" },
              ]}
            />
            <GroupedColumnChart
              label="Velocidad real y en llano por clase de pendiente"
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
        table={
          <table className="table">
            <thead>
              <tr>
                <th>Pendiente</th>
                <th className="num">Traslado</th>
                <th className="num">% del traslado</th>
                <th className="num">Real</th>
                <th className="num">En llano</th>
              </tr>
            </thead>
            <tbody>
              {classes.map((c) => (
                <tr key={c.class}>
                  <td>
                    {GRADE_LABELS[c.class].name} ({GRADE_LABELS[c.class].range})
                  </td>
                  <td className="num">{clock(c.transit_s)}</td>
                  <td className="num">
                    {total > 0 ? `${decimal((c.transit_s / total) * 100, 0)} %` : "—"}
                  </td>
                  <td className="num">{dash(c.speed_mps, (v) => `${decimal(kmh(v), 1)} km/h`)}</td>
                  <td className="num">
                    {dash(c.flat_speed_mps, (v) => `${decimal(kmh(v), 1)} km/h`)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        }
      />
      <ChartPanel
        id="race-physical-grade-heart-rate"
        title="Pulso por pendiente"
        help={{ page: "carrera-fisico", section: "Por pendiente" }}
        description="Pulso medio del traslado en cada clase. El pulso de muñeca tarda en reaccionar: en las subidas cortas se queda corto."
        cases={`${used} ${used === 1 ? "clase" : "clases"}`}
        chart={
          <LineChart
            label="Pulso medio por clase de pendiente"
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
        table={
          <table className="table">
            <thead>
              <tr>
                <th>Pendiente</th>
                <th className="num">Traslado</th>
                <th className="num">Pulso</th>
              </tr>
            </thead>
            <tbody>
              {classes.map((c) => (
                <tr key={c.class}>
                  <td>
                    {GRADE_LABELS[c.class].name} ({GRADE_LABELS[c.class].range})
                  </td>
                  <td className="num">{clock(c.transit_s)}</td>
                  <td className="num">{dash(c.heart_rate_bpm, (v) => `${decimal(v, 0)} ppm`)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        }
      />
    </>
  );
}

/** Tiempo en cada zona de pulso de Ajustes, con sus colores. */
function ZonesPanel({ data, onSettings }: { data: PhysicalReady; onSettings: () => void }) {
  const { readOnly } = useViewer();
  const { zones } = data;
  const result = data.physical.heart_rate_zones;
  const id = "race-physical-zones";
  const title = "Tiempo en zonas de pulso";
  if (zones === null || result === null) {
    const message: ReactNode =
      zones === null ? (
        readOnly ? (
          <>En la vista de un atleta no hay zonas de pulso: su paquete no las lleva.</>
        ) : (
          <>
            No tienes zonas de pulso.{" "}
            <button type="button" className="btn btn-ghost" onClick={onSettings}>
              Defínelas en Ajustes
            </button>
          </>
        )
      ) : (
        <>El FIT no trae pulso (o lo trae en menos de la mitad de la carrera).</>
      );
    return (
      <ChartPanel
        id={id}
        title={title}
        description="Minutos en cada zona de pulso, de la salida a la meta, con los colores de Ajustes."
        help={{ page: "carrera-fisico", section: "Tiempo en zonas de pulso" }}
        cases="sin datos"
        chart={<p className="muted">{message}</p>}
        table={<p className="muted">{message}</p>}
      />
    );
  }
  const total = result.seconds.reduce((a, b) => a + b, 0);
  const label = (k: number) => zoneLabel(zones.limits, k, (v) => decimal(v, 0));
  return (
    <ChartPanel
      id={id}
      title={title}
      description="Minutos en cada zona de pulso de Ajustes, de la salida a la meta (sin huecos). La carga suma los minutos de cada zona por su número."
      note={`Carga: ${decimal(result.load, 1)}.`}
      help={{ page: "carrera-fisico", section: "Tiempo en zonas de pulso" }}
      cases={clock(total)}
      chart={
        <ColumnChart
          label="Minutos en cada zona de pulso"
          formatTick={(v) => `${decimal(v, 0)} min`}
          columns={result.seconds.map((s, k) => ({
            key: k,
            label: label(k),
            sublabel: total > 0 ? `${decimal((s / total) * 100, 0)} %` : undefined,
            value: s / 60,
            color: zones.colors[k],
            tooltip: {
              value: clock(s),
              detail: `Zona ${k + 1} · ${label(k)} ppm`,
            },
          }))}
        />
      }
      table={
        <table className="table">
          <thead>
            <tr>
              <th>Zona</th>
              <th className="num">Tiempo</th>
              <th className="num">%</th>
            </tr>
          </thead>
          <tbody>
            {result.seconds.map((s, k) => (
              <tr key={k}>
                <td>
                  {k + 1} · {label(k)} ppm
                </td>
                <td className="num">{clock(s)}</td>
                <td className="num">{total > 0 ? `${decimal((s / total) * 100, 0)} %` : "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      }
    />
  );
}
