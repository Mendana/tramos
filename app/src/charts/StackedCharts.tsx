// Gráficas apiladas con el mismo eje X (p. ej. la carrera, de la salida a la meta): una gráfica por
// magnitud, cada una con su propia escala en Y (nunca dos escalas en una), línea de 2 px, y un
// cursor vertical con su tooltip en todas a la vez (ratón y teclado). Admite franjas de fondo y
// líneas verticales finas con etiqueta (capas opcionales). Ver la guía de visualización y
// `docs/app.md`, "Gráficas".
import { KeyboardEvent, PointerEvent, useId, useState } from "react";
import { Legend, MARGIN, Tooltip, TooltipText, YGrid, lineRuns, useWidth } from "./common";
import { linear, niceRange, niceTicks } from "./scale";

export interface StackSeries {
  key: string;
  /** Nombre corto, para la leyenda y el tooltip. */
  label: string;
  color: string;
  /** Un valor por punto del eje X; `null` corta la línea. */
  values: (number | null)[];
}

export interface StackRow {
  key: string;
  /** Qué magnitud es, con su unidad. */
  title: string;
  /** La magnitud sola, para el tooltip. */
  name: string;
  /** Pasos «redondos» de las marcas del eje Y (p. ej. 15 y 30 s para un ritmo). */
  tickSteps?: number[];
  series: StackSeries[];
  /** Velo del 10 % bajo la línea hasta la base de la gráfica (p. ej. la altitud). */
  area?: boolean;
  /** Los valores pequeños arriba (p. ej. el ritmo: más rápido = más arriba). */
  inverted?: boolean;
  /** El dibujo se ajusta al grueso de los datos (percentiles 2 a 98): los extremos (p. ej. andar
   *  cuesta arriba) se salen de la gráfica pero siguen en el tooltip. */
  robust?: boolean;
  formatTick: (value: number) => string;
  formatValue: (value: number | null) => string;
  /** Si no hay ningún valor, qué falta (en vez de dejar la gráfica vacía). */
  missing: string;
}

/** Franja de fondo entre dos valores del eje X. */
export interface StackBand {
  from: number;
  to: number;
  className: string;
}

/** Línea vertical fina en un valor del eje X, con su etiqueta en la primera gráfica. */
export interface StackMarker {
  x: number;
  label: string;
}

/** Alto del dibujo de cada gráfica (px), sin márgenes. */
const PLOT_H = 112;
/** Margen superior de la primera gráfica: ahí van las etiquetas de las líneas verticales. */
const FIRST_TOP = 22;
const TOP = 8;
const LAST_BOTTOM = 28;
const BOTTOM = 6;
/** Separación mínima entre dos etiquetas de líneas verticales (px). */
const MARKER_GAP = 18;
/** Pasos del teclado: una flecha, con Mayús. */
const KEY_STEP = 1;
/** Lo que el puntero puede salirse del dibujo por los lados y seguir marcando el extremo (px). */
const SIDE_SLACK = 6;
const KEY_STEP_FAST = 12;

/** Percentil `q` (0 a 1) de los valores ya ordenados. */
function quantile(sorted: number[], q: number): number {
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.round((sorted.length - 1) * q)))];
}

/** Índice del valor de `xs` (ordenado) más cercano a `target`. */
function nearest(xs: number[], target: number): number {
  let lo = 0;
  let hi = xs.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (xs[mid] < target) lo = mid + 1;
    else hi = mid;
  }
  return lo > 0 && target - xs[lo - 1] <= xs[lo] - target ? lo - 1 : lo;
}

