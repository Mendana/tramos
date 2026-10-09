# Pestaña Análisis

Cuatro paneles para entender cómo fue la carrera de principio a fin.

## Pérdida acumulada

El tiempo perdido sumado tramo a tramo desde la salida. Solo sube en los tramos con error,
marcados con un punto, y acaba en el tiempo perdido de la carrera. Se ve si los errores llegaron
al principio, al final o repartidos.

## Dónde gano y dónde pierdo

Para cada tramo, cuánto ganaste o perdiste frente a lo que te tocaba con tu ritmo de ese día:
hacia arriba en azul si ganaste, hacia abajo en naranja si perdiste. Encima, una línea con el
acumulado. Las **rachas** de dos o más tramos seguidos perdiendo van sobre una franja de color.

A diferencia de la pérdida acumulada, aquí cuentan **todos** los tramos, no solo los errores: es
«voy tantos segundos por delante o por detrás de lo que me tocaba».

> Pierdes 8 s, ganas 2 s, pierdes 1,5 s, 20 s y 0,5 s, ganas 4 s y pierdes 3 s y 6 s. El
> acumulado acaba en −33 s. Hay dos rachas: tramos 3 a 5 (22 s) y 7 a 8 (9 s). El tramo 1 pierde
> solo, y uno solo no es racha.

## Rendimiento por tramo

El [IR](ir.md) de cada tramo como columna, con la línea del 100 % (ir como los mejores). Los
tramos muy por debajo de tus columnas habituales son los que se te torcieron.

## ¿Lento o desorientado?

Reparte lo que perdiste en cada error en tres partes: **desvío** (metros de más), **paradas** y
**ritmo** (ir más despacio de lo normal por el mismo camino). Necesita el FIT del reloj y al menos
3 tramos sin error con track. Explicado en [¿Lento o desorientado?](lento-o-desorientado.md).

Detalle técnico:
[tiempo perdido, «Dónde gano y dónde pierdo»](https://github.com/Mendana/tramos/blob/main/docs/tiempo-perdido.md#dónde-gano-y-dónde-pierdo-p5).
