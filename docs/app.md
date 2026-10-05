# App de escritorio

`app/` es la app Tauri 2 + React. La interfaz solo llama a comandos de `app/src-tauri/src/lib.rs`,
que a su vez llaman al núcleo (`tramos-core`) y al almacenamiento (`tramos-store`): no calcula
nada por su cuenta.

## Base de datos

Una base SQLite por usuario (`docs/almacenamiento.md`), `tramos.sqlite`, en el directorio de
datos de la app que da Tauri para el identificador `io.github.mendana.tramos`:

| Sistema | Ruta |
| --- | --- |
| Linux | `~/.local/share/io.github.mendana.tramos/` |
| Windows | `%APPDATA%\io.github.mendana.tramos\` |
| macOS | `~/Library/Application Support/io.github.mendana.tramos/` |

Se abre al arrancar (aplicando las migraciones pendientes) y la comparten todos los comandos.

## Comandos

| Comando | Qué hace |
| --- | --- |
| `core_version` | Versión del núcleo. |
| `get_settings` / `save_settings(settings)` | Ajustes del usuario (abajo). Guardar valida todos y, si alguno no vale, no guarda ninguno. |
| `preview_import(splPath, fitPath, identity)` | Primer paso de importar: lee los ficheros sin guardar nada. |
| `import_race(request)` | Segundo paso: guarda la carrera con lo que ha confirmado el usuario. |
| `list_races` | Carreras del usuario, de la más reciente a la más antigua, con su tiempo perdido. |
| `race_detail(resultId)` | Una carrera con la tabla de tramos del resultado. |
| `set_race_format(resultId, format)` | Cambia el formato de la carrera del resultado (`sprint`, `middle`, `long` o `null` = sin formato). Es de la carrera entera. |
| `race_comparison(resultId)` | Corredores del recorrido del resultado, para compararse con ellos (P4): `course_comparison` del núcleo con los umbrales de los ajustes. |
| `race_breakdown(resultId)` | ¿Lento o desorientado? (P2): `tramos_core::loss_breakdown::race_breakdown` con las métricas del track guardado (`docs/tiempo-perdido.md`). `null` sin track o si ya no se puede alinear ni trocear. |
| `race_map(resultId)` | El mapa del resultado: track por tramos coloreado por ritmo y pulso, balizas y escalas (abajo, "Mapa"). |
| `history(filter)` | Histórico de las carreras del usuario por formato (P6): `tramos_core::history` con los umbrales de los ajustes. `filter` = `{from, to, format}` (fechas `AAAA-MM-DD` incluidas y formato; `null` no filtra). Devuelve además cuántas carreras tiene el usuario sin filtrar, la fecha de la primera y la última, una fila por carrera que pasa el filtro (`races`), la pérdida según duración del tramo (P7, `by_leg_length`: `tramos_core::leg_length`) y la pérdida según desnivel (P13, `by_slope`: `tramos_core::slope` con el umbral por defecto; para cada carrera con track, el track guardado se alinea y se trocea como en `race_map` y sus métricas son las de `tramos_core::metrics::leg_metrics`). |

Los errores llegan a la interfaz como texto en español. La lógica está en
`app/src-tauri/src/import.rs`, `races.rs`, `race_map.rs`, `history.rs` y `settings.rs`, en Rust sin Tauri, y se prueba con los fixtures (`cargo test` en
`app/src-tauri`).

## Importar una carrera

1. **Ficheros**: el usuario arrastra a la ventana el .spl y, si lo tiene, el .fit, o los elige
   con el diálogo del sistema (`tauri-plugin-dialog`). Se distinguen por la extensión.
2. **Identidad**: tarjeta SI y nombre y apellidos, rellenados con los de la última importación.
3. **Revisar** (`preview_import`): lee y valida los dos ficheros (las horas del .spl, en la zona
   horaria de los ajustes), identifica al corredor
   (`docs/identificacion.md`) y sugiere el formato (`docs/modelo.md`, "Formato de carrera").
   - Un resultado: se propone. Si casa por tarjeta pero no por nombre, se pide comprobarlo.
   - Varios: el usuario elige.
   - Ninguno (o sin identidad): se listan todos los resultados con un filtro; con más de 50
     coincidencias se pide acotar.
   - Si ese .spl ya se importó, se avisa de que la carrera no se duplicará.
4. **Importar** (`import_race`), con el resultado y el formato elegidos:
   - Se leen y validan los ficheros otra vez antes de guardar nada: si alguno no vale, no se
     guarda nada.
   - Se guarda el .spl original. Si ya había una carrera enlazada a ese mismo fichero (mismo
     SHA-256), se reutiliza: **reimportar no duplica**. Si no, se guarda la carrera entera.
   - Se fija el formato elegido (si eligió uno).
   - El resultado se vincula con la **persona del usuario** (`docs/almacenamiento.md`,
     Personas). La primera vez se crea con el nombre que escribió (o «Yo» si lo dejó vacío), nunca
     con datos del .spl. Si el resultado ya estaba vinculado a otra persona, no se cambia y se avisa.
   - Se guardan la tarjeta y el nombre en los ajustes para la próxima vez (un campo vacío no
     borra el que había).
   - Con FIT, se alinea con las picadas del resultado (`docs/alineacion.md`). Si se puede, se
     guardan el FIT original y el track, y se muestran el desfase, la confianza y los avisos. Si
     no (track de otra hora o de otra carrera), **no se guarda el track** y se muestra el error,
     con la sugerencia de desplazamiento si la hay. La carrera sí queda importada. Reimportar con
     otro FIT sustituye el track.
5. La lista de carreras se actualiza.

El análisis (tiempo perdido, tramos, métricas) no se guarda al importar: se calcula al mostrarlo
a partir de la carrera y el track guardados.

## Ajustes

Pantalla **Ajustes** (botón de la cabecera). Se guardan en la tabla `settings` de la base:

| Clave | Valor | Por defecto | Efecto |
| --- | --- | --- | --- |
| `lost_time.error_threshold_s` | Pérdida mínima en segundos para que un tramo sea error (≥ 0). | 15 | Inmediato: el análisis se calcula al mostrarlo, así que cambiarlo recalcula la lista y todas las vistas de carrera. |
| `lost_time.error_threshold_pct` | Pérdida mínima en % del tiempo esperado (≥ 0). | 10 | Igual. |
| `import.time_zone` | Zona horaria IANA de las horas del .spl (`Europe/Madrid`, `Atlantic/Canary`…). | `Europe/Madrid` | Solo en las carreras que se importen después: las ya guardadas tienen sus horas en UTC. Una carrera reimportada se reutiliza tal cual, así que para corregir su hora habría que borrarla antes (aún no se puede desde la app). |
| `self.si_card` | Tarjeta SI del usuario. | — | Rellena el formulario de importar. |
| `self.full_name` | Nombre y apellidos, tal y como los escribió. | — | Igual. |
| `self.person_id` | Id de la persona del usuario en `people` (no se edita). | — | A ella se vinculan sus resultados. |

Un valor guardado que no se entiende (número negativo, zona desconocida) se trata como si no
estuviera y toma el valor por defecto. El tiempo ideal sigue siendo la suma de referencias.

Con una zona horaria equivocada, el FIT no se solapa con la carrera y la alineación lo dice, con
la sugerencia de desplazamiento (`docs/alineacion.md`).

## Lista de carreras

Los resultados vinculados a la persona del usuario, de la carrera más reciente a la más antigua:
fecha, nombre de la carrera, categoría, puesto (o estado), formato, tiempo, tiempo perdido (con el
número de errores) y si tiene track del reloj. Una fila abre la vista de la carrera.

## Vista de carrera (P1)

- **Cabecera**: carrera, fecha, categoría, corredor y resultado. A la derecha, el **formato**
  en un desplegable (sprint, media, larga o sin formato): se sugiere al importar y aquí se
  puede corregir (#97). El cambio se guarda al momento y mueve la carrera de grupo en la vista
  histórica.
- **Totales**: tiempo, tiempo perdido, tiempo sin errores, número de errores y rendimiento
  habitual, con la consistencia de la carrera debajo («Consistencia ± 23 %», P10,
  `docs/tiempo-perdido.md`): van juntos porque son el centro y la dispersión del IR. Aviso si la
  referencia es débil.
- **Tabla de tramos**: tramo, balizas (S = salida, M = meta), split, puesto en el tramo,
  referencia, IR, pérdida en segundos y en % y notas (error, último tramo, referencia corta). Los
  tramos con error van resaltados.

Los números salen de `tramos_core::runner_report::runner_report`, la misma función que usa
`tramos analizar` (`docs/cli.md`), sobre la carrera guardada: la tabla coincide con la de la CLI.
Los umbrales son los de los ajustes.

Las filas de la tabla se pueden seleccionar (clic, o Intro o espacio con el foco): el tramo
seleccionado se resalta a la vez en la tabla y en el mapa. Otro clic en el mismo lo quita.

## Mapa

Entre las gráficas y la tabla de tramos (#20). Sin mapa de orientación en el MVP: la ruta se
pinta sobre OpenStreetMap con **MapLibre GL JS** (BSD-3), que se carga aparte (`lazy`) al abrir
una carrera. Componente: `app/src/MapView.tsx`; tipos: `app/src/mapApi.ts`; estilos:
`app/src/styles/map.css`.

**Comando `race_map(resultId)`** (`app/src-tauri/src/race_map.rs`). La interfaz no calcula
nada: recibe el track ya troceado y clasificado. Devuelve, según `status`:

| `status` | Cuándo | La vista enseña |
| --- | --- | --- |
| `no_track` | La carrera se importó sin FIT. | Un estado vacío: «Sin track del reloj», con la sugerencia de reimportarla con el FIT. |
| `not_aligned` | Hay track, pero no se puede alinear o segmentar (no debería pasar: solo se guarda si se alinea). | El error, en `message`. |
| `ready` | Lo normal. | El mapa. |

Con `ready`:

| Campo | Qué es |
| --- | --- |
| `bounds` | `[oeste, sur, este, norte]` de todos los tramos, para encuadrar. |
| `legs` | Un tramo por par de picadas consecutivas (`docs/segmentacion.md`): `index`, `from`, `to`, `coordinates` (de baliza a baliza), `bounds` y `missing`. |
| `pieces` | Trozos del track en orden, con su tramo (`leg`), `coordinates`, `pace_class` y `heart_rate_class`. |
| `controls` | Balizas situadas: `position` (0 = salida; el tramo *n* acaba en la baliza *n*), `code`, `role` (`start`, `control`, `finish`), `coordinate` e `in_gap`. |
| `pace` / `heart_rate` | Escalas: `edges`, los 6 límites de las 5 clases, de menor a mayor (s/km y ppm). `heart_rate` es `null` si el track no trae pulso en al menos la mitad del tiempo de carrera. |
| `warnings` | Avisos de la alineación y balizas que no se pueden situar, en español. |

Coordenadas `[longitud, latitud]` como en GeoJSON, redondeadas a 6 decimales (~10 cm).

Cómo se calcula:

1. Se alinea el track guardado con las picadas (`docs/alineacion.md`) y se trocea en tramos
   (`docs/segmentacion.md`), con las opciones por defecto. Solo se pinta de la salida a la meta.
2. **Balizas**: la posición de la segmentación, es decir, dónde estaba el corredor en el instante
   de cada picada alineada. Las que no tienen posición (picada sin hora o fuera del track) no
   salen y lo dice un aviso; las que caen en un hueco del track salen con trazo discontinuo.
3. **Ritmo** de cada intervalo entre dos puntos de un tramo: distancia recorrida en la ventana
   del intervalo ampliada 5 s a cada lado (recortada al track), entre la duración de la ventana.
   La distancia es la de `tramos_core::metrics::interval_distance_m` (la del reloj o, si no, la
   del GPS, `docs/metricas.md`) acumulada sobre el track entero, así que la ventana cruza sin
   saltos el límite entre tramos. Por debajo de 0,5 m/s (parado) o más lento de 20:00 min/km, el
   ritmo es 20:00 min/km. Un intervalo de más de 10 s es un **hueco**: sin ritmo ni pulso.
4. **Pulso** de un intervalo: la media de sus dos extremos, si los dos lo tienen.
5. **Clases**: 5, por cuantiles ponderados por la duración de los intervalos (cada tono ocupa más
   o menos el mismo tiempo de carrera). La clase de un valor es el número de límites interiores
   que supera: 0 es lo más rápido (o el pulso más bajo) y 4 lo más lento (o el más alto). Con
   cuantiles, el mapa enseña dónde fue el corredor más despacio *en esa carrera*, sin depender
   de su forma ni del terreno.
6. Los intervalos seguidos del mismo tramo con las mismas clases se juntan en un trozo: con el
   FIT sintético, 376 trozos para unos 1 500 puntos (unos 100 kB de JSON).

Lo que se ve:

- **Track** con un borde blanco, coloreado por **ritmo** o, si hay pulso, por **pulso** (control
  segmentado *Ritmo / Pulso*; de entrada, ritmo). Escala secuencial de un solo tono (azul, de
  claro a oscuro, `--map-seq-1…5`), validada con la guía de visualización: luminosidad monótona,
  saltos visibles entre tonos y el más claro a más de 2:1 sobre el fondo del mapa. Las teselas
  de OSM son claras también en modo oscuro, así que estos colores no cambian con el modo; la
  leyenda se pinta sobre una tira del color del mapa (`--map-paper`) para que los tonos se vean
  igual. Los huecos (y, con pulso, los trozos sin pulso) van en gris discontinuo.
- **Leyenda** bajo el mapa: los cinco tonos, los cuatro límites entre clases (min/km o ppm) y
  los extremos «Más rápido / Más lento» (o «Más bajo / Más alto»).
- **Balizas** como en un mapa de orientación, en el magenta del recorrido: triángulo en la
  salida, círculo con el número de orden en cada baliza (el código, en la etiqueta accesible) y
  doble círculo en la meta.
- **Tramo seleccionado**: halo magenta, el resto del track atenuado, sus dos balizas más
  marcadas y, bajo el mapa, su resumen (balizas, split y pérdida). Si no se ve entero, el mapa se
  encuadra en él (sin animación con `prefers-reduced-motion`). Clic en un tramo del mapa (la zona
  de clic es más ancha que la línea) lo selecciona y resalta su fila; otro clic lo quita.
- **Botones** propios para acercar, alejar y ver toda la carrera (los de MapLibre traen iconos en
  `data:`, que la CSP no deja cargar). Sin rotación ni inclinación.

Los tramos del mapa y los de la tabla se emparejan por número. Para un corredor clasificado son
los mismos; con picadas que no casan con el recorrido (baliza fallida) el mapa sigue las picadas
y la tabla el recorrido (`docs/segmentacion.md`).

**Teselas de OpenStreetMap** (`https://tile.openstreetmap.org/{z}/{x}/{y}.png`), según su
[política de uso](https://operations.osmfoundation.org/policies/tiles/):

- Atribución siempre visible, «© OpenStreetMap contributors», con enlace a
  `https://www.openstreetmap.org/copyright`. El enlace se abre en el navegador del sistema con
  `tauri-plugin-opener`, al que la ventana solo deja abrir URL de `https://www.openstreetmap.org/`.
- Solo se piden las teselas de lo que se mira, al moverse por el mapa: nada de descargas
  masivas ni de precarga. Zoom máximo 19 (el de OSM). MapLibre no vuelve a pedir las teselas
  caducadas mientras el mapa está abierto (`refreshExpiredTiles: false`) y la caché del
  navegador respeta las cabeceras del servidor.
- Es un servicio gratuito para un uso moderado. Si el grupo creciera mucho, habría que pasar a
  un proveedor de teselas con clave o a uno propio.
- Pedir teselas revela la zona que se mira (`docs/datos-y-privacidad.md`).

## Vista histórica (P6)

Pantalla **Histórico** de la barra lateral (`HistoryScreen.tsx`): todas las carreras del usuario
agregadas por formato. Las definiciones (qué carreras y tramos cuentan, IR medio, tasa de error,
pérdida media por tramo, carreras sin formato) están en `docs/historico.md`.

- **Filtros**: desde y hasta (fechas incluidas) y formato (todos, sprint, media, larga). Cambiar
  uno vuelve a pedir el histórico. «Quitar filtros» los borra.
- **Cifras** del total: carreras, IR medio (con la consistencia media debajo, P10), tasa de
  error y pérdida media por tramo.
- **Tabla por formato**: sprint, media y larga (aunque no tengan carreras) y, si hay, las
  carreras sin formato, más la fila del total: carreras, tramos que cuentan, errores, IR medio,
  tasa de error, pérdida media por tramo en segundos y en % y consistencia media. Debajo del título, qué tramos
  cuentan y los umbrales de error.
- **Carreras** (#98): las que entran con los filtros, de la más reciente a la más antigua, con
  fecha, nombre, formato, categoría y sus números (IR, tramos que cuentan, errores, tasa de error
  y pérdida por tramo). Las que no cuentan lo dicen. Una fila abre la carrera.
- **Gráficas por formato**: paneles de IR medio (con la línea del 100 %), tasa de error y pérdida
  media por tramo en % (los segundos no se comparan entre formatos; la tabla del panel da los
  dos).
- **Estados vacíos**: sin carreras importadas, invita a importar; con carreras pero ninguna con
  esos filtros, ofrece quitarlos. Si las fechas están al revés, se avisa. Las carreras sin
  números (`races_without_data`) se mencionan en un aviso.

Los análisis que se apoyan en el histórico (P7, P10, P11 y P13) añaden su sección de paneles
debajo de «Gráficas por formato», con los mismos filtros y, si cuentan tramos, los mismos
(`pattern_legs`).

- **Por duración del tramo (P7)** (`LegLengthPanel.tsx`): panel «Pérdida según duración del
  tramo». Una columna por cubo de referencia (20–30 s, 30–60 s, 1–2 min, 2–4 min, 4–8 min y
  ≥ 8 min, `docs/historico.md`) con la **tasa de error**, la línea de la tasa de error del total
  («Tu media») y, bajo cada etiqueta, `n` (los tramos del cubo), porque un cubo con pocos tramos
  es poco fiable. Va la tasa en la gráfica porque es la respuesta directa a «¿fallo más en los
  tramos largos o en los cortos?» y `n` es su denominador; la tabla añade la pérdida media por
  tramo en % y en segundos. Los cubos vacíos salen sin columna y con `n = 0`.
- **Consistencia (P10)** (`ConsistencyPanel.tsx`): panel «Consistencia por carrera». Línea con
  la consistencia de cada carrera que la tiene (`races[].stats.mean_consistency`), de la más
  antigua a la más reciente, con un marcador por carrera; la descripción da la media con esos
  filtros y la tabla, fecha, carrera, formato y consistencia. Las carreras sin valor no salen.
- **Días sin competir (P11)** (`DaysOffPanel.tsx`): dos paneles con los cuatro cubos de días
  desde la carrera anterior (≤ 7, 8–14, 15–30 y > 30, `docs/historico.md`) y, bajo cada
  etiqueta, sus carreras. «IR al entrar en mapa»: IR medio de los tres primeros tramos frente al
  IR medio del total («Tu IR medio»). «Errores al principio de la carrera»: tasa de error del
  primer tercio frente a la del total («Tu media»). Son dos paneles porque son dos medidas de
  escala distinta (nada de dos ejes). Comparten la tabla, con todas las cifras y sus `n`. El
  contador del panel dice cuántas carreras no tienen anterior.
- **Por desnivel (P13)** (`SlopePanel.tsx`): una línea con cuántas carreras aportan tramos (las
  que tienen FIT, de las que pasan el filtro), cuántos tramos se han clasificado, cuántos no se
  pueden clasificar y cuántos son de carreras sin FIT (`by_slope`, `docs/historico.md`). Debajo,
  dos paneles con una columna por clase (subida, llano y bajada) y `n` bajo cada etiqueta:
  «IR medio según desnivel», con la línea del 100 % de la referencia, porque es la respuesta
  directa a «¿me frena el desnivel?», y «Tasa de error según desnivel». Las descripciones dan la
  regla de clasificación con el umbral aplicado. Las dos tablas dan, por clase, tramos, errores,
  IR medio y tasa de error. Una clase vacía sale sin columna y con `n = 0`. El umbral aún no se
  puede cambiar desde Ajustes: se usa el valor por defecto del núcleo (4 m por cada 100 m).
- **¿Lento o desorientado? (P2)** (`BreakdownPanel.tsx`): panel «De qué está hecha la pérdida de
  tus errores». Una columna por parte (desvío, paradas y ritmo) con su % de la pérdida de todos
  los errores repartidos (`loss_breakdown`, `docs/historico.md`); el tooltip y la tabla dan los
  segundos. La descripción dice cuántos errores no se pueden repartir (sin FIT o sin datos
  suficientes).
- **Después de fallar (P8)** (`AfterErrorPanel.tsx`). Dos paneles, los dos con la tasa de error
  del total como referencia («Tu media») y `n` bajo cada columna:
  - «¿Un error trae otro?»: la tasa de error del tramo siguiente a uno limpio, a un error, a un
    error acelerando y a un error sin acelerar.
  - «Rachas limpias»: la tasa de error según los tramos limpios seguidos previos.

  Las definiciones están en `docs/historico.md`. Las tablas dan tramos, errores y tasa.

## Diseño

Base visual común a todas las pantallas (#88), en CSS propio y sin librerías de componentes. La
CSP no deja inyectar estilos en tiempo de ejecución y no hay fuentes externas: se usan las del
sistema.

- **Variables** (`app/src/styles/tokens.css`): colores, tipografía, espaciado (múltiplos de 4),
  radios y sombras, con modo claro y oscuro según el sistema (`prefers-color-scheme`). Ninguna
  pantalla usa colores ni medidas sueltas.
- **Paleta** inspirada en el mapa de orientación: el **magenta** de los recorridos es el acento
  (navegación, botón principal, selección) y el **naranja** de la baliza marca los errores (filas
  de tramo con error, pérdidas, cifras). Las ganancias van en verde.
- **Estilos base** (`styles/base.css`): documento, títulos, botones (`btn`, `btn-primary`,
  `btn-ghost`, `btn-lg`), campos (`field`, `field-label`, `field-hint`, `input`) y utilidades.
- **Componentes** (`styles/components.css` y `src/ui.tsx`): estructura con barra lateral, tarjetas,
  avisos (`Notice`: información, aviso, error, éxito), píldoras, cifras destacadas (`Stat`),
  tablas (números tabulares a la derecha, filas clicables, tramos con error resaltados), zona para
  soltar ficheros, lista de opciones, control segmentado, secciones de formulario y estado vacío.
  Los iconos son SVG en línea en `ui.tsx`; el de la app es una baliza.
- **Estructura**: barra lateral con Carreras, Histórico, Importar y Ajustes; el contenido, centrado hasta
  1080 px. Por debajo de 860 px de ancho la barra lateral pasa arriba. La ventana abre a
  1180 × 780 (mínimo 760 × 520).

## Gráficas

Cada análisis se enseña en un **panel desplegable** (`app/src/charts/ChartPanel.tsx`) en la
vista de carrera o en la histórica: título, número de casos en los que se
apoya, una frase que explica cómo leerlo y un selector **Gráfica / Tabla**. La tabla es la vista
accesible de la gráfica: todo valor que se ve al pasar el ratón está también en ella.

**Sin librería de gráficas** (#21): son componentes propios en SVG y React (`app/src/charts/`).

- La CSP no deja inyectar estilos en tiempo de ejecución, que es lo que hacen varias librerías.
  Aquí los estilos van en `styles/charts.css` y los colores son las variables de diseño, así que
  el modo oscuro sale solo.
- Las gráficas que piden las preguntas (barras, barras con signo, líneas, puntos) son pocas y
  sencillas. Con componentes propios siguen al pie de la letra las reglas de la guía de
  visualización: columnas de 24 px como mucho con el extremo redondeado y la base recta desde la
  línea del 0, rejilla y ejes en líneas finas y tenues, texto siempre en los colores de texto y
  nunca en el de la serie, y tooltip por columna con una zona activa más grande que la columna,
  también con el teclado.
- Si algún día hace falta algo que no merezca la pena dibujar a mano, la alternativa sería una
  librería que pinte en SVG con atributos de React (por ejemplo, Recharts, MIT).

Piezas:

- `scale.ts`: escala lineal y dominio «redondo» con sus marcas, que siempre incluye el 0.
- `ColumnChart`: una serie de columnas (también negativas), línea de referencia opcional, color por
  columna opcional y tooltip con el valor delante y el detalle detrás. Opcionalmente, una línea de
  datos en la **misma unidad y escala** (p. ej. un acumulado; sigue siendo un solo eje), franjas
  de fondo resaltadas (p. ej. rachas) y una segunda línea de etiqueta en el eje X (p. ej. el
  número de casos de cada columna).
- `LineChart`: una línea de 2 px con un velo del 10 % hasta el 0, marcadores opcionales con un
  anillo del color de la superficie y un cursor vertical que se ajusta al punto más cercano.
- `MultiLineChart`: varias líneas sobre los mismos puntos, la destacada (el corredor) más
  gruesa, etiqueta directa al final de cada línea y tooltip con el valor de cada serie.
- `GroupedColumnChart`: columnas agrupadas, una por serie y punto, con 2 px de separación.
- `common.tsx`: tamaño, rejilla con marcas, tooltip (al lado de la marca, para no taparla) y
  leyenda (cuadrado para barras, raya para líneas). Con dos o más series siempre hay leyenda.
- Dos medidas de escala distinta nunca comparten eje: van en paneles separados.

Colores de las gráficas (`tokens.css`): `--chart-series-1` es el acento, validado con la guía de
visualización contra la superficie de cada modo (en oscuro, un tono más oscuro que el acento de
la interfaz para quedar en la banda de luminosidad). `--chart-grid`, `--chart-axis` y
`--chart-reference` son la rejilla, la línea del 0 y la línea de referencia. `--chart-error`
(naranja de baliza) marca los tramos con error y `--chart-muted` (gris) el resto; validados igual,
se distinguen también con daltonismo. Para ganar o perder (P5), el par divergente `--chart-gain` (azul)
y `--chart-loss` (el mismo naranja), validado igual; la dirección de la columna también lo dice.
Para varios corredores (P4), la paleta categórica `--chart-series-1` a `--chart-series-4`
(magenta para el corredor; azul, verde y ámbar para los compañeros, en ese orden fijo), validada
en los dos modos entre todas las parejas. `--chart-line-neutral` es una línea de datos en tinta neutra y `--chart-highlight` el fondo de una
franja resaltada.

Paneles de la vista de carrera:

| Panel | Qué enseña | Casos |
| --- | --- | --- |
| Pérdida por tramo (P1), abierto de entrada | Pérdida de cada tramo en segundos, hacia arriba si pierde y hacia abajo si gana; los tramos con error en naranja y el resto en gris. | Tramos con pérdida. |
| Pérdida acumulada (P3) | Tiempo perdido sumado tramo a tramo desde la salida: solo suben los tramos con error, marcados con un punto. Acaba en el tiempo perdido de la carrera. | Errores y tramos. |
| Dónde gano y dónde pierdo (P5) | Ganancia de cada tramo frente a lo esperado (`gain_s`, arriba gano en azul, abajo pierdo en naranja), la línea del acumulado (`cumulative_gain_s`) y una franja por cada racha de dos o más tramos seguidos perdiendo (`losing_streaks`). Todo sale del núcleo (`docs/tiempo-perdido.md`). | Tramos con ganancia y rachas. |
| Rendimiento por tramo | IR de cada tramo como columna, con la línea del 100 % (la referencia). | Tramos con IR. |
| ¿Lento o desorientado? (P2) | Cada error de los tramos que cuentan con tres columnas, desvío, paradas y ritmo (colores de serie 1–3, en ese orden), que suman su pérdida; la descripción resume el total de los errores en segundos y en %. La tabla da todos los tramos repartidos y la relación habitual `r0`. Sin FIT, o con menos de 3 tramos sin error con track, el panel lo dice en lugar de la gráfica. Lo pide aparte (`race_breakdown`). | Errores repartidos. |

Bajo «Frente al grupo» (P4, `GroupComparison.tsx`), el corredor junto a compañeros elegidos de
su mismo recorrido (hasta 3, de entrada el ganador del recorrido). Las fichas de arriba eligen a
los compañeros y hacen de leyenda; la elección vale para los dos paneles:

| Panel | Qué enseña | Casos |
| --- | --- | --- |
| Diferencia con el tiempo ideal | Diferencia acumulada respecto al tiempo ideal tras cada tramo, una línea por corredor; hacia abajo es por detrás, como en la gráfica clásica de WinSplits. | Corredores. |
| Pérdida por tramo comparada | Pérdida de cada tramo frente a lo esperado con el rendimiento habitual de cada uno, en columnas agrupadas. | Corredores. |

Los paneles van entre las cifras destacadas y la tabla de tramos, y salen de los mismos tramos que
la tabla: sus valores coinciden con ella.

## Seguridad

La ventana tiene una CSP restrictiva (`docs/datos-y-privacidad.md`). Los permisos de la ventana
(`app/src-tauri/capabilities/default.json`) son los de `core:default`, `dialog:allow-open`, este
solo para el diálogo de abrir ficheros, y `opener:allow-open-url` limitado a
`https://www.openstreetmap.org/*`, para el enlace de la atribución del mapa.

MapLibre y la CSP:

- La CSS de MapLibre va en el bundle (`import` en `MapView.tsx`), así que `style-src 'self'`
  basta. MapLibre no inserta hojas de estilo: solo cambia propiedades de estilo de sus elementos
  desde JavaScript (posición de los marcadores, tamaño del lienzo), que la CSP no bloquea. Las
  balizas son elementos propios con clases de `map.css`, sin estilos en línea.
- Su worker se empaqueta como un fichero más de la app (`?worker&url` de Vite, en formato ES) y
  se le pasa con `setWorkerUrl`: `worker-src 'self'`, sin `blob:`.
- Las teselas: `img-src` y `connect-src` admiten `https://tile.openstreetmap.org` (MapLibre las
  descarga con `fetch` y las decodifica en un `ImageBitmap`).
- No se usan los controles de MapLibre con iconos en `data:` (zoom, brújula): `img-src` no
  admite `data:`.
