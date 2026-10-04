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
| `Runner` | Corredor en una carrera: valor `id` de la fuente (**no es clave única**, ver Decisiones), nombre, apellidos, club, dorsal, tarjeta SI y sexo. |
| `Sex` | `male` o `female`. |
| `Punch` | Picada: código de baliza e instante UTC, `None` si no se registró. |
| `RaceResult` | Resultado: corredor, estado, puesto (solo clasificados) y picadas en orden de salida a meta. |
| `RaceStatus` | `ok` (clasificado), `not_classified`, `did_not_start` o `unknown(código)` para los estados aún sin interpretar. |
| `Leg` | Tramo: número (desde 1, el que sale de la salida), baliza de origen y destino, split en segundos opcional. |
| `Track` | Registro del reloj: puntos ordenados por instante. |
| `TrackPoint` | Punto con posición: instante, lat/lon en grados WGS84 y, opcionales, altitud (m), pulso (ppm), cadencia (pasos/min, ambos pies) y distancia acumulada (m). |

## Decisiones

- `Runner` es la persona tal y como aparece en una carrera; lo que depende de la participación
  (estado, puesto, picadas) va en `RaceResult`.
- Solo `RaceStatus::Ok` cuenta para la referencia del tiempo perdido (`docs/tiempo-perdido.md`).
  La correspondencia con los códigos del .spl la fija su lector (`docs/formato-spl.md`).
- Un registro del FIT sin posición no produce `TrackPoint` (`docs/formato-fit.md`).
- **`Runner::id` no identifica al corredor.** Guarda el campo `0x80` del .spl, que es la
  longitud del registro y se repite entre corredores, también en la misma categoría
  (`docs/formato-spl.md`). El nombre es heredado; se conserva para no perder el dato, pero no
  se usa como clave. Un resultado se identifica por su posición en la carrera: índice de la
  categoría en `Event::classes` e índice en `Class::results` (es lo que usan `ClassRef` y el
  almacenamiento). La identidad entre carreras es la persona (`docs/almacenamiento.md`).

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
