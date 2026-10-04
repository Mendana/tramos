# Formato .spl de WinSplits (cabecera `spl4`)

No hay especificación pública. Este documento se dedujo de un fichero real (Liga Madrid MTBO,
Chinchón, 3 de octubre de 2026: 13 categorías, 54 corredores, 927 tramos), con la cabecera
completada a partir del sprint de Baltanás y de una prueba de Soria (19 de julio de 2026:
26 categorías, 465 corredores), y se comprueba con el lector de referencia
`tools/reference/winsplits_spl.py`. Si un fichero nuevo trae una etiqueta
desconocida, el lector debe fallar indicando la etiqueta y su posición, nunca ignorarla.

El fixture público es `fixtures/spl/baltanas-anon.spl` (sprint de Baltanás, anonimizado:
18 categorías, 275 corredores, 9 recorridos), con la salida del lector de referencia en
`fixtures/spl/baltanas-anon.expected.json`. El lector del núcleo (`tramos_core::importers::spl`)
tiene un test de paridad contra ese JSON.

## Codificación

- Secuencia de registros `<etiqueta: 1 byte><valor>`.
- Texto: `u16` longitud + bytes Latin-1.
- Enteros little-endian. `f64` IEEE-754 little-endian.
- Fechas en formato OLE: días desde 1899-12-30 en `f64`.

## Cabecera

- Bytes 0–3: `spl4`.
- Bytes 4–11: preámbulo de 8 bytes sin identificar. No es constante: `10 dc 00 00 00 00 00 00`
  en Baltanás, `10 d4 00 00 00 00 00 00` en Soria. Se salta sin interpretarlo.
- Desde el byte 12: registros `<etiqueta><valor>` hasta el marcador de fin `0x2c`.

La posición de cada campo depende de la longitud de los textos anteriores, así que la cabecera
se recorre **por etiquetas, nunca por offsets fijos**. (Las primeras versiones de los lectores
leían la fecha en el offset `0x4C`, donde cae en Baltanás por casualidad; en Soria la etiqueta
`0x19` está en `0x45` y en `0x4C` hay otros datos.)

| Etiqueta | Tipo | Significado | Baltanás / Soria |
| --- | --- | --- | --- |
| `0x14` | texto | nombre de la prueba | «Cto. SPRINT Liga Norte/Liga FOCYL Baltanas» |
| `0x18` | texto | organizador | un club |
| `0x1b` | texto | país | `ESP` |
| `0x19` | f64 | fecha de la carrera (OLE, parte entera). **Obligatoria.** | 46298,0 = 2026-10-03 / 46222,0 = 2026-07-19 |
| `0x22` | f64 | fecha y hora (OLE), ¿creación del fichero? | 2026-10-03 19:45:22 / 2026-07-19 13:52:44 |
| `0x23` | texto | software asociado a `0x22` | `WinSplits Online Upload 4.0` |
| `0x24` | f64 | fecha y hora (OLE), ¿subida o última modificación? | 2026-10-03 19:46:09 / 2026-07-19 13:53:19 |
| `0x25` | texto | software asociado a `0x24` | `WinSplits Online Upload 4.0` |
| `0x26` | texto | origen de los resultados | `IOFXML3 / SportSoftware OE2010 (M) V.11.0` / `… OE12 (M) V.12.1` |
| `0x27` | u8 | desconocido | 3 / 3 |
| `0x28` | u32 | desconocido (¿id del evento en WinSplits Online?; crece con la fecha) | 115954 / 114093 |
| `0x29` | u32 | desconocido | 0 / 0 |
| `0x1f` | u16 | número de categorías | 18 / 26 |
| `0x2b` | u16 | desconocido | 0 / 0 |
| `0x21` | u32 | posición del primer registro de categoría + 1 | `0x178` / `0x1b0` |
| `0x20` | n × (u32, u32) | tabla de categorías, con n = `0x1f`: desplazamiento y tamaño de cada registro de categoría (con sus corredores), contados desde el primer registro de categoría y acumulativos | |
| `0x2c` | — | fin de cabecera, **sin valor**: el primer registro de categoría (`0x40`) va en el byte siguiente | `0x176` / `0x1ae` |

Orden observado en los dos ficheros: `0x14 0x18 0x1b 0x19 0x22 0x23 0x24 0x25 0x26 0x27 0x28
0x29 0x1f 0x2b 0x21 0x20 0x2c`. Los lectores no dependen del orden, salvo que `0x20` necesita
haber leído antes `0x1f`.

