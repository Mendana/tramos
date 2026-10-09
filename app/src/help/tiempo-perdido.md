# Tiempo perdido

Cuánto te ha costado cada error. Se mide frente a **tu propio ritmo de ese día**, no frente al
ganador: así un corredor que va más despacio que los mejores no tiene «errores» en todos los
tramos, solo en los que de verdad se le torcieron. Es el método de WinSplits, adaptado.

## Paso a paso

1. **Referencia** de un tramo: lo que tardaron los mejores. Es la media del 25 % más rápido de los
   clasificados de tu recorrido (todas las categorías que corrieron las mismas balizas).
2. **IR** del tramo: referencia entre tu split. Si tardas lo mismo que los mejores, 100 %
   ([IR](ir.md)).
3. **Rendimiento habitual**: tu IR «típico» en esa carrera (la mediana de tus tramos). Es tu ritmo
   de ese día cuando las cosas salen bien.
4. **Tiempo esperado** en cada tramo: lo que habrías tardado yendo a tu ritmo habitual, es decir,
   la referencia entre tu rendimiento habitual.
5. **Pérdida**: tu split menos el tiempo esperado, en segundos y en % del esperado. Puede ser
   negativa: ese tramo lo hiciste mejor de lo que te tocaba.
6. **Error**: un tramo con pérdida de **más de 15 s y más del 10 %**, las dos cosas. Los umbrales
   se cambian en [Ajustes](ajustes.md).
7. **Tiempo perdido** de la carrera: la suma de las pérdidas de los tramos con error. **Tiempo sin
   errores**: tu tiempo menos el tiempo perdido.

> En un tramo los mejores tardan 2:00 (referencia). Tu rendimiento habitual ese día es del 80 %,
> así que lo esperado para ti es 2:00 ÷ 0,8 = 2:30. Tardas 3:10: pierdes 40 s, un 27 % de lo
> esperado. Como pasa de 15 s y de 10 %, es error. En otro tramo con referencia 1:00 esperabas
> 1:15 y tardas 1:25: pierdes 10 s, que no llegan a 15 s, así que no es error.

## Qué hay que saber

- **Referencia débil**: con menos de 4 clasificados en tu recorrido, la referencia se apoya en muy
  poca gente y la app te avisa.
- El **último tramo** (a meta) y los tramos que los mejores hacen en **menos de 20 s** se
  calculan, pero no entran en las [Estadísticas](estadisticas.md).
- Un tramo **sin picada** en una de sus balizas no tiene split ni pérdida.
- Los umbrales son tuyos: cambiarlos recalcula al momento todas las carreras.

Detalle técnico:
[tiempo perdido](https://github.com/Mendana/tramos/blob/main/docs/tiempo-perdido.md).
