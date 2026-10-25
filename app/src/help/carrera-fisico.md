# Pestaña Físico

Cómo corriste, sin mezclarlo con la orientación: qué tan rápido, cuánto pulso te costó, cómo te
fuiste cansando y cómo subes y bajas. Sale del track del reloj, así que necesita el FIT: si
importaste la carrera sin él, la pestaña te lo dice. Si el FIT no trae pulso, cadencia o altitud,
cada bloque dice qué le falta en vez de quedarse vacío.

## Qué cuenta como «correr»

En orientación no todo el tiempo lento es físico: cerca de la baliza frenas para buscarla y, parado,
lees el mapa. Por eso las cifras de esta pestaña se calculan solo sobre el **traslado**: el tiempo
en que te mueves **fuera de la zona de ataque** de cada baliza (los últimos 50 m antes de picar).
Las paradas, el ataque y los huecos del track quedan fuera. Si miras la carrera entera, el traslado
será menos tiempo que el de tu resultado.

## Las cifras de arriba

- **Traslado**: el tiempo corriendo, y debajo cuánto fue de ataque y cuánto parado.
- **Velocidad de traslado**: tu ritmo medio corriendo. Debajo, el ritmo **en llano**: lo que
  habrías corrido sin subidas ni bajadas, ajustando cada trozo por su pendiente. Sirve para
  comparar un día de monte con uno de parque.
- **Eficiencia**: metros por latido. Sube si corres más rápido con el mismo pulso: es la forma de
  medir que estás en mejor forma.
- **Desacople** (deriva cardiaca): cuánto cae la eficiencia de la primera mitad de tu traslado a la
  segunda. Positivo quiere decir que en la segunda mitad te cuesta más pulso ir igual de rápido.
  Debajo, cuánto cambió el pulso (en pulsaciones por minuto) y la velocidad entre las dos mitades.
- **Pulso medio** y **máximo**: el medio es de toda la carrera, parado incluido. El máximo es el
  mayor pulso suavizado unos segundos: un pico suelto del sensor no cuenta.
- **VAM**: metros que subes por hora cuando la cuesta es de un 5 % o más. Solo sale si subiste al
  menos un minuto.
- **Cadencia** y **zancada**: pasos por minuto y metros por paso. Debajo de la zancada, cuánto
  cambió entre mitades: si se acorta al final, suele ser cansancio.
- **Desnivel**: lo que subiste y lo que bajaste.
- **Carga**: los minutos en cada zona de pulso por el número de la zona (1, 2, 3…), sumados. Solo
  sirve para comparar tus carreras entre sí, con las mismas zonas.

> Corres 20 minutos de traslado a 5:00 min/km. En la primera mitad llevas 150 ppm y en la segunda
> 160, a la misma velocidad. La eficiencia baja un 6 %: ese es tu desacople. Un día fresco sale
> cerca de 0; con calor, cansancio o deshidratación, suele subir.

## La carrera, de la salida a la meta

Cuatro gráficas una debajo de otra, con el mismo eje horizontal: **altitud**, **ritmo** (el real y
el ajustado a la pendiente), **pulso** y **cadencia** o **zancada** (tú eliges). Cada magnitud
tiene su propia gráfica y su propia escala: no se mezclan en la misma. En el ritmo, lo más rápido
va arriba.

- **Eje**: tiempo o distancia, el que prefieras. Abajo a la derecha dice cuál es: «tiempo de
  carrera (mm:ss)» o «distancia (km)». La unidad de cada magnitud (m, min/km, ppm, pasos/min) está
  arriba a la izquierda de su gráfica.
- Al pasar el ratón (o con las flechas del teclado, Mayús para ir más deprisa) sale una línea
  vertical en las cuatro y, en cada una, el valor de ese momento, con el tramo y si estabas en
  ataque o parado.
- Donde no hay línea, el track no tiene datos o estabas parado.
- La **Tabla** da la misma información cada minuto.

### Capas

Apagadas de entrada, para no mezclar el físico con la orientación hasta que lo pidas:

- **Picadas**: una línea fina en cada baliza, con su número.
- **Tramos con error**: sombrea los tramos que cuentan como error, según tu etiqueta (la que
  confirmaste o cambiaste en la pestaña [Tramos](carrera-tramos.md)). Así ves si fallar vino
  junto a un bajón de ritmo o de pulso.
- **Ataque y paradas**: sombrea en un tono el ataque y en otro las paradas.

## Deriva

Tres gráficas con tu traslado partido en diez partes iguales de tiempo: **eficiencia**, **pulso** y
**velocidad en llano** (la real, si el FIT no trae altitud). Cada una tiene su escala, y no empieza
en cero para que se vean los cambios. Si el pulso sube y la velocidad baja a la vez, te estás
cansando. Hacen falta al menos dos minutos de traslado en cada mitad.

## Por pendiente

Tu traslado se reparte en cinco clases: bajada fuerte, bajada, llano, subida y subida fuerte. En
cada una ves la velocidad real junto a la equivalente en llano, y el pulso. Cada clase dice
cuánto tiempo hubo en ella: una con muy pocos segundos no dice nada. Necesita altitud.

## Tiempo en zonas de pulso

Los minutos en cada zona de pulso, con los colores de tus zonas de [Ajustes](ajustes.md)
([Zonas del mapa](zonas-del-mapa.md)). Si no tienes zonas, un botón te lleva a Ajustes para
definirlas. Al ver a un [atleta](atletas.md), sus zonas no vienen en el paquete y esta parte no sale.

## Qué no te dice

- **El pulso de muñeca** llega con retraso (sube y baja medio minuto después que el esfuerzo) y da
  saltos. Por eso el pulso de una subida corta sale más bajo de lo real, y el desacople es más
  fiable que cualquier pulso suelto.
- **El ajuste a pendiente** es una media de muchos corredores (el de Strava, sacado del pulso), no
  el tuyo: si bajas muy bien o muy mal, te sales de él. Tampoco sabe del terreno: vegetación, piedras o barro te frenan y la
  altitud no lo ve.
- **El desacople mezcla cansancio y terreno**: si la segunda mitad tiene más bajada fuerte o un
  terreno peor, sale más alto. Compáralo entre carreras parecidas.
- **El traslado no es solo correr**: dentro también hay trozos andando mientras lees el mapa, y
  bajan la velocidad y la cadencia sin ser cansancio.

Detalle técnico:
[rendimiento físico](https://github.com/Mendana/tramos/blob/main/docs/rendimiento-fisico.md).
