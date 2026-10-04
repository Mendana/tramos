# Corte del track en tramos y posición de las balizas

`tramos_core::segmentation::segment(&Track, &Alignment) -> Result<Segmentation,
SegmentationError>` sitúa cada baliza en la posición del corredor en el instante de su picada y
corta el track en un sub-track por tramo. `median_control_positions` combina las posiciones de
varios corredores del mismo recorrido. Parte de la alineación (`docs/alineacion.md`) y lo
consumen las métricas de tramo (#13). Los tests están en `crates/tramos-core/src/segmentation.rs`
y `crates/tramos-core/tests/segmentation.rs`.

En el MVP no hay mapa: la única fuente de la posición de una baliza es el GPS del corredor.

## Tiempos

Todo va en hora del reloj: el instante de cada picada es su `track_time` de la alineación
(`punch_time + offset_s`). Los puntos del track no se tocan.

## Balizas

Hay una `ControlPosition` por picada, en el orden de `Alignment::punches` (salida, balizas del
recorrido, meta). `position` es su posición en esa secuencia: 0 = salida, 1 = primera baliza del
recorrido (`Course::controls[0]`), …, la última = meta.

**Posición** = la del corredor en el instante de la picada, interpolada linealmente entre los
dos puntos del track que lo rodean (`location`: `points[index]` y `points[index + 1]` a la
fracción `fraction`):

- Latitud y longitud se interpolan en grados. Entre dos puntos separados unos segundos el error
  frente a la geodésica es despreciable.
- La altitud solo si los dos puntos la tienen; si falta en alguno, `altitude_m` es `None`.
- Con `fraction = 0` (también en el último punto del track) es el propio punto.

Se usa `location`, no `usage`: una picada `near_edge` (cerca del borde del track, como la meta
cuando el reloj se para al picarla) o `no_signal` con `location` (todas si el desfase no se pudo
estimar y es 0) se sitúa igual. Sin `location`
no hay posición y `missing` dice por qué:

| `missing` | Cuándo |
| --- | --- |
| `no_punch_time` | La picada no tiene hora en el cronometraje. |
| `outside_track` | Tiene hora, pero con el desfase cae fuera del track. |

**Salida** (`role: start`, código 32736). Su hora en el .spl es la de salida asignada, no una
picada física. Se sitúa igual que las demás: es dónde estaba el corredor a su hora de salida
(con el desfase de la alineación), que normalmente es el triángulo o la línea de salida. Si el
corredor salió tarde, la posición es la de donde esperaba. El tramo 1 empieza en esa hora, que
es también el origen del split del .spl, así que la duración del sub-track coincide con el
split. `role` es `control` para las balizas del recorrido y `finish` para la meta (32752).

**Huecos** (`in_gap`). Si la picada cae en un hueco del track (dos puntos separados más de
`max_gap_s`), la posición se interpola igual, pero es una línea recta entre dos puntos lejanos y
no es fiable: la baliza sale con `in_gap: true` y la mediana de grupo no la usa. El corte del
tramo sí se hace en ese punto (ver abajo).

## Tramos

Un `LegTrack` por cada par de picadas consecutivas: con `n` balizas, `n + 1` tramos (salida →
primera, …, última → meta). `index` empieza en 1 y `from`/`to` son los códigos, como en `Leg`.

**Sub-track** de un tramo:

1. El límite inicial: el punto interpolado en la picada de `from` (con todas las magnitudes del
   `TrackPoint` interpoladas: altitud, pulso redondeado, cadencia y distancia acumulada, cada una
   solo si la tienen los dos puntos) y con el instante de la picada en hora del reloj.
2. Los puntos del track estrictamente posteriores al límite inicial y anteriores al final.
3. El límite final: el punto interpolado en la picada de `to`.

El límite final de un tramo es **el mismo punto** que el inicial del siguiente: la distancia es
continua y la suma de los tramos es el recorrido de la salida a la meta. Si una picada coincide
con un punto del track, ese punto no se repite. La duración del sub-track es el split (el
desfase es el mismo en los dos extremos).

Un límite en un hueco se corta igual: el sub-track va en línea recta hasta el punto interpolado,
como iría el track sin cortar, y la distancia sigue siendo continua. Los huecos dentro de un
tramo no se marcan aquí; los ve quien calcule métricas sobre el sub-track.

**Tramos sin límite**. Si no se puede situar alguno de los dos extremos, el tramo sale con
`track: null` y el motivo en `missing`; los demás tramos no se ven afectados. Una picada sin
hora deja sin sub-track sus dos tramos (no se fusionan en uno).

| `missing.reason` | Cuándo |
| --- | --- |
| `no_punch_time` | Un extremo (`code`) no tiene hora. Si fallan los dos, el de `from`. |
| `outside_track` | Un extremo (`code`) cae fuera del track. |
| `out_of_order` | El final es anterior al principio (horas de picada desordenadas). |

Los tramos salen de las picadas del resultado, no de `Course`: para un corredor clasificado son
lo mismo. Con picadas que no casan con el recorrido (baliza fallida, picadas de más) los tramos
siguen las picadas tal cual.

## Errores

Solo si la alineación no corresponde al track que se pasa:

- `LocationOutsideTrack`: una picada apunta a un punto (o al siguiente, con `fraction > 0`) que
  el track no tiene.
- `InvalidFraction`: `fraction` fuera de 0–1 (o no finita).

## Combinar corredores

`median_control_positions(runners) -> Vec<MedianControlPosition>` recibe las `controls` de
varios corredores del mismo recorrido (cualquier iterador de `&[ControlPosition]`):

- Agrupa por `(position, code)`: la misma baliza en el mismo punto del recorrido. Una baliza que
  se visita dos veces en el recorrido da dos grupos.
- No cuentan las balizas sin `point` ni las `in_gap`.
- Latitud, longitud y altitud: mediana por separado (con número par, media de las dos
  centrales). La altitud, de los corredores que la tienen; `None` si ninguno.
- `runners` = corredores que aportan posición.
- Salen ordenadas por `position` y, a igual posición, por `code`. Las que nadie sitúa no salen.

Quien la llama elige a los corredores: conviene pasar solo clasificados, porque una baliza
fallida puede descolocar la secuencia de picadas y sus balizas formarían grupos aparte. La
salida entra como las demás (todos salen del mismo triángulo; la mediana absorbe al que salió
tarde). Aún no se usa: hace falta el track de varios corredores de una carrera.

## Tipos y JSON

Campos en `snake_case`, instantes en RFC 3339:

```json
{
  "controls": [
    {"position": 0, "code": 32736, "role": "start", "time": "2026-10-03T16:13:00.140Z",
     "point": {"lat": 41.938, "lon": -4.249, "altitude_m": 812.4}, "in_gap": false,
     "missing": null},
    {"position": 3, "code": 32, "role": "control", "time": null, "point": null,
     "in_gap": false, "missing": "no_punch_time"}
  ],
  "legs": [
    {"index": 1, "from": 32736, "to": 49, "track": {"points": ["..."]}, "missing": null},
    {"index": 3, "from": 51, "to": 32, "track": null,
     "missing": {"reason": "no_punch_time", "code": 32}}
  ]
}
```

`MedianControlPosition`:
`{"position": 1, "code": 49, "lat": 41.93898, "lon": -4.25036, "altitude_m": 815.0, "runners": 4}`.

## Pruebas

- Unitarias con un track de cuatro puntos calculado a mano: interpolación (con y sin altitud),
  picada en un punto exacto, límites compartidos sin repetir puntos, interpolación de distancia
  y pulso, picada sin hora, fuera del track, desordenada, en un hueco, alineación que no casa
  con el track y mediana con número par e impar de corredores.
- FIT sintético (`fixtures/fit/baltanas-sintetico.fit` con su corredor del .spl): 22 balizas
  (salida y meta incluidas) y 21 tramos, todas las balizas a menos de 10 m de `truth.json`
  (máximo 0,31 m), duración de cada sub-track igual al split, recorrido de cada tramo a menos de
  1 m del de la verdad y suma de tramos igual al recorrido salida–meta.
- Mismo FIT con una picada sin hora y el track recortado antes de la meta: tres tramos sin
  sub-track con su motivo.
