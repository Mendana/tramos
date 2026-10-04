---
id: B31
title: Errores más comunes, cruzables (P9)
milestone: M5 Tanda 3: comportamiento y etiquetas
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-3
depends: B30, B28
---
## Contexto

Ver `docs/preguntas.md`, P9.

## Qué hacer

- Reparto por tipo y subtipo, cruzable con cubos de duración y formato.
- Mostrar el porcentaje de errores sin tipo.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test del cruce con etiquetas sintéticas.

## Depende de

B30, B28
