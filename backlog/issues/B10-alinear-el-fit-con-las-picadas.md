---
id: B10
title: Alinear el FIT con las picadas
milestone: M1 Núcleo de datos
labels: area:core, tipo:feature, listo-para-agente
depends: B06, B07
---
## Contexto

Los splits van en hora local y el FIT en UTC; además el reloj y el cronometraje pueden diferir unos segundos.

## Qué hacer

- Comprobar que el track cubre la ventana salida–meta del corredor.
- Estimar un desfase fino (±60 s) que minimice paradas o cambios de dirección cerca de cada picada; documentar el método en `docs/alineacion.md`.
- Devolver avisos legibles si no cuadra (track que no cubre la carrera, desfase excesivo).

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Con el FIT sintético desplazado 7 s, el desfase estimado está a ±2 s.
- [ ] Test con cambio de hora (carrera en horario de invierno).

## Depende de

B06, B07
