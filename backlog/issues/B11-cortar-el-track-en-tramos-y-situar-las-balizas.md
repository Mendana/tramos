---
id: B11
title: Cortar el track en tramos y situar las balizas
milestone: M1 Núcleo de datos
labels: area:core, tipo:feature, listo-para-agente
depends: B10
---
## Contexto

Cada baliza se sitúa con la posición GPS en el instante de su picada; no hay mapa en el MVP.

## Qué hacer

- Cortar el `Track` en sub-tracks por tramo según los instantes de picada alineados.
- Posición de cada baliza: la del corredor en el instante de picada (interpolada).
- Preparar la función para combinar varios corredores del mismo recorrido con la mediana (se usará cuando haya datos del grupo).

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Con el FIT sintético, posiciones a menos de 10 m de `truth.json` y número de tramos correcto.

## Depende de

B10
