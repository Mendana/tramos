---
id: B12
title: Tiempo perdido con umbral configurable (P1)
milestone: M1 Núcleo de datos
labels: area:core, tipo:feature, listo-para-agente, tanda-1
depends: B08
---
## Contexto

Núcleo del producto. Algoritmo completo en `docs/tiempo-perdido.md`.

## Qué hacer

- Referencia por tramo (media del 25 % más rápido), índice de rendimiento, rendimiento habitual (mediana ponderada), tiempo esperado, pérdida y error con umbral en segundos y porcentaje.
- Tiempo sin errores, puesto por tramo y diferencia acumulada respecto al tiempo ideal.
- Marcar las exclusiones (último tramo, referencia < 20 s) y la "referencia débil" (< 4 corredores).

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Tests con casos pequeños calculados a mano (incluido el ejemplo del documento).
- [ ] Test sobre el fixture comparado con un `.expected.json` revisado por una persona.

## Depende de

B08