Comprobado con el fixture de Baltanás: cada desplazamiento de la tabla `0x20` cae en un `0x40`,
y el tamaño de la última categoría pasa 10 bytes del final del fichero, justo lo que falta del
último corredor (el valor `u8` de `0x9a` y el registro `0x9b` completo): el fichero viene
truncado de origen (ver Notas). Los lectores no usan la tabla ni `0x21` para avanzar: localizan
el primer registro de categoría con el marcador `0x2c`.

## Categoría (empieza con `0x40`)

| Etiqueta | Tipo | Significado |
| --- | --- | --- |
| `0x40` | u32 | id de la categoría |
| `0x43` | texto | nombre |
| `0x44` | texto | nombre corto |
| `0x45` | u16 | desconocido |
| `0x4e` | u8 | desconocido |
| `0x4f` | u16 | desconocido |
| `0x50` | u16 | desconocido |
| `0x47` | u32 n + n bytes | tramos: bloques de 8 bytes (desde u16, hasta u16, longitud u32 = 0 en el ejemplo) |
| `0x48`, `0x49`, `0x4a` | u8 | desconocidos |
| `0x4d` | u32 | desconocido |

## Corredor (empieza con `0x80`)

| Etiqueta | Tipo | Significado |
| --- | --- | --- |
| `0x80` | u32 | longitud en bytes del resto del registro del corredor. **No es un id** (ver abajo) |
| `0x81` | u32 | dorsal (probable) |
| `0x84` | u32 | tarjeta SportIdent |
| `0x87` | texto | nombre |
| `0x88` | texto | apellidos |
| `0x89` | u32 | id del club |
| `0x8c` | texto | club |
| `0x8d` | texto | país |
| `0x8e` | texto | nacionalidad |
| `0x97` | u16 n + n × (código u16, hora u24) | picadas; hora en centésimas desde medianoche, hora local; `0xFFFFFF` = sin picada |
| `0x98` | u8 | estado: 0 = clasificado; 10 en los no presentados; 6 aparece en no clasificados (ver correspondencia abajo) |
| `0x99` | u16 | puesto |
| `0x9a` | u8 | sexo: 1 = M, 2 = F |
| `0x9b` | f64 | fecha de nacimiento (OLE). **Se descarta al importar.** |

Códigos especiales de baliza: `32736` = salida, `32752` = meta.

### `0x80` no es un id: es la longitud del registro

Los primeros lectores trataban `0x80` como «id del corredor», pero no identifica a nadie. En el
fixture de Baltanás:

- Toma **70 valores distintos para 275 corredores**, todos entre 108 y 208. 26 valores salen
  una sola vez y el más repetido, 16 veces.
- Se repite **dentro de una misma categoría** en 16 de las 18 (en M-SEN, el 203 es del 4.º y
  del 6.º, de clubes distintos) y entre categorías.
- No sigue al club, al sexo, al puesto ni a la hora de salida. Tiende a parecerse dentro de una
  categoría (en ALEVÍN va de 147 a 159; en M-SEN, de 163 a 208), porque sus corredores tienen el
  mismo número de picadas, pero los rangos de las categorías se solapan.
- Es exactamente el **número de bytes del resto del registro**: desde el byte siguiente a su
  `u32` hasta el siguiente `0x80` o `0x40`. Se cumple en 274 de los 275 corredores; el último
  declara 10 bytes más de los que quedan, justo el valor de `0x9a` y el registro `0x9b` que
  faltan por el truncado de origen (ver Notas), igual que la tabla `0x20` de la cabecera.
  El primer corredor, por ejemplo, declara 154 = `0x81` 5 + `0x84` 5 + `0x87` 3 + 4 +
  `0x88` 3 + 14 + `0x89` 5 + `0x8c` 3 + 16 + `0x8d` 3 + 3 + `0x8e` 3 + 3 + `0x97` 3 + 13 × 5 +
  `0x98` 2 + `0x99` 3 + `0x9a` 2 + `0x9b` 9.

Por eso se repite: dos corredores con nombres, club y número de picadas de la misma longitud
tienen el mismo valor. El anonimizador no cambia la longitud de ningún campo, así que la
propiedad vale igual en el fichero original. Se ha comprobado también con el fichero de Soria:
cuadran todos menos el último, al que le faltan los mismos 10 bytes por el
mismo truncado. Los lectores la **validan** (ver Importación); no la usan para avanzar, porque
cada campo ya dice su propia longitud.

Consecuencia: para el núcleo **no hay id de corredor en el .spl**. Un resultado se identifica
por su posición en la carrera (categoría e índice dentro de ella), y la identidad del corredor
entre carreras la da la persona a la que el usuario lo vincula (`docs/almacenamiento.md`).

## Notas

