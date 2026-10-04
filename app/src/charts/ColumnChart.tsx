// Gráfica de columnas en SVG: una serie, valores positivos y negativos desde la línea del 0,
// línea de referencia opcional, línea de datos opcional en la misma escala (p. ej. el acumulado),
// franjas resaltadas opcionales y tooltip por columna (ratón y teclado). Sigue la guía de
// visualización: columnas finas (≤ 24 px) con el extremo redondeado, rejilla tenue y texto en
// los colores de texto, nunca en el de la serie.
import { useState } from "react";
import { HEIGHT, MARGIN, Tooltip, TooltipText, YGrid, lineRuns, useWidth } from "./common";
import { linear, niceDomain } from "./scale";

export interface Column {
  /** Clave estable (p. ej. el número de tramo). */
  key: string | number;
  /** Etiqueta del eje X. */
  label: string;
  /** `null` = sin dato: no se dibuja columna. */
  value: number | null;
  tooltip: TooltipText;
  /** Color de la columna; por defecto, el de la serie. */
  color?: string;
}

/** Línea sobre las columnas, en la misma unidad y escala (un solo eje). */
export interface ColumnLine {
  /** Un valor por columna; `null` corta la línea. */
  values: (number | null)[];
  color: string;
}

/** Franja de fondo de la columna `from` a la `to` (índices, ambas incluidas). */
export interface Highlight {
  from: number;
  to: number;
}

const MAX_BAR = 24;
const RADIUS = 4;

/** Columna con el extremo de datos redondeado y la base recta, sobre la línea del 0. */
export function columnPath(x: number, w: number, y0: number, y1: number): string {
  const h = Math.abs(y1 - y0);
  const r = Math.min(RADIUS, h, w / 2);
  if (y1 <= y0) {
    // Hacia arriba: redondeada arriba.
    return `M${x},${y0} V${y1 + r} Q${x},${y1} ${x + r},${y1} H${x + w - r} Q${x + w},${y1} ${x + w},${y1 + r} V${y0} Z`;
  }
  // Hacia abajo: redondeada abajo.
  return `M${x},${y0} V${y1 - r} Q${x},${y1} ${x + r},${y1} H${x + w - r} Q${x + w},${y1} ${x + w},${y1 - r} V${y0} Z`;
}

export function ColumnChart({
  columns,
  formatTick,
  reference,
  line,
  highlights = [],
  label,
}: {
  columns: Column[];
  formatTick: (value: number) => string;
  /** Línea horizontal de referencia (p. ej. el 100 %), con su nombre. */
  reference?: { value: number; label: string };
  line?: ColumnLine;
  /** Franjas resaltadas detrás de las columnas (p. ej. rachas). */
  highlights?: Highlight[];
  /** Descripción de la gráfica para lectores de pantalla. */
  label: string;
}) {
  const { ref, width } = useWidth();
  const [active, setActive] = useState<number | null>(null);

  const values = columns.flatMap((c) => (c.value === null ? [] : [c.value]));
  if (reference !== undefined) values.push(reference.value);
  if (line !== undefined) values.push(...line.values.flatMap((v) => (v === null ? [] : [v])));
  const { domain, ticks } = niceDomain(Math.min(...values, 0), Math.max(...values, 0));

  const plotW = Math.max(width - MARGIN.left - MARGIN.right, 0);
  const plotH = HEIGHT - MARGIN.top - MARGIN.bottom;
  const y = linear(domain, [MARGIN.top + plotH, MARGIN.top]);
  const band = columns.length > 0 ? plotW / columns.length : 0;
  const barW = Math.max(Math.min(MAX_BAR, band * 0.6), 2);
  const x = (i: number) => MARGIN.left + i * band;
  // Con muchas columnas, una etiqueta del eje X de cada `labelEvery`.
  const labelEvery = Math.max(1, Math.ceil(28 / Math.max(band, 1)));
  const zero = y(0);
  const activeColumn = active === null ? undefined : columns[active];
  const center = (i: number) => x(i) + band / 2;

  return (
    <div className="chart" ref={ref}>
      {width > 0 && (
        <svg width={width} height={HEIGHT} role="img" aria-label={label}>
          {highlights.map((h) => (
            <rect
              key={h.from}
              className="chart-highlight"
              x={x(h.from)}
              y={MARGIN.top}
              width={(h.to - h.from + 1) * band}
              height={plotH}
            />
          ))}
          <YGrid ticks={ticks} y={y} plotW={plotW} format={formatTick} />

          {reference !== undefined && (
            <g>
              <line
                className="chart-reference"
                x1={MARGIN.left}
                x2={MARGIN.left + plotW}
                y1={y(reference.value)}
                y2={y(reference.value)}
              />
              <text
                className="chart-tick"
                x={MARGIN.left + plotW}
                y={y(reference.value) - 6}
                textAnchor="end"
              >
                {reference.label}
              </text>
            </g>
          )}

          {line !== undefined &&
            lineRuns(line.values, center, y).map((run) => (
              <path key={run.first} className="chart-line" d={run.d} stroke={line.color} />
            ))}

          {columns.map((c, i) => {
            const cx = x(i) + (band - barW) / 2;
            return (
              <g key={c.key}>
                {c.value !== null && (
                  <path
                    className={active === i ? "chart-bar is-active" : "chart-bar"}
                    d={columnPath(cx, barW, zero, y(c.value))}
                    fill={c.color ?? "var(--chart-series-1)"}
                  />
                )}
                {line !== undefined && line.values[i] !== null && (
                  <circle
                    className="chart-marker"
                    cx={center(i)}
                    cy={y(line.values[i] ?? 0)}
                    r={active === i ? 4 : 2.5}
                    fill={line.color}
                  />
                )}
                {i % labelEvery === 0 && (
                  <text
                    className="chart-tick"
                    x={center(i)}
                    y={MARGIN.top + plotH + 18}
                    textAnchor="middle"
                  >
                    {c.label}
                  </text>
                )}
                {/* Zona activa: toda la franja, más grande que la columna. */}
                <rect
                  className="chart-hit"
                  x={x(i)}
                  y={MARGIN.top}
                  width={band}
                  height={plotH}
                  tabIndex={0}
                  aria-label={`${c.tooltip.detail}: ${c.tooltip.value}`}
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
      {activeColumn !== undefined && active !== null && (
        <Tooltip text={activeColumn.tooltip} x={center(active)} width={width} />
      )}
    </div>
  );
}
