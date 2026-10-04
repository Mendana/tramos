---
id: B33
title: Días sin competir (P11)
milestone: M5 Tanda 3: comportamiento y etiquetas
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-3
depends: B25
---
## Contexto

Ver `docs/preguntas.md`, P11.

## Qué hacer

- Días desde la carrera anterior importada, en cubos.
- IR de los tres primeros tramos y tasa de error del primer tercio por cubo.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test con fechas sintéticas.

## Depende de

B25
