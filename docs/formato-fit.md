# Formato FIT del reloj

El FIT (Flexible and Interoperable Data Transfer, de Garmin/ANT) es el fichero de actividad que
graba el reloj del corredor: posición, altitud, pulso y cadencia, normalmente cada segundo. El
lector del núcleo es `tramos_core::importers::fit::read(&[u8]) -> Result<Track, FitError>` y
decodifica con el crate [`fitparser`](https://crates.io/crates/fitparser) (MIT; sus dependencias
son MIT o MIT/Apache-2.0).

El fixture público es `fixtures/fit/baltanas-sintetico.fit` (sintético, generado por
`tools/fit_sintetico.py`), con su respuesta conocida en `baltanas-sintetico.truth.json`. Los
tests están en `crates/tramos-core/tests/fit_import.rs`.

## Qué se lee

Solo los mensajes `record` (número global 20). Cada uno con instante y posición válidos da un
`TrackPoint` (`docs/modelo.md`). El resto de mensajes se decodifica pero no se usa:

- `file_id`, `event` (timer), `lap`, `session`, `activity`… no aportan al track.
- Los mensajes que no están en el perfil FIT (los propietarios de Garmin: 113, 140, 147,
  160, 216, 233, 288, 312, 313, 325, 326, 327, 394, 499…) se ignoran sin error.
- Los campos que no están en el perfil, también en `record` (los de Garmin de números 107,
  134–143…), se descartan.
- Si un mensaje que no es `record` tiene un campo que `fitparser` no sabe convertir (por
  ejemplo, un enumerado escrito con otro tipo base), se ignora ese mensaje. En un `record`, en
  cambio, es un error: preferimos fallar a perder puntos en silencio.

Todos los mensajes se decodifican aunque se descarten porque cualquiera con `timestamp` fija
la referencia de las cabeceras de tiempo comprimido. Los ficheros encadenados (varios FIT
seguidos, cada uno con su cabecera y su CRC) se leen enteros. Las cabeceras y el CRC se
validan: un fichero truncado o corrupto es un error (`FitError::Decode`).

## Campos de `record` y unidades

`fitparser` aplica la escala y el desplazamiento del perfil a todos los campos salvo la
posición. Un campo con el valor «no válido» del FIT (`0xFF` en un `uint8`, `0x7FFFFFFF` en un
`sint32`…) se trata como ausente.

| Campo FIT (número) | `TrackPoint` | Conversión |
| --- | --- | --- |
| `timestamp` (253) | `time` | Segundos desde 1989-12-31T00:00:00Z → `DateTime<Utc>`. El FIT ya va en UTC: no hay conversión de zona. |
| `position_lat` (0), `position_long` (1) | `lat`, `lon` | Semicírculos × 180 / 2^31 → grados WGS84. Si viniera en grados (unidad `deg`), tal cual. |
| `enhanced_altitude` (78) o `altitude` (2) | `altitude_m` | Metros. Se prefiere `enhanced_altitude`. |
| `heart_rate` (3) | `heart_rate_bpm` | Pulsaciones por minuto. |
| `cadence` (4) + `fractional_cadence` (53) | `cadence_spm` | `2 × (cadence + fractional_cadence)` pasos por minuto. |
| `distance` (5) | `distance_m` | Metros acumulados desde el inicio del registro. |

Los demás campos de `record` (velocidad, potencia, dinámica de carrera…) no se leen por ahora.

## Decisiones

- **Registros sin posición.** Un `record` sin `position_lat` o `position_long` válidos (el reloj
  aún sin GPS, una pausa sin señal) o con la posición fuera de rango no produce punto, aunque
  traiga pulso o cadencia. Tampoco uno sin `timestamp`. Los demás campos son opcionales.
- **Altitud `enhanced_*`.** Los relojes actuales escriben `enhanced_altitude` (`uint32`) en vez
  de `altitude` (`uint16`), y `fitparser` expande `altitude` a `enhanced_altitude`. Se acepta
  cualquiera de las dos; si vienen ambas, gana `enhanced_altitude`.
- **Cadencia × 2.** En carrera el FIT guarda la cadencia en rpm de un pie (zancadas de una
  pierna por minuto, unas 85–95 corriendo). El modelo la quiere en pasos por minuto contando
  los dos pies (unas 170–190), así que se suma `fractional_cadence` (si viene) y se multiplica
  por 2. Un `fractional_cadence` sin `cadence` no da cadencia.
- **Deporte.** `Track::sport` es el campo `sport` del primer mensaje `session` que lo traiga
  o, si no hay ninguno, el del mensaje `sport`. Va con el nombre del perfil FIT que da
  `fitparser` (`running`, `cycling`…) o, si no lo conoce, con el número del enum. La cadencia
  solo se multiplica por 2 en los deportes a pie (`generic`, `running`, `walking`, `hiking` y
  `mountaineering`) o si el FIT no trae deporte, que es lo normal en orientación a pie. En los
  demás (por ejemplo, `cycling`, donde la cadencia ya son rpm de pedal) se guarda tal cual.
- **Orden.** Los puntos se ordenan por instante con un orden estable: si dos `record` comparten
  instante, se conservan los dos en el orden del fichero.
- **Sin `record` con posición.** El resultado es un `Track` vacío, no un error.
