// Columnas agrupadas en SVG: por cada punto del eje X (p. ej. un tramo), una columna por serie,
// en el orden fijo de las series y con 2 px de separación entre columnas del grupo. Valores
// positivos y negativos desde la línea del 0, tooltip por grupo con el valor de cada serie
// (ratón y teclado). Ver la guía de visualización y `docs/app.md`, "Gráficas".
import { useState } from "react";
import { HEIGHT, MARGIN, Tooltip, TooltipText, YGrid, useWidth } from "./common";
import { columnPath } from "./ColumnChart";
import { linear, niceDomain } from "./scale";

export interface ColumnSeries {
  key: string;
  label: string;
  color: string;
  /** Un valor por grupo; `null` = sin columna. */
  values: (number | null)[];
}

const MAX_BAR = 16;
const GAP = 2;
const SUBLABEL_GAP = 14;

export function GroupedColumnChart({
  series,
  xLabels,
  xSublabels,
  tooltipTitle,
  formatValue,
  formatTick,
  label,
}: {
  series: ColumnSeries[];
  xLabels: string[];
  /** Segunda línea de cada etiqueta del eje X (p. ej. el número de casos del grupo). */
  xSublabels?: string[];
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
  const plotW = Math.max(width - MARGIN.left - MARGIN.right, 0);
  // Con segunda línea de etiquetas, el eje X necesita una línea más.
  const plotH = HEIGHT - MARGIN.top - MARGIN.bottom - (xSublabels !== undefined ? SUBLABEL_GAP : 0);
  const y = linear(domain, [MARGIN.top + plotH, MARGIN.top]);
  const n = xLabels.length;
  const band = n > 0 ? plotW / n : 0;
  const k = Math.max(series.length, 1);
  const barW = Math.max(Math.min(MAX_BAR, (band * 0.75 - GAP * (k - 1)) / k), 1.5);
  const groupW = barW * k + GAP * (k - 1);
  const x = (i: number) => MARGIN.left + i * band;
  const labelEvery = Math.max(1, Math.ceil(28 / Math.max(band, 1)));
  const zero = y(0);

  const tooltip = (i: number): TooltipText => ({
    ...tooltipTitle(i),
    rows: series.map((s) => ({ label: s.label, color: s.color, value: formatValue(s.values[i]) })),
  });

  return (
    <div className="chart" ref={ref}>
      {width > 0 && (
        <svg width={width} height={HEIGHT} role="img" aria-label={label}>
          <YGrid ticks={ticks} y={y} plotW={plotW} format={formatTick} />
          {xLabels.map((xl, i) => {
            const left = x(i) + (band - groupW) / 2;
            return (
              <g key={i}>
                {series.map((s, j) => {
                  const v = s.values[i];
                  return v === null ? null : (
                    <path
                      key={s.key}
                      className={active === i ? "chart-bar is-active" : "chart-bar"}
                      d={columnPath(left + j * (barW + GAP), barW, zero, y(v))}
                      fill={s.color}
                    />
                  );
                })}
                {i % labelEvery === 0 && (
                  <text
                    className="chart-tick"
                    x={x(i) + band / 2}
                    y={MARGIN.top + plotH + 18}
                    textAnchor="middle"
                  >
                    {xl}
                  </text>
                )}
                {i % labelEvery === 0 && xSublabels?.[i] !== undefined && (
                  <text
                    className="chart-tick"
                    x={x(i) + band / 2}
                    y={MARGIN.top + plotH + 18 + SUBLABEL_GAP}
                    textAnchor="middle"
                  >
                    {xSublabels[i]}
                  </text>
                )}
                <rect
                  className="chart-hit"
                  x={x(i)}
                  y={MARGIN.top}
                  width={band}
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
            );
          })}
        </svg>
      )}
      {active !== null && <Tooltip text={tooltip(active)} x={x(active) + band / 2} width={width} />}
    </div>
  );
}
