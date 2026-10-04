---
id: B34
title: ¿El cansancio anticipa el error? (P14)
milestone: M5 Tanda 3: comportamiento y etiquetas
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-3
depends: B13, B30
---
## Contexto

Dato débil con pulso de muñeca; la interfaz debe decirlo. Ver `docs/preguntas.md`, P14.

## Qué hacer

- Deriva pulso/velocidad por tercio.
- Pulso antes de error frente a antes de tramo limpio en la misma fase.
- Esfuerzo percibido de las etiquetas.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] La vista muestra el aviso de dato débil y el número de casos.

## Depende de

B13, B30
