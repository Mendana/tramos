---
id: B29
title: Pérdida según desnivel (P13)
milestone: M4 Tanda 2: FIT
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-2
depends: B13, B25
---
## Contexto

Ver `docs/preguntas.md`, P13.

## Qué hacer

- Clasificar tramos en subida, bajada y llano con umbral configurable.
- Panel histórico con IR medio, tasa de error y n por clase.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test de clasificación con perfiles sintéticos.

## Depende de

B13, B25
