# Identificar al corredor dentro de una carrera

Al importar un .spl hay que saber cuál de sus resultados es el del usuario. Lo propone
`tramos_core::identify::identify_runner(&Event, &RunnerIdentity) -> Identification`; la
interfaz confirma o pregunta y el vínculo con la persona lo hace quien llama
(`docs/almacenamiento.md`, Personas).

## Entrada: la identidad configurada

`RunnerIdentity` es lo que el usuario escribe en los ajustes (#17). Los dos campos son
opcionales:

| Campo | Qué es |
| --- | --- |
| `si_card` | Número de su tarjeta SportIdent. |
| `full_name` | Nombre y apellidos, con la grafía que quiera: `"José Ángel Muñoz Castaño"`, `"Muñoz Castaño, José Ángel"`… |

En JSON: `{"si_card": 2001234, "full_name": "Ana Pérez García"}`. Un campo `null` o ausente es
«no configurado»; un nombre que se queda vacío al normalizar (solo espacios o signos), también.

## Salida

Los resultados se señalan por su **posición** en la carrera, `ResultRef { class_index,
result_index }` (índice en `Event::classes` y en `Class::results`). Nunca por `Runner::id`, que
no es único (`docs/formato-spl.md`).

| `Identification` | Cuándo | Qué hace la interfaz |
| --- | --- | --- |
| `unique { candidate, name_mismatch }` | Un solo resultado casa. | Lo propone. Si `name_mismatch`, pide confirmación (ver abajo). |
| `ambiguous { candidates }` | Varios casan, en el orden de la carrera. | Pregunta cuál. |
| `not_found` | Ninguno casa o no hay identidad configurada. | Deja elegir de la lista completa. |

Cada `Candidate` lleva su `result` y si casa por tarjeta (`si_card_matches`) y por nombre
(`name_matches`). En JSON el tipo va en `kind`:

```json
{"kind": "unique",
 "candidate": {"result": {"class_index": 0, "result_index": 2},
               "si_card_matches": true, "name_matches": false},
 "name_mismatch": true}
{"kind": "ambiguous", "candidates": [ … ]}
{"kind": "not_found"}
```

## Reglas

1. **Tarjeta primero.** Si hay tarjeta configurada y algún resultado la lleva, se parte de esos.
   - Sin nombre configurado: esos son los candidatos.
   - Con nombre: si alguno casa también por nombre, solo cuentan los que casan por las dos
     cosas (así una tarjeta repetida en la carrera se resuelve por el nombre).
   - Si ninguno de la tarjeta casa por nombre, la tarjeta puede ser **prestada o
     reutilizada**: los candidatos son los de la tarjeta más los que casan por nombre. Si al
     final hay uno solo (la tarjeta, y nadie con el nombre), sale como `unique` con
     `name_mismatch = true`: es un **aviso**, no un descarte, porque el nombre del .spl puede
     venir mal escrito o con otra grafía. Si el nombre casa con otro resultado (el usuario
     prestó su tarjeta y corrió con otra), salen los dos como `ambiguous`.
2. **Nombre si la tarjeta no sirve.** Sin tarjeta configurada, o si nadie la lleva (por ejemplo,
   el usuario corrió con una alquilada), cuentan los resultados que casan por nombre.
3. Con un candidato es `unique`, con varios `ambiguous` y con ninguno `not_found`. Dos personas
   con el mismo nombre en categorías distintas dan `ambiguous` con las dos.

## Nombres

El .spl trae el nombre (`0x87`) y los apellidos (`0x88`) por separado; el usuario escribe un
solo texto. Se comparan normalizados (`normalize_name`):

- Minúsculas y sin diacríticos: `José` = `Jose`, `Muñoz` = `Munoz` (la `ñ` pasa a `n`, así que
  `Peña` = `Pena`), `ç` → `c`, `ü` → `u`, y el resto de letras de Latin-1 (`æ` → `ae`,
  `ß` → `ss`, `ø` → `o`…). También se quitan las marcas combinantes, por si el texto llega
  descompuesto (NFD). Las letras fuera de Latin-1 se quedan como están, en minúscula: un .spl
  no puede contenerlas.
- Todo lo que no es letra ni dígito separa palabras (espacios, tabuladores, guiones,
  apóstrofos, comas, puntos, `_`), y se colapsa en un solo espacio.

Un resultado casa si `nombre + apellidos` **o** `apellidos + nombre`, normalizados, son
exactamente el nombre configurado normalizado. Así:

| Casa | No casa |
| --- | --- |
| `JOSE ANGEL  munoz castaño` | `José Muñoz Castaño` (falta un nombre) |
| `Íñigo De La Peña Gómez` con apellidos `de la Peña-Gómez` (guion = espacio) | `José Ángel Muñoz` (falta un apellido) |
| `de la Peña Gómez, Íñigo` (apellidos primero) | `Íñigo Peña Gómez` (falta la partícula) |
| `María José Ruiz` aunque el .spl parta mal nombre y apellidos | `Íñigo Gómez de la Peña` (apellidos en otro orden) |
| `D'Ávila` = `d avila` = `D-Ávila` | `DÁvila` (sin separador), `Mª` frente a `María` |

No hay coincidencia parcial ni aproximada (un solo apellido, erratas, abreviaturas): daría
falsos positivos con nombres comunes. En esos casos sale `not_found` y la interfaz deja elegir.
