# Importar

Para añadir carreras. Arriba eliges **Una carrera** o **Una carpeta**.

## Una carrera

1. **Ficheros**: arrastra a la ventana el .spl de WinSplits y, si lo tienes, el .fit de tu reloj,
   o elígelos con el botón. La app los distingue por la extensión.
2. **Quién eres**: tu tarjeta SI y tu nombre. Se rellenan con los de la última vez
   ([Mi perfil](perfil.md)).
3. **Revisar**: la app lee los ficheros sin guardar nada, te busca en los resultados y te
   propone el que es tuyo. Si casa la tarjeta pero no el nombre, te pide comprobarlo; si hay
   varios posibles, eliges tú; si no hay ninguno, te buscas en la lista. También sugiere el
   [formato](formatos.md) de la carrera, que puedes cambiar.
4. **Importar**: guarda la carrera. Con FIT, lo encaja con tus picadas
   ([Reloj y desfase](reloj.md)) y te dice qué tal ha ido.

Volver a importar el mismo .spl **no duplica** la carrera: la reutiliza. Sirve, por ejemplo, para
añadirle el FIT que no tenías.

> Importas una carrera sin FIT porque el reloj se quedó sin batería. Una semana después
> recuperas el fichero: vuelves a importar el mismo .spl con el .fit y la carrera, la misma,
> gana su track y su mapa.

## Una carpeta

Para cargar de golpe una temporada. Eliges una carpeta y la app busca todos los .spl y .fit de
dentro (también en subcarpetas).

- Usa la tarjeta y el nombre de [Mi perfil](perfil.md): sin ellos no importa nada.
- **Solo importa una carrera si te encuentra sin dudas**. Si hay dudas (varios posibles, o casa la
  tarjeta pero no el nombre), no la importa y te dice que la importes sola, donde puedes elegir.
- Empareja cada carrera con el FIT que coincide en fecha y hora. Si dos FIT encajan igual de bien
  (el mismo entrenamiento exportado dos veces), la importa sin FIT y te avisa.

Al acabar, un resumen con cuatro cifras (importadas, con reloj, sin pareja y con avisos) y tres
bloques: las **importadas** (un clic abre la carrera), las que se han quedado **sin pareja** y las
que tienen **avisos**, con el motivo.

## Si el reloj no encaja

Si el FIT es de otra carrera, el track no se guarda (la carrera sí). Si encaja desplazado una o
dos horas (cambio de horario o zona horaria mal elegida), se guarda y se corrige luego en el mapa
de la carrera: [Reloj y desfase](reloj.md).

Detalle técnico:
[app, «Importar una carrera»](https://github.com/Mendana/tramos/blob/main/docs/app.md#importar-una-carrera).

Las carreras de tus atletas, si entrenas, no se importan aquí: llegan por la carpeta compartida
([Atletas](atletas.md)).
