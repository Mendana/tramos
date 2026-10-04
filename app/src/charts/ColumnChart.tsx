// Gráfica de columnas en SVG: una serie, valores positivos y negativos desde la línea del 0,
// línea de referencia opcional y tooltip por columna (ratón y teclado). Sigue la guía de
// visualización: columnas finas (≤ 24 px) con el extremo redondeado, rejilla tenue y texto en
// los colores de texto, nunca en el de la serie.
import { useEffect, useRef, useState } from "react";
import { linear, niceDomain } from "./scale";

export interface Column {
  /** Clave estable (p. ej. el número de tramo). */
  key: string | number;
  /** Etiqueta del eje X. */
  label: string;
  /** `null` = sin dato: no se dibuja columna. */
  value: number | null;
  /** Texto del tooltip y de la etiqueta accesible: primero el valor, luego qué es. */
  tooltip: { value: string; detail: string };
  /** Color de la columna; por defecto, el de la serie. */
  color?: string;
}

const HEIGHT = 220;
const MARGIN = { top: 12, right: 12, bottom: 28, left: 48 };
const MAX_BAR = 24;
const RADIUS = 4;

/** Ancho del contenedor, para que el SVG ocupe todo el panel. */
function useWidth() {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const element = ref.current;
    if (element === null) return;
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  return { ref, width };
}

/** Columna con el extremo de datos redondeado y la base recta, sobre la línea del 0. */
function columnPath(x: number, w: number, y0: number, y1: number): string {
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
  label,
}: {
  columns: Column[];
  formatTick: (value: number) => string;
  /** Línea horizontal de referencia (p. ej. el 100 %), con su nombre. */
  reference?: { value: number; label: string };
  /** Descripción de la gráfica para lectores de pantalla. */
  label: string;
}) {
  const { ref, width } = useWidth();
  const [active, setActive] = useState<number | null>(null);

  const values = columns.flatMap((c) => (c.value === null ? [] : [c.value]));
  if (reference !== undefined) values.push(reference.value);
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
  const activeColumn = active === null ? null : columns[active];

  return (
    <div className="chart" ref={ref}>
      {width > 0 && (
        <svg width={width} height={HEIGHT} role="img" aria-label={label}>
          {ticks.map((t) => (
            <g key={t}>
              <line
                className={t === 0 ? "chart-axis" : "chart-grid"}
                x1={MARGIN.left}
                x2={MARGIN.left + plotW}
                y1={y(t)}
                y2={y(t)}
              />
              <text className="chart-tick" x={MARGIN.left - 8} y={y(t)} dy="0.32em" textAnchor="end">
                {formatTick(t)}
              </text>
            </g>
          ))}

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
                {i % labelEvery === 0 && (
                  <text
                    className="chart-tick"
                    x={x(i) + band / 2}
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
      {activeColumn !== null && active !== null && (
        <div
          className="chart-tooltip"
          style={{
            left: Math.min(Math.max(x(active) + band / 2, 70), width - 70),
            top: MARGIN.top,
          }}
        >
          <strong>{activeColumn.tooltip.value}</strong>
          <span>{activeColumn.tooltip.detail}</span>
        </div>
      )}
    </div>
  );
}
