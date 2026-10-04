---
id: B30
title: Etiquetado de errores en tres niveles
milestone: M5 Tanda 3: comportamiento y etiquetas
labels: area:core, area:ui, tipo:feature, listo-para-agente, tanda-3
depends: B19
---
## Contexto

Ver `docs/taxonomia.md`. Ningún nivel es obligatorio.

## Qué hacer

- Cargar la taxonomía desde un fichero de datos versionado.
- En la vista de carrera: confirmar los tramos propuestos con un clic; tipo y subtipo; contexto (causas, parte del tramo, segundos, esfuerzo, nota).
- Añadir errores en tramos no propuestos.
- Guardar la versión de la taxonomía en cada etiqueta.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Etiquetar una carrera completa a nivel 1 lleva menos de un minuto con el fixture.

## Depende de

B19
