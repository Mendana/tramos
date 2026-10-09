# Pestaña Mapa

Tu track del reloj sobre un mapa, tramo a tramo, con las balizas donde estabas al picarlas. Solo
si importaste la carrera con el FIT del reloj; si no, la pestaña te dice que la vuelvas a
[importar](importar.md) con él.

## Qué se ve

- **El track**, coloreado por **ritmo** o por **pulso** (el control _Ritmo / Pulso_; pulso solo si
  el reloj lo grabó). Los tonos se explican en la leyenda de debajo y en
  [Zonas del mapa](zonas-del-mapa.md). Los huecos sin datos van en gris discontinuo.
- **Las balizas**, como en un mapa de orientación: triángulo en la salida, círculo con el número
  de orden en cada baliza y doble círculo en la meta. Si una cae en un hueco del track, va con
  trazo discontinuo.
- **La lista de tramos** a la derecha, con su split y su pérdida. Un clic en un tramo (en la lista
  o en el mapa) lo selecciona: se resalta en magenta, el resto se atenúa y el mapa se encuadra en
  él. Otro clic lo quita.
- Botones para acercar, alejar y ver toda la carrera.

> Seleccionas el tramo 12, en el que perdiste 1:10. El track hace una curva grande hacia el norte
> antes de llegar a la baliza y, en ese trozo, el color pasa al azul más oscuro: ahí ibas más
> despacio que en el resto de la carrera, buscando.

## Reloj y cronometraje

Encima del mapa, plegado, el **desfase** entre la hora de tu reloj y la del cronometraje. Si las
balizas no caen donde estaban, se corrige aquí. Se despliega solo, con «Revisar», si la app no
está segura. Explicado en [Reloj y desfase](reloj.md).

## El fondo del mapa

No hay mapa de orientación: el fondo es OpenStreetMap. Tu track y tus balizas se dibujan en tu
equipo y no salen de él, pero para pintar el fondo la app pide a los servidores de OpenStreetMap
los trozos de mapa de la zona que miras: quien gestione esos servidores puede saber qué zona estás
mirando. Si la carrera no tiene track, no se pide nada.

Detalle técnico: [app, «Mapa»](https://github.com/Mendana/tramos/blob/main/docs/app.md#mapa).
