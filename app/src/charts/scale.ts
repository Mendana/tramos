// Escalas para las gráficas en SVG (docs/app.md, "Gráficas").

/** Escala lineal de `domain` a `range`. */
export function linear(domain: [number, number], range: [number, number]) {
  const [d0, d1] = domain;
  const [r0, r1] = range;
  const span = d1 - d0 || 1;
  return (value: number) => r0 + ((value - d0) / span) * (r1 - r0);
}

/** Paso «redondo» (1, 2 o 5 por una potencia de 10) para unas `count` divisiones de `span`. */
function niceStep(span: number, count: number): number {
  const raw = span / Math.max(count, 1);
  const power = 10 ** Math.floor(Math.log10(raw));
  const fraction = raw / power;
  const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10;
  return nice * power;
}

/**
 * Dominio ampliado a valores redondos que contiene `min`, `max` y el 0, con sus marcas.
 * Así las barras crecen siempre desde la línea del 0.
 */
export function niceDomain(
  min: number,
  max: number,
  count = 5,
): { domain: [number, number]; ticks: number[] } {
  const lo = Math.min(0, min);
  const hi = Math.max(0, max);
  if (lo === hi) return { domain: [0, 1], ticks: [0, 1] };
  const step = niceStep(hi - lo, count);
  const start = Math.floor(lo / step) * step;
  const end = Math.ceil(hi / step) * step;
  const ticks: number[] = [];
  for (let t = start; t <= end + step / 2; t += step) {
    // Evita -0 y restos de coma flotante en las etiquetas.
    ticks.push(Math.abs(t) < step / 1e6 ? 0 : Number(t.toPrecision(12)));
  }
  return { domain: [start, end], ticks };
}

/**
 * Marcas «redondas» de `min` a `max` (unas `count`): con el paso entre 1, 2 y 5 por una potencia
 * de 10 o, si se dan, el menor de `steps` con el que no salen más de `count`.
 */
export function niceTicks(min: number, max: number, count = 5, steps?: number[]): number[] {
  if (!(max > min)) return [min];
  const span = max - min;
  const step =
    steps === undefined
      ? niceStep(span, count)
      : (steps.find((s) => span / s <= count) ?? steps[steps.length - 1]);
  const ticks: number[] = [];
  for (let t = Math.ceil(min / step) * step; t <= max + step / 1e6; t += step) {
    ticks.push(Number(t.toPrecision(12)));
  }
  return ticks;
}

/**
 * Dominio redondo que contiene `min` y `max` sin forzar el 0 (para magnitudes que no parten de
 * él, como la altitud o el pulso), con sus marcas. Un valor constante da un dominio de ±1 en
 * torno a él. Con `steps`, el paso es el menor de ellos con el que no salen más de `count`
 * marcas (p. ej. 15 o 30 s para un ritmo).
 */
export function niceRange(
  min: number,
  max: number,
  count = 4,
  steps?: number[],
): { domain: [number, number]; ticks: number[] } {
  if (!(max > min)) {
    const mid = Number.isFinite(min) ? min : 0;
    return { domain: [mid - 1, mid + 1], ticks: [mid - 1, mid, mid + 1] };
  }
  const span = max - min;
  const step = steps?.find((s) => span / s <= count) ?? niceStep(span, count);
  const start = Math.floor(min / step) * step;
  const end = Math.ceil(max / step) * step;
  const ticks: number[] = [];
  for (let t = start; t <= end + step / 2; t += step) ticks.push(Number(t.toPrecision(12)));
  return { domain: [start, end], ticks };
}
