# Físico

Cómo evoluciona tu rendimiento físico de carrera en carrera: cuánto pulso te cuesta ir rápido, si
mejoras con el tiempo, cómo corres en cada pendiente y si te caes siempre al final. Usa los
filtros de [Estadísticas](estadisticas.md). Es lo mismo que la pestaña
[Físico de una carrera](carrera-fisico.md), pero con todas juntas.

Solo cuentan las carreras que importaste **con el FIT del reloj**: sin él no hay pulso ni ritmo.
Si algunas de las carreras con esos filtros no lo tienen, un aviso lo dice. Con pocas carreras las
gráficas dicen poco: una línea con tres puntos no es una tendencia.

## Cómo leer las gráficas

- **Cada formato, su línea.** Una gráfica por magnitud, con la fecha abajo y una línea por
  [formato](formatos.md): sprint, media y larga. No se mezclan porque no se comparan: en una larga
  el pulso y la carga son otros que en un sprint. Compara cada línea con ella misma.
- **Cada magnitud, su escala.** Nunca hay dos escalas en la misma gráfica. El eje vertical no
  empieza en cero, para que se vean los cambios; fíjate en los números del eje.
- **Punto hueco**: la carrera no tiene altitud, así que su velocidad (y su eficiencia) usa los
  metros reales y no los equivalentes en llano. No se compara bien con las demás: un aviso cuenta
  cuántos hay.
- Una carrera que no tiene el dato (sin pulso, sin cadencia…) no sale en esa gráfica, y un aviso
  dice cuántas faltan.
- Al pasar el ratón (o con el teclado) sale el valor de la carrera más cercana con su fecha. La
  **Tabla** de cada panel da lo mismo en números.

## Evolución

### Eficiencia

Los metros por latido de todo tu traslado (el tiempo corriendo, fuera de la zona de ataque de cada
baliza y de las paradas). Sube si corres más rápido con el mismo pulso, así que si la línea sube
con los meses, estás en mejor forma. Con altitud son metros **en llano** (ajustados por
pendiente); sin ella, metros reales (punto hueco).

> En sprint tu eficiencia pasa de 1,45 a 1,58 m/latido en seis meses: corres más rápido con el
> mismo pulso. (Números inventados.)

### Desacople

Cuánto cae la eficiencia de la primera mitad de tu traslado a la segunda, en %. Positivo: en la
segunda mitad te cuesta más pulso ir igual de rápido (deriva cardiaca). Un día fresco sale cerca de
0; con calor, cansancio o sin haber comido, sube. Si con los meses baja, aguantas mejor. Necesita
pulso y al menos dos minutos de traslado en cada mitad.

### Velocidad de traslado

Tus km/h corriendo. Con altitud es la velocidad **en llano**: descuenta las subidas y las bajadas
para poder comparar un día de monte con uno de parque. Sin altitud es la real (punto hueco).

### Pulso medio

El pulso medio de la salida a la meta, parado incluido. Por sí solo dice poco: depende de lo
fuerte que corras. Míralo junto a la velocidad: a la misma velocidad, un pulso más bajo es mejor.

### VAM

Los metros que subes por hora en los trozos de un 5 % de pendiente o más. Una carrera plana, o sin
altitud, o con menos de un minuto de subida, no tiene punto.

### Cadencia

Los pasos por minuto en el traslado. Cambia con el terreno y con el ritmo (en una bajada o a más
velocidad, más pasos), así que compárala dentro de un mismo formato.

### Zancada

Los metros por paso. A igual velocidad, una zancada que se acorta con los meses puede ser falta de
fuerza o un cambio en la técnica. Necesita cadencia.

### Carga

Los minutos en cada zona de pulso por el número de la zona (1, 2, 3…), sumados, con las zonas de
[Ajustes](ajustes.md) ([Zonas del mapa](zonas-del-mapa.md)). Crece con la duración y la
intensidad, así que compárala dentro de un formato, y solo entre carreras con las mismas zonas. Sin
zonas, el panel te lo dice. Al ver a un [atleta](atletas.md), sus zonas no vienen en el paquete y
no sale.

## Por pendiente

Los trozos de traslado de **todas** tus carreras se reparten en cinco clases (bajada fuerte,
bajada, llano, subida y subida fuerte) y se suman. Cada carrera pesa según el tiempo que corriste en
esa clase: una larga pesa más que un sprint. Solo cuentan las carreras con altitud en todo su
traslado; un aviso dice cuántas se quedan fuera. Si hay varios formatos, juntarlos mezcla terrenos
muy distintos: filtra por formato para separarlos.

### Velocidad por pendiente

La velocidad real y la equivalente en llano en cada clase. Si en la subida vas mucho más despacio
de lo que tu ritmo en llano haría esperar, ahí pierdes. En **bajada fuerte** el ajuste exagera lo
que se ahorra (ahí manda la técnica, no la energía): mira la velocidad real.

### Pulso por pendiente

El pulso medio en cada clase. El pulso de muñeca llega con retraso, así que en las subidas cortas
se queda más bajo de lo real.

## Deriva

### Deriva media por décimas

Tu traslado de cada carrera se parte en diez partes iguales de tiempo, y para cada una se mide su
eficiencia **frente a la de esa carrera entera** (0 % es igual que su media). La gráfica es la
media de eso entre todas tus carreras, con el número de carreras (n) de cada décima. Si las
últimas décimas salen casi siempre por debajo de cero, te caes al final; si la primera sale muy
por encima, sales demasiado fuerte. Necesita pulso y al menos dos minutos de traslado en cada
mitad.

> Las décimas 1 a 7 están en torno a +2 %, y las 9 y 10 en −6 %, con 14 carreras cada una. Te caes
> casi siempre en el último cuarto de la carrera. (Números inventados.)

## Tabla de carreras

### Carreras con FIT

Las carreras que cuentan, de la más reciente a la más antigua, con todas sus cifras. Un clic abre
la carrera. «Sin altitud» marca las que usan la velocidad real.

## Qué no te dice

- **El pulso de muñeca** llega con retraso y da saltos: la eficiencia y el desacople de un día
  suelto pueden bailar. Fíate de la tendencia de varias carreras, no de un punto.
- **El ajuste a pendiente** usa un modelo general de lo que cuesta correr cada pendiente, no el
  tuyo, y exagera la bajada fuerte.
- **Compara dentro de un formato**, y mejor entre carreras de terreno parecido: el calor, la
  vegetación o las piedras mueven la eficiencia sin que cambie tu forma.
- **El traslado no es solo correr**: dentro también hay trozos andando mientras lees el mapa.
- **Una tendencia no es una causa**: que la eficiencia suba no dice si es por entrenar, por el
  terreno o por el clima.

Detalle técnico:
[rendimiento físico, «Histórico»](https://github.com/Mendana/tramos/blob/main/docs/rendimiento-fisico.md).
