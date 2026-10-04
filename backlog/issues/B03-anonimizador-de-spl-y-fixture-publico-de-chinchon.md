---
id: B03
title: Anonimizador de .spl y fixture público de Chinchón
milestone: M0 Cimientos
labels: area:tools, tipo:infra, necesita-humano
depends: 
---
## Contexto

El .spl real trae nombres, clubes, tarjetas y fechas de nacimiento y no puede subirse al repo (ver `docs/datos-y-privacidad.md`). Necesitamos un fixture público con la misma estructura y los mismos tiempos.

## Qué hacer

- Herramienta en Python (`tools/anonimizar_spl.py`), apoyada en `tools/reference/winsplits_spl.py`, que reescriba en el binario: nombres y apellidos por seudónimos de la misma longitud, clubes por "Club A", "Club B"…, tarjeta SI y dorsal por números secuenciales y fecha de nacimiento a 0.
- Los tiempos, códigos, categorías y estados no cambian.
- La persona ejecuta la herramienta sobre el .spl real (en `fixtures/private/`) y sube el resultado a `fixtures/spl/chinchon-anon.spl`, junto a su `.expected.json` generado con el lector de referencia.

## Criterios de aceptación

- [ ] El fichero anonimizado se lee con el lector de referencia sin errores.
- [ ] Splits y puestos idénticos al original; ningún nombre, club ni fecha real en el fichero (comprobado con `strings`).

## Depende de

Nada.
