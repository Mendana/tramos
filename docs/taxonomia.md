# Taxonomía de errores (versión 0, se refina con el uso)

El corredor etiqueta; la entrenadora solo consulta. Ningún nivel es obligatorio: sin etiquetas la
app sigue calculando tiempo perdido, fase de carrera y ritmo.

## Niveles

1. **Confirmar**: ¿hubo error? Sí, no o "fue físico". Un clic por tramo propuesto.
2. **Tipo y subtipo**: de la tabla.
3. **Contexto**: causas percibidas (varias), parte del tramo (salida, mitad, ataque), segundos que
   cree haber perdido, esfuerzo percibido (1–10) y nota libre.

## Tipos y subtipos

| Tipo | Subtipos |
| --- | --- |
| Salida de baliza | Salir en mala dirección; no planificar antes de picar |
| Elección de ruta | Ruta más lenta; no ver la alternativa; mal cálculo de desnivel o terreno |
| Navegación | Dirección o rumbo; paralelo; pasarse; perder contacto con el mapa |
| Ataque | Mal punto de ataque; precipitación al final; zona confusa junto a la baliza |
| Físico | Bajada de ritmo sin error de orientación: cansancio; desnivel o terreno |
| Otro | Problema con la picada; caída; seguir a otro corredor; distracción |

## Causas percibidas

Precipitación, falta de concentración, cansancio, planificación insuficiente, otros corredores,
mapa poco legible.

## Implementación

La taxonomía vive en un fichero de datos versionado, `crates/tramos-core/data/taxonomy.json`, y no
en el código: cambiar tipos, subtipos o causas es editar ese fichero, sin tocar código. De momento
la app lo incluye al compilar (`tramos_core::taxonomy::Taxonomy::builtin`), así que un cambio
llega con la siguiente versión de la app; leerlo de fuera queda para cuando haga falta.

El fichero es la fuente de verdad de las tablas de arriba. Lleva una `version` y, para cada tipo,
subtipo y causa, una **clave** estable en inglés (`navigation`, `parallel`, `rush`…) y el nombre
que se muestra. Al leerlo se comprueba que hay versión, que ninguna clave ni ningún nombre está
vacío y que no se repiten claves (los tipos y las causas entre sí, los subtipos dentro de su tipo).

- Las etiquetas guardan las **claves**, no los nombres: se puede cambiar un nombre sin tocar las
  etiquetas. Quitar o renombrar una clave deja las etiquetas antiguas con una clave que ya no
  existe; la app la muestra tal cual. Una versión nueva del fichero debería conservar las claves.
- Cada etiqueta guarda la `version` de la taxonomía con la que se escribió por última vez.
- Antes de guardar, la etiqueta se comprueba contra la taxonomía (`Taxonomy::validate`): el tipo
  existe, el subtipo es de ese tipo (no hay subtipo sin tipo), las causas existen y no se repiten,
  el esfuerzo va de 1 a 10 y los segundos perdidos son un número finito y no negativo.
- Nivel 1 y parte del tramo no son de la taxonomía sino fijos: `error`, `no_error`, `physical` y
  `start`, `middle`, `attack`.
- «Físico» aparece dos veces a propósito: como respuesta del nivel 1 (perdí tiempo, pero no por
  orientarme mal) y como tipo, para quien quiera precisar la causa. No se enlazan: elegir uno no
  rellena el otro.
- Dónde y cómo se etiqueta en la app: `docs/app.md`, «Etiquetar errores».
