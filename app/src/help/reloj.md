# Reloj y desfase

El .spl dice a qué hora picaste cada baliza según el **cronometraje** de la carrera. El FIT dice
dónde estabas a cada hora según **tu reloj**. Para saber dónde estabas al picar, la app junta las
dos cosas. Pero los dos relojes nunca marcan exactamente la misma hora: esa diferencia es el
**desfase**.

## Automático

Al importar, la app busca el desfase sola: mira en qué momentos del track giraste o frenaste, como
se hace al picar una baliza, y busca el desfase que hace coincidir esos momentos con tus picadas.
También da una **confianza** (de 0 a 1): cuántas balizas apoyan ese desfase y cuánto destaca
sobre otros posibles.

> La app dice «tu reloj va 7,0 s adelantado». Picaste la baliza 5 a las 10:32:10 del
> cronometraje; en tu reloj eran las 10:32:17, y es en ese punto del track donde se coloca la
> baliza.

## Cuándo tocarlo

En la pestaña [Mapa](carrera-mapa.md) de la carrera, «Reloj y cronometraje», plegado encima del
mapa. Se abre solo, con «Revisar», si la alineación ha fallado o la confianza es baja (menos de
0,5). Tócalo si en el mapa **las balizas no caen donde estaban**.

- **A mano**: escribe el desfase en segundos (positivo si tu reloj va adelantado) y **Aplicar**.
  Si con ese desfase la carrera no cae en el track, te lo dice y no lo guarda.
- **Volver al automático** borra el que pusiste a mano.
- **Aplicar ±1 h o ±2 h**: si el track encaja desplazado horas enteras (el día del cambio de
  horario, o una zona horaria mal elegida al importar), la app te lo propone y afina el resto
  sola.

Cambiar el desfase mueve las balizas en el mapa y cambia lo que sale del track (el desvío y las
paradas de [¿Lento o desorientado?](lento-o-desorientado.md), el desnivel…). La tabla de tramos no
cambia: sale de las picadas, no del reloj.

Detalle técnico:
[alineación](https://github.com/Mendana/tramos/blob/main/docs/alineacion.md) y
[app, «Reloj y cronometraje»](https://github.com/Mendana/tramos/blob/main/docs/app.md#reloj-y-cronometraje-68).
