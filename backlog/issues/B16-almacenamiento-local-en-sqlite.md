---
id: B16
title: Almacenamiento local en SQLite
milestone: M2 Esqueleto de la app
labels: area:core, tipo:feature, listo-para-agente
depends: B05
---
## Contexto

Todo se guarda en local (ver `docs/datos-y-privacidad.md`).

## Qué hacer

- Crate `crates/tramos-store` con `rusqlite` y migraciones versionadas.
- Tablas: corredores, carreras, recorridos, tramos (con resultados del análisis y versión del algoritmo), etiquetas, ficheros originales (blob o ruta) y ajustes.
- Documentar el esquema en `docs/almacenamiento.md`.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Tests de migración en limpio y de ida y vuelta de una carrera completa.

## Depende de

B05
