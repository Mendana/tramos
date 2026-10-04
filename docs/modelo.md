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
| `Runner` | Corredor en una carrera: id del fichero, nombre, apellidos, club, dorsal, tarjeta SI y sexo. |
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
- Un registro del FIT sin posición no produce `TrackPoint`.
