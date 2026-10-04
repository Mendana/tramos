---
id: B04
title: Generador de FIT sintético coherente con el fixture de Chinchón
milestone: M0 Cimientos
labels: area:tools, tipo:infra, listo-para-agente
depends: B03
---
## Contexto

Los FIT reales no pueden subirse. Un FIT sintético que pase por posiciones inventadas de las balizas en los instantes de las picadas de un corredor del fixture da un test de extremo a extremo con la respuesta conocida.

## Qué hacer

- Herramienta en `tools/` que, dado el fixture .spl y un corredor, invente coordenadas para cada baliza del recorrido y genere un track a 1 Hz que pase por cada baliza en su hora de picada (convertida a UTC), con pulso, cadencia y altitud plausibles.
- Introducir al menos un "error" conocido: un tramo con un rodeo y una parada de 30 s.
- Escribir el FIT con una librería de licencia compatible con MIT y guardar también las coordenadas inventadas en `fixtures/fit/<nombre>.truth.json`.
- Subir el FIT generado a `fixtures/fit/`.

## Criterios de aceptación

- [ ] El FIT se abre con el SDK de FIT o con `fitparser` sin errores.
- [ ] `truth.json` contiene posiciones de balizas, tramo del rodeo y duración de la parada.

## Depende de

B03
