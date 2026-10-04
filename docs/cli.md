# CLI de desarrollo (`tramos`)

`crates/tramos-cli` expone el núcleo en la terminal para probarlo sin interfaz. No calcula nada
por su cuenta: lee los ficheros, llama a `tramos_core` y escribe el resultado. El recorrido y el
tiempo perdido (`course` y `lost_time` del JSON) salen de `tramos_core::runner_report`, lo mismo
que la vista de carrera de la app (`docs/app.md`).

```bash
cargo run -p tramos-cli -- analizar --spl carrera.spl --fit reloj.fit --corredor 2001234
# o, ya compilado:
tramos analizar --spl carrera.spl --corredor "Ana Pérez García" --formato tabla
```

## `tramos analizar`

Tiempo perdido de un corredor en una carrera y, si se pasa su FIT, la alineación del reloj con
sus picadas.

| Opción | Por defecto | Qué es |
| --- | --- | --- |
| `--spl <FICHERO>` | (obligatoria) | Splits de WinSplits (`docs/formato-spl.md`). |
| `--fit <FICHERO>` | sin FIT | Actividad del reloj (`docs/formato-fit.md`). Sin FIT, solo el análisis de splits. |
| `--corredor <NOMBRE\|TARJETA>` | (obligatoria) | Si es un número, la tarjeta SI; si no, el nombre y apellidos completos. |
| `--umbral-s <SEGUNDOS>` | 15 | Pérdida mínima en segundos para que un tramo sea error. |
| `--umbral-pct <PORCENTAJE>` | 10 | Pérdida mínima en % del tiempo esperado (10 = 10 %). |
| `--ideal suma-referencias\|suma-mejores` | `suma-referencias` | Definición del tiempo ideal (`docs/tiempo-perdido.md`, "Tiempo ideal"). |
| `--formato json\|tabla` | `json` | JSON a la salida estándar, o una tabla para leer en la terminal. |

Los umbrales tienen que ser números mayores o iguales que 0. Un tramo es error si la pérdida
supera **los dos** (`docs/tiempo-perdido.md`).

### Identificación del corredor

`--corredor` se pasa a `identify_runner` (`docs/identificacion.md`) como tarjeta SI (si es un
número entero) o como nombre (si no). El nombre se compara normalizado y completo: sin acentos ni
mayúsculas, en el orden «nombre apellidos» o «apellidos nombre», sin coincidencias parciales.

- **Un resultado**: se analiza. Si casara por tarjeta pero no por nombre, se sigue y se avisa en
  `warnings` (con un solo `--corredor` no puede pasar hoy, pero la salida lo prevé).
- **Varios** (dos personas con el mismo nombre, una tarjeta repetida): error con la lista de
  candidatos (categoría, nombre, tarjeta y puesto) y la pista para desempatar.
- **Ninguno**, o falta `--corredor`: error.

### Errores y código de salida

Todo error (fichero que no se puede leer, .spl o FIT inválidos, corredor no encontrado o
ambiguo, FIT que no se puede alinear con las picadas) se escribe en la salida de error como
`error: …`, en español, y la CLI acaba con código 1. Los errores de sintaxis de las opciones los
da `clap` (en inglés) con código 2. Con error no se escribe nada en la salida estándar.

## Salida JSON

Un objeto con estas claves (campos en `snake_case`, duraciones en segundos, instantes en
RFC 3339 y UTC, `null` lo que no se puede calcular):

