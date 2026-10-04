---
id: B02
title: CI en GitHub Actions: formato, clippy y tests en Linux y Windows
milestone: M0 Cimientos
labels: area:ci, tipo:infra, listo-para-agente
depends: B01
---
## Contexto

Los agentes dependen de la CI para saber si su PR está bien. Los usuarios usan Windows, así que hay que probar ahí también.

## Qué hacer

- Workflow que se ejecute en cada push y PR.
- Matriz `ubuntu-latest` y `windows-latest`.
- Pasos: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`.
- Caché de cargo.

## Criterios de aceptación

- [ ] Un PR con un error de formato hace fallar la CI.
- [ ] La CI pasa en `main` en ambos sistemas.

## Depende de

B01
