// ¿El cansancio anticipa el error? (P14, docs/historico.md): deriva del pulso frente a la
// velocidad, pulso antes de un error frente a antes de un tramo limpio y esfuerzo percibido, por
// tercio de carrera. El pulso de muñeca es un dato débil y la sección lo dice. Los números vienen
// del núcleo; aquí solo se dibujan.
import { Effort, Fatigue, HeartRateBefore, ThirdFatigue, clock, decimal, signed } from "./api";
import { ChartPanel } from "./charts/ChartPanel";
import { ColumnChart } from "./charts/ColumnChart";
import { Legend } from "./charts/common";
import { GroupedColumnChart } from "./charts/GroupedColumnChart";
import { legsLabel, percent, racesLabel, tickPercent } from "./HistoryPanels";
import { Notice } from "./ui";

const THIRD_LABELS = ["1.er tercio", "2.º tercio", "3.er tercio"];
const thirdLabel = (t: ThirdFatigue) => THIRD_LABELS[t.third - 1] ?? `Tercio ${t.third}`;

const bpm = (v: number | null) => (v === null ? "—" : `${decimal(v, 0)} ppm`);
const relativeBpm = (v: number | null) => (v === null ? "—" : `${signed(v)} ppm`);
const pace = (speed: number | null) =>
  speed === null || speed <= 0 ? "—" : `${clock(1000 / speed)} min/km`;
const ratio = (v: number | null) => (v === null ? null : v * 100);
const showRatio = (v: number | null) => (v === null ? "—" : percent(v));
const effortValue = (v: number | null) => (v === null ? "—" : decimal(v, 1));
/** Marca del eje con decimales solo si los tiene. */
const tick = (v: number) => String(Number(v.toFixed(1))).replace(".", ",");
const tickBpm = (v: number) => `${v > 0 ? "+" : ""}${tick(v)} ppm`;

const BEFORE = [
  { key: "before_error", label: "Antes de un error", color: "var(--chart-series-1)" },
  { key: "before_clean", label: "Antes de un tramo limpio", color: "var(--chart-series-2)" },
] as const;

const EFFORT = [
  { key: "effort_error", label: "Errores", color: "var(--chart-series-1)" },
  { key: "effort_clean", label: "Tramos limpios", color: "var(--chart-series-2)" },
  { key: "effort_physical", label: "Físico", color: "var(--chart-series-3)" },
] as const;

const before = (t: ThirdFatigue, key: (typeof BEFORE)[number]["key"]): HeartRateBefore => t[key];
const effort = (t: ThirdFatigue, key: (typeof EFFORT)[number]["key"]): Effort => t[key];

