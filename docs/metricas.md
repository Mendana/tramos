# Métricas de tramo

`tramos_core::metrics::leg_metrics(&Track, &Segmentation, &MetricsOptions) ->
Result<Vec<LegMetrics>, MetricsError>` mide cada tramo sobre el sub-track que da la segmentación
(`docs/segmentacion.md`). Son la base de P2, P8, P13 y P14 (`docs/preguntas.md`). Los tests están
en `crates/tramos-core/src/metrics.rs` (tracks calculados a mano) y
`crates/tramos-core/tests/metrics.rs` (FIT sintético).

`Track` es el track entero del que sale la segmentación. Solo se usa para suavizar la altitud
sin efectos de borde en cada tramo.

## Intervalos

El sub-track de un tramo va del punto interpolado en la picada de `from` al de `to`, con los
puntos del track en medio. Se mide por **intervalos** entre puntos consecutivos:

- **Distancia del intervalo**: la del reloj (diferencia de `distance_m`) si la tienen los dos
  puntos y no retrocede. Si no, la de círculo máximo entre las dos posiciones (radio
  6 371 008,8 m). El reloj filtra el ruido del GPS, que con el corredor parado suma metros que no
  se han recorrido.
- **Hueco**: un intervalo de más de `max_gap_s` (10 s). Su distancia cuenta en la del tramo, pero
  su tiempo va aparte (`gap_s`), no como parado ni en movimiento: no se sabe qué pasó dentro.
- **En movimiento**: un intervalo con velocidad (distancia / tiempo) de `stop_speed_mps`
  (0,5 m/s) o más.
- **Parado**: un intervalo más lento. No cuentan los primeros `punch_grace_s` (5 s) del tramo, que
  son el corredor picando y saliendo de la baliza; un intervalo que los cruza cuenta solo por la
  parte posterior.

Por eso `duration_s` es al menos `moving_s + stopped_s + gap_s`.

La distancia de un intervalo es pública (`tramos_core::metrics::interval_distance_m`): el mapa
la usa para el ritmo (`docs/app.md`, "Mapa"). La diferencia son los intervalos
lentos de los primeros 5 s.

## Métricas

| Campo | Qué es |
| --- | --- |
| `duration_s` | Del primer al último punto del sub-track: el split (el desfase es el mismo en los dos extremos). |
| `distance_m` | Suma de las distancias de los intervalos (`d_run` en P2). |
| `straight_m` | Línea recta entre las posiciones de las dos balizas (`d_line` en P2): los extremos del sub-track. |
| `distance_ratio` | `distance_m / straight_m`; `null` si la línea recta mide menos de 1 m (baliza que se repite, ida y vuelta). |
| `moving_s`, `stopped_s`, `gap_s` | Ver "Intervalos". |
| `moving_speed_mps` | Distancia en movimiento / `moving_s` (`v_mov` en P2); `null` si no hay movimiento. |
| `ascent_m`, `descent_m` | Subida y bajada acumuladas de la altitud suavizada (abajo); `null` sin altitud. La bajada es positiva. |
| `heart_rate_bpm` | Pulso medio ponderado por tiempo: cada intervalo (sin huecos) vale la media de sus dos extremos. `null` sin pulso. |
| `cadence_spm` | Cadencia media (pasos/min, los dos pies) calculada igual. Incluye los segundos parados (cadencia 0). |

Un tramo sin sub-track (picada sin hora, fuera del track o desordenada) sale con `track: null` y
el mismo `missing` que en la segmentación.

## Altitud suavizada

La altitud del reloj, sea de GPS o barométrica, tiene ruido de unos decímetros de un segundo al
siguiente. Sumando las subidas segundo a segundo, ese ruido se acumula. En el FIT sintético, con
0,15 m de ruido, la subida sin suavizar sale hasta 34 m por encima de la real en un tramo.

- **Media móvil en el tiempo** sobre el track entero: cada punto con altitud toma la media de los
  puntos con altitud a `altitude_smoothing_s` (5 s) o menos. En los bordes del track la ventana
  se recorta, y **no cruza huecos** de más de `max_gap_s`: a un lado y otro de un hueco el
  terreno puede ser otro.
- La altitud suavizada en los extremos del tramo (instantes interpolados) se interpola entre los
  dos puntos que los rodean.
- **Subida** = suma de los aumentos de la altitud suavizada entre puntos consecutivos del
  sub-track; **bajada**, la de las disminuciones. Sin umbral: con el suavizado basta, y un umbral
  se comería las cuestas cortas de un sprint.

Calibración con el FIT sintético: frente al desnivel del terreno sin ruido, el peor tramo se desvía
1,15 m con ±5 s, 2,7 m con ±3 s y 2,1 m con ±20 s (±10 s da un error total algo mayor). Con
±5 s el peor es el tramo de la parada, donde 30 s de ruido sin moverse se suavizan peor. Hay que
revisarlo con altitud barométrica real, que tiene escalones y deriva en lugar de ruido blanco.

## Opciones (`MetricsOptions`, valores por defecto)

| Opción | Valor | Qué es |
| --- | --- | --- |
| `stop_speed_mps` | 0,5 | Velocidad por debajo de la cual el corredor está parado (> 0). |
| `punch_grace_s` | 5 | Segundos tras la picada de salida del tramo que no cuentan como parado. |
| `max_gap_s` | 10 | Intervalo que se considera hueco (≥ 1 s), como en la alineación. |
| `altitude_smoothing_s` | 5 | Semiancho de la media móvil de la altitud (0–120 s; 0 = sin suavizar). |

En JSON se pueden pasar parcialmente: los campos que faltan toman el valor por defecto. Fuera de
rango, `MetricsError::InvalidOptions`.

## JSON

```json
{"index": 9, "from": 42, "to": 46,
 "track": {"duration_s": 427.0, "distance_m": 1215.5, "straight_m": 249.8, "distance_ratio": 4.87,
           "moving_s": 397.0, "stopped_s": 30.0, "gap_s": 0.0, "moving_speed_mps": 3.06,
           "ascent_m": 2.9, "descent_m": 8.0, "heart_rate_bpm": 165.0, "cadence_spm": 165.0},
 "missing": null}
```

## Pruebas

- Unitarias con tracks hechos a mano: distancia, velocidad y paradas; margen tras la picada;
  huecos; distancia del reloj frente al GPS (también si falta o retrocede); ida y vuelta sin
  relación; medias ponderadas de pulso y cadencia; suavizado frente a ruido alterno; suavizado
  que no cruza huecos; tramos sin sub-track; opciones inválidas; JSON.
- FIT sintético: en los 21 tramos, duración = split, distancia a ±0,5 m, línea recta a ±1 m y
  subida y bajada a ±1,5 m del terreno sin ruido (campos `ascent_m` y `descent_m` del
  `.truth.json`). El rodeo (tramo 9) da una relación > 4 y el resto < 2. La parada mide 30 ± 2 s
  (sale 30,0) y ningún otro tramo pasa de 2 s parado.
- Par real opcional (`private_metrics`), con las mismas variables que `private_alignment`
  (`docs/alineacion.md`): imprime los números de cada tramo para revisarlos a ojo, nunca nombres
  ni coordenadas.

  ```bash
  TRAMOS_PRIVATE_CLASS=<categoría> TRAMOS_PRIVATE_RESULT_INDEX=<n> \
      cargo test -p tramos-core --test metrics private_metrics -- --nocapture
  ```
