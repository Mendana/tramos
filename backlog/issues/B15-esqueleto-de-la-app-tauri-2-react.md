---
id: B15
title: Esqueleto de la app Tauri 2 + React
milestone: M2 Esqueleto de la app
labels: area:app, tipo:infra, listo-para-agente
depends: B01, B02
---
## Contexto

La app solo llama al núcleo; no reimplementa cálculos.

## Qué hacer

- Crear `app/` con Tauri 2, React, TypeScript y Vite.
- Un comando Tauri de ejemplo que devuelva la versión del núcleo.
- Añadir a la CI la compilación de la app en Linux y Windows (sin publicar).

## Criterios de aceptación

- [ ] `npm run tauri dev` abre una ventana que muestra la versión del núcleo.
- [ ] La CI compila la app en ambos sistemas.

## Depende de

B01, B02
