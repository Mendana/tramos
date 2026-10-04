---
id: B28
title: Pérdida según duración del tramo (P7)
milestone: M4 Tanda 2: FIT
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-2
depends: B12, B25
---
## Contexto

Cubos logarítmicos por tiempo de referencia. Ver `docs/preguntas.md`, P7.

## Qué hacer

- Asignar cubo por `ref_i` con exclusiones.
- Panel histórico: tasa de error, pérdida media y n por cubo, filtrable por formato.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test de asignación en los límites de cada cubo.

## Depende de

B12, B25
