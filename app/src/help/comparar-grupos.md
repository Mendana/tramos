# Comparar grupos

Desde [Grupos](grupos.md), con dos grupos o más: un grupo de atletas frente a otro, por ejemplo
juveniles frente a sénior. Cada grupo junta todas las carreras y todos los tramos de sus
atletas, cada uno con sus umbrales.

## Opciones

- **Grupo A** y **Grupo B**: de entrada, los dos primeros.
- **Si alguien está en los dos**: «Cuenta en los dos» (lo normal) o «Se deja fuera», para que
  los grupos no se parezcan solo por tener a la misma gente.
- **Carreras**: «Las de los dos grupos» (lo normal) o «Todas». Con la primera solo cuentan las
  carreras que han corrido un atleta de cada grupo, y no la misma persona: así se comparan en el
  mismo terreno, porque hay carreras mucho más difíciles que otras.
- Los filtros de fechas y formato de [Estadísticas](estadisticas.md).

Un aviso dice cuántas carreras entran y qué pasa con quien está en los dos.

## Qué se compara

- Una tabla con los atletas, las carreras, los tramos, el **IR medio**, la **tasa de error** y la
  **pérdida media** de cada grupo, y la **diferencia** A − B en puntos ([IR](ir.md),
  [tiempo perdido](tiempo-perdido.md)).
- **Tipos de error de cada grupo**: qué parte de sus errores es de cada
  [tipo](tipos-de-error.md), los seis más comunes entre los dos.
- **Tasa de error según duración del tramo**: en qué tramos, por lo que duran, falla más cada
  grupo.
- **IR medio según desnivel**: en subida, llano y bajada; solo las carreras con track.

Los tres paneles se pueden ocultar, como los de Estadísticas. En las gráficas, A va siempre en el
primer color y B en el segundo, no en el color de cada grupo, que podría ser el mismo.

> Juveniles tiene IR medio 86 % y Sénior 91 %: la diferencia es −5 puntos. En la gráfica por
> duración, Juveniles falla el doble en los tramos largos. Con «Las de los dos grupos», esa
> comparación sale solo de las carreras que corrieron los dos. (Números inventados.)

Detalle técnico:
[histórico, «Comparar grupos»](https://github.com/Mendana/tramos/blob/main/docs/historico.md#comparar-grupos-121).
