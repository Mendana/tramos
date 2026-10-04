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
| Físico | Bajada de ritmo por cansancio, desnivel o terreno, sin error de orientación |
| Otro | Problema con la picada; caída; seguir a otro corredor; distracción |

## Causas percibidas

Precipitación, falta de concentración, cansancio, planificación insuficiente, otros corredores,
mapa poco legible.

## Implementación

La taxonomía vive en un fichero de datos versionado (no en el código), para poder cambiarla sin
recompilar. Cada etiqueta guarda la versión de la taxonomía con la que se creó.
