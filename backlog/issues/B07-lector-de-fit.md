---
id: B07
title: Lector de FIT
milestone: M1 Núcleo de datos
labels: area:core, tipo:feature, listo-para-agente
depends: B04, B05
---
## Contexto

El FIT del reloj aporta GPS, altitud, pulso y cadencia por segundo.

## Qué hacer

- Módulo `importers::fit` con el crate `fitparser` (licencia MIT).
- Extraer los mensajes `record` a `Track` con instantes UTC; coordenadas en grados.
- Tolerar campos ausentes (sin pulso, sin cadencia).

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test con el FIT sintético de `fixtures/fit/`: número de puntos, primer y último instante y que hay pulso.
- [ ] Test opcional con FIT reales de `fixtures/private/` que se salta si no existen.

## Depende de

B04, B05
