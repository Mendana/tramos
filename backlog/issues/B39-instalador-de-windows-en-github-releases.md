---
id: B39
title: Instalador de Windows en GitHub Releases
milestone: M7 Distribución
labels: area:ci, tipo:infra, listo-para-agente
depends: B15
---
## Contexto

Los usuarios tienen Windows y el desarrollo se hace en Linux: el instalador se genera en la CI.

## Qué hacer

- Workflow con la acción oficial de Tauri que, al crear un tag `v*`, compile y publique el instalador de Windows en una release.
- Documentar en el README cómo instalar y el aviso de SmartScreen mientras no esté firmado.

## Criterios de aceptación

- [ ] Un tag de prueba genera una release con el instalador.

## Depende de

B15
