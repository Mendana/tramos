# Zonas del mapa

En el [Mapa](carrera-mapa.md) de una carrera, el track se colorea por **ritmo** (min/km) o por
**pulso** (pulsaciones por minuto). Hay dos maneras de elegir los colores, en
[Ajustes](ajustes.md), «Colores del mapa», por separado para ritmo y para pulso.

## Por cuantiles de cada carrera (lo de entrada)

La app reparte **esa carrera** en cinco tonos de azul que ocupan más o menos el mismo tiempo cada
uno: del más claro (lo más rápido, o el pulso más bajo) al más oscuro (lo más lento, o el pulso
más alto). La leyenda de debajo del mapa dice dónde empieza cada tono.

Enseña dónde fuiste más despacio **en esa carrera**, sea cual sea el terreno o tu forma ese día.
Pero el mismo tono no significa lo mismo en dos carreras.

> En un sprint, el azul más oscuro empieza en 5:30 min/km; en una larga por monte, en 9:00. En las
> dos, el más oscuro son los trozos en los que fuiste más despacio.

## Mis zonas

Tú pones los límites y los colores, y **el mismo color significa lo mismo en todas las carreras**.
Al elegir «Mis zonas» se proponen cinco (pulso: desde 120, 140, 160 y 175; ritmo: desde 4:30,
5:30, 6:30 y 8:00 min/km), que puedes cambiar. Puedes tener de 2 a 10 zonas. Un valor justo en un
límite va a la zona de arriba.

> Con zonas de pulso desde 120, 140 y 160: 119 ppm es la primera zona, 120 la segunda y 165 la
> cuarta. Si en todas tus carreras el final sale en el color de 160 o más, vas al límite al final.

- No se guardan si los límites no van de menor a mayor o alguno no es un número mayor que 0.
- Mientras editas, avisa si un color **se ve poco** sobre el fondo del mapa o si dos zonas
  seguidas tienen **colores muy parecidos**. Son avisos: puedes guardar igual.

## Lo que no tiene color

Los **huecos** (más de 10 s sin datos del reloj) van en gris discontinuo. Por debajo de 0,5 m/s
(parado) el ritmo cuenta como 20:00 min/km. Con pulso, los trozos sin pulso también van en gris.

Al ver a un [atleta](atletas.md), el mapa sale siempre por cuantiles: las zonas de cada
corredor no viajan con sus carreras.

Detalle técnico: [app, «Mapa»](https://github.com/Mendana/tramos/blob/main/docs/app.md#mapa) y
[«Ajustes»](https://github.com/Mendana/tramos/blob/main/docs/app.md#ajustes).
