# Resultados de OReplay

[OReplay](https://www.oreplay.es) publica resultados en directo de carreras de orientación. Es
código abierto (MIT: [backend](https://github.com/oreplay/oreplay-backend) en PHP,
[frontend](https://github.com/oreplay/oreplay-frontend) en React) y su web lee los resultados de
una API pública, sin autenticación. Tramos usa esa API para importar una carrera pegando su enlace
(#185), como alternativa al .spl de WinSplits (`docs/formato-spl.md`).

Solo se toman los **datos** (corredores, estados y picadas). Nada de lo que calcula OReplay
(posiciones por tramo, tiempo perdido…): el análisis es el de Tramos.

La API es la interna de su web: no está documentada para terceros y puede cambiar. Lo que sigue
se ha comprobado en octubre de 2026 con varias carreras reales y con el código del backend
(`plugins/Results`).

## Enlaces y API

La URL que se comparte es `https://www.oreplay.es/competitions/<evento>` (el evento) o
`…/competitions/<evento>/<etapa>` (los resultados de una etapa), con UUID.
`tramos_core::importers::oreplay::parse_url` acepta las dos, con o sin `www.` y con lo que vaya
detrás (más ruta, `?…`, `#…`).

| Petición (`GET`, bajo `https://www.oreplay.es/api/v1`) | Qué da |
| --- | --- |
| `/events/<evento>` | Nombre (`description`), `timezone` (IANA) y etapas: `id`, `description`, `start` (UTC) y `stage_type.description` (`Classic`, `Relay`, `Score`, `Overall`, `One Man Relay`…). |
| `/events/<evento>/stages/<etapa>/classes` | Categorías: `id` (UUID), `short_name`, `long_name`. |
| `/events/<evento>/stages/<etapa>/results?class_id=<cat>&forceSameDay=true` | Corredores de una categoría (la llamada que hace la web de OReplay). |

Las respuestas llevan lo útil en `data`. Hay también `…/resultsByClass`, con todas las
categorías de una vez, pero es pesada (varios MB y unos 10 s con unos 600 corredores) y en las
pruebas dio algún 502: se pide categoría a categoría.

De cada corredor se usa:

| Campo | Qué es |
| --- | --- |
| `full_name` | Nombre y apellidos, en ese orden (el backend los junta; no da los dos por separado). |
| `club.short_name` | Club. |
| `sicard`, `bib_number` | Tarjeta SI y dorsal (texto o número, según la carrera). |
| `sex` | `M` o `F`. |
| `is_nc` | Fuera de concurso. |
| `stage` | Resultado en la etapa; `null` si solo está en la lista de salida. |
| `stage.start_time`, `stage.finish_time` | Salida y meta, en UTC (`2026-…T07:35:03.000+00:00`). |
| `stage.status_code` | `0` OK, `1` no salió, `2` abandono, `3` falta baliza, `4` descalificado, `5` fuera de tiempo, `6` en carrera, `9` no competitivo, `F` llegado sin validar (`StatusCode.php`). |
| `stage.position` | Puesto. |
| `stage.splits[]` | Picadas: `order_number` (posición en el recorrido), `reading_time` (UTC, `null` si no picó), `control.station` (código de la baliza, como texto) e `is_intermediate` (radio). **Vienen desordenadas.** |

No trae fecha de nacimiento.

## Lo observado

- Las horas van en segundos enteros y en UTC: no hay que convertir de hora local ni hay
  ambigüedad en el cambio de hora.
- **No hay recorrido aparte**: se deduce de las picadas. Dentro de una categoría, todos los
  clasificados con picadas tenían la misma secuencia.
- Una baliza no picada sale con su código y `reading_time` nulo, como `0xFFFFFF` en el .spl. Un
  DNS puede traer todas las balizas sin hora.
- La salida y la meta no van en `splits`. La última baliza suele ser la de llegada (100, 200…),
  distinta de la meta.
- Un organizador puede validar a mano a un corredor: OK, con puesto y con una baliza sin hora.
- Algunos corredores no traen picadas aunque tengan resultado (`upload_type` `res_finish` o
  `entry_list`): el organizador solo subió la meta o la lista de salida.

## Descarga (`Download`)

La app pide el evento, las categorías de la etapa y los resultados de cada categoría
(`docs/app.md`, "Importar una carrera") y junta lo necesario en una `Download`, que guarda como
JSON en el directorio de datos de la app. Solo lleva los campos de arriba (y los de evento,
etapa y categoría), así que dos descargas de los mismos resultados dan el mismo fichero, y
reimportarla no duplica la carrera (mismo SHA-256, `docs/app.md`). Si el organizador corrige
resultados entre una descarga y otra, la nueva es otro fichero y se importa como otra carrera.

```json
{
  "format": "tramos-oreplay",
  "version": 1,
  "event": {"id": "…", "description": "Trofeo…", "timezone": "Europe/Madrid"},
  "stage": {"id": "…", "description": "Media", "start": "2026-10-04T07:00:00Z",
            "stage_type": {"description": "Classic"}},
  "classes": [
    {"class": {"id": "…", "short_name": "M21", "long_name": "M21"},
     "runners": [{"full_name": "…", "club": {"short_name": "…"}, "sicard": "…",
                  "sex": "M", "bib_number": null, "is_nc": false,
                  "stage": {"start_time": "…", "finish_time": "…", "status_code": "0",
                            "position": 1, "is_nc": false,
                            "splits": [{"is_intermediate": false, "reading_time": "…",
                                        "order_number": 1, "control": {"station": "31"}}]}}]}
  ]
}
```

`oreplay::read` la lee; `is_download` la distingue de un .spl (empieza por `{`, el .spl por
`spl4`). Un JSON sin `"format": "tramos-oreplay"` no es una descarga; una `version` mayor que la
que entiende la app es un error que pide actualizar.

En la base se guarda en `source_files` con `kind = 'spl'` (`docs/almacenamiento.md`): son los
splits del cronometraje, venga de donde venga, y así no cambia el esquema.

## Importación al modelo (`oreplay::to_event`)

- **Solo etapas `Classic`**: relevos, score y totales no tienen un recorrido por categoría que
  comparar. Otro tipo es un error (`UnsupportedStage`).
- **Nombre**: el del evento y, si la etapa tiene uno distinto, `evento · etapa`.
- **Fecha**: el día local de la salida de la etapa (`stage.start`) en la zona del evento
  (`timezone`; `Europe/Madrid` si falta o no es válida). Sin `start`, la primera salida de un
  corredor. Sin ninguna, error (`MissingDate`).
- **Categorías**: en el orden de la API. `Class::id` es la posición, desde 1 (los id de OReplay
  son UUID y el modelo usa `u32`). `name` es `long_name` o, si está vacío, `short_name`;
  `short_name` se guarda tal cual.
- **Picadas de cada corredor**: las de `splits` sin radios (`is_intermediate`), ordenadas por
  `order_number`. Un `control.station` que no es un número (o es el código de salida o meta) es
  un error (`InvalidStation`).
- **Recorrido**: la secuencia de códigos más repetida entre los clasificados (`ok`) con picadas;
  con empate, la primera que aparece. Si no hay clasificados con picadas, entre todos los que
  traen picadas (los no picados también traen el código). Sin nadie con picadas, vacío.
- **`RaceResult::punches`**: salida (`start_time`), una por baliza del recorrido y meta
  (`finish_time`), como con el .spl. Cada baliza toma, en orden, la primera picada del corredor
  con su código que no se haya usado; si el corredor tiene otra secuencia, lo que no casa queda
  sin hora. No se intenta detectar variantes (#72).
- **Estado**:

  | OReplay | `RaceStatus` |
  | --- | --- |
  | `0`, sin `is_nc` (ni en el corredor ni en el resultado) | `ok` |
  | `0` fuera de concurso | `not_classified` |
  | `1`, o sin resultado en la etapa | `did_not_start` |
  | cualquier otro (`2`–`6`, `9`, `F`…) | `not_classified` |

  A diferencia del .spl, no se usa `unknown(n)`: OReplay documenta sus códigos y todos los que no
  son `0` ni `1` son no clasificados.
- **Puesto**: `position` si es `ok` y mayor que 0.
- **Corredor**: `full_name` va entero en `given_name` y `family_name` queda vacío. La
  identificación (`docs/identificacion.md`) compara «nombre + apellidos», así que casa con
  «Nombre Apellidos» pero no con «Apellidos Nombre». La tarjeta SI viene casi siempre. Tarjeta y
  dorsal: el número si es mayor que 0. Sexo: `M` → `male`, `F` → `female`.
- **Sin parciales**: si ningún corredor tiene hora en ninguna baliza (solo tiempos de meta, o
  nada subido aún), es un error (`NoSplits`) en vez de una carrera vacía.

## Fixture

`fixtures/oreplay/baltanas-anon.json` es el .spl anonimizado de Baltanás convertido a una
descarga de OReplay (mismas picadas en el formato de la API). Los tests comprueban que da la misma
carrera que el .spl (recorridos, picadas, estados) y que se importa con el FIT sintético igual
(`crates/tramos-core/tests/oreplay.rs`, `app/src-tauri/src/import.rs`). Para regenerarlo:

```bash
cargo test -p tramos-core --test oreplay -- --ignored write_baltanas_fixture
```
