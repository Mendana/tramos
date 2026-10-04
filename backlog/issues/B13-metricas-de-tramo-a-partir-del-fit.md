---
id: B13
title: Métricas de tramo a partir del FIT
milestone: M1 Núcleo de datos
labels: area:core, tipo:feature, listo-para-agente
depends: B11
---
## Contexto

Base de P2, P8, P13 y P14 (ver `docs/preguntas.md`).

## Qué hacer

- Por tramo: distancia recorrida, línea recta entre balizas, velocidad en movimiento, tiempo parado (< 0,5 m/s, sin los 5 s tras la picada), subida y bajada con altitud suavizada, pulso medio y cadencia media.
- Documentar el suavizado de altitud en `docs/preguntas.md`.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Con el FIT sintético: el tramo del rodeo tiene ratio distancia/línea recta alto y la parada mide 30 ± 2 s.

## Depende de

B11
