# Comparar atletas

En el bloque [Atletas](atletas.md), si entrenas: todos los atletas de los que han llegado
paquetes, juntos. Arriba, los mismos filtros que en [Estadísticas](estadisticas.md) (fechas y
formato), que se aplican a todos. Solo cuentan las carreras compartidas con los tramos: las de
solo resumen no traen bastante.

**Grupo**: si tienes [grupos](grupos.md), elige «Todos los atletas» o uno de ellos. Con un grupo
salen solo sus miembros (tú, si estás en él), con su nombre y su descripción arriba. Cambiar de
grupo no cuenta para el botón de volver. Las gráficas con todo el grupo junto están en
[Estadísticas del grupo](estadisticas-grupo.md).

**Incluirme** (solo con «Todos los atletas»): con esta casilla marcada, tus propias carreras cuentan como las de un atleta más
(«Tu nombre (tú)», la primera fila), en la tabla, en el cara a cara y en las carreras
compartidas. Desmarcada por defecto; se queda como la dejes.

## Atletas

Una fila por atleta, con sus carreras y:

- **IR medio**, **tasa de error** y **pérdida media** (en %): como en sus Estadísticas
  ([IR](ir.md), [tiempo perdido](tiempo-perdido.md)).
- **Su error más común**: el [tipo](tipos-de-error.md) que más ha etiquetado y qué parte de sus
  errores supone.
- **Duración de tramo con más errores**: el tipo de tramo (por lo que dura) en el que más falla,
  si tiene al menos 10 tramos de esa duración.
- **IR en subida, llano y bajada**, si ha compartido el track.

Un clic en la fila abre sus carreras (en la tuya, las tuyas).

Con dos o más, la última fila (**Todos** o **Total de** el grupo) junta todas sus carreras y
todos sus tramos: el IR medio de todas las carreras, y la tasa de error y la pérdida media de
todos los tramos.

> Ana tiene 2 carreras con IR medio 90 % y Bruno 3 con 80 %. La fila de totales dice 5 carreras e
> IR medio 84 %: (90 × 2 + 80 × 3) / 5. Cuenta más quien más ha corrido. (Nombres inventados.)

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
atletas de más a menos IR y sus errores.

Detalle técnico:
[histórico, «Vista de grupo»](https://github.com/Mendana/tramos/blob/main/docs/historico.md#vista-de-grupo-p15).
