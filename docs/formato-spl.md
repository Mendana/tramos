# Formato .spl de WinSplits (cabecera `spl4`)

No hay especificación pública. Este documento se dedujo de un fichero real (Liga Madrid MTBO,
Chinchón, 3 de octubre de 2026: 13 categorías, 54 corredores, 927 tramos) y se comprueba con el
lector de referencia `tools/reference/winsplits_spl.py`. Si un fichero nuevo trae una etiqueta
desconocida, el lector debe fallar indicando la etiqueta y su posición, nunca ignorarla.

## Codificación

- Secuencia de registros `<etiqueta: 1 byte><valor>`.
- Texto: `u16` longitud + bytes Latin-1.
- Enteros little-endian. `f64` IEEE-754 little-endian.
- Fechas en formato OLE: días desde 1899-12-30 en `f64`.

## Cabecera

- Bytes 0–3: `spl4`.
- Offset `0x4C`: `f64` fecha de la carrera (OLE).
- El resto de la cabecera (nombre de la prueba, organizador, software de origen) aún no está
  mapeado por etiquetas. Los registros de categoría empiezan en el primer byte `0x40` seguido
  de 4 bytes y de `0x43`.

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
| `0x98` | u8 | estado: 0 = clasificado; 10 en los no presentados; 6 aparece en no clasificados |
| `0x99` | u16 | puesto |
| `0x9a` | u8 | sexo: 1 = M, 2 = F |
| `0x9b` | f64 | fecha de nacimiento (OLE). **Se descarta al importar.** |

Códigos especiales de baliza: `32736` = salida, `32752` = meta.

## Notas

- No trae coordenadas de balizas ni longitudes de tramo.
- Las horas son absolutas (hora local de la carrera). Sirven para alinear con el FIT, que va en UTC.
- Varias categorías comparten recorrido: en el ejemplo, 13 categorías en 6 recorridos.
- El último registro del fichero de ejemplo está truncado; el lector debe tolerarlo.
