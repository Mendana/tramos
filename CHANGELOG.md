# Qué hay de nuevo en Tramos

Lo que cambia en cada versión, explicado para quien usa la app. Este texto es el que ve el usuario
en el aviso de «hay una versión nueva» (botón «Qué trae») y en la descripción de la release.

Cómo se mantiene (`docs/distribucion.md`, «Publicar una versión»):

- Una sección por versión, con el título exacto `## X.Y.Z`, la más nueva arriba.
- Solo párrafos y listas con `- `, en español y en llano: lo que se ve o se puede hacer ahora, no
  detalles técnicos. Sin títulos dentro de la sección.
- Mientras no se publica, los cambios van en `## Sin publicar

## 0.5.0

- Nueva pestaña **Físico** en cada carrera con FIT: tu rendimiento sin mezclarlo con la
  orientación. Altitud, ritmo (real y ajustado a la pendiente), pulso y cadencia o zancada de la
  salida a la meta, con el cursor sincronizado, y capas opcionales con las picadas, los tramos con
  error y el ataque y las paradas. Debajo, la velocidad de traslado, la eficiencia, el desacople
  (deriva cardiaca), el pulso, la VAM, la cadencia y la zancada, cómo cambian a lo largo de la
  carrera, cómo corres según la pendiente y el tiempo en cada zona de pulso.
- Nueva pestaña **Físico** en Estadísticas: cómo evoluciona tu rendimiento físico de carrera en
  carrera (eficiencia, desacople, velocidad, pulso, VAM, cadencia, zancada y carga), con una línea
  por formato; cómo corres en cada pendiente con todas las carreras juntas; y si te caes siempre
  al final de la carrera. Usa las carreras con FIT, con los mismos filtros que el resto, y también
  la ve quien entrena en las Estadísticas de un atleta.
- El ritmo ajustado a la pendiente es más realista en las bajadas: ahora sale de cómo corre la
  gente de verdad (el ajuste de Strava, basado en el pulso) y no de un modelo de laboratorio que
  suponía que bajar ahorra mucho. En las bajadas empinadas ya no parece que vayas lentísimo, y la
  eficiencia y la velocidad en llano por pendiente se pueden comparar entre subidas y bajadas.
- Exportar datos para análisis: en Ajustes, junto a la copia de seguridad, un botón escribe en
  la carpeta que elijas tus carreras y tramos en tablas CSV (con un esquema que explica cada
  columna), para analizarlas fuera de la app con pandas, R o un modelo. No llevan nombres, notas
  ni coordenadas, ni los tiempos de los demás corredores. Opcionalmente (casilla desmarcada),
  también la serie de cada carrera con FIT (`samples.csv`): una fila por segundo con la
  distancia, la altitud, la velocidad, el pulso, la cadencia, la zancada, el tramo y el estado,
  para modelos de secuencias; y, con otra casilla también desmarcada, las coordenadas de cada
  segundo.
- Las gráficas tienen menos texto a la vista: la explicación de cómo leer cada una está ahora en
  un **?** junto a su título. Pasa el ratón por encima (o enfócalo con el teclado) para leerla, y
  púlsalo para abrir la ayuda en la sección de esa gráfica. Los avisos sobre tus datos, como «sin
  altitud en 2 carreras», se quedan a la vista.
- Las gráficas dicen la unidad de cada eje a simple vista: «ppm», «s», «min/km» arriba a la
  izquierda del eje vertical y «tramo», «fecha (dd/mm/aa)» o «tiempo de carrera (mm:ss)» abajo a
  la derecha del horizontal, una sola vez en lugar de en cada número. Los ejes de porcentajes,
  IR o recuentos dicen qué miden, y los lectores de pantalla oyen las dos unidades.

## 0.4.2

- Lo que etiquetas manda en toda la app: si marcas «No» o «Físico» en un tramo, deja de contar
  como error en las cifras de la carrera (errores, tiempo perdido, tiempo y puesto sin errores),
  en las estadísticas y en lo que ve quien te entrena. Si marcas «Sí» en un tramo que la app no
  propuso, pasa a contar. Las cifras cambian en el momento.

## 0.4.1

- Tramos para Linux: cada versión trae también un AppImage, que vale para casi cualquier
  distribución, y paquetes `.deb` y `.rpm`. Se actualizan solos, como en Windows.

## 0.4.0

- Carreras de OReplay: en Importar, pega el enlace de la carrera en OReplay y la app descarga los
  resultados con sus parciales, sin tener que buscar el .spl en WinSplits. Vale para carreras
  individuales con parciales (no relevos ni score).
- Sincronizar el reloj con una baliza: en «Reloj y cronometraje», elige una baliza que recuerdes
  dónde picaste y pulsa ese punto en tu track del mapa. La app saca de ahí la diferencia entre tu
  reloj y el cronometraje, aunque el cálculo automático no haya encajado.
- Puesto sin errores: en el resumen de la carrera, debajo del tiempo sin errores, el puesto que
  te habría dado en tu categoría («Puesto 4.º de 18»).
- Entrenamientos: al importar, marca «Es un entrenamiento» (o cámbialo después en la carrera).
  Se analizan igual que las carreras, y en Estadísticas y Mis carreras puedes ver solo carreras o
  solo entrenamientos. Quien te entrena también lo ve.
- «Después de fallar» y «Cansancio» descuentan el desnivel: la velocidad de cada tramo se pasa a su
  equivalente en llano, así que una bajada ya no cuenta como acelerar ni una subida como
  cansancio (si el reloj guarda la altitud).
- ¿Por el camino o al llegar?: con el FIT, la app separa lo que pierdes al llegar a la baliza (en
  los últimos 50 m) de lo que pierdes antes. Sale en el análisis de cada carrera y en
  Estadísticas.
- ¿Estoy mejorando?: en Estadísticas, «¿Cómo evoluciono?», la tendencia de tu IR y de tu tasa de
  error con su margen. Solo dice que mejoras (o empeoras) cuando hay carreras suficientes y el
  margen no deja dudas.
- Tus rivales: en Estadísticas, quién coincide contigo en el mismo recorrido en 3 carreras o más,
  cuántas veces le ganas y si le pierdes por ritmo o por fallar.
- Elección de ruta: en la carrera de varios atletas, los tramos en los que fueron por sitios
  distintos, con qué opción salió más rápida y quién fue por cada una. Al elegir una opción, sus
  rutas se resaltan en el mapa.

## 0.3.0

- Copia de seguridad, en Ajustes: guarda todas tus carreras, etiquetas y ajustes en un fichero y
  recupéralas en otro ordenador (o en este, si algo se estropea). Al restaurar, lo que tenías se
  guarda al lado por si acaso. La copia lleva todos tus datos: no la compartas.
- Si entrenas y también corres, tus carreras van a tu propia subcarpeta dentro de la carpeta de
  tus atletas, como un atleta más. La app te la propone, la crea y lleva allí las que ya tenías.
- Las carreras de tus atletas que corrieron lo mismo se ven juntas: una pantalla «Carreras de
  atletas» en el bloque Atletas, con el mapa del recorrido con los tracks de todos y los tramos
  que fallaron varios. Cada atleta conserva su color aunque quites a otro de la comparación.
- En «Frente al grupo» se marcan tus atletas entre los rivales y salen los primeros.
- En las estadísticas de un grupo, los textos hablan del grupo («fallan», «media del grupo») en
  vez de «tus carreras».
- Si un atleta te manda otra vez una carrera que ya habías visto, ya no cuenta como carrera nueva
  en las novedades: sale como actualizada.
- El resumen en frases avisa cuando, a lo largo de tus carreras, tu pulso va más alto antes de
  los errores que antes de un tramo limpio.
- Nuevo apartado «Acerca de Tramos» en Ajustes, con la versión, el autor, la licencia y los
  créditos. Desde ahí puedes informar de un fallo por correo o copiar los datos para el informe.
- El tema claro u oscuro elegido se aplica desde que se abre la ventana, sin un destello del otro.
- Desde la próxima actualización, el aviso de versión nueva tendrá un «Qué trae» con estas notas.

## 0.2.1

- Tramos tiene ya su propio icono: la baliza sobre el magenta, en el instalador, en la ventana y
  en la barra de tareas.

## 0.2.0

- Primera versión que se actualiza sola. Al arrancar, Tramos mira si hay una versión nueva y te
  avisa arriba; si quieres, la instala con un botón y se vuelve a abrir. Nada se descarga sin que
  lo pidas, y tus carreras y ajustes se quedan como están.
- En Ajustes, «Buscar actualizaciones» lo comprueba cuando quieras.
