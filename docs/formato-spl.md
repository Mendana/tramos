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
| `0x80` | u32 | id |
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
  `DateTime<Utc>`. Ejemplos: 17:31:00 del 3-10-2026 (verano, UTC+2) → 15:31:00Z;
  10:00:00,12 del 12-12-2026 (invierno, UTC+1) → 09:00:00,12Z. `0xFFFFFF` → `time = None`.
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

  Un corredor sin `0x98` es un error.
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
  byte). Solo se tolera, como el lector de referencia, que el fichero acabe en una etiqueta sin
  valor (en el cuerpo; en la cabecera es un error).
