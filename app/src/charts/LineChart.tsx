// Gráfica de línea en SVG: una serie de puntos equiespaciados (p. ej. tras cada tramo), línea de
// 2 px con un velo del 10 % hasta el 0, marcadores opcionales con anillo del color de la
// superficie y un cursor vertical que se ajusta al punto más cercano, con tooltip (ratón y
// teclado). Ver la guía de visualización y `docs/app.md`, "Gráficas".
import { useState } from "react";
import { HEIGHT, MARGIN, Tooltip, TooltipText, YGrid, lineRuns, useWidth } from "./common";
import { linear, niceDomain } from "./scale";

export interface LinePoint {
  key: string | number;
  /** Etiqueta del eje X. */
  label: string;
  /** `null` corta la línea. */
  value: number | null;
  tooltip: TooltipText;
  /** Marcador visible en este punto (p. ej. donde hubo un error). */
  marker?: boolean;
}

const MARKER_R = 4;

export function LineChart({
  points,
  formatTick,
  label,
  color = "var(--chart-series-1)",
  markerColor = color,
}: {
  points: LinePoint[];
  formatTick: (value: number) => string;
  /** Descripción de la gráfica para lectores de pantalla. */
  label: string;
  color?: string;
  markerColor?: string;
}) {
  const { ref, width } = useWidth();
  const [active, setActive] = useState<number | null>(null);

  const values = points.flatMap((p) => (p.value === null ? [] : [p.value]));
  const { domain, ticks } = niceDomain(Math.min(...values, 0), Math.max(...values, 0));
  const plotW = Math.max(width - MARGIN.left - MARGIN.right, 0);
  const plotH = HEIGHT - MARGIN.top - MARGIN.bottom;
  const y = linear(domain, [MARGIN.top + plotH, MARGIN.top]);
  const step = points.length > 1 ? plotW / (points.length - 1) : 0;
  const x = (i: number) => MARGIN.left + i * step;
  const labelEvery = Math.max(1, Math.ceil(28 / Math.max(step, 1)));

  const runs = lineRuns(
    points.map((p) => p.value),
    x,
    y,
  );
  const activePoint = active === null ? undefined : points[active];

  return (
    <div className="chart" ref={ref}>
      {width > 0 && (
        <svg width={width} height={HEIGHT} role="img" aria-label={label}>
          <YGrid ticks={ticks} y={y} plotW={plotW} format={formatTick} />
          {runs.map((run) => (
            <g key={run.first}>
              <path
                className="chart-area"
                d={`${run.d} L${x(run.last)},${y(0)} L${x(run.first)},${y(0)} Z`}
                fill={color}
              />
              <path className="chart-line" d={run.d} stroke={color} />
            </g>
          ))}
          {active !== null && (
            <line
              className="chart-crosshair"
              x1={x(active)}
              x2={x(active)}
              y1={MARGIN.top}
              y2={MARGIN.top + plotH}
            />
          )}
          {points.map((p, i) => (
            <g key={p.key}>
              {p.value !== null && (p.marker === true || active === i) && (
                <circle
                  className="chart-marker"
                  cx={x(i)}
                  cy={y(p.value)}
                  r={MARKER_R}
                  fill={p.marker === true ? markerColor : color}
                />
              )}
              {i % labelEvery === 0 && (
                <text
                  className="chart-tick"
                  x={x(i)}
                  y={MARGIN.top + plotH + 18}
                  textAnchor="middle"
                >
                  {p.label}
                </text>
              )}
              {/* Franja de cada punto: el cursor se ajusta al más cercano. */}
              <rect
                className="chart-hit"
                x={x(i) - step / 2}
                y={MARGIN.top}
                width={Math.max(step, 1)}
                height={plotH}
                tabIndex={0}
                aria-label={`${p.tooltip.detail}: ${p.tooltip.value}`}
                onPointerEnter={() => setActive(i)}
                onPointerLeave={() => setActive(null)}
                onFocus={() => setActive(i)}
                onBlur={() => setActive(null)}
              />
            </g>
          ))}
        </svg>
      )}
      {activePoint !== undefined && active !== null && (
        <Tooltip text={activePoint.tooltip} x={x(active)} width={width} />
      )}
    </div>
  );
}
