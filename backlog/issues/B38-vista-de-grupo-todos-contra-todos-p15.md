---
id: B38
title: Vista de grupo: todos contra todos (P15)
milestone: M6 Tanda 4: entrenadora
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-4
depends: B37, B28, B29, B31
---
## Contexto

Ver `docs/preguntas.md`, P15.

## Qué hacer

- Tabla de corredores por métricas: IR, tasa de error, tipos de error, P7 y P13.
- Comparación de todos contra todos en las carreras compartidas.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test del agregado con tres corredores sintéticos.

## Depende de

B37, B28, B29, B31
