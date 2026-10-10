# Estadísticas del grupo

Desde [Grupos](grupos.md), con el botón **Estadísticas del grupo** de cada tarjeta: las gráficas
de [Estadísticas](estadisticas.md) con todos los miembros del grupo juntos, como si todas sus
carreras fueran de un solo corredor. Sirve para ver cómo va el grupo en sí, no cada atleta.

Arriba, los mismos filtros (fechas y formato), que se aplican a todos, y los miembros con sus
carreras. Solo cuentan las carreras compartidas con los tramos. Si de algún miembro ya no hay
carreras, se dice y no cuenta.

## Cómo se junta

Cada atleta cuenta con sus carreras y **sus umbrales de error**, como en sus propias
Estadísticas. Después se suma todo: los tramos y los errores se suman, y cada media se pondera
por el número de casos de la que sale. Cuenta más quien más ha corrido.

> Ana tiene 2 sprints con IR medio 90 % y Bruno 1 con 80 %. El IR medio del grupo en sprint es
> (90 × 2 + 80) / 3 = 86,7 %: el de las tres carreras juntas. Si Ana falla en 4 de sus 20 tramos y
> Bruno en 1 de 10, la tasa de error del grupo es 5 de 30, un 16,7 %. (Nombres inventados.)

Un atleta que está en varios grupos cuenta entero en cada uno.

## Qué sale

Las mismas pestañas que en Estadísticas, con los mismos paneles:

- **Resumen**: carreras, [IR](ir.md) medio, tasa de error y pérdida por tramo del grupo, por
  formato y en total, con sus tres gráficas ([tiempo perdido](tiempo-perdido.md)).
- **¿Dónde falla?**: pérdida según la duración del tramo, [tipos de error](tipos-de-error.md),
  desnivel y [¿lento o desorientado?](lento-o-desorientado.md). Lo explica
  [¿Dónde fallo?](estadisticas-donde.md).
- **¿Cómo evoluciona?**: cómo entra en mapa tras días sin competir. Cada atleta cuenta los días
  desde **su** carrera anterior. Lo explica [¿Cómo evoluciono?](estadisticas-evolucion.md).
- **Cabeza y piernas**: qué pasa después de un error y si el cansancio lo anticipa. Lo explica
  [Cabeza y piernas](estadisticas-cabeza.md).

**Lo que no sale.** Solo salen los análisis que se pueden juntar sin aproximar. La
**consistencia** no: la de cada atleta ya es una media de sus carreras, y no se puede juntar con
la de otro sin inventarse números. Tampoco la lista de carreras ni el resumen en frases. Todo eso
está en las Estadísticas de cada atleta.

Los paneles se ocultan con **Personalizar**, como en Estadísticas: ocultar uno lo oculta en las
dos pantallas, porque es el mismo panel.

Para comparar a los atletas entre sí, usa [Comparar atletas](grupo.md) con el grupo elegido; para
poner un grupo frente a otro, [Comparar grupos](comparar-grupos.md).

Detalle técnico:
[histórico, «Estadísticas de un grupo»](https://github.com/Mendana/tramos/blob/main/docs/historico.md#estadísticas-de-un-grupo-145).
