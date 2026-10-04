// Piezas comunes de las gráficas: tamaño, rejilla con marcas del eje Y, tooltip y leyenda.
import { useEffect, useRef, useState } from "react";

export const HEIGHT = 220;
export const MARGIN = { top: 12, right: 12, bottom: 28, left: 52 };

/** Ancho del contenedor, para que el SVG ocupe todo el panel. */
export function useWidth() {
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

/** Líneas de la rejilla (la del 0, más marcada) con sus marcas a la izquierda. */
export function YGrid({
  ticks,
  y,
  plotW,
  format,
}: {
  ticks: number[];
  y: (value: number) => number;
  plotW: number;
  format: (value: number) => string;
}) {
  return (
    <>
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
            {format(t)}
          </text>
        </g>
      ))}
    </>
  );
}

/**
 * Trazado de una línea por los puntos `(x(i), y(values[i]))`; un `null` la corta. Devuelve un
 * trazo por tramo continuo, con su primer y último índice (para cerrar el velo hasta el 0).
 */
export function lineRuns(
  values: (number | null)[],
  x: (i: number) => number,
  y: (value: number) => number,
): { first: number; last: number; d: string }[] {
  const runs: number[][] = [];
  values.forEach((v, i) => {
    if (v === null) return;
    const last = runs[runs.length - 1];
    if (last !== undefined && last[last.length - 1] === i - 1) last.push(i);
    else runs.push([i]);
  });
  return runs.map((run) => ({
    first: run[0],
    last: run[run.length - 1],
    d: run.map((i, k) => `${k === 0 ? "M" : "L"}${x(i)},${y(values[i] ?? 0)}`).join(" "),
  }));
}

/** Lo que enseña el tooltip: el valor delante y el detalle detrás. */
export interface TooltipText {
  value: string;
  detail: string;
}

/** Separación entre el tooltip y la marca a la que acompaña (px). */
const TOOLTIP_GAP = 16;

/**
 * Tooltip junto a la marca de `x`: a su derecha y, en la mitad derecha de la gráfica, a su
 * izquierda, para no taparla nunca.
 */
export function Tooltip({ text, x, width }: { text: TooltipText; x: number; width: number }) {
  const onRight = x < width / 2;
  return (
    <div
      className={onRight ? "chart-tooltip" : "chart-tooltip is-left"}
      style={{ left: onRight ? x + TOOLTIP_GAP : x - TOOLTIP_GAP, top: MARGIN.top }}
    >
      <strong>{text.value}</strong>
      <span>{text.detail}</span>
    </div>
  );
}

/**
 * Leyenda: una muestra de color por serie (cuadrado para barras, raya para líneas, rectángulo con
 * borde para franjas de fondo).
 */
export function Legend({
  items,
}: {
  items: { label: string; color: string; shape?: "rect" | "line" | "band" }[];
}) {
  return (
    <div className="legend">
      {items.map((item) => (
        <span className="legend-item" key={item.label}>
          {item.shape === "line" ? (
            <svg width="16" height="10" aria-hidden="true">
              <line x1="1" x2="15" y1="5" y2="5" stroke={item.color} strokeWidth="2" strokeLinecap="round" />
            </svg>
          ) : (
            <span
              className={item.shape === "band" ? "legend-swatch is-band" : "legend-swatch"}
              style={{ background: item.color }}
            />
          )}
          {item.label}
        </span>
      ))}
    </div>
  );
}