function DriftTable({ thirds }: { thirds: ThirdFatigue[] }) {
  return (
    <table className="table">
      <thead>
        <tr>
          <th>Fase</th>
          <th className="num">Tramos limpios (n)</th>
          <th className="num">Pulso medio</th>
          <th className="num">Ritmo en movimiento</th>
          <th className="num">Pulso / velocidad</th>
        </tr>
      </thead>
      <tbody>
        {thirds.map((t) => (
          <tr key={t.third}>
            <td>{thirdLabel(t)}</td>
            <td className="num muted">{t.drift.legs}</td>
            <td className="num">{bpm(t.drift.mean_heart_rate_bpm)}</td>
            <td className="num">{pace(t.drift.mean_speed_mps)}</td>
            <td className="num">{showRatio(ratio(t.drift.mean_relative_ratio))}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function BeforeTable({ thirds }: { thirds: ThirdFatigue[] }) {
  return (
    <table className="table">
      <thead>
        <tr>
          <th>Fase</th>
          <th>Tramo siguiente</th>
          <th className="num">Tramos (n)</th>
          <th className="num">Pulso del anterior</th>
          <th className="num">Sobre la mediana de la carrera</th>
        </tr>
      </thead>
      <tbody>
        {thirds.flatMap((t) =>
          BEFORE.map((b) => {
            const s = before(t, b.key);
            return (
              <tr key={`${t.third}-${b.key}`}>
                <td>{thirdLabel(t)}</td>
                <td>{b.label}</td>
                <td className="num muted">{s.legs}</td>
                <td className="num">{bpm(s.mean_heart_rate_bpm)}</td>
                <td className="num">{relativeBpm(s.mean_relative_bpm)}</td>
              </tr>
            );
          }),
        )}
      </tbody>
    </table>
  );
}

function EffortTable({ thirds }: { thirds: ThirdFatigue[] }) {
  return (
    <table className="table">
      <thead>
        <tr>
          <th>Fase</th>
          <th>Tramo</th>
          <th className="num">Tramos con esfuerzo (n)</th>
          <th className="num">Esfuerzo medio (1–10)</th>
        </tr>
      </thead>
      <tbody>
        {thirds.flatMap((t) =>
          EFFORT.map((e) => {
            const s = effort(t, e.key);
            return (
              <tr key={`${t.third}-${e.key}`}>
                <td>{thirdLabel(t)}</td>
                <td>{e.label}</td>
                <td className="num muted">{s.legs}</td>
                <td className="num">{effortValue(s.mean_effort)}</td>
              </tr>
            );
          }),
        )}
      </tbody>
    </table>
  );
}

/** Sección «Cansancio» de la vista histórica (P14). */
export function FatiguePanel({ data }: { data: Fatigue }) {
  const thirds = data.by_third;
  const races = data.races_with_heart_rate + data.races_without_heart_rate;
  const driftLegs = thirds.reduce((sum, t) => sum + t.drift.legs, 0);
  const beforeLegs = thirds.reduce(
    (sum, t) => sum + t.before_error.legs + t.before_clean.legs,
    0,
  );
  const effortLegs = thirds.reduce(
    (sum, t) => sum + EFFORT.reduce((s, e) => s + effort(t, e.key).legs, 0),
    0,
  );
  const withoutHeartRate =
    data.errors_without_heart_rate > 0
      ? ` ${data.errors_without_heart_rate === 1 ? "Un error no tiene" : `${data.errors_without_heart_rate} errores no tienen`} pulso del tramo anterior (sin FIT o sin pulso) y no se compara.`
      : "";
  return (
    <>
      <h3 className="section-title">Cansancio</h3>
      <Notice kind="warning">
        <strong>Dato débil.</strong> El pulso de muñeca llega con retraso, da saltos y en un tramo
        corto apenas le da tiempo a reaccionar. Tómalo como una pista, no como una medida, y mira
        el número de tramos (n) de cada columna.
      </Notice>
      <p className="small muted">
        El pulso sale de {data.races_with_heart_rate} de {racesLabel(races)} (las que tienen FIT
        con pulso). Fases por número de tramos: cada tercio del recorrido.
      </p>
      <ChartPanel
        title="Deriva del pulso"
        description="Pulso dividido entre la velocidad en movimiento de tus tramos limpios, en cada tercio de la carrera, frente a lo habitual en esa carrera (100 %: la mediana de sus tramos limpios). Si sube al final, necesitas más pulso para ir igual de rápido: cansancio. Debajo de cada tercio, sus tramos (n)."
        cases={legsLabel(driftLegs)}
        chart={
          <ColumnChart
            label="Pulso entre velocidad de los tramos limpios por tercio de carrera, frente a lo habitual en cada carrera"
            formatTick={tickPercent}
            reference={{ value: 100, label: "Lo habitual" }}
            columns={thirds.map((t) => {
              const v = ratio(t.drift.mean_relative_ratio);
              return {
                key: t.third,
                label: thirdLabel(t),
                sublabel: `n = ${t.drift.legs}`,
                value: v,
                tooltip: {
                  value: v === null ? "Sin tramos" : percent(v),
                  detail: `${thirdLabel(t)} · ${legsLabel(t.drift.legs)} · ${bpm(t.drift.mean_heart_rate_bpm)} · ${pace(t.drift.mean_speed_mps)}`,
                },
              };
            })}
          />
        }
        table={<DriftTable thirds={thirds} />}
      />
      <ChartPanel
        title="Pulso antes del error"
        description={`Pulso del tramo anterior a un error y a un tramo limpio, en la misma fase de la carrera, en ppm sobre la mediana de los tramos de esa carrera. Si antes de fallar vas más alto que antes de un tramo limpio, el cansancio puede anticipar el error. Debajo, n antes de error / antes de limpio. Lo marcado como físico no entra.${withoutHeartRate}`}
        cases={legsLabel(beforeLegs)}
        chart={
          <>
            <Legend items={BEFORE.map((b) => ({ label: b.label, color: b.color }))} />
            <GroupedColumnChart
              label="Pulso del tramo anterior a un error y a un tramo limpio por tercio de carrera, sobre la mediana de cada carrera"
              xLabels={thirds.map(thirdLabel)}
              xSublabels={thirds.map(
                (t) => `n = ${t.before_error.legs} / ${t.before_clean.legs}`,
              )}
              series={BEFORE.map((b) => ({
                key: b.key,
                label: b.label,
                color: b.color,
                values: thirds.map((t) => before(t, b.key).mean_relative_bpm),
              }))}
              tooltipTitle={(i) => {
                const t = thirds[i];
                return {
                  value: t === undefined ? "" : thirdLabel(t),
                  detail:
                    t === undefined
                      ? ""
                      : `${legsLabel(t.before_error.legs)} antes de error · ${legsLabel(t.before_clean.legs)} antes de limpio`,
                };
              }}
              formatValue={relativeBpm}
              formatTick={tickBpm}
            />
          </>
        }
        table={<BeforeTable thirds={thirds} />}
      />
      <ChartPanel
        title="Esfuerzo percibido"
        description="Esfuerzo (1–10) que apuntaste al etiquetar tus errores, tus tramos limpios y los marcados como físico, por tercio de carrera. Solo cuenta donde lo apuntaste (en la tabla de tramos de cada carrera). Debajo, n de errores / limpios / físico."
        cases={`${legsLabel(effortLegs)} con esfuerzo`}
        chart={
          effortLegs === 0 ? (
            <p className="muted">
              Aún no has apuntado el esfuerzo de ningún tramo que cuente con estos filtros.
            </p>
          ) : (
            <>
              <Legend items={EFFORT.map((e) => ({ label: e.label, color: e.color }))} />
              <GroupedColumnChart
                label="Esfuerzo percibido medio de errores, tramos limpios y físicos por tercio de carrera"
                xLabels={thirds.map(thirdLabel)}
                xSublabels={thirds.map(
                  (t) => `n = ${EFFORT.map((e) => effort(t, e.key).legs).join(" / ")}`,
                )}
                series={EFFORT.map((e) => ({
                  key: e.key,
                  label: e.label,
                  color: e.color,
                  values: thirds.map((t) => effort(t, e.key).mean_effort),
                }))}
                tooltipTitle={(i) => {
                  const t = thirds[i];
                  return {
                    value: t === undefined ? "" : thirdLabel(t),
                    detail:
                      t === undefined
                        ? ""
                        : EFFORT.map((e) => `${effort(t, e.key).legs} ${e.label.toLowerCase()}`).join(
                            " · ",
                          ),
                  };
                }}
                formatValue={effortValue}
                formatTick={tick}
              />
            </>
          )
        }
        table={<EffortTable thirds={thirds} />}
      />
    </>
  );
}