| Clave | Qué es |
| --- | --- |
| `event` | `name` (o `null`) y `date` (fecha local, `AAAA-MM-DD`). |
| `runner` | `class_index` y `result_index` (posición en la carrera, como en el núcleo), `class_id`, `class_name`, `given_name`, `family_name`, `club`, `bib`, `si_card`, `status` (`ok`, `not_classified`, `did_not_start` o `{"unknown": n}`) y `place`. |
| `course` | `controls` (balizas sin salida ni meta), `classes` (categorías que comparten el recorrido: `index`, `id`, `name`), `valid_runners` (clasificados del recorrido) y `weak_reference`. |
| `config` | `error_threshold_s`, `error_threshold_pct` e `ideal_time` (`sum_of_references` o `sum_of_best_splits`) con los que se ha calculado. |
| `lost_time` | Totales del corredor y `legs[]` (abajo). |
| `alignment` | Solo con `--fit`; `null` sin él (abajo). |
| `warnings` | Avisos de la identificación, en español (normalmente vacío). |

`lost_time` reúne el `RunnerAnalysis` del corredor y las referencias del recorrido
(`docs/tiempo-perdido.md`, "Salida"):

- Totales: `total_s`, `usual_performance` (1 = 100 %), `lost_time_s`, `error_count`,
  `time_without_errors_s`, `ideal_time_s` (tiempo ideal del recorrido entero, es decir, el
  `ideal_elapsed_s` del último tramo), `behind_ideal_s` (diferencia con el ideal en meta) y
  `losing_streaks` (rachas de dos o más tramos seguidos perdiendo: `first_leg`, `last_leg`,
  `loss_s`; ver `docs/tiempo-perdido.md`, "Dónde gano y dónde pierdo").
- `legs[]`, un elemento por tramo, con la referencia del recorrido y los números del corredor en
  el mismo objeto: `index` (desde 1), `from`, `to` (32736 = salida, 32752 = meta), `split_s`,
  `elapsed_s`, `place`, `reference_s`, `reference_count`, `valid_splits`, `performance_index`,
  `expected_s`, `loss_s`, `loss_pct`, `is_error`, `gain_s` (`−loss_s`), `cumulative_gain_s`,
  `ideal_elapsed_s`, `behind_ideal_s`, `is_last`, `short_reference` y `excluded_from_patterns`.

`alignment` es el `Alignment` del núcleo (`docs/alineacion.md`, "Resultado") **sin** `punches`
(las picadas situadas en el track): `offset_s`, `offset_estimated`, `confidence`, `quality`,
`coverage` y `warnings` (cada aviso con `kind`, sus datos y `message`). Si la alineación falla
(track vacío, track de otra hora…), la CLI acaba con error.

Ejemplo recortado (fixture público de Baltanás, M-SEN, tarjeta 143, con el FIT sintético):

```json
{
  "event": {"name": "Cto. SPRINT Liga Norte/Liga FOCYL Baltanas", "date": "2026-10-03"},
  "runner": {"class_index": 9, "result_index": 15, "class_id": 603, "class_name": "M-SEN",
             "given_name": "N143_", "family_name": "Apellido143_______", "club": "Club F________",
             "bib": 143, "si_card": 143, "status": "ok", "place": 16},
  "course": {"controls": [49, 51, 32, "…", 100],
             "classes": [{"index": 9, "id": 603, "name": "M-SEN"}],
             "valid_runners": 18, "weak_reference": false},
  "config": {"error_threshold_s": 15.0, "error_threshold_pct": 10.0,
             "ideal_time": "sum_of_references"},
  "lost_time": {
    "total_s": 1540.0, "usual_performance": 0.8649, "lost_time_s": 360.43, "error_count": 3,
    "time_without_errors_s": 1179.57, "ideal_time_s": 1039.8, "behind_ideal_s": 500.2,
    "losing_streaks": [{"first_leg": 7, "last_leg": 10, "loss_s": 347.34}],
    "legs": [
      {"index": 1, "from": 32736, "to": 49, "split_s": 54.0, "elapsed_s": 54.0, "place": 16,
       "reference_s": 42.2, "reference_count": 5, "valid_splits": 18,
       "performance_index": 0.7815, "expected_s": 48.79, "loss_s": 5.21, "loss_pct": 10.67,
       "is_error": false, "gain_s": -5.21, "cumulative_gain_s": -5.21, "ideal_elapsed_s": 42.2,
       "behind_ideal_s": 11.8, "is_last": false, "short_reference": false,
       "excluded_from_patterns": false}
    ]
  },
  "alignment": {
    "offset_s": 0.135, "offset_estimated": true, "confidence": 1.0,
    "quality": {"controls_used": 21, "support": 0.952, "margin": 0.871, "runner_up_offset_s": 22.0},
    "coverage": {"track_start": "2026-10-03T16:12:00Z", "track_end": "2026-10-03T16:39:40Z",
                 "race_start": "2026-10-03T16:13:00.135245Z",
                 "race_finish": "2026-10-03T16:38:40.135245Z",
                 "missing_start_s": 0.0, "missing_end_s": 0.0, "gaps": []},
    "warnings": []
  },
  "warnings": []
}
```

