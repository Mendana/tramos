# Mis atletas

En el bloque [Atletas](atletas.md), si entrenas: una tarjeta por cada atleta que te comparte sus
carreras. Un clic en la tarjeta entra en el atleta y abre sus carreras, en solo lectura.

## Cada tarjeta

- **Nombre** y, si hay, cuántas carreras **nuevas** tiene: las que te han llegado desde la última
  vez que entraste en él. Debajo, la fecha de su última carrera.
- **Línea de evolución**: su [rendimiento](ir.md) en sus 10 últimas carreras, de la más antigua a
  la más reciente. Pasa el ratón por un punto para ver la carrera y su cifra.
- **Carreras**, **rendimiento** (su IR medio) y **tasa de error** (qué parte de sus tramos fueron
  error), de todas sus carreras con tramos.
- **Error más común**: el [tipo de error](tipos-de-error.md) que más ha etiquetado y qué parte de
  sus errores es.
- **Grupos**: los [grupos](grupos.md) en los que está, con su color.

Las cifras son las mismas que su fila en [Comparar atletas](grupo.md) sin filtros: salen de sus
carreras, recalculadas en tu app con sus umbrales.

**Personalizar**, arriba a la derecha, elige qué lleva cada tarjeta: quita lo que no mires. Se
guarda en tus ajustes, como los paneles de [Estadísticas](estadisticas.md).

## Buscar y filtrar

Escribe parte del nombre para encontrar a un atleta (da igual mayúsculas o tildes). Si tienes
grupos, sus botones dejan solo a los atletas de ese grupo; **Todos** los vuelve a enseñar.

## Nuevas

Una carrera es nueva si te ha llegado después de la última vez que entraste en ese atleta, desde
esta página, desde [Inicio](inicio.md) o desde Comparar atletas. Al entrar, sus nuevas vuelven a
cero. Cuenta también una carrera que vuelve a llegar cambiada, por ejemplo porque el atleta ha
etiquetado sus errores. En la barra lateral, junto a **Mis atletas**, sale cuántas carreras nuevas
hay entre todos.

> Llevas una semana sin mirar a una atleta y ves «2 nuevas» en su tarjeta: corrió el sábado una
> media y el domingo un sprint. Entras, las miras y, al volver, su tarjeta ya no dice nada nuevo.
> (Ejemplo inventado.)

Detalle técnico:
[app, «Atletas»](https://github.com/Mendana/tramos/blob/main/docs/app.md#atletas-37-119).
