# Pestaña Tramos

La tabla de tramos de la carrera y donde se etiquetan los errores. La pestaña lleva un número con
los errores que quedan por revisar.

## La tabla

Una fila por tramo, de la salida (S) a la meta (M):

| Columna         | Qué es                                                                                                                        |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Tramo y balizas | El número de tramo y de qué baliza a qué baliza va.                                                                           |
| Split           | Lo que tardaste en ese tramo.                                                                                                 |
| Puesto          | Tu puesto en ese tramo entre los clasificados de tu recorrido.                                                                |
| Referencia      | Lo que tardaron los mejores: la media del 25 % más rápido.                                                                    |
| IR              | Tu [índice de rendimiento](ir.md): referencia entre tu split. 100 % es ir como los mejores.                                   |
| Pérdida y %     | Lo que perdiste frente a lo que te tocaba con tu ritmo de ese día ([Tiempo perdido](tiempo-perdido.md)). Negativo si ganaste. |
| Notas           | Último tramo, referencia corta, el tipo de error si lo etiquetaste y los botones para etiquetar.                              |

Los tramos con error van resaltados. **Solo errores** deja solo los propuestos y los que hayas
marcado como error. Un clic en una fila la selecciona, también en el mapa; otro clic la quita.

## Etiquetar un error

La app **propone** como error los tramos que pasan los umbrales. Tú decides si lo fueron. Ninguna
etiqueta es obligatoria, pero con ellas las [Estadísticas](estadisticas.md) saben qué te pasa.

1. **¿Fue un error?** En cada tramo propuesto, **Sí**, **No** o **Físico** (perdiste tiempo, pero
   no por orientarte mal: cansancio, cuesta, terreno). Un clic guarda; otro clic en la marcada la
   quita. Arriba, un contador dice cuántos propuestos llevas revisados.
2. **Tipo y contexto**: el lápiz de la fila abre el formulario completo: tipo y subtipo de error,
   causas, en qué parte del tramo, cuántos segundos crees que perdiste, el esfuerzo (1 a 10) y una
   nota. **Guardar** lo guarda y **Quitar etiqueta** lo borra.

El lápiz está en todos los tramos: sirve también para apuntar un error que la app no propuso.

> El tramo 7 sale propuesto: perdiste 48 s. Recuerdas que saliste de la baliza hacia el lado
> contrario. Marcas **Sí** y, con el lápiz, eliges «Salida de baliza → Salir en mala dirección»,
> causa «Precipitación», parte del tramo «Salida».

Qué tipos hay y cómo elegir: [Tipos de error](tipos-de-error.md).

Al ver a un atleta, las etiquetas se ven como texto, sin botones.

Detalle técnico:
[app, «Etiquetar errores»](https://github.com/Mendana/tramos/blob/main/docs/app.md#etiquetar-errores-30).
