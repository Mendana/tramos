// Gráfica de evolución en SVG con la fecha en el eje X: una serie por grupo (p. ej. por formato),
// cada carrera un punto de 8 px y una línea de 2 px que los une en orden. El eje X es la fecha
// de verdad (no puntos equiespaciados): dos carreras de febrero y una de octubre quedan lejos.
// El eje Y parte del rango de los datos, no del 0. Etiqueta directa al final de cada línea (con
// leyenda aparte, `Legend`), cursor y tooltip del punto más cercano (ratón y teclado). Un punto
// `hollow` se dibuja hueco: un dato que no se lee igual que los demás. Ver la guía de
// visualización y `docs/app.md`, "Gráficas".
import { useState } from "react";
import { HEIGHT, MARGIN, Tooltip, TooltipText, YGrid, lineRuns, useWidth } from "./common";
import { linear, niceRange } from "./scale";

export interface DatePoint {
  /** Fecha `AAAA-MM-DD`. */
  date: string;
  /** `null`: la carrera no tiene el dato y no se dibuja. */
  value: number | null;
  tooltip: TooltipText;
  /** Punto hueco: el dato existe pero no es comparable con los demás. */
  hollow?: boolean;
}

export interface DateSeries {
  key: string;
  /** Nombre corto, para la etiqueta del final de la línea. */
  label: string;
  color: string;
  /** De la más antigua a la más reciente. */
  points: DatePoint[];
}

/** Espacio a la derecha para las etiquetas del final de las líneas. */
const END_LABELS = 64;
/** Separación mínima entre etiquetas del final (px). */
const LABEL_GAP = 14;
const MARKER_R = 4;
/** Separación mínima entre marcas del eje X (px). */
const TICK_SPACING = 84;
const DAY_MS = 86_400_000;
/** Pasos de meses para las marcas del eje X. */
const MONTH_STEPS = [1, 2, 3, 6, 12, 24];

/** Días desde 1970 de una fecha `AAAA-MM-DD`. */
export const dayOf = (date: string) => Date.parse(`${date}T00:00:00Z`) / DAY_MS;

const two = (n: number) => String(n).padStart(2, "0");

interface Tick {
  day: number;
  label: string;
}

/**
 * Marcas del eje X entre los días `min` y `max`: con más de tres meses, primeros de mes cada 1, 2,
 * 3, 6, 12 o 24 meses (`MM/AA`); con menos, días repartidos por igual (`DD/MM/AA`).
 */
export function dateTicks(min: number, max: number, count: number): Tick[] {
  const from = new Date(min * DAY_MS);
  if (max - min >= 90) {
    for (const step of MONTH_STEPS) {
      const ticks: Tick[] = [];
      for (let k = 0; ; k += step) {
        const d = new Date(Date.UTC(from.getUTCFullYear(), from.getUTCMonth() + 1 + k, 1));
        const day = d.getTime() / DAY_MS;
        if (day > max) break;
        ticks.push({
          day,
          label: `${two(d.getUTCMonth() + 1)}/${two(d.getUTCFullYear() % 100)}`,
        });
      }
      if (ticks.length <= count) return ticks;
    }
  }
  const n = Math.max(1, count - 1);
  return Array.from({ length: n + 1 }, (_, k) => {
    const day = Math.round(min + ((max - min) * k) / n);
    const d = new Date(day * DAY_MS);
    return {
      day,
      label: `${two(d.getUTCDate())}/${two(d.getUTCMonth() + 1)}/${two(d.getUTCFullYear() % 100)}`,
    };
  }).filter((t, k, all) => k === 0 || t.day !== all[k - 1].day);
}

