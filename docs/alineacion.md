# Alineación del FIT con las picadas

`tramos_core::alignment::align(&Track, &RaceResult, &AlignmentOptions) -> Result<Alignment,
AlignmentError>` comprueba que el track del reloj cubre la carrera de un corredor, estima el
desfase fino entre el reloj y el cronometraje y sitúa cada picada en el track. Lo consume el
corte del track en tramos (#11). `align_with_offset` hace lo mismo con un desfase fijado a mano,
sin estimarlo (ver [Desfase fijado a mano](#desfase-fijado-a-mano)). Los tests están en
`crates/tramos-core/tests/alignment.rs`.

## Tiempos

- Todo va en UTC. El FIT ya viene en UTC (`docs/formato-fit.md`) y el lector del .spl convierte
  la hora local de la carrera (`Europe/Madrid`, con su horario de verano o de invierno) a UTC
  (`docs/formato-spl.md`). La alineación no sabe nada de zonas horarias: una carrera en invierno
  (UTC+1) y otra en verano (UTC+2) se alinean igual.
- **Convenio del desfase**: `instante en el track = instante de la picada + offset_s`. Un reloj
  que va 7 s adelantado respecto al cronometraje da `offset_s = +7`.
- Un track con la hora mal convertida (una o dos horas de diferencia) no se solapa con la
  carrera ni con el desfase máximo: es un error (`TrackOutsideRace`), no un aviso.

## Ventana de la carrera

De la picada de salida (código 32736) a la de meta (32752), en hora del cronometraje. Si la
salida o la meta no tienen hora, se toma la primera o la última picada con hora (con aviso). Sin
ninguna picada con hora, o sin dos instantes distintos en orden, es un error.

`race_window(&RaceResult) -> Result<RaceWindow, AlignmentError>` da esta misma ventana sin los
avisos, para buscar el track de una carrera antes de alinearlo (importar una carpeta,
`docs/app.md`).

La **salida no se usa para estimar el desfase** (sí para la ventana): en el .spl suele ser la
hora de salida asignada (minutos en punto), no una marca física del corredor. La meta sí es una
marca física y cuenta como una baliza más. `use_start_punch = true` la incluye si algún día
hay salidas con picada real.

## Señal

El track se remuestrea a 1 Hz por interpolación lineal (posición en un plano local en metros,
cadencia) entre la salida − `max_offset_s` − margen y la meta + `max_offset_s` + margen. Dentro
de un hueco (dos puntos separados más de `max_gap_s`) no hay muestra.

En cada segundo *t* se mide cuánto "parece una baliza", combinando tres señales para no
depender de una sola (el corredor no siempre se para: a menudo pica en marcha):

| Componente | Cálculo | Peso |
| --- | --- | --- |
| Giro | Ángulo θ entre el desplazamiento de *t − w* a *t* y el de *t* a *t + w* (`w = turn_window_s`, 5 s): `(1 − cos θ) / 2`, de 0 (recto) a 1 (media vuelta). Se multiplica por `min(1, d / min_displacement_m)`, con *d* el menor de los dos desplazamientos, porque con el corredor casi parado el rumbo es ruido del GPS. | 0,50 |
| Bajada de velocidad | `clamp(1 − v / v_ref, 0, 1)`, con *v* la velocidad en *t* (diferencia central de 2 s) y `v_ref` la mediana de la velocidad del corredor entre salida y meta. Si `v_ref < 0,5 m/s` no se usa. | 0,35 |
| Bajada de cadencia | `clamp(1 − c / c_ref, 0, 1)`, con `c_ref` la mediana de la cadencia en la carrera. Solo si al menos la mitad de los segundos de carrera tienen cadencia. | 0,15 |

La señal es la media ponderada de los componentes disponibles. El giro es obligatorio: donde no
se puede calcular (a menos de `w` del borde del track o de un hueco) no hay señal. Después se
suaviza con un núcleo triangular de semiancho `smoothing_s` (3 s) y se le resta su media entre
salida y meta, de modo que un instante cualquiera vale 0 de media.

## Función de coste y búsqueda

Para cada desfase candidato δ:

`S(δ) = (1 / N) · Σ_i señal(t_i + δ)`

donde *t_i* son los instantes de las picadas con hora (sin la salida) y *N* su número. La señal
se interpola linealmente entre segundos. Una picada sin señal en `t_i + δ` (fuera del track o en
un hueco) suma 0, que es el valor medio: no favorece ningún desfase.

1. **Rejilla**: δ = −`max_offset_s` … +`max_offset_s` (±60 s) de segundo en segundo. Se elige
   el δ de mayor `S`; en caso de empate, el más cercano a 0.
2. **Refinamiento**: vértice de la parábola que pasa por `S(δ − 1)`, `S(δ)` y `S(δ + 1)`,
   limitado a ±0,5 s.
3. **Picadas útiles**: las que tienen señal en el δ elegido. Con menos de `min_controls` (3) no se
   estima: `offset_s = 0`, `offset_estimated = false`, confianza 0 y aviso.

Con el FIT sintético el desfase sale a +0,14 s del real para desplazamientos de −45 a +45 s
(en ±60 s, el borde de la rejilla, sale exacto y sin refinar), también con ±4 m de ruido en cada
posición y sin cadencia.

## Confianza

Para cada picada útil, su **desfase propio** (`local_offset_s`) es el máximo de su señal en
±15 s alrededor del desfase global. Con eso:

- **Apoyo** (`support`): fracción de picadas útiles cuyo desfase propio está a ±3 s del global.
- **Separación** (`margin`): `(S(δ) − S(δ₂)) / (S(δ) − media de S)`, con δ₂ el mejor desfase a
  más de 10 s de δ (`runner_up_offset_s`). 0 = empate; 1 = la alternativa no destaca sobre la
  media.

`confianza = clamp((apoyo − 0,4) / 0,4, 0, 1) × clamp(separación / 0,6, 0, 1) ×
min(1, picadas útiles / few_controls)`

Calibración con los fixtures: el corredor correcto da apoyo 0,95 y separación 0,8–0,9
(confianza 1; con ruido de GPS, 0,90). Las picadas de otros corredores del .spl, desplazadas para
salir a la vez, dan apoyo 0,2–0,6 y separación 0–0,6: confianza máxima 0,39. Por eso el aviso de
confianza baja salta por debajo de 0,5. Hay que revisar estos números con carreras reales.

## Cobertura

Con el desfase aplicado, la ventana de la carrera en hora del reloj es `[salida + δ, meta + δ]`.
`Coverage` da el principio y el final del track, los segundos de carrera que quedan fuera
(`missing_start_s`, `missing_end_s`) y los huecos de más de `max_gap_s` que tocan la carrera.

## Picadas en el borde del track

La señal necesita `turn_window_s` a cada lado (y algo más por el suavizado), así que no existe en
los primeros y últimos segundos del track ni junto a un hueco. Lo habitual es la meta: el
corredor para el reloj al picarla y el track acaba ahí. Esas picadas no cuentan para el desfase,
pero se sitúan igual:

- Una picada **dentro del track** y fuera de un hueco, sin señal en su instante, sale con
  `usage: near_edge` y su `location` normal.
- Una picada que cae **fuera del track por `EDGE_SNAP_S` (2 s) o menos** se sitúa en el primer o
  el último punto (`fraction` 0) y también sale `near_edge`. Así la meta tiene posición aunque,
  con el desfase estimado, caiga unas centésimas después del último punto, y el corte en tramos
  (`docs/segmentacion.md`) tiene el último tramo. Su `track_time` no cambia.
- Más lejos, o dentro de un hueco, sigue siendo `no_signal` (sin `location` si cae fuera).

Con el FIT sintético recortado a 2 s después de la meta o justo en ella (y desplazado −7, 0 y
+7 s), la meta queda situada, el último tramo se corta y el desfase cambia menos de 0,1 s.

## Resultado

| Campo | Qué es |
| --- | --- |
| `offset_s` | Desfase reloj − cronometraje (s). |
| `offset_estimated` | `false` si no se ha estimado: no había picadas útiles suficientes (el desfase es 0) o se ha fijado a mano. |
| `confidence` | 0–1 (ver arriba); 0 si no se ha estimado. |
| `quality` | `controls_used`, `support`, `margin`, `runner_up_offset_s`. |
| `coverage` | Ver arriba; instantes en hora del reloj. |
| `punches` | Una entrada por picada, en el orden de `RaceResult::punches`. |
| `warnings` | Avisos: `kind`, sus datos y `message` en español. |

Cada `AlignedPunch` lleva el código, la hora de la picada (`punch_time`, UTC), su instante en el
reloj (`track_time = punch_time + offset_s`), su posición en el track (`location`: entre
`points[index]` y `points[index + 1]` a la fracción `fraction`, e `in_gap` si esos dos puntos
están separados más de `max_gap_s`), su papel (`usage`: `used`, `start`, `no_time`,
`near_edge` o `no_signal`, ver "Picadas en el borde del track"; o `fixed` con el desfase fijado a
mano) y su desfase propio. Las picadas sin hora no tienen `track_time` ni `location`. La
posición geográfica de cada baliza y el corte en tramos salen de `location`
(`docs/segmentacion.md`).

En JSON (campos en `snake_case`, instantes en RFC 3339):

```json
{
  "offset_s": 7.14,
  "offset_estimated": true,
  "confidence": 1.0,
  "quality": {"controls_used": 21, "support": 0.95, "margin": 0.87, "runner_up_offset_s": 29.0},
  "coverage": {"track_start": "2026-10-03T16:12:07Z", "...": "..."},
  "punches": [
    {"code": 32736, "punch_time": "2026-10-03T16:13:00Z", "track_time": "2026-10-03T16:13:07.140Z",
     "location": {"index": 60, "fraction": 0.14, "in_gap": false}, "usage": "start",
     "local_offset_s": null}
  ],
  "warnings": [
    {"kind": "punches_without_time", "count": 1,
     "message": "1 picada(s) sin hora: no se usan para alinear."}
  ]
}
```

## Avisos

| `kind` | Cuándo |
| --- | --- |
| `start_without_time` / `finish_without_time` | La salida o la meta no tienen hora. |
| `punches_without_time` | Hay balizas intermedias sin hora (se ignoran). |
| `track_starts_late` / `track_ends_early` | Faltan más de `coverage_tolerance_s` (5 s) de carrera al principio o al final. |
| `track_gaps` | Hay huecos de más de `max_gap_s` (10 s) durante la carrera. |
| `offset_not_estimated` | Menos de `min_controls` (3) picadas útiles: desfase 0. |
| `few_controls` | Menos de `few_controls` (5) picadas útiles. |
| `large_offset` | \|desfase\| > `large_offset_s` (30 s). |
| `offset_at_search_limit` | El desfase está a 1 s o menos del límite de la búsqueda: el real puede estar fuera (en lugar de `large_offset`). |
| `ambiguous_offset` | Separación < 0,15: hay otro desfase casi igual de bueno. |
| `low_confidence` | Confianza < `low_confidence` (0,5). |

## Errores

`InvalidOptions` (opciones fuera de rango), `EmptyTrack`, `NoPunchTimes`, `NoRaceWindow` y
`TrackOutsideRace` (el track no se solapa con `[salida − max_offset_s, meta + max_offset_s]`).

Con `TrackOutsideRace` se prueban desplazamientos de +1 h, −1 h, +2 h y −2 h, en ese orden
(`SUGGESTED_SHIFTS_S`), por si la hora se ha convertido mal (horario de verano o zona horaria mal
elegida). El primero con el que el track sí se solaparía va en `suggested_shift_s`, con el
convenio de `offset_s` (hora del track = hora de la picada + desplazamiento), y el mensaje lo
explica en español. Si ninguno solapa, `suggested_shift_s` es `None` y el mensaje pregunta si el
FIT es de otra carrera. El desplazamiento **no se aplica**: decide el usuario. En la app, la
vista de carrera lo ofrece como botón (#68, `docs/app.md`).

Con el desfase fijado a mano: `InvalidOffset` (no es finito o pasa de `MAX_FIXED_OFFSET_S`, un
día) y `TrackOutsideFixedOffset` (con ese desfase, `[salida + δ, meta + δ]` no se solapa con el
track; sin margen de búsqueda).

## Desfase fijado a mano

`align_with_offset(&Track, &RaceResult, offset_s, &AlignmentOptions)` (#68) es para cuando la
estimación falla o no convence (track con huecos, reloj parado, hora mal convertida): el usuario
fija el desfase y las picadas se sitúan con él, con el mismo convenio
(`track_time = punch_time + offset_s`).

- No se calcula la señal ni se estima nada: `offset_estimated = false`, `confidence = 0`,
  `quality` vacía (0 picadas usadas, sin alternativa) y `local_offset_s` nulo.
- Ventana, cobertura y posición de cada picada (`location`, también el ajuste al borde del
  track de `EDGE_SNAP_S`) se calculan igual que en `align`, así que el corte en tramos y las
  métricas no cambian de forma: solo el instante de cada picada.
- `usage` es `fixed` para toda picada con hora y `no_time` para las demás.
- Avisos: solo los de la ventana y la cobertura (salida o meta sin hora, picadas sin hora, track
  que empieza tarde o acaba pronto, huecos). Los del desfase (`large_offset`, `low_confidence`…)
  no salen: el desfase lo ha elegido el usuario.

Aplicar la sugerencia de ±1/2 h de `TrackOutsideRace` no es fijar solo esas horas: la app
desplaza las picadas esas horas, vuelve a estimar con `align` el desfase fino que queda y fija la
suma (`docs/app.md`).

Con el FIT sintético desplazado 1 h, fijar 1 h más el desfase estimado sin desplazar sitúa cada
picada en el mismo punto del track que la alineación automática sin desplazar, y el corte da los
mismos 21 tramos. Con 30 s más, cada picada cae unos 30 puntos más allá.

## Opciones (`AlignmentOptions`, valores por defecto)

| Opción | Valor | Qué es |
| --- | --- | --- |
| `max_offset_s` | 60 | Semiancho de la búsqueda (s, máximo 600). |
| `turn_window_s` | 5 | Ventana del giro (s, 1–60). |
| `smoothing_s` | 3 | Semiancho del suavizado (s, 0–60). |
| `min_displacement_m` | 5 | Desplazamiento a partir del cual el giro pesa entero. |
| `max_gap_s` | 10 | Separación entre puntos que se considera hueco. |
| `coverage_tolerance_s` | 5 | Carrera sin track que se tolera sin aviso. |
| `large_offset_s` | 30 | Umbral del aviso de desfase grande. |
| `min_controls` | 3 | Picadas útiles para estimar. |
| `few_controls` | 5 | Por debajo, aviso. |
| `low_confidence` | 0,5 | Por debajo, aviso. |
| `use_start_punch` | `false` | Usar la salida para estimar el desfase. |

En JSON se pueden pasar parcialmente: los campos que faltan toman el valor por defecto.

## Pruebas

- FIT sintético desplazado −7, 0, +7 y +45 s: desfase a ±2 s (sale a +0,14 s), confianza ≥ 0,8.
- Con ruido de GPS (±4 m) y sin cadencia: sigue a ±2 s.
- Track recortado al final o al principio y con un hueco de 40 s: avisos y picadas sin señal.
- Track que acaba 2 s después de la meta o justo en ella: la meta sale `near_edge`, situada, y el
  desfase cambia menos de 1 s. Si acaba 5 s antes, la meta queda sin posición.
- Picadas sin hora (también salida y meta): se ignoran y el desfase sigue a ±2 s.
- Verano e invierno: el mismo .spl con la fecha cambiada al 12 de diciembre (UTC+1) da las
  mismas horas locales, 1 h de diferencia en UTC y el mismo desfase; convertido con la hora de
  verano, el track no se solapa.
- Picadas de otros corredores: confianza < 0,5 y aviso.
- Desfase fijado a mano: con el track desplazado 1 h y el desfase fijado a 1 h más el estimado,
  las picadas caen donde las sitúa la alineación automática del track sin desplazar; un desfase
  que deja la carrera fuera del track es un error, y uno no finito o de más de un día también.
- Par real opcional (`private_alignment`): `fixtures/private/soria-intermedia.fit` y `.spl`; la
  categoría (nombre, nombre corto o id) y la posición del resultado en ella van en
  `TRAMOS_PRIVATE_CLASS` y `TRAMOS_PRIVATE_RESULT_INDEX`. Si falta algo, se salta. Solo imprime
  datos genéricos (desfase, confianza, avisos y el residuo de cada picada: su desfase propio
  menos el global, en segundos), nunca nombres ni coordenadas:

  ```bash
  TRAMOS_PRIVATE_CLASS=<categoría> TRAMOS_PRIVATE_RESULT_INDEX=<n> \
      cargo test -p tramos-core --test alignment private_alignment -- --nocapture
  ```