La salida completa está en `fixtures/cli/baltanas-msen-143.expected.json`.

Todavía **no** incluye el corte del track en tramos (#11) ni las métricas del FIT por tramo
(#13): las añade el PR de #13.

## Salida en tabla

`--formato tabla` escribe una cabecera (carrera, corredor, recorrido y categorías, umbrales),
una fila por tramo y los totales; con FIT, una línea con la alineación y sus avisos. Duraciones
redondeadas al segundo (`m:ss`), coma decimal, `S` = salida, `M` = meta y `-` donde no hay
valor. La columna "Notas" marca `ERROR`, `último` (último tramo) y `ref. corta` (referencia
< 20 s), que son los tramos excluidos de los análisis de patrones.

```text
Cto. SPRINT Liga Norte/Liga FOCYL Baltanas · 2026-10-03
Corredor: N143_ Apellido143_______ (M-SEN, tarjeta 143) · puesto 16
Recorrido: 20 balizas, 21 tramos · categorías: M-SEN · 18 clasificados
Error: pérdida > 15 s y > 10 % · tiempo ideal: suma de referencias

Tramo  Balizas      Split   Ref.      IR   Pérdida        %  Notas
    1  S→49          0:54   0:42  78,1 %    +5,2 s  +10,7 %
    2  49→51         0:22   0:19  88,2 %    -0,4 s   -1,9 %  ref. corta
    3  51→32         1:22   0:57  69,3 %   +16,3 s  +24,9 %  ERROR
  …
    9  42→46         7:07   2:04  29,0 %  +283,6 s +197,8 %  ERROR
   10  46→63         2:05   0:56  44,6 %   +60,5 s  +93,7 %  ERROR
  …
   21  100→M         0:17   0:14  81,2 %    +1,0 s   +6,5 %  último, ref. corta

Total 25:40 · rendimiento habitual 86,5 % · 3 error(es) · tiempo perdido 6:00 · sin errores 19:40
Tiempo ideal 17:20 · diferencia en meta +8:20

Alineación del FIT: desfase +0,1 s · confianza 1,00 · 21 picadas usadas (apoyo 0,95, separación 0,87)
```

La tabla es para leer; para procesar la salida, usa el JSON.

## Pruebas

`crates/tramos-cli/tests/analizar.rs` ejecuta el binario con los fixtures públicos y compara el
JSON con `fixtures/cli/baltanas-msen-143.expected.json` (flotantes con tolerancia 1e-6). Para que
no sea circular, comprueba además los números del corredor contra el oráculo de Python
(`fixtures/spl/baltanas-anon.tiempo-perdido.expected.json`) y que el desfase con el FIT
sintético, construido con las horas reales de picada, sale a menos de 2 s de 0. Para regenerar
el JSON esperado tras un cambio intencionado de la salida:

```bash
cargo run -q -p tramos-cli -- analizar --spl fixtures/spl/baltanas-anon.spl \
  --fit fixtures/fit/baltanas-sintetico.fit --corredor 143 \
  > fixtures/cli/baltanas-msen-143.expected.json
```
