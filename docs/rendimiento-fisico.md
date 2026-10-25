# Rendimiento físico

`tramos_core::physical::race_physical(&Track, &Segmentation, &MetricsOptions, Option<&[f64]>) ->
Result<RacePhysical, MetricsError>` analiza solo el rendimiento físico de una carrera (#212): cómo
de rápido corre el corredor, cuánto pulso le cuesta, cómo se degrada a lo largo de la carrera y
cómo sube y baja. No mira la orientación: ni la pérdida, ni los errores, ni los demás corredores.

Usa la segmentación (`docs/segmentacion.md`) y las mismas definiciones que las métricas de tramo
(`docs/metricas.md`): intervalos, paradas, huecos, altitud suavizada, coste de correr con
pendiente y tiempo de ataque. El último argumento son los límites de las zonas de pulso de Ajustes
(ppm, de menor a mayor), si las hay. Los tests están en `crates/tramos-core/src/physical.rs`
(tracks calculados a mano) y `crates/tramos-core/tests/physical.rs` (FIT sintético).

## Traslado

En orientación no todo el tiempo lento es físico: parado se lee el mapa y en la zona de ataque se
frena para buscar la baliza. Las cifras físicas se calculan sobre el **traslado**:

- los intervalos **en movimiento** (`stop_speed_mps` o más, sin huecos),
- que **no empiezan en la zona de ataque** de su tramo: desde la entrada en el radio de la baliza
  (`attack_radius_m`, 50 m) en la última aproximación hasta la picada (`docs/metricas.md`, "Tiempo
  de ataque"). El intervalo que cruza el radio aún es traslado.

Los intervalos de traslado seguidos se juntan en **trozos** de al menos 20 m (`GRADE_CHUNK_M`),
como para la velocidad equivalente en llano. Una parada, un hueco, la entrada en la zona de ataque o
el final del tramo cierran el trozo, aunque mida menos. Cada trozo tiene:

- duración, distancia y, con altitud, **pendiente** (diferencia de la altitud suavizada entre su
  final y su principio, entre su distancia) y **distancia equivalente en llano** (distancia ×
  `C(pendiente) / C(0)`, el coste de Minetti);
- **pulso medio** (cada intervalo vale la media de sus dos extremos, ponderada por tiempo), si hay
  pulso en al menos la mitad del trozo;
- **cadencia media**, igual.

Las mitades, las décimas y las clases de pendiente son grupos de trozos. Un trozo va a la mitad y
a la décima de su punto medio en tiempo de traslado.

### Qué velocidad se compara (`speed`)

`flat` (equivalente en llano) si todos los trozos tienen altitud; si no, `moving` (la real). Es la
misma para la eficiencia de la carrera, sus mitades y sus décimas, para no comparar una con otra,
como `RaceSpeed` en P8 y P14.

## Cifras de un grupo de trozos (`TransitStats`)

| Campo | Qué es |
| --- | --- |
| `transit_s`, `distance_m` | Tiempo y distancia de traslado. |
| `speed_mps` | Distancia / tiempo. |
| `flat_speed_mps` | Distancia equivalente en llano / tiempo; `null` si a algún trozo le falta altitud. |
| `heart_rate_bpm` | Media ponderada por tiempo de los trozos con pulso; `null` si esos trozos no suman la mitad del tiempo del grupo (`MIN_COVERAGE`). |
| `heart_rate_s` | Tiempo de los trozos con pulso (s); 0 si `heart_rate_bpm` es `null`. Es el peso del pulso y de la eficiencia al juntar carreras (abajo, "Histórico"). |
| `efficiency_m_per_beat` | **Eficiencia**: metros (en llano o reales, según `speed`) por latido en los trozos con pulso. Es la velocidad en m/min entre el pulso. Sube si el corredor va más rápido con el mismo pulso. |
| `cadence_spm` | Cadencia media de los trozos con cadencia, con la misma condición. |
| `cadence_s` | Tiempo de los trozos con cadencia (s); 0 si `cadence_spm` es `null`. El peso de la cadencia y de la zancada al juntar carreras. |
| `stride_m` | **Zancada**: metros reales por paso en los trozos con cadencia. |

## Carrera (`RacePhysical`)

| Campo | Qué es |
| --- | --- |
| `duration_s`, `distance_m`, `ascent_m`, `descent_m`, `moving_s`, `stopped_s`, `gap_s` | Sumas de los tramos con track, igual que en las métricas de tramo. |
| `attack_s` | Tiempo de los intervalos en las zonas de ataque, parado o en movimiento. |
| `speed` | Ver arriba. |
| `transit` | Cifras de todo el traslado. |
| `heart_rate_bpm` | Pulso medio de todos los intervalos sin huecos, parados incluidos; `null` con pulso en menos de la mitad de ese tiempo. |
| `heart_rate_max_bpm` | Mayor pulso con una media móvil de ±5 s (`HEART_RATE_SMOOTHING_S`), de la salida a la meta: un pico de un segundo del sensor no es el máximo. Con la misma condición. |
| `halves` | Primera y segunda mitad del tiempo de traslado; `null` si no llega a 2 min cada una (`MIN_HALF_S`). |
| `decoupling_pct` | **Desacople** (deriva cardiaca): `(E₁ − E₂) / E₁ × 100`, con `E` la eficiencia de cada mitad. Positivo: en la segunda mitad cuesta más pulso cada metro. |
| `speed_change_pct` | Cambio de la velocidad comparada (`speed`) de la primera mitad a la segunda, en %. |
| `heart_rate_change_bpm` | Pulso de la segunda mitad menos el de la primera. |
| `stride_change_pct` | Cambio de la zancada, en %. Si se acorta al final, es una señal de cansancio. |
| `blocks` | **Deriva**: el traslado en 10 partes iguales de tiempo (`BLOCKS`), con sus cifras y, para las gráficas, `from_s` y `to_s` desde la salida (principio de su primer trozo y final del último). Solo las que tienen trozos; vacío sin mitades. |
| `by_grade` | Las 5 clases de pendiente, siempre las 5 y en orden; vacío si `speed` es `moving`. |
| `vam_m_per_h`, `climb_s` | **VAM**: metros subidos por hora en los trozos con un 5 % de pendiente o más (`CLIMB_GRADE`), y su tiempo; la VAM es `null` con menos de 1 min de subida (`MIN_CLIMB_S`). |
| `heart_rate_zones` | Segundos en cada zona de pulso, de la salida a la meta sin huecos, y la **carga**; `null` sin zonas o sin pulso (la condición de `heart_rate_bpm`). |

### Clases de pendiente

| Clase | Pendiente |
| --- | --- |
| `steep_down` | menos de −8 % |
| `down` | de −8 % a −3 % |
| `flat` | de −3 % a 3 % |
| `up` | de 3 % a 8 % |
| `steep_up` | 8 % o más |

Un valor justo en un límite va a la clase de arriba (`GRADE_LIMITS`).

### Zonas y carga

La zona de un intervalo es la de la media de sus dos pulsos: cuántos límites alcanza, como en el
mapa (`docs/app.md`, "Mapa"). Con límites 140 y 160, 140 ya es la zona 1 y 160 la 2.

La **carga** suma los minutos en cada zona por su número, empezando en 1: con 3 zonas,
`min₀ · 1 + min₁ · 2 + min₂ · 3`. Es la idea de la carga de Edwards, que usa 5 zonas por
porcentaje del pulso máximo. Si las zonas de Ajustes son otras, la carga solo sirve para comparar
carreras del mismo corredor con las mismas zonas.

## Serie de la carrera

`tramos_core::physical_series::race_series(&Track, &Segmentation, &MetricsOptions, step_s) ->
Result<RaceSeries, MetricsError>` da el track de la salida a la meta cada `step_s` segundos (al
menos 1 s; la app usa 5 s) para las gráficas de la pestaña Físico (#213). La interfaz solo dibuja
lo que recibe: ni suaviza, ni decide qué es un ataque o una parada. Los tests están en
`crates/tramos-core/src/physical_series.rs` (tracks calculados a mano) y
`crates/tramos-core/tests/physical.rs` (FIT sintético, contra `race_physical`).

La serie empieza en el primer punto del primer tramo con track y acaba en el último punto del
último; son `duration_s` segundos. Hay un instante en `0, step_s, 2·step_s…` y otro en la meta
(que puede quedar más cerca del anterior). Cada instante lleva:

| Campo | Qué es |
| --- | --- |
| `t_s` | Segundos desde la salida. |
| `distance_m` | Metros recorridos desde la salida, sumando los intervalos de los tramos (los huecos también, como `RacePhysical::distance_m`). |
| `altitude_m` | Altitud suavizada (`altitude_smoothing_s`); `null` sin altitud. |
| `speed_mps` | **Velocidad real**: metros recorridos en una ventana de ±5 s (`SPEED_WINDOW_S`) entre su duración, la misma ventana que el ritmo del mapa (`docs/app.md`, "Mapa"). La ventana se recorta al tramo continuo: no cruza un hueco ni un tramo sin track. |
| `flat_speed_mps` | **Velocidad en llano**: la real por `C(pendiente) / C(0)` (Minetti, como los trozos). La pendiente es la de la altitud suavizada en esa misma ventana; si no llega a 20 m (`GRADE_CHUNK_M`), se ensancha al doble y luego al cuádruple. `null` sin altitud o sin pendiente medible. |
| `heart_rate_bpm` | Media de las muestras de pulso a ±5 s (`HEART_RATE_SMOOTHING_S`, la del pulso máximo); `null` si no hay ninguna. |
| `cadence_spm` | Media de la cadencia a ±5 s (`CADENCE_SMOOTHING_S`). |
| `stride_m` | **Zancada**: velocidad real entre cadencia suavizada, en m por paso; `null` sin cadencia o parado (menos de `stop_speed_mps`). |
| `leg` | Número del tramo (empezando en 1); `null` en un hueco. |
| `state` | `transit`, `attack`, `stopped` o `gap`, abajo. |

La serie no lleva la posición de cada instante: la pestaña Físico no la usa y haría más pesado lo
que recibe la interfaz. Quien la necesita (la exportación para análisis con coordenadas, #219,
`docs/exportar-datos.md`) la pide con `series_positions(&Track, &Segmentation, &RaceSeries) ->
Vec<Option<(f64, f64)>>`: la latitud y la longitud de cada instante, en el mismo orden, interpoladas
en línea recta entre los dos puntos del track que lo rodean (como la segmentación con las picadas).
En un instante `gap` no hay posición (`None`): no hay track que interpolar.

### Estado de un instante

El instante cae en un intervalo entre dos puntos seguidos del track y toma su estado, con las
mismas reglas que `race_physical`: por este orden,

1. `gap`: el intervalo dura más de `max_gap_s`, o no hay track en ese tiempo (un tramo sin
   sub-track). No tiene datos: todo `null` menos `t_s` y `distance_m`.
2. `attack`: empieza dentro de la zona de ataque de su tramo (`docs/metricas.md`, "Tiempo de
   ataque"), parado o en movimiento.
3. `transit`: va a `stop_speed_mps` o más.
4. `stopped`: va más lento.

Los intervalos `transit` son justo el traslado de `race_physical`, y los `attack` y `gap` sus
`attack_s` y `gap_s`. `stopped` incluye también los segundos de después de picar que
`stopped_s` no cuenta (`punch_grace_s`): no son traslado ni ataque. Con `step_s` = 1 y un FIT a 1 Hz,
los segundos de cada estado coinciden con las cifras de la carrera, salvo por los intervalos
partidos por una picada, que no empiezan en un segundo entero (como mucho 1 s por tramo).

### Tramos y picadas

`legs` pone cada tramo con track en la serie (`from_s`, `to_s`, `from_m`, `to_m`, y sus balizas) y
`controls`, cada picada con hora que cae dentro de la serie (`position`, `code`, `t_s`,
`distance_m`). Con eso la interfaz sitúa las líneas de las picadas y los tramos con error sin
buscar nada en el track.

## Histórico

`tramos_core::physical_history` (#214) junta las `RacePhysical` de varias carreras con FIT para la
pestaña Físico de Estadísticas (`docs/historico.md`, "Rendimiento físico"). La app calcula
`race_physical` de cada carrera que pasa el filtro del histórico con su track guardado (alineado
y troceado como el mapa) y las zonas de pulso de Ajustes, y el núcleo hace tres cosas. Los tests
están en `crates/tramos-core/src/physical_history.rs` (cifras calculadas a mano) y en
`app/src-tauri/src/physical_history.rs` (fixture de Baltanás).

### Cifras de cada carrera (`RaceFigures`)

Para las gráficas de evolución, una por carrera con FIT y todas `null` si no se pueden dar:

| Campo | De dónde sale |
| --- | --- |
| `speed`, `transit_s` | `RacePhysical::speed` y `transit.transit_s`. |
| `efficiency_m_per_beat` | `transit.efficiency_m_per_beat`: del traslado entero, en llano o real según `speed`. |
| `decoupling_pct` | `decoupling_pct`. |
| `transit_speed_mps` | `transit.flat_speed_mps` si `speed` es `flat`; `transit.speed_mps` si es `moving`. |
| `heart_rate_bpm` | `heart_rate_bpm` de la carrera: de la salida a la meta, parado incluido. |
| `vam_m_per_h`, `cadence_spm`, `stride_m` | Los de la carrera (`transit.cadence_spm`, `transit.stride_m`). |
| `load` | `heart_rate_zones.load`; `null` sin zonas de Ajustes o sin pulso. |

Cada formato es una serie aparte (sprint, media, larga y sin formato) y no se compara con los
demás: lo decide la interfaz con el formato de la carrera. Dos carreras de distinto `speed` no
se comparan bien (la eficiencia en metros reales no es la de metros en llano), así que la interfaz
marca las de `moving` y las cuenta en un aviso.

### Juntar grupos de trozos (`merge_stats`)

Las cifras de un grupo (`TransitStats`) son medias, y la media de dos medias no es la media de
los dos grupos: cada una pesa por el tiempo en que se midió. Para juntar varios grupos (la misma
clase de pendiente de varias carreras) como si fueran uno:

| Cifra | Cómo se junta |
| --- | --- |
| `transit_s`, `distance_m` | La suma. |
| `speed_mps` | Σ distancia / Σ tiempo de traslado. |
| `flat_speed_mps` | Σ (velocidad en llano × tiempo) / Σ tiempo: los metros en llano de cada grupo, sumados, entre el tiempo total. `null` si a algún grupo con traslado le falta. |
| `heart_rate_bpm` | Σ (pulso × `heart_rate_s`) / Σ `heart_rate_s`. Solo se da si `heart_rate_s` suma la mitad del traslado total (`MIN_COVERAGE`), como en una carrera. |
| `efficiency_m_per_beat` | Σ metros comparados / Σ latidos, con latidos = pulso × `heart_rate_s` / 60 y metros comparados = eficiencia × latidos. Es la eficiencia de cada grupo ponderada por sus latidos. |
| `cadence_spm` | Σ (cadencia × `cadence_s`) / Σ `cadence_s`, con la misma condición que el pulso. |
| `stride_m` | Σ metros / Σ pasos, con pasos = cadencia × `cadence_s` / 60 y metros = zancada × pasos. |

Todos los grupos deben comparar con la misma velocidad (`speed`): la eficiencia mezcla metros, y
los metros en llano y los reales no se suman.

### Por pendiente (`GradeHistory`)

Las cinco clases, siempre las cinco y en orden. Cada una junta con `merge_stats` el grupo de esa
clase de cada carrera que pasa el filtro y la tiene con traslado (`races` cuenta cuántas). Solo
aportan las carreras con altitud en todo su traslado (`speed` `flat`, las únicas con `by_grade`):
`PhysicalHistory::graded_races` dice cuántas son de las `races` totales. Como solo aportan
carreras en llano, la eficiencia es siempre de metros en llano por latido. Con varios formatos
en el filtro se juntan; la interfaz lo avisa y deja el filtro de formato para separarlos.

### Deriva media (`DriftHistory`)

Para cada una de las 10 décimas: para cada carrera con eficiencia en el traslado entero y en esa
décima, `eficiencia de la décima / eficiencia del traslado entero` (1 = igual que su carrera);
`relative_efficiency` es la **media simple** de esos cocientes entre las carreras (cada carrera
pesa lo mismo, dure lo que dure) y `races`, cuántas la aportan. Una décima sin carreras da
`null`. Es un cociente, así que carreras en llano y con velocidad real se pueden promediar. La
interfaz lo dibuja como la diferencia con 1, en %: 0 % es la media de la carrera.

### Ejemplo de test

Grupo A: 100 s a 4 m/s reales y 5 m/s en llano, pulso 150 todo el tiempo (250 latidos, 500 m en
llano: 2 m por latido). Grupo B: 300 s a 2 m/s reales y 3 m/s en llano, pulso 170 (850 latidos, 900
m en llano). Juntos: 400 s y 1000 m, así que **2,5 m/s** reales (la media de las velocidades, 3,
sería falsa); (500 + 900) / 400 = **3,5 m/s** en llano; (250 + 850) latidos en 400 s = **165
ppm**; (500 + 900) / (250 + 850) = **1,2727 m/latido**.

Décima 1 en tres carreras, con eficiencias en la décima y en la carrera de 2,2 y 2,0, 1,2 y 1,0, y
1,5 y 1,5: cocientes 1,1, 1,2 y 1,0, media **1,1** (+10 %).

## Límites

- **Pulso de muñeca**: tiene retraso (sube y baja medio minuto después que el esfuerzo) y saltos.
  Por eso el pulso de una clase de pendiente subestima el de las subidas cortas, y el desacople es
  más fiable que cualquier pulso suelto.
- **El ajuste a pendiente es el coste metabólico** (Minetti). En bajadas fuertes exagera lo que
  se ahorra: bajando, el límite es la técnica y no la energía. Con una carrera real, la clase
  `steep_down` sale con una velocidad en llano muy baja y una eficiencia que dice más del modelo
  que del corredor. Para la bajada, mejor la velocidad real.
- **El desacople mezcla terreno y cansancio**: si la segunda mitad tiene más bajada fuerte o un
  terreno más lento (vegetación, piedras, que no se ven en la altitud), sale más alto. Compararlo
  entre carreras parecidas.
- **El traslado no es solo correr**: dentro también hay tramos andando con el mapa, que bajan la
  velocidad y la cadencia sin ser cansancio.

## Ejemplo de test

600 s a 4 m/s en llano (trozos de 5 s y 20 m). Pulso 150 hasta el segundo 299 y 165 desde el 300.
El intervalo de 299 a 300 está en la primera mitad y vale la media de sus extremos: 157,5.

- Primera mitad: pulso `(299 · 150 + 157,5) / 300 = 150,025`; eficiencia `240 / 150,025 = 1,5997`
  m por latido.
- Segunda mitad: pulso 165; eficiencia `240 / 165 = 1,4545`.
- Desacople: `1 − 150,025 / 165 = 9,08 %`. Cambio de velocidad: 0 %. Cambio de pulso: +14,98 ppm.

Clases de pendiente: 100 s a 4 m/s en llano, 100 s subiendo al 10 % y 100 s bajando al 5 %. La
subida es `steep_up` con velocidad en llano `4 · 5,968214 / 3,6 = 6,6313` m/s y la bajada, `down`
con `4 · 2,745924 / 3,6 = 3,0510` m/s. VAM: 40 m en 100 s, 1440 m/h.

Zonas con límites 140 y 160 y pulso 130, 150 y 170 durante 60, 120 y 60 s: los intervalos de los
cambios valen 140 y 160, así que caen en la zona de arriba: 59, 120 y 61 s. Carga:
`(59 · 1 + 120 · 2 + 61 · 3) / 60 = 8,03`.

Serie (60 s a 4 m/s, pulso 150 hasta el segundo 29 y 170 desde el 30, un instante cada 5 s): en
el segundo 30 la ventana de ±5 s tiene 5 muestras a 150 (25 a 29) y 6 a 170 (30 a 35), así que el
pulso es `(5 · 150 + 6 · 170) / 11 = 160,91`. Con 180 pasos/min, la zancada es `4 · 60 / 180 =
1,3333` m. Subiendo al 10 % a 4 m/s, la velocidad en llano es `4 · 5,968214 / 3,6 = 6,6313` m/s.
