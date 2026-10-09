# Grupo

Solo en [modo entrenadora](modo-entrenadora.md): todos los corredores de los que han llegado
paquetes, juntos. Arriba, los mismos filtros que en [Estadísticas](estadisticas.md) (fechas y
formato), que se aplican a todos. Solo cuentan las carreras compartidas con los tramos: las de
solo resumen no traen bastante.

## Corredores

Una fila por corredor, con sus carreras y:

- **IR medio**, **tasa de error** y **pérdida media** (en %): como en sus Estadísticas
  ([IR](ir.md), [tiempo perdido](tiempo-perdido.md)).
- **Su error más común**: el [tipo](tipos-de-error.md) que más ha etiquetado y qué parte de sus
  errores supone.
- **Duración de tramo con más errores**: el tipo de tramo (por lo que dura) en el que más falla,
  si tiene al menos 10 tramos de esa duración.
- **IR en subida, llano y bajada**, si ha compartido el track.

Un clic en la fila abre sus carreras.

## Cara a cara

Una tabla de todos contra todos. En cada celda, en cuántas carreras compartidas tuvo el de la
fila más IR que el de la columna y en cuántas menos («2–1», en verde si va por delante, en rojo si
va por detrás) y, debajo, la diferencia media de IR en puntos. «—» si no tienen carreras en común.

> En la fila de Ana y la columna de Bruno pone «3–1» y «+4,0»: de cuatro carreras que corrieron
> los dos, Ana tuvo más IR en tres, y de media 4 puntos más. (Nombres inventados.)

Se comparan por IR y no por tiempo, así que valen también carreras en las que corrieron
categorías o recorridos distintos: el IR ya es relativo a los mejores de cada recorrido.

## Carreras compartidas

Las carreras que han corrido al menos dos, de la más reciente a la más antigua, con los
corredores de más a menos IR y sus errores.

Detalle técnico:
[histórico, «Vista de grupo»](https://github.com/Mendana/tramos/blob/main/docs/historico.md#vista-de-grupo-p15).
