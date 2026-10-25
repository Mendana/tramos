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
| `efficiency_m_per_beat` | **Eficiencia**: metros (en llano o reales, según `speed`) por latido en los trozos con pulso. Es la velocidad en m/min entre el pulso. Sube si el corredor va más rápido con el mismo pulso. |
| `cadence_spm` | Cadencia media de los trozos con cadencia, con la misma condición. |
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
