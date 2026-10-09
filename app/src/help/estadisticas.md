# Estadísticas

Todas tus carreras juntas, para ver patrones que en una sola carrera no se ven. Arriba los filtros
y las cifras; debajo, una pestaña por pregunta: **Resumen**,
[¿Dónde fallo?](estadisticas-donde.md), [¿Cómo evoluciono?](estadisticas-evolucion.md) y
[Cabeza y piernas](estadisticas-cabeza.md). El botón **?** abre la página de la pestaña abierta.

## Filtros

**Desde** y **hasta** (las dos fechas entran) y **formato** (todos, sprint, media o larga). Valen
para todas las pestañas. **Quitar filtros** los borra.

## Cifras

- **Carreras** que entran con esos filtros.
- **IR medio**: la media de tu [rendimiento habitual](ir.md) en esas carreras, con la
  consistencia media debajo.
- **Tasa de error**: de cada 100 tramos, cuántos fueron error.
- **Pérdida por tramo**: lo que pierdes en errores de media en cada tramo
  ([Tiempo perdido](tiempo-perdido.md)).

> IR medio 93 %, tasa de error 12 % y pérdida por tramo 9 s. Cuando no fallas vas al 93 % de los
> mejores; fallas uno de cada ocho tramos, más o menos, y repartido entre todos los tramos eso te
> cuesta 9 s en cada uno.

## Qué tramos cuentan

En las estadísticas **no cuentan** el último tramo (a meta: casi nunca es error y no dice nada de
cómo te orientas) ni los tramos que los mejores hacen en menos de 20 s (ahí un par de segundos
son un porcentaje enorme). Por eso la pérdida de aquí no es la suma del tiempo perdido de cada
carrera. Las carreras sin [formato](formatos.md) salen en su propia fila.

## Pestaña Resumen

- **Por formato**: una tabla con sprint, media, larga (y sin formato, si hay) y el total:
  carreras, tramos que cuentan, errores, IR medio, tasa de error, pérdida por tramo en segundos y
  en % y consistencia.
- **Gráficas por formato**: IR medio (con la línea del 100 %), tasa de error y pérdida por tramo
  en %. En %, porque los tramos de un sprint duran mucho menos que los de una larga y los segundos
  no se pueden comparar.
- **Carreras**: las que entran con los filtros, con sus números. Un clic abre la carrera.

## Ocultar paneles

Cada panel tiene un aspa para ocultarlo. **Personalizar**, junto a las pestañas, abre la lista de
todos los paneles (también los de la vista de carrera) para elegir cuáles ver, con **Enseñar
todos**. De entrada están ocultos los que menos se miran: rachas limpias, pulso antes del error y
esfuerzo percibido. Si ocultas un panel en modo entrenadora, se oculta para todos los corredores.

Cada panel dice en cuántos casos se apoya (n): con pocos casos, una cifra llamativa puede ser
casualidad.

Detalle técnico:
[histórico](https://github.com/Mendana/tramos/blob/main/docs/historico.md).
