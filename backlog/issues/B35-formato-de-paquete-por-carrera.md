---
id: B35
title: Formato de paquete por carrera
milestone: M6 Tanda 4: entrenadora
labels: area:core, tipo:feature, listo-para-agente, tanda-4
depends: B16
---
## Contexto

Unidad de intercambio corredor → entrenadora. Debe servir igual el día que haya servidor.

## Qué hacer

- Especificar en `docs/paquete.md`: versión, IDs estables de corredor y carrera, nivel de permiso (agregados, tramos, track completo) y contenido por nivel.
- Exportar e importar; reexportar sustituye al anterior.
- Si el paquete trae originales, la app destino recalcula con su versión del algoritmo.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test de ida y vuelta por cada nivel de permiso.
- [ ] Reimportar una versión nueva no duplica.

## Depende de

B16
