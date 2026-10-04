---
id: B26
title: Consistencia (P10)
milestone: M3 Tanda 1: splits
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-1
depends: B25
---
## Contexto

Ver `docs/preguntas.md`, P10.

## Qué hacer

- Desviación típica ponderada de IR por carrera.
- Valor en la vista de carrera y serie en la vista histórica.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test con valores calculados a mano.

## Depende de

B25
