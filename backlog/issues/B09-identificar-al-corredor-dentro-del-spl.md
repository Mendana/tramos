---
id: B09
title: Identificar al corredor dentro del .spl
milestone: M1 Núcleo de datos
labels: area:core, tipo:feature, listo-para-agente
depends: B06
---
## Contexto

Al importar hay que saber quién es el usuario dentro de la carrera.

## Qué hacer

- Buscar por tarjeta SI configurada; si no hay, por nombre normalizado (sin acentos, mayúsculas, espacios dobles).
- Si hay varios candidatos o ninguno, devolver la lista para que la interfaz pregunte.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Tests con nombres con y sin acentos y con tarjeta.

## Depende de

B06
