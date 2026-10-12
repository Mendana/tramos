# ¿Cómo evoluciono?

Si estás mejorando, tu regularidad a lo largo del tiempo y cómo entras en mapa cuando llevas días
sin competir. Usa los filtros de [Estadísticas](estadisticas.md).

## ¿Estoy mejorando?

Dos gráficas, una para tu [IR](ir.md) y otra para tu tasa de error. Cada punto es una carrera; la
línea gruesa es la tendencia y las dos finas, su **margen**: dónde podría estar de verdad la
tendencia con el ruido que tienen tus carreras.

La app solo dice «mejoras» o «empeoras» cuando el margen entero va hacia un lado. Si el margen
incluye «ni sube ni baja», te dice que no hay un cambio claro. Y con menos de 8 carreras o menos
de 3 meses entre la primera y la última, que todavía no se puede decir.

> Tu IR sube unos 6 puntos cada 3 meses (entre 2 y 10): en un año, de rondar el 80 % a rondar el
> 100 %. Con 12 carreras y el margen entero por encima de cero, es una mejora de verdad.

En la tasa de error, **bajar es mejorar**. Combinado con el filtro de formato verás si mejoras en
todo o solo en sprint, por ejemplo.

## Consistencia por carrera

Una línea con la **consistencia** de cada carrera, de la más antigua a la más reciente. La
consistencia mide cuánto cambia tu [IR](ir.md) de un tramo a otro dentro de una carrera: **cuanto
más baja, más regular**. Si la línea baja con el tiempo, cada vez corres más parejo.

> En otoño tus carreras rondan ± 20 % y en primavera ± 11 %: ya no alternas tramos muy buenos con
> tramos muy malos.

Si tus tres últimas carreras se distancian claramente de las anteriores (al menos 5 puntos y una
quinta parte), arriba de [Estadísticas](estadisticas.md) sale una frase y **Ver** te trae a este
panel.

## Días sin competir

Las carreras se agrupan por los días que pasaron desde la carrera anterior que tengas importada:
7 o menos, 8–14, 15–30 y más de 30. Dos paneles:

- **IR al entrar en mapa**: el IR medio de los tres primeros tramos, frente a tu IR medio.
- **Errores al principio de la carrera**: la tasa de error del primer tercio de la carrera, frente
  a tu media.

> Tras más de 30 días sin competir, tu IR en los tres primeros tramos cae al 84 %, cuando tu media
> es del 93 %. Te cuesta entrar en mapa después de un parón: quizá te venga bien un calentamiento
> con mapa.

Debajo de cada columna, cuántas carreras hay (n). La primera carrera importada no tiene anterior
y no entra.

Si subes entrenamientos, cuentan como «carrera anterior»: también son mapa. Con el filtro de solo
carreras se cuentan los días entre carreras, y con el de solo entrenamientos, entre
entrenamientos.

Detalle técnico:
[histórico, «Días sin competir»](https://github.com/Mendana/tramos/blob/main/docs/historico.md#días-sin-competir-p11).