- No trae coordenadas de balizas ni longitudes de tramo.
- Las horas son absolutas (hora local de la carrera). Sirven para alinear con el FIT, que va en UTC.
- Varias categorías comparten recorrido: en el ejemplo, 13 categorías en 6 recorridos.
- El último registro de los ficheros de ejemplo está truncado: el fichero acaba en una etiqueta
  sin valor (en Baltanás, el `0x9a` del último corredor). El lector debe tolerarlo.

## Importación al modelo (`tramos_core::importers::spl`)

Cómo convierte el lector del núcleo cada campo a `docs/modelo.md`:

- **Cabecera**: se recorre por etiquetas hasta `0x2c`. La fecha OLE de `0x19` se queda en el día
  (`Event::date`; la parte fraccionaria se ignora) y el texto de `0x14` es `Event::name`
  (`None` si falta). El resto de etiquetas de la cabecera se leen para avanzar y no se guardan
  (el organizador no tiene uso en el modelo por ahora). Si la cabecera trae `0x1f`, el número de
  categorías leídas debe coincidir.
- **Horas → UTC**: cada hora de picada se interpreta como hora local de `Europe/Madrid` el día de
  la carrera (todas las carreras del grupo son en España peninsular) y se convierte a
  `DateTime<Utc>`. `read_with_time_zone(datos, zona)` usa otra zona (Canarias, Portugal…); la app
  la toma de sus ajustes (`docs/app.md`). Ejemplos: 17:31:00 del 3-10-2026 (verano, UTC+2) →
  15:31:00Z; 10:00:00,12 del 12-12-2026 (invierno, UTC+1) → 09:00:00,12Z. `0xFFFFFF` →
  `time = None`.
  Una hora de 24 h o más cae en el día siguiente.
- **Cambio de hora**: si una hora local no existe (último domingo de marzo, de 02:00 a 03:00) o es
  ambigua (último domingo de octubre, de 02:00 a 03:00, que ocurre dos veces), la lectura falla
  con un error que da la hora, la baliza y la posición. No se adivina: el fichero no trae el
  desfase y una carrera a esas horas es casi imposible; si apareciera, se decidirá con el caso real.
- **Estado** (`0x98` → `RaceStatus`):

  | Código | `RaceStatus` |
  | --- | --- |
  | 0 | `ok` |
  | 6 | `not_classified` |
  | 10 | `did_not_start` |
  | otro `n` | `unknown(n)` |

  Un corredor sin `0x98` es un error, que señala al corredor por la posición de su `0x80` y por
  su categoría.
- **`0x80`**: es la longitud del resto del registro y no pasa al modelo (`Runner` no tiene id).
  El lector comprueba que cada corredor ocupa exactamente lo que declara, desde el byte siguiente
  a su `u32` hasta el siguiente `0x80` o `0x40`. Si no cuadra, falla con
  `RunnerLengthMismatch`, que da la categoría, la posición del `0x80` y las dos longitudes. La
  única excepción es el **último corredor del fichero**, que puede ocupar menos de lo que
  declara si el fichero acaba antes (el truncado de origen de las Notas); más, nunca. No se
  avisa: es lo normal en los ficheros de WinSplits. El lector de referencia hace la misma
  comprobación y tampoco saca el valor en su JSON.
- **Puesto** (`0x99`): 0 → `None`. **Dorsal** (`0x81`) y **tarjeta** (`0x84`): 0 → `None`.
- **Sexo** (`0x9a`): 1 → `male`, 2 → `female`, otro valor o ausente → `None`.
- **Nombre y apellidos**: si faltan, cadena vacía. **Club** (`0x8c`): ausente → `None`.
  El id de club, el país y la nacionalidad se leen pero no se guardan.
- **Fecha de nacimiento** (`0x9b`): se lee para avanzar y se descarta.
- **Recorrido**: los tramos de `0x47` deben formar una cadena salida (32736) → … → meta (32752),
  sin salida ni meta intermedias. `Course::controls` son los destinos de los tramos sin la meta
  (el lector de referencia da los destinos con la meta al final). Una categoría sin tramos, con
  una cadena rota o sin nombre (`0x43`) es un error.
- **Errores** (`SplError`): cabecera que no empieza por `spl4`, demasiado corta o sin el
  marcador `0x2c`; sin fecha (`0x19`) o con una fecha OLE inválida; tabla `0x20` antes de `0x1f`;
  tras la cabecera no hay registros de categoría o el primero no es `0x40`; número de categorías
  distinto del de `0x1f`; etiqueta desconocida, en la cabecera o en el cuerpo (valor y byte);
  etiqueta de corredor fuera de un corredor y registro cortado a mitad de su valor (etiqueta y
  byte); corredor que no ocupa lo que declara su `0x80` (categoría, byte y longitudes). Solo se
  tolera, como el lector de referencia, que el fichero acabe en una etiqueta sin
  valor (en el cuerpo; en la cabecera es un error).
