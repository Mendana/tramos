// Gráfica de varias líneas en SVG sobre los mismos puntos equiespaciados (p. ej. tras cada tramo):
// una línea de 2 px por serie (3 px la destacada), etiqueta directa al final de cada línea, cursor
// vertical que se ajusta al punto más cercano y tooltip con el valor de cada serie (ratón y
// teclado). Ver la guía de visualización y `docs/app.md`, "Gráficas".
import { useState } from "react";
import { HEIGHT, MARGIN, Tooltip, TooltipText, YGrid, lineRuns, useWidth } from "./common";
import { linear, niceDomain } from "./scale";

export interface LineSeries {
  key: string;
  /** Nombre corto, para la etiqueta del final de la línea y el tooltip. */
  label: string;
  color: string;
  /** Un valor por punto; `null` corta la línea. */
  values: (number | null)[];
  /** Línea más gruesa (p. ej. el propio corredor). */
  emphasis?: boolean;
}

/** Espacio a la derecha para las etiquetas del final de las líneas. */
const END_LABELS = 96;
/** Separación mínima entre etiquetas del final (px). */
const LABEL_GAP = 14;

export function MultiLineChart({
  series,
  xLabels,
  tooltipTitle,
  formatValue,
  formatTick,
  label,
}: {
  series: LineSeries[];
  /** Etiqueta del eje X de cada punto. */
  xLabels: string[];
  /** Título del tooltip de cada punto (p. ej. el tramo). */
  tooltipTitle: (i: number) => { value: string; detail: string };
  formatValue: (value: number | null) => string;
  formatTick: (value: number) => string;
  /** Descripción de la gráfica para lectores de pantalla. */
  label: string;
}) {
  const { ref, width } = useWidth();
  const [active, setActive] = useState<number | null>(null);

  const values = series.flatMap((s) => s.values.flatMap((v) => (v === null ? [] : [v])));
  const { domain, ticks } = niceDomain(Math.min(...values, 0), Math.max(...values, 0));
  const plotW = Math.max(width - MARGIN.left - MARGIN.right - END_LABELS, 0);
  const plotH = HEIGHT - MARGIN.top - MARGIN.bottom;
  const y = linear(domain, [MARGIN.top + plotH, MARGIN.top]);
  const n = xLabels.length;
  const step = n > 1 ? plotW / (n - 1) : 0;
  const x = (i: number) => MARGIN.left + i * step;
  const labelEvery = Math.max(1, Math.ceil(28 / Math.max(step, 1)));

  // Etiqueta al final de cada línea (su último valor), separadas para no pisarse.
  const ends = series
    .flatMap((s) => {
      const last = s.values.reduce<number | null>((acc, v, i) => (v === null ? acc : i), null);
      return last === null ? [] : [{ s, last, y: y(s.values[last] ?? 0) }];
    })
    .sort((a, b) => a.y - b.y);
  for (let k = 1; k < ends.length; k++) {
    ends[k].y = Math.max(ends[k].y, ends[k - 1].y + LABEL_GAP);
  }

  const tooltip = (i: number): TooltipText => ({
    ...tooltipTitle(i),
    rows: series.map((s) => ({ label: s.label, color: s.color, value: formatValue(s.values[i]) })),
  });

  return (
    <div className="chart" ref={ref}>
      {width > 0 && (
        <svg width={width} height={HEIGHT} role="img" aria-label={label}>
          <YGrid ticks={ticks} y={y} plotW={plotW} format={formatTick} />
          {active !== null && (
            <line
              className="chart-crosshair"
              x1={x(active)}
              x2={x(active)}
              y1={MARGIN.top}
              y2={MARGIN.top + plotH}
            />
          )}
          {/* La serie destacada, la última: queda por encima. */}
          {[...series]
            .sort((a, b) => Number(a.emphasis === true) - Number(b.emphasis === true))
            .map((s) =>
              lineRuns(s.values, x, y).map((run) => (
                <path
                  key={`${s.key}-${run.first}`}
                  className={s.emphasis === true ? "chart-line is-self" : "chart-line"}
                  d={run.d}
                  stroke={s.color}
                />
              )),
            )}
          {active !== null &&
            series.map((s) => {
              const v = s.values[active];
              return v === null ? null : (
                <circle
                  key={s.key}
                  className="chart-marker"
                  cx={x(active)}
                  cy={y(v)}
                  r={4}
                  fill={s.color}
                />
              );
            })}
          {ends.map(({ s, last, y: ly }) => (
            <g key={s.key}>
              <line
                x1={x(last) + 4}
                x2={MARGIN.left + plotW + 10}
                y1={y(s.values[last] ?? 0)}
                y2={ly}
                stroke={s.color}
                strokeWidth={1}
              />
              <text
                className="chart-end-label"
                x={MARGIN.left + plotW + 14}
                y={ly}
                dy="0.32em"
              >
                {s.label}
              </text>
            </g>
          ))}
          {xLabels.map((xl, i) => (
            <g key={i}>
              {i % labelEvery === 0 && (
                <text
                  className="chart-tick"
                  x={x(i)}
                  y={MARGIN.top + plotH + 18}
                  textAnchor="middle"
                >
                  {xl}
                </text>
              )}
              <rect
                className="chart-hit"
                x={x(i) - step / 2}
                y={MARGIN.top}
                width={Math.max(step, 1)}
                height={plotH}
                tabIndex={0}
                aria-label={`${tooltipTitle(i).value}: ${series
                  .map((s) => `${s.label} ${formatValue(s.values[i])}`)
                  .join(", ")}`}
                onPointerEnter={() => setActive(i)}
                onPointerLeave={() => setActive(null)}
                onFocus={() => setActive(i)}
                onBlur={() => setActive(null)}
              />
            </g>
          ))}
        </svg>
      )}
      {active !== null && <Tooltip text={tooltip(active)} x={x(active)} width={width} />}
    </div>
  );
}
