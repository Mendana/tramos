---
id: B25
title: Vista histórica y análisis por formato (P6)
milestone: M3 Tanda 1: splits
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-1
depends: B21
---
## Contexto

Primera vista sobre todas las carreras. Ver `docs/preguntas.md`, P6.

## Qué hacer

- Vista histórica con filtros por fechas y formato.
- Por formato: IR medio, tasa de error, pérdida media por tramo y número de carreras.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Tests del agregado con varias carreras sintéticas.

## Depende de

B21
