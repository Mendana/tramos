---
id: B32
title: Comportamiento después de fallar (P8)
milestone: M5 Tanda 3: comportamiento y etiquetas
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-3
depends: B13, B25
---
## Contexto

Ver `docs/preguntas.md`, P8: encadenamiento, recuperación acelerada y rachas limpias.

## Qué hacer

- Calcular las tres métricas sobre el histórico.
- Panel con las tasas comparadas y el número de casos.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Tests con secuencias de errores sintéticas.

## Depende de

B13, B25