export function DateLineChart({
  series,
  formatTick,
  label,
}: {
  series: DateSeries[];
  formatTick: (value: number) => string;
  /** Descripción de la gráfica para lectores de pantalla. */
  label: string;
}) {
  const { ref, width } = useWidth();
  const [active, setActive] = useState<{ series: number; point: number } | null>(null);

  const drawn = series.flatMap((s) =>
    s.points.flatMap((p) => (p.value === null ? [] : [{ day: dayOf(p.date), value: p.value }])),
  );
  const values = drawn.map((p) => p.value);
  const days = drawn.map((p) => p.day);
  const { domain, ticks } = niceRange(Math.min(...values), Math.max(...values));
  // Un solo día (o ninguno): el eje se abre un día a cada lado para que el punto no quede en el borde.
  const lo = Math.min(...days);
  const hi = Math.max(...days);
  const dayDomain: [number, number] = hi > lo ? [lo, hi] : [lo - 1, lo + 1];
  const plotW = Math.max(width - MARGIN.left - MARGIN.right - END_LABELS, 0);
  const plotH = HEIGHT - MARGIN.top - MARGIN.bottom;
  const y = linear(domain, [MARGIN.top + plotH, MARGIN.top]);
  // Un poco de aire a los lados para que los puntos de los extremos no se corten.
  const pad = MARKER_R + 2;
  const xOf = linear(dayDomain, [MARGIN.left + pad, MARGIN.left + plotW - pad]);
  const xTicks = dateTicks(
    dayDomain[0],
    dayDomain[1],
    Math.max(2, Math.floor(plotW / TICK_SPACING)),
  );

  const px = (s: DateSeries, i: number) => xOf(dayOf(s.points[i].date));

  // Etiqueta al final de cada línea (su último punto), separadas para no pisarse.
  const ends = series
    .flatMap((s, si) => {
      const last = s.points.reduce<number | null>(
        (acc, p, i) => (p.value === null ? acc : i),
        null,
      );
      return last === null ? [] : [{ s, si, last, y: y(s.points[last].value ?? 0) }];
    })
    .sort((a, b) => a.y - b.y);
  for (let k = 1; k < ends.length; k++) {
    ends[k].y = Math.max(ends[k].y, ends[k - 1].y + LABEL_GAP);
  }

  const activePoint = active === null ? undefined : series[active.series]?.points[active.point];
  const activeS = active === null ? undefined : series[active.series];

  // Cursor: el punto más cercano en horizontal (y, a igual distancia, en vertical).
  const nearest = (clientX: number, clientY: number, svg: SVGSVGElement) => {
    const box = svg.getBoundingClientRect();
    const mx = clientX - box.left;
    const my = clientY - box.top;
    let best: { series: number; point: number } | null = null;
    let bestDistance = Infinity;
    series.forEach((s, si) =>
      s.points.forEach((p, pi) => {
        if (p.value === null) return;
        const dx = px(s, pi) - mx;
        const dy = y(p.value) - my;
        const distance = dx * dx + dy * dy * 0.25;
        if (distance < bestDistance) {
          bestDistance = distance;
          best = { series: si, point: pi };
        }
      }),
    );
    return best;
  };

  return (
    <div className="chart" ref={ref}>
      {width > 0 && (
        <svg
          width={width}
          height={HEIGHT}
          role="img"
          aria-label={label}
          onPointerMove={(e) => setActive(nearest(e.clientX, e.clientY, e.currentTarget))}
          onPointerLeave={() => setActive(null)}
        >
          <YGrid ticks={ticks} y={y} plotW={plotW} format={formatTick} />
          {xTicks.map((t) => (
            <text
              key={t.day}
              className="chart-tick"
              x={xOf(t.day)}
              y={MARGIN.top + plotH + 18}
              textAnchor="middle"
            >
              {t.label}
            </text>
          ))}
          {active !== null && activeS !== undefined && (
            <line
              className="chart-crosshair"
              x1={px(activeS, active.point)}
              x2={px(activeS, active.point)}
              y1={MARGIN.top}
              y2={MARGIN.top + plotH}
            />
          )}
          {series.map((s) =>
            lineRuns(
              s.points.map((p) => p.value),
              (i) => px(s, i),
              y,
            ).map((run) => (
              <path
                key={`${s.key}-${run.first}`}
                className="chart-line"
                d={run.d}
                stroke={s.color}
              />
            )),
          )}
          {series.map((s, si) =>
            s.points.map((p, pi) =>
              p.value === null ? null : (
                <circle
                  key={`${s.key}-${p.date}-${pi}`}
                  className="chart-marker"
                  cx={px(s, pi)}
                  cy={y(p.value)}
                  r={active?.series === si && active.point === pi ? MARKER_R + 1.5 : MARKER_R}
                  fill={p.hollow === true ? "var(--surface)" : s.color}
                  style={p.hollow === true ? { stroke: s.color } : undefined}
                  tabIndex={0}
                  aria-label={`${p.tooltip.detail}: ${p.tooltip.value}`}
                  onFocus={() => setActive({ series: si, point: pi })}
                  onBlur={() => setActive(null)}
                />
              ),
            ),
          )}
          {ends.map(({ s, last, y: ly }) => (
            <g key={s.key}>
              <line
                x1={px(s, last) + MARKER_R + 2}
                x2={MARGIN.left + plotW + 4}
                y1={y(s.points[last].value ?? 0)}
                y2={ly}
                stroke={s.color}
                strokeWidth={1}
              />
              <text className="chart-end-label" x={MARGIN.left + plotW + 8} y={ly} dy="0.32em">
                {s.label}
              </text>
            </g>
          ))}
        </svg>
      )}
      {activePoint !== undefined && activeS !== undefined && active !== null && (
        <Tooltip text={activePoint.tooltip} x={px(activeS, active.point)} width={width} />
      )}
    </div>
  );
}
