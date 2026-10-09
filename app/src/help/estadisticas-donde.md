# ¿Dónde fallo?

Qué tramos y qué terreno te cuestan más, y de qué tipo son tus errores. Usa los filtros de
[Estadísticas](estadisticas.md).

## Pérdida según duración del tramo

¿Fallas más en los tramos cortos o en los largos? Los tramos se agrupan por lo que tardan **los
mejores** (20–30 s, 30–60 s, 1–2 min, 2–4 min, 4–8 min y 8 min o más), no por lo que tardaste tú:
así un error no cambia un tramo de grupo. Una columna por grupo con la **tasa de error** y la
línea de tu media. Debajo de cada columna, cuántos tramos hay (n).

> La columna de 4–8 min llega al 25 % y tu media es del 11 %: en los tramos largos fallas el doble
> de lo normal. Si debajo pone n = 8, son pocos tramos todavía; con n = 60, es un patrón claro.

## Tipos de error

El reparto de tus errores por [tipo](tipos-de-error.md), de más a menos, con una columna gris para
los errores sin tipo. Dos desplegables lo cruzan con el **formato** y con la **duración del
tramo**. Lo marcado como **Físico** no cuenta como error de orientación: se da aparte.

Solo funciona si etiquetas: la descripción del panel dice cuántos errores no tienen tipo y
cuántos están sin revisar.

## Pérdida según desnivel

¿Te frena el desnivel? Con el FIT, cada tramo se clasifica en **subida**, **llano** o **bajada**
según lo que sube o baja por cada 100 m recorridos (subida o bajada con 4 m o más cada 100 m). Dos
paneles: el **IR medio** y la **tasa de error** en cada clase. Arriba, cuántas carreras y tramos
entran y cuántos no se pueden clasificar (sin FIT, o tramos muy cortos).

## ¿Lento o desorientado?

De qué está hecha la pérdida de todos tus errores juntos: qué parte es **desvío**, qué parte
**paradas** y qué parte **ritmo**. Explicado en [¿Lento o desorientado?](lento-o-desorientado.md).

Detalle técnico:
[histórico, «Pérdida según duración del tramo»](https://github.com/Mendana/tramos/blob/main/docs/historico.md#pérdida-según-duración-del-tramo-p7).
