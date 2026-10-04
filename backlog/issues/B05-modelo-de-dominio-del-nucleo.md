---
id: B05
title: Modelo de dominio del núcleo
milestone: M0 Cimientos
labels: area:core, tipo:feature, listo-para-agente
depends: B01
---
## Contexto

Tipos compartidos por importadores, análisis y app. Glosario en `CLAUDE.md`.

## Qué hacer

- Tipos: `Event` (fecha, nombre), `Class`, `Course` (secuencia de códigos), `Runner`, `Punch` (código, instante UTC opcional), `RaceResult` (estado, puesto, picadas), `Track` y `TrackPoint` (instante, lat, lon, altitud, pulso, cadencia, distancia).
- `Leg` con índice, desde, hasta y split opcional.
- Derivar `serde` en todo; documentar en `docs/modelo.md`.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] `docs/modelo.md` describe cada tipo en una línea.

## Depende de

B01
