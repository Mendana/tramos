---
id: B06
title: Lector de .spl en Rust con test de paridad
milestone: M1 Núcleo de datos
labels: area:core, tipo:feature, listo-para-agente
depends: B03, B05
---
## Contexto

Portar el lector de referencia `tools/reference/winsplits_spl.py` según `docs/formato-spl.md`.

## Qué hacer

- Módulo `importers::spl` que lea un `.spl` al modelo de dominio.
- Convertir las horas (centésimas, hora local) a UTC con la zona Europe/Madrid y la fecha de la carrera.
- Descartar la fecha de nacimiento.
- Fallar con un error claro ante etiquetas desconocidas; tolerar el último registro truncado.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test de paridad: la salida coincide con `fixtures/spl/chinchon-anon.expected.json` (13 categorías, 54 corredores, mismos splits).
- [ ] Test con un fichero corrupto que devuelve error, no pánico.

## Depende de

B03, B05
