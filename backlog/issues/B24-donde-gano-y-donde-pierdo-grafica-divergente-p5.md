---
id: B24
title: Dónde gano y dónde pierdo: gráfica divergente (P5)
milestone: M3 Tanda 1: splits
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-1
depends: B21
---
## Contexto

Ver `docs/preguntas.md`, P5.

## Qué hacer

- Calcular `g_i = esp_i − t_i` por tramo y las rachas de dos o más tramos seguidos perdiendo.
- Barras por encima y por debajo del eje a lo largo de la carrera, más la línea acumulada.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test del cálculo de rachas.

## Depende de

B21
