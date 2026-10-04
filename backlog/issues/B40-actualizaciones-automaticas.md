---
id: B40
title: Actualizaciones automáticas
milestone: M7 Distribución
labels: area:app, tipo:infra, necesita-humano
depends: B39
---
## Contexto

Que el grupo reciba versiones nuevas sin reinstalar a mano.

## Qué hacer

- Configurar el plugin de actualizaciones de Tauri contra GitHub Releases, con su clave de firma de actualizaciones guardada como secreto.
- La persona genera la clave y la añade a los secretos del repositorio.

## Criterios de aceptación

- [ ] Una versión instalada detecta y aplica la siguiente release.

## Depende de

B39
