---
id: B01
title: Workspace de Rust con el núcleo y la CLI
milestone: M0 Cimientos
labels: area:core, tipo:infra, listo-para-agente
depends: 
---
## Contexto

Punto de partida del código. Ver la estructura en `CLAUDE.md`.

## Qué hacer

- Crear el workspace con `crates/tramos-core` (lib) y `crates/tramos-cli` (bin `tramos`).
- Configurar `rustfmt.toml` y lints de clippy a nivel de workspace.
- Añadir dependencias base: `serde`, `chrono`, `thiserror` (core) y `anyhow`, `clap` (cli).
- `tramos --version` funciona.

## Criterios de aceptación

- [ ] `cargo build --workspace` funciona en limpio.
- [ ] Hay al menos un test de humo en cada crate.

## Depende de

Nada.
