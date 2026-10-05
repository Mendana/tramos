# Histórico

Primera vista sobre todas las carreras del corredor (P6 de `docs/preguntas.md`: «¿En qué formato
rindo peor?»). Implementado en `tramos_core::history` (#25); la pantalla, en `docs/app.md`,
"Vista histórica". Notación de `docs/tiempo-perdido.md`.

## Entrada

Una carrera del histórico (`HistoryRace`) es un resultado del corredor con:

- `date`: la fecha local de la carrera;
- `format`: el formato confirmado al importar (`sprint`, `middle`, `long`) o ninguno;
- `lost_time`: su tiempo perdido (`RunnerLostTime` de `tramos_core::runner_report`), calculado
  al pedirlo con los umbrales de los ajustes. Nada se guarda: cambiar un umbral cambia el
  histórico.

## Filtro

`HistoryFilter { from, to, format }`; un campo ausente o `null` no filtra.

- **Fechas**: `from ≤ date ≤ to`, las dos **incluidas**, sobre la fecha local de la carrera. Con
  `from > to` no entra ninguna carrera (no es un error).
- **Formato**: solo las carreras de ese formato. Las carreras **sin formato** solo entran sin
  filtro de formato.

## Qué cuenta

- **Carreras**: las que pasan el filtro y tienen **rendimiento habitual** (`usual_performance`).
  Las que no lo tienen (no presentado, o sin ningún tramo con split y referencia) no tienen
  ningún número: no cuentan en ningún grupo y se dan aparte (`races_without_data`) para que se
  vea que existen.
- **Tramos que cuentan** (`pattern_legs`): los que tienen pérdida (`loss_s`) y **no** están
  excluidos de los patrones (`excluded_from_patterns`: el último tramo y los de referencia menor
  de 20 s, `docs/tiempo-perdido.md`, "Exclusiones"). Es un análisis de patrones a lo largo de
  muchas carreras, que es justo para lo que existe esa marca:
  - el último tramo (a meta) casi nunca es error y diluiría la tasa de error de forma distinta
    según el formato (un sprint tiene más tramos que una larga);
  - en un tramo de referencia corta unos pocos segundos son un porcentaje enorme y el umbral en
    segundos casi nunca se supera: tampoco dicen nada de cómo se orienta el corredor.

  Un tramo sin pérdida (sin split o sin referencia) no tiene números y no cuenta.

Por eso la pérdida del histórico **no** es el tiempo perdido de las carreras sumado: el de la
carrera incluye también el último tramo y los cortos.

## Números de un grupo (`HistoryStats`)

Para un grupo de carreras (un formato, las sin formato o el total), con `R` carreras y `N`
tramos que cuentan, `E` de ellos con error:

| Campo | Definición |
| --- | --- |
| `races` | `R`. |
| `legs`, `errors` | `N` y `E`. |
| `mean_performance` | **IR medio**: media aritmética del rendimiento habitual de cada carrera (1 = 100 %). |
| `error_rate` | **Tasa de error**: `E / N` (0–1). |
| `mean_loss_s` | **Pérdida media por tramo**: `Σ p_i` de los tramos con error, entre `N` (s). Los tramos sin error suman 0. |
| `mean_loss_pct` | Lo mismo en %: `Σ loss_pct_i` de los tramos con error, entre `N`. |
| `mean_consistency` | **Consistencia media** (P10): media aritmética de la consistencia de cada carrera que la tiene (`docs/tiempo-perdido.md`, "Consistencia"; 1 = 100 puntos de IR). Una carrera con menos de 2 tramos que cuenten no tiene consistencia y no entra en esta media, aunque sí en las demás. |

Sin carreras (o sin tramos), las medias van a `null`; los recuentos, a 0.

### Por qué el IR medio es la media de los rendimientos habituales

Había dos opciones: la media de los rendimientos habituales de cada carrera o la media de
`IR_i` de todos los tramos. Se elige la primera:

- **Es el número que el corredor ya conoce**: el «Rendimiento» de la vista de carrera. El IR
  medio de un formato es la media de lo que ve carrera a carrera.
- **Cada carrera pesa lo mismo**, tenga 25 tramos (sprint) o 12 (larga).
- **Separa velocidad de errores.** El rendimiento habitual es una mediana: un error no lo mueve.
  La media de `IR_i` mezclaría las dos cosas (un tramo con error tiene un `IR_i` muy bajo) y
  repetiría lo que ya miden la tasa de error y la pérdida. Así las tres cifras son
  complementarias: lo rápido que va cuando no falla, cuántas veces falla y cuánto le cuesta.
- No depende de los umbrales de error, que solo cambian la tasa y la pérdida.

El rendimiento habitual incluye todos los tramos con `IR_i`, también el último y los cortos
(`docs/tiempo-perdido.md`); es la definición de la carrera y no se recalcula. Las preguntas que
miden el IR de un subconjunto de tramos (P11, los tres primeros; P13, por desnivel) definen su
propio IR medio sobre esos tramos.

### Por qué la pérdida cuenta solo los errores, y también en %

- **Solo errores**: en toda la app, «tiempo perdido» es la suma de `p_i` de los tramos con error.
  La pérdida media por tramo es eso repartido entre los tramos: «de media pierdo tantos segundos
  por tramo en errores». La media de `p_i` de todos los tramos mezclaría pérdidas y ganancias:
  lo ganado en los tramos buenos taparía parte de lo perdido en los errores, y eso ya lo enseña
  «Dónde gano y dónde pierdo» (P5) en cada carrera.
- **En %**: los tramos de un sprint duran unos 40 s y los de una larga varios minutos, así que
  los segundos no se comparan entre formatos. La versión en % del tiempo esperado sí, y es la
  que dibuja la gráfica (la tabla da las dos).

## Grupos y total (`History`)

- `by_format`: **sprint, media y larga, siempre y en ese orden**, aunque no tengan carreras (con
  sus medias a `null`), para que la tabla y las gráficas tengan siempre las mismas columnas. Con
  filtro de formato, solo ese formato. Al final, el grupo **sin formato** (`format: null`) si hay
  alguna carrera con números sin formato.
- **Carreras sin formato**: van **aparte**, en su grupo, y no se excluyen. Esconderlas haría
  que el histórico no cuadrase con la lista de carreras; separarlas deja ver que falta
  asignarles formato. Entran en el total.
- `total`: todas las carreras que pasan el filtro juntas, con o sin formato, con las mismas
  definiciones (no es la media de los grupos). Con filtro de formato coincide con ese grupo.
- `races_without_data`: carreras que pasan el filtro sin rendimiento habitual.
- `filter`: el filtro aplicado.

## Una fila por carrera (`race_stats`)

`race_stats(&RunnerLostTime) -> Option<HistoryStats>` da los números de una carrera sola, con las
mismas definiciones que un grupo (`races` = 1, el IR medio es su rendimiento habitual). `None` si
no tiene rendimiento habitual: tampoco cuenta en ningún grupo. Sumadas las filas de un grupo dan
sus carreras, tramos y errores, y la pérdida media del grupo es la media de las de sus filas
ponderada por sus tramos.

El comando `history` de la app devuelve, además del agregado, una fila por carrera que pasa el
filtro (`races`), de la más reciente a la más antigua, con su `result_id` para abrirla (#98).
Es la base de las series por carrera de P10 y P11.

## Ejemplo de test

Cinco carreras (tramos que cuentan / errores / suma de `p_i` y de `loss_pct` de los errores):

| Carrera | Fecha | Formato | Habitual | Tramos | Errores | Pérdida |
| --- | --- | --- | --- | --- | --- | --- |
| A | 1-mar | sprint | 0,90 | 3 (el último, error, no cuenta) | 1 | 30 s, 50 % |
| B | 10-abr | sprint | 0,80 | 2 (no cuentan uno corto con error, uno sin split ni el último) | 1 | 60 s, 100 % |
| C | 20-may | media | 1,00 | 4 | 2 | 65 s, 32,5 % |
| D | 15-jun | — | 0,70 | 1 | 1 | 20 s, 25 % |
| E | 1-jul | larga | — | 0 | 0 | — |

- Sprint: 2 carreras, IR (0,90 + 0,80) / 2 = **85 %**, tasa 2 / 5 = **40 %**, pérdida
  90 / 5 = **18 s** y 150 / 5 = **30 %**.
- Media: 1 carrera, **100 %**, 2 / 4 = **50 %**, 65 / 4 = **16,25 s** y **8,125 %**.
- Larga: sin carreras con números (E no tiene habitual): todo a `null`.
- Sin formato: 1 carrera, 70 %, 100 %, 20 s y 25 %.
- Total: 4 carreras, IR 3,4 / 4 = **85 %**, 5 / 10 = **50 %**, 175 / 10 = **17,5 s** y
  207,5 / 10 = **20,75 %**; una carrera sin datos (E).
- Del 10-abr al 20-may (incluidos): B y C; total 2 carreras, 90 %, 3 / 6 y 125 / 6 s.
- Consistencia, con A 0,10, B 0,20, C sin valor, D 0,30 y E 0,50: sprint (0,10 + 0,20) / 2 =
  **0,15**; media sin valor; sin formato 0,30; total (0,10 + 0,20 + 0,30) / 3 = **0,20** (C no
  tiene y E no cuenta).

Los tests están en `crates/tramos-core/src/history.rs`; los del comando, que comprueban que el
histórico sale de los mismos números que la vista de carrera, en `app/src-tauri/src/history.rs`.

## Para los análisis que vienen

P7, P10, P11 y P13 se apoyan en este histórico: mismas carreras, mismo filtro y, cuando cuentan
tramos, los mismos `pattern_legs`. Cada uno añade sus números al resultado del comando y sus
paneles a la pantalla.

## Pérdida según duración del tramo (P7)

«¿Tramos largos o cortos?» (`docs/preguntas.md`, P7). Implementado en `tramos_core::leg_length`
(#28); el comando `history` lo devuelve en `by_leg_length` y la pantalla lo dibuja en el panel
«Pérdida según duración del tramo» (`docs/app.md`, "Vista histórica").

- **Mismas carreras y tramos**: las del histórico con el mismo filtro (fechas y formato) y, de
  cada una, sus tramos que cuentan (`pattern_legs`): ni el último ni los de referencia menor de
  20 s. Las carreras sin rendimiento habitual no aportan nada. Cada tramo que cuenta cae en un
  cubo y solo en uno, así que la suma de tramos y errores de los cubos es la del total.
- **Cubos por `ref_i`**, no por el split del corredor: un error alarga el split y movería el
  tramo a un cubo más largo. Escala logarítmica, cada cubo el doble que el anterior (salvo el
  primero). El **inicio entra y el final no**:

  | Cubo | `ref_i` (s) |
  | --- | --- |
  | 20–30 s | `[20, 30)` |
  | 30–60 s | `[30, 60)` |
  | 1–2 min | `[60, 120)` |
  | 2–4 min | `[120, 240)` |
  | 4–8 min | `[240, 480)` |
  | ≥ 8 min | `[480, ∞)` |

  Así el primer cubo empieza justo donde acaba la exclusión de referencia corta (`ref_i < 20`
  estricto, `docs/tiempo-perdido.md`) y un tramo de exactamente 1 min es de 1–2 min.
- **Por cubo** (`LegLengthStats`), con `n` tramos que cuentan en el cubo y `E` errores:
  `from_s` y `to_s` (el final, `null` en el último), `legs` = `n`, `errors` = `E`,
  `error_rate` = `E / n`, `mean_loss_s` y `mean_loss_pct` con la misma definición que en
  `HistoryStats`: la pérdida de los errores (`p_i` o `loss_pct`) entre `n`, y los tramos sin
  error suman 0. Siempre salen los seis cubos, en orden; uno vacío tiene `n = 0` y las medias a
  `null`.
- **n importa**: un cubo con pocos tramos es poco fiable, y la pantalla enseña `n` en cada uno.

Ejemplo de test (`crates/tramos-core/src/leg_length.rs`): un sprint con tramos que cuentan de 25 s
(error, 15 s y 60 %), 28 s, 45 s (error, 20 s y 40 %) y 50 s da en 20–30 s una tasa de 1 / 2 =
**50 %**, 15 / 2 = **7,5 s** y 60 / 2 = **30 %**, y en 30–60 s, 50 %, 10 s y 20 %. Su tramo de
15 s con error y el último (300 s, error) no cuentan en ningún cubo.

## Días sin competir (P11)

«¿Entro peor en mapa tras días sin competir?» (`docs/preguntas.md`, P11). Implementado en
`tramos_core::days_off` (#33); el comando `history` lo devuelve en `days_off` y la pantalla lo
dibuja en la sección «Días sin competir» (`docs/app.md`, "Vista histórica").

- **Mismas carreras**: las del histórico con el mismo filtro (fechas y formato) y con
  rendimiento habitual.
- **Días desde la carrera anterior**: `fecha − fecha de la anterior`, en días. La anterior es la
  carrera de fecha más reciente **estrictamente anterior** entre **todas** las del usuario en
  las que tomó la salida (estado distinto de no presentado), **pasen o no el filtro**: competir
  es competir, sea del formato que sea, y filtrar por fechas no borra la carrera de la semana
  anterior. Un no presentado no cuenta como anterior. Dos carreras del mismo día tienen la misma
  anterior (sin horas, no se sabe cuál fue antes). Una carrera sin anterior (la primera
  importada) no cae en ningún cubo y se cuenta en `without_previous`.
- **Cubos**, con los dos extremos incluidos:

  | Cubo | Días |
  | --- | --- |
  | ≤ 7 días | 1–7 |
  | 8–14 días | 8–14 |
  | 15–30 días | 15–30 |
  | > 30 días | 31 o más |

- **Por cubo** (`DaysOffStats`): `from_days` y `to_days` (`null` en el último), `races` y:
  - **IR de entrada en mapa**: `first_legs_performance` = media de los `IR_i` de los tramos 1, 2
    y 3 de cada carrera que cuentan (`pattern_legs`: con pérdida y sin referencia corta), todos
    juntos; `first_legs` = cuántos son. Un tramo corto o sin split entre los tres primeros no se
    sustituye por el cuarto: la pregunta es por el principio de la carrera.
  - **Errores del primer tercio**: los tramos que cuentan con `index ≤ ⌈L / 3⌉`, siendo `L` el
    número de tramos del recorrido (con el último): 21 tramos → los 7 primeros. Por número de
    tramos y no por tiempo, que con picadas que faltan no se sabe. `first_third_legs`,
    `first_third_errors` y `first_third_error_rate` = errores / tramos.
  - Siempre salen los cuatro cubos, en orden; uno vacío tiene las medias a `null`.
- **Referencias**: la pantalla compara el IR de entrada con el IR medio del total y la tasa del
  primer tercio con la tasa de error del total, con los mismos filtros.

Ejemplo de test (`crates/tramos-core/src/days_off.rs`): A 1-mar, X 4-mar (no presentado), B y C
6-mar, D 16-mar, E 15-abr y F 20-may. A no tiene anterior; B y C van a 5 días (de A: X no
corrió); D, a 10; E, a 30 (marzo tiene 31 días); F, a 35. En ≤ 7 días, los tres primeros tramos
de B (0,7, 0,9 y 1,1) y C (0,8 y 1,0, porque su tramo 2 es corto) dan un IR de 4,5 / 5 =
**90 %**. Filtrando solo sprint, F sigue a 35 días de E aunque E sea una larga.
