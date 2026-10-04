---
id: B18
title: Importar una carrera
milestone: M2 Esqueleto de la app
labels: area:app, area:ui, tipo:feature, listo-para-agente
depends: B14, B15, B16
---
## Contexto

Flujo principal: el corredor arrastra el .spl y el FIT de una carrera.

## Qué hacer

- Arrastrar y soltar o elegir ficheros; FIT opcional (sin FIT, solo análisis de splits).
- Elegir corredor si la identificación es ambigua.
- Sugerir el formato (sprint, media, larga) por la duración del ganador y pedir confirmación.
- Guardar originales y resultados; avisar de los problemas de alineación.

## Criterios de aceptación

- [ ] Importar los fixtures deja una carrera visible en la lista.
- [ ] Reimportar la misma carrera no la duplica.

## Depende de

B14, B15, B16
