---
id: B27
title: ¿Lento o desorientado? (P2)
milestone: M4 Tanda 2: FIT
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-2
depends: B13, B21
---
## Contexto

Heurística inicial en `docs/preguntas.md`, P2. Es la pregunta más interpretativa: dejar los parámetros en un solo sitio.

## Qué hacer

- Descomponer la pérdida de cada tramo en desvío, paradas y ritmo.
- Panel por carrera y agregado en la vista histórica.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Con el FIT sintético, el tramo del rodeo atribuye la mayor parte de su pérdida a desvío y parada.

## Depende de

B13, B21