export function StackedCharts({
  xs,
  formatX,
  xSteps,
  rows,
  bands = [],
  markers = [],
  tooltipTitle,
  label,
}: {
  /** Valor del eje X de cada punto (segundos, metros…), de menor a mayor. */
  xs: number[];
  formatX: (value: number) => string;
  /** Pasos «redondos» de las marcas del eje X, de menor a mayor. */
  xSteps: number[];
  rows: StackRow[];
  bands?: StackBand[];
  markers?: StackMarker[];
  /** Título del tooltip de cada punto: el valor de X y el detalle (tramo, estado…). */
  tooltipTitle: (i: number) => { value: string; detail: string };
  /** Descripción de las gráficas para lectores de pantalla. */
  label: string;
}) {
  const { ref, width } = useWidth();
  // Sin los «:» de React: en `url(#…)` pueden dar problemas.
  const clipId = useId().replace(/:/g, "");
  const [active, setActive] = useState<number | null>(null);

  const plotW = Math.max(width - MARGIN.left - MARGIN.right, 0);
  const x0 = xs[0] ?? 0;
  const x1 = xs[xs.length - 1] ?? 1;
  const xScale = linear([x0, x1], [MARGIN.left, MARGIN.left + plotW]);
  const ticks = niceTicks(x0, x1, Math.max(2, Math.floor(plotW / 80)), xSteps);

  /**
   * Qué punto está bajo el puntero. Se mira en todo el conjunto (no en cada gráfica) para que el
   * cursor no parpadee al cruzar el hueco entre dos; fuera de las columnas del dibujo, no hay.
   */
  const onPointer = (e: PointerEvent<HTMLDivElement>) => {
    const box = e.currentTarget.getBoundingClientRect();
    const px = e.clientX - box.left - MARGIN.left;
    if (plotW <= 0 || xs.length === 0 || px < -SIDE_SLACK || px > plotW + SIDE_SLACK) {
      setActive(null);
      return;
    }
    const fraction = Math.min(Math.max(px / plotW, 0), 1);
    setActive(nearest(xs, x0 + fraction * (x1 - x0)));
  };
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? KEY_STEP_FAST : KEY_STEP;
    const move = (to: number) => {
      e.preventDefault();
      setActive(Math.min(Math.max(to, 0), xs.length - 1));
    };
    if (e.key === "ArrowRight") move((active ?? -1) + step);
    else if (e.key === "ArrowLeft") move((active ?? xs.length) - step);
    else if (e.key === "Home") move(0);
    else if (e.key === "End") move(xs.length - 1);
    else if (e.key === "Escape") setActive(null);
  };

  // Etiquetas de las líneas verticales, sin pisarse.
  let lastLabelX = -Infinity;
  const labelled = [...markers]
    .sort((a, b) => a.x - b.x)
    .map((m) => {
      const px = xScale(m.x);
      const show = px - lastLabelX >= MARKER_GAP;
      if (show) lastLabelX = px;
      return { ...m, px, show };
    });

  // La gráfica que lleva el instante en su tooltip: la primera con algún valor.
  const header = rows.findIndex((row) => row.series.some((s) => s.values.some((v) => v !== null)));

  return (
    <div
      className="chart-stack"
      ref={ref}
      tabIndex={0}
      role="group"
      aria-label={`${label}. Con las flechas del teclado se mueve el cursor.`}
      onKeyDown={onKey}
      onBlur={() => setActive(null)}
      onPointerMove={onPointer}
      onPointerDown={onPointer}
      onPointerLeave={() => setActive(null)}
    >
      {width > 0 &&
        rows.map((row, r) => {
          const first = r === 0;
          const last = r === rows.length - 1;
          const top = first ? FIRST_TOP : TOP;
          const bottom = last ? LAST_BOTTOM : BOTTOM;
          const height = top + PLOT_H + bottom;
          const values = row.series.flatMap((s) =>
            s.values.flatMap((v) => (v === null ? [] : [v])),
          );
          const sorted = [...values].sort((a, b) => a - b);
          const [lo, hi] =
            row.robust === true && sorted.length > 20
              ? [quantile(sorted, 0.02), quantile(sorted, 0.98)]
              : [sorted[0], sorted[sorted.length - 1]];
          const empty = values.length === 0;
          // Un 4 % de aire arriba y abajo para que la línea no se pegue a los bordes.
          const pad = empty ? 0 : (hi - lo) * 0.04;
          const { domain, ticks: yTicks } = niceRange(
            empty ? 0 : lo - pad,
            empty ? 1 : hi + pad,
            4,
            row.tickSteps,
          );
          const y = linear(
            domain,
            row.inverted === true ? [top, top + PLOT_H] : [top + PLOT_H, top],
          );
          const bottomY = top + PLOT_H;
          // La primera gráfica con datos lleva el instante y el tramo; las demás, su valor.
          const tooltip = (i: number): TooltipText => {
            const seriesRows = row.series.map((s) => ({
              label: s.label,
              color: s.color,
              value: row.formatValue(s.values[i]),
            }));
            if (r === header) return { ...tooltipTitle(i), rows: seriesRows };
            return row.series.length === 1
              ? { value: row.formatValue(row.series[0].values[i]), detail: row.name }
              : { value: row.name, detail: "", rows: seriesRows };
          };
          return (
            <div className="stack-row" key={row.key}>
              <div className="stack-row-head">
                <h4 className="stack-row-title">{row.title}</h4>
                {row.series.length > 1 && (
                  <Legend
                    items={row.series.map((s) => ({
                      label: s.label,
                      color: s.color,
                      shape: "line" as const,
                    }))}
                  />
                )}
              </div>
              <div className="chart">
                <svg width={width} height={height} role="img" aria-label={`${row.title}`}>
                  <clipPath id={`${clipId}-${row.key}`}>
                    <rect x={MARGIN.left} y={top} width={plotW} height={PLOT_H} />
                  </clipPath>
                  {bands.map((b, k) => {
                    const from = xScale(b.from);
                    return (
                      <rect
                        key={k}
                        className={b.className}
                        x={from}
                        y={top}
                        width={Math.max(xScale(b.to) - from, 1.5)}
                        height={PLOT_H}
                        clipPath={`url(#${clipId}-${row.key})`}
                      />
                    );
                  })}
                  {!empty && <YGrid ticks={yTicks} y={y} plotW={plotW} format={row.formatTick} />}
                  {ticks.map((t) => (
                    <g key={t}>
                      <line
                        className="chart-grid"
                        x1={xScale(t)}
                        x2={xScale(t)}
                        y1={top}
                        y2={bottomY}
                      />
                      {last && (
                        <text
                          className="chart-tick"
                          x={xScale(t)}
                          y={bottomY + 18}
                          textAnchor="middle"
                        >
                          {formatX(t)}
                        </text>
                      )}
                    </g>
                  ))}
                  <line
                    className="chart-axis"
                    x1={MARGIN.left}
                    x2={MARGIN.left + plotW}
                    y1={bottomY}
                    y2={bottomY}
                  />
                  {labelled.map((m) => (
                    <g key={m.x}>
                      <line
                        className="stack-marker"
                        x1={m.px}
                        x2={m.px}
                        y1={first && m.show ? top - 4 : top}
                        y2={bottomY}
                      />
                      {first && m.show && (
                        <text className="chart-tick" x={m.px} y={top - 8} textAnchor="middle">
                          {m.label}
                        </text>
                      )}
                    </g>
                  ))}
                  {row.series.map((s) =>
                    lineRuns(s.values, (i) => xScale(xs[i]), y).map((run) => (
                      <g key={`${s.key}-${run.first}`} clipPath={`url(#${clipId}-${row.key})`}>
                        {row.area === true && (
                          <path
                            className="chart-area"
                            d={`${run.d} L${xScale(xs[run.last])},${bottomY} L${xScale(xs[run.first])},${bottomY} Z`}
                            fill={s.color}
                          />
                        )}
                        <path className="chart-line" d={run.d} stroke={s.color} />
                      </g>
                    )),
                  )}
                  {empty && (
                    <text
                      className="stack-missing"
                      x={MARGIN.left + plotW / 2}
                      y={top + PLOT_H / 2}
                      textAnchor="middle"
                      dy="0.32em"
                    >
                      {row.missing}
                    </text>
                  )}
                  {active !== null && (
                    <>
                      <line
                        className="chart-crosshair"
                        x1={xScale(xs[active])}
                        x2={xScale(xs[active])}
                        y1={top}
                        y2={bottomY}
                      />
                      {row.series.map((s) => {
                        const v = s.values[active];
                        return v === null ? null : (
                          <circle
                            key={s.key}
                            className="chart-marker"
                            cx={xScale(xs[active])}
                            cy={Math.min(Math.max(y(v), top), bottomY)}
                            r={4}
                            fill={s.color}
                          />
                        );
                      })}
                    </>
                  )}
                </svg>
                {active !== null && !empty && (
                  <Tooltip text={tooltip(active)} x={xScale(xs[active])} width={width} />
                )}
              </div>
            </div>
          );
        })}
    </div>
  );
}
