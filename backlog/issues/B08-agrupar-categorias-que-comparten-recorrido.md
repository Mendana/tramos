---
id: B08
title: Agrupar categorías que comparten recorrido
milestone: M1 Núcleo de datos
labels: area:core, tipo:feature, listo-para-agente
depends: B06
---
## Contexto

El tiempo perdido se calcula por recorrido, no por categoría (ver `docs/tiempo-perdido.md`). En Chinchón, 13 categorías comparten 6 recorridos.

## Qué hacer

- Función que agrupe categorías por secuencia idéntica de códigos de baliza.
- Cada grupo conserva la lista de categorías que contiene.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test con el fixture: 6 recorridos y la agrupación esperada.

## Depende de

B06
