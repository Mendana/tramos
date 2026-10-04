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
