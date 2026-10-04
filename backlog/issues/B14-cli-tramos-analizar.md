---
id: B14
title: CLI `tramos analizar`
milestone: M1 Núcleo de datos
labels: area:cli, tipo:feature, listo-para-agente
depends: B09, B12, B13
---
## Contexto

Permite probar todo el núcleo sin interfaz; los agentes la usan para comprobar su trabajo.

## Qué hacer

- `tramos analizar --spl <f> --fit <f> [--corredor <nombre|si>] [--umbral-s 15 --umbral-pct 10]`.
- Salida JSON con la carrera, el recorrido, los tramos con split, referencia, IR, pérdida, error y métricas del FIT.
- `--formato tabla` para leerlo en la terminal.

## Criterios de aceptación

- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test --workspace` pasan
- [ ] Test de integración con los fixtures que compara con un JSON esperado.

## Depende de

B09, B12, B13
