# Modelo de dominio

Tipos compartidos por importadores, análisis y app (`tramos_core::model`). Todos derivan
`serde`; en JSON los campos y variantes van en `snake_case`.

## Convenciones

- **Instantes**: `DateTime<Utc>`, en JSON como RFC 3339 (`2026-10-03T09:00:00Z`). La hora local
  del .spl (zona `Europe/Madrid`) se convierte a UTC en el importador (`docs/formato-spl.md`).
- **Duraciones**: segundos en `f64`.
- **Unidades**: van en el nombre del campo (`split_s`, `altitude_m`, `heart_rate_bpm`…).
- **Datos personales**: ningún tipo tiene fecha de nacimiento (ver `docs/datos-y-privacidad.md`).

## Tipos

| Tipo | Qué es |
| --- | --- |
| `ControlCode` | Código numérico de una baliza (`u16`). `START_CODE` = 32736 (salida) y `FINISH_CODE` = 32752 (meta). |
| `Event` | Carrera: nombre opcional, fecha local (`NaiveDate`) y sus categorías. |
| `Class` | Categoría: id del fichero de origen, nombre, nombre corto, recorrido y resultados. |
| `Course` | Recorrido: códigos de las balizas en orden, **sin salida ni meta**; con `n` balizas hay `n + 1` tramos. |
| `Runner` | Corredor en una carrera: nombre, apellidos, club, dorsal, tarjeta SI y sexo. **Sin id** (ver Decisiones). |
| `Sex` | `male` o `female`. |
| `Punch` | Picada: código de baliza e instante UTC, `None` si no se registró. |
| `RaceResult` | Resultado: corredor, estado, puesto (solo clasificados) y picadas en orden de salida a meta. |
| `RaceStatus` | `ok` (clasificado), `not_classified`, `did_not_start` o `unknown(código)` para los estados aún sin interpretar. |
| `Leg` | Tramo: número (desde 1, el que sale de la salida), baliza de origen y destino, split en segundos opcional. |
| `Track` | Registro del reloj: puntos ordenados por instante y deporte de la actividad (`sport`, opcional; ver `docs/formato-fit.md`). |
| `TrackPoint` | Punto con posición: instante, lat/lon en grados WGS84 y, opcionales, altitud (m), pulso (ppm), cadencia (pasos/min, ambos pies) y distancia acumulada (m). |

## Decisiones

- `Runner` es la persona tal y como aparece en una carrera; lo que depende de la participación
  (estado, puesto, picadas) va en `RaceResult`.
- Solo `RaceStatus::Ok` cuenta para la referencia del tiempo perdido (`docs/tiempo-perdido.md`).
  La correspondencia con los códigos del .spl la fija su lector (`docs/formato-spl.md`).
- Un registro del FIT sin posición no produce `TrackPoint` (`docs/formato-fit.md`).
- **`Runner` no tiene id.** El .spl no trae ninguno: el campo `0x80`, que antes se guardaba
  como `Runner::id`, es la longitud del registro y se repite entre corredores
  (`docs/formato-spl.md`); se quitó en #64. Un resultado se identifica por su posición en la
  carrera: índice de la categoría en `Event::classes` e índice en `Class::results` (es lo que
  usan `ClassRef`, `ResultRef` y el almacenamiento). La identidad entre carreras es la persona
  (`docs/almacenamiento.md`).

## Agrupación por recorrido

`tramos_core::courses::group_by_course(&Event) -> Vec<CourseGroup>` reúne las categorías que
corren el mismo recorrido, que es la unidad sobre la que se calcula el tiempo perdido
(`docs/tiempo-perdido.md`).

| Tipo | Qué es |
| --- | --- |
| `CourseGroup` | Un recorrido (`course`) y las categorías que lo corren (`classes`). |
| `ClassRef` | Categoría dentro del `Event`: posición en `Event::classes` (`index`), `id` y `name`. |

- **Criterio**: dos categorías comparten recorrido si su `Course` es **exactamente igual**:
  mismos códigos de baliza, en el mismo orden y en el mismo número. Las mismas balizas en otro
  orden, o un recorrido que es prefijo de otro, son recorridos distintos. No se intenta detectar
  variantes (mariposas, horquillas) ni recorridos "casi iguales".
- **Orden**: los grupos salen en el orden de la primera categoría de cada recorrido en el
  fichero; dentro de cada grupo, las categorías conservan su orden en `Event::classes`.
- **Recorrido vacío**: las categorías sin balizas forman un grupo más, como cualquier otro.
- **Acceso a los datos**: `CourseGroup::classes_in(&event)` devuelve las categorías y
  `CourseGroup::results_in(&event)` los resultados de todas ellas, que son la población de la
  referencia del tiempo perdido. El filtrado por estado lo hace quien lo consume. Hay que
  pasar el mismo `Event` que se agrupó (se busca por `index`; los índices fuera de rango se
  ignoran).
- En JSON: `{"course": {"controls": [31, 45]}, "classes": [{"index": 0, "id": 7, "name": "F21A"}]}`.

## Identificación del corredor

`tramos_core::identify::identify_runner(&Event, &RunnerIdentity) -> Identification` propone qué
resultado de la carrera es del usuario a partir de su tarjeta SI y su nombre. Señala los
resultados con `ResultRef` (`class_index`, `result_index`). Reglas,
normalización de nombres y formato JSON en `docs/identificacion.md`.

## Tramos y posición de las balizas

`tramos_core::segmentation::segment(&Track, &Alignment) -> Result<Segmentation, _>` corta el
track en un sub-track por tramo (límites interpolados en cada picada y compartidos por los dos
tramos contiguos) y sitúa cada baliza en la posición del corredor en el instante de su picada.
`median_control_positions` combina las balizas de varios corredores del mismo recorrido con la
mediana. Algoritmo, casos borde y formato JSON en `docs/segmentacion.md`.

## Métricas de tramo

`tramos_core::metrics::leg_metrics(&Track, &Segmentation, &MetricsOptions)` mide cada sub-track:
distancia recorrida y en línea recta, tiempo en movimiento, parado y en huecos, velocidad en
movimiento, subida y bajada con la altitud suavizada, pulso y cadencia medios. Definiciones y
JSON en `docs/metricas.md`.
