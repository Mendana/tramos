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
paneles a la pantalla. P13 necesita además el FIT: solo aportan tramos las carreras con track.

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
    tramos y no por tiempo, que con picadas que faltan no se sabe. Es el primero de los tercios
    de `tramos_core::history::race_third`, que usa también P14. `first_third_legs`,
    `first_third_errors` y `first_third_error_rate` = errores / tramos.
  - Siempre salen los cuatro cubos, en orden; uno vacío tiene las medias a `null`.
- **Referencias**: la pantalla compara el IR de entrada con el IR medio del total y la tasa del
  primer tercio con la tasa de error del total, con los mismos filtros.

Ejemplo de test (`crates/tramos-core/src/days_off.rs`): A 1-mar, X 4-mar (no presentado), B y C
6-mar, D 16-mar, E 15-abr y F 20-may. A no tiene anterior; B y C van a 5 días (de A: X no
corrió); D, a 10; E, a 30 (marzo tiene 31 días); F, a 35. En ≤ 7 días, los tres primeros tramos
de B (0,7, 0,9 y 1,1) y C (0,8 y 1,0, porque su tramo 2 es corto) dan un IR de 4,5 / 5 =
**90 %**. Filtrando solo sprint, F sigue a 35 días de E aunque E sea una larga.

## Pérdida según desnivel (P13)

«¿Me frena el desnivel?» (`docs/preguntas.md`, P13). Implementado en `tramos_core::slope` (#29);
el comando `history` lo devuelve en `by_slope` y la pantalla lo dibuja en la sección «Por
desnivel» (`docs/app.md`, "Vista histórica").

### Clasificación de un tramo

Con las métricas del FIT del tramo (`tramos_core::metrics::leg_metrics`, `docs/metricas.md`):
subida `S` y bajada `B` acumuladas con la altitud suavizada y distancia recorrida `d`, y un
umbral `U` (4 m por cada 100 m por defecto):

- `s = S / d · 100` y `b = B / d · 100` (m por cada 100 m recorridos);
- **subida** si `s ≥ U`, **bajada** si `b ≥ U` y **llano** en otro caso. El umbral **entra**:
  un tramo que sube justo 4 m/100 m es de subida.
- **Si cumple las dos** (sube y baja mucho, un tramo «rompepiernas»), **gana la mayor**; a
  igualdad, **subida**. Se descartó una cuarta clase «mixto»: con 5–10 corredores y tramos de
  P13 solo en las carreras con FIT, una clase más dejaría muy pocos tramos en cada una. Y la que
  más pesa es la que más define el tramo: si sube 6 y baja 5, el corredor ha pasado más tiempo
  subiendo. El empate va a subida porque subir cuesta más que bajar lo mismo.
- Se compara cada sentido por separado, no el desnivel neto (`s − b`): un tramo que sube 30 m y
  baja 30 m no es llano.

**No se clasifican** (y no entran en ninguna clase):

- los tramos de carreras **sin track** (importadas sin FIT) o cuyo track ya no se puede alinear
  ni trocear: se cuentan aparte (`races_without_track`, `legs_without_track`);
- dentro de una carrera con track, los tramos **sin sub-track** (picada sin hora o fuera del
  track, `docs/segmentacion.md`), **sin altitud** (`ascent_m` o `descent_m` nulos) o con una
  **distancia recorrida menor de 50 m** (`MIN_DISTANCE_M`): ahí un metro de error de la altitud
  suavizada ya son 2 m/100 m, la mitad del umbral. Los tramos que cuentan rara vez son tan
  cortos (la referencia mínima es 20 s). Van en `unclassified_legs`;
- los tramos cuyo número y balizas no casan con los de la segmentación (picadas que no casan con
  el recorrido: la tabla de tramos sigue el recorrido y la segmentación, las picadas). También
  van en `unclassified_legs`.

**Umbral configurable**: `SlopeConfig { threshold_m_per_100m }`, con el valor por defecto en un
solo sitio (`DEFAULT_THRESHOLD_M_PER_100M` = 4). Tiene que ser un número finito mayor que 0; si
no, `slope` devuelve un error y `classify` no clasifica. La app usa el valor por defecto (aún no
hay ajuste en la pantalla de ajustes) y lo devuelve en `by_slope.config`.

### Mismas carreras y tramos

Las del histórico con el mismo filtro (fechas y formato) y, de cada una, sus tramos que cuentan
(`pattern_legs`): ni el último ni los de referencia menor de 20 s. Las carreras sin rendimiento
habitual no aportan nada, ni siquiera a los recuentos. Cada tramo que cuenta acaba en una clase,
en `unclassified_legs` o en `legs_without_track`, así que esas cifras suman los tramos del total
del histórico.

### Por clase (`SlopeStats`)

Subida, llano y bajada, **siempre y en ese orden**. Con `n` tramos de la clase y `E` errores:
`legs` = `n`, `errors` = `E`, `error_rate` = `E / n` y

| Campo | Definición |
| --- | --- |
| `mean_performance` | **IR medio de la clase**: `Σ ref_i · IR_i / Σ ref_i` sobre los tramos de la clase (1 = 100 %). |

Una clase vacía tiene `n = 0` y las medias a `null`.

### Por qué el IR medio aquí es la media de `IR_i` ponderada por `ref_i`

Aquí no sirve el IR medio del histórico (la media de los rendimientos habituales): el habitual es
de toda la carrera, y P13 pregunta por un subconjunto de tramos. Había dos opciones:

- **Media simple de `IR_i`**: cada tramo pesa lo mismo. Un tramo de 25 s pesaría como uno de
  5 min, y en los tramos cortos unos pocos segundos mueven mucho `IR_i`.
- **Media ponderada por `ref_i`** (la elegida): cada tramo pesa según lo que dura para la
  referencia, que no depende de lo que haya hecho el corredor. Es la misma ponderación que el
  rendimiento habitual y la consistencia (`docs/tiempo-perdido.md`), así que un tramo pesa lo
  mismo en P13 que en el resto de la app, y los tramos cortos y ruidosos pesan menos.

Entran **todos** los tramos de la clase, también los errores. A diferencia del IR medio del
histórico, aquí no se separan velocidad y errores: la pregunta es si el desnivel frena, y una
cuesta que hunde el ritmo puede pasar del umbral de error (el tipo «Físico» de
`docs/taxonomia.md`). Sacar los errores escondería justo lo que se busca. La tasa de error de la
clase, al lado, deja ver cuánto de un IR bajo viene de los errores.

**Limitación**: los `IR_i` de carreras distintas se mezclan sin normalizar por el habitual de cada
carrera. Si las subidas salen sobre todo de carreras en las que el corredor rinde menos (monte
frente a sprint urbano), la clase subida sale peor también por eso. El filtro de formato lo
atenúa.

### Resultado (`SlopeHistory`)

`config` (el umbral aplicado), `by_class` (las tres clases), `races_with_track` (carreras con
números y track: las que aportan tramos), `races_without_track`, `legs_without_track` y
`unclassified_legs`.

### Ejemplo de test

`crates/tramos-core/src/slope.rs`. Clasificación con perfiles sintéticos (distancia, subida,
bajada): 200 m, +2 −3 → llano; 200 m, +8 −1 (justo 4 m/100 m) → subida; 200 m, +7,98 → llano;
250 m, −10 → bajada; 200 m, +12 −10 → subida (6 frente a 5); +10 −12 → bajada; +10 −10 →
subida (empate); 1000 m, +39 −39 → llano; 50 m, +2 → subida; 49,9 m → sin clasificar. Y un
track sintético de punta a punta (track → métricas → clase) con un tramo en subida, uno llano,
uno en bajada, uno que sube 18 m y baja 14 m en 240 m (subida) y uno sin sub-track.

Agregado con dos carreras con track (referencia, `IR_i`, error):

- Subida: 40 s (1,0), 60 s (0,5, error) y 200 s (0,6, error): `n = 3`, tasa 2 / 3, IR
  (40 + 30 + 120) / 300 = **63,3 %**.
- Llano: 30 s (0,9) y 100 s (0,8): `n = 2`, tasa 0, IR 107 / 130 = **82,3 %**.
- Bajada: 50 s (0,8, error) y 120 s (1,2): `n = 2`, tasa 1 / 2, IR 184 / 170 = **108,2 %**.
- Tres tramos sin clasificar (uno sin altitud, uno sin sub-track y uno con las balizas
  cambiadas); una carrera sin track con 2 tramos que cuentan, y una sin habitual que no cuenta.

El test del comando (`app/src-tauri/src/history.rs`) comprueba con el FIT sintético que cada
clase sale de los tramos de la vista de carrera y que los tramos claramente lejos del umbral caen
en la clase que da el desnivel real (sin ruido) del generador.

## ¿Lento o desorientado? (P2)

El reparto de la pérdida de cada tramo en desvío, paradas y ritmo está en
`docs/tiempo-perdido.md`, "¿Lento o desorientado? (P2)". En el histórico
(`tramos_core::loss_breakdown::breakdown_history`, #27), el comando `history` devuelve en
`loss_breakdown`:

- `errors`: la suma, en todas las carreras que pasan el filtro con rendimiento habitual y track,
  de los errores repartidos (`legs`, `loss_s`, `detour_s`, `stopped_s` y `pace_s`). La pantalla
  enseña cada parte en % de `loss_s`.
- `races_with_track` y `races_without_track`.
- `errors_without_breakdown`: errores que cuentan pero no se reparten (carreras sin track, sin
  `r0` o tramos sin sub-track). Con `errors.legs`, suman los errores del total del histórico.

Solo los errores, como la pérdida del resto del histórico: la pregunta es de qué está hecho lo
que se pierde al fallar.

## Después de fallar (P8)

«¿Qué hago después de fallar?» (`docs/preguntas.md`, P8). Implementado en
`tramos_core::after_error` (#32). El comando `history` lo devuelve en `after_error` y la pantalla
lo dibuja en la sección «Después de fallar» (`docs/app.md`, "Vista histórica").

- **Mismas carreras y tramos:** las del histórico con el mismo filtro y, de cada una, sus tramos
  que cuentan (`pattern_legs`), **en orden**. El «tramo siguiente» es el siguiente de esa
  secuencia: los excluidos (el último, los de referencia corta y los que no tienen split) ni
  cuentan ni cortan nada, y un error en un tramo corto no cuenta como error. El primer tramo de
  cada carrera no tiene anterior y no entra. Nada cruza de una carrera a otra.
- **Encadenamiento:** tasa de error de los tramos cuyo anterior fue un error (`after_error`),
  frente a la de los tramos cuyo anterior fue limpio (`after_clean`). Cada uno es un `Rate` con
  `legs` (n), `errors` y `error_rate`.
- **Recuperación:** de los tramos que siguen a un error, cuáles se corren **más de un 5 % más
  rápido** (`ACCELERATION`), en velocidad en movimiento (`moving_speed_mps`, `docs/metricas.md`),
  que la **mediana de los tramos limpios de esa carrera**. Se usa la de esa carrera porque la
  velocidad depende mucho del terreno. Hacen falta al menos 3 tramos limpios con velocidad
  (`MIN_CLEAN_LEGS`). Salen dos `Rate`, `accelerated` y `not_accelerated`, con su tasa de error:
  si acelerar tras un error acaba en otro error. Los tramos tras un error sin velocidad para
  comparar (carrera sin FIT, sin sub-track o con pocos tramos limpios) van en
  `after_error_without_speed` y solo cuentan en el encadenamiento.
- **Rachas limpias:** cada tramo cae en un cubo según los tramos limpios seguidos justo antes
  (0, 1–2, 3–5, 6–10 y más de 10; `streaks[]` con `from`, `to` y `rate`). El cubo 0 es
  exactamente «tras un error».

Ejemplo de test (`crates/tramos-core/src/after_error.rs`, `x` = error, `o` = limpio):

- `oxxooxo` y `xosx` (con `s` corto, que no cuenta): tras un error, 1 de 4; tras un limpio, 3 de 4.
- `ooooxoxx` + 12 `o` + `x`: rachas 0 → 3 tramos y 1 error; 1–2 → 5 y 1; 3–5 → 5 y 1;
  6–10 → 5 y 0; más de 10 → 2 y 1.
- `oxooxxo` con los limpios a 3,0, 3,4, 3,2 y 3,2 m/s: mediana 3,2 y umbral 3,36. Tras los
  errores: a 3,4 m/s acelera y es limpio; a 3,3 m/s no acelera y es error; a 3,2 m/s no acelera
  y es limpio.

## Errores más comunes (P9)

«¿Qué errores son los más comunes?» (`docs/preguntas.md`, P9). Implementado en
`tramos_core::common_errors` (#31) sobre las etiquetas de `docs/taxonomia.md`. El comando
`history` lo devuelve en `common_errors` y la pantalla lo dibuja en la sección «Errores más
comunes» (`docs/app.md`, "Vista histórica").

- **Mismas carreras y tramos:** las del histórico con el mismo filtro y, de cada una, sus tramos
  que cuentan (`pattern_legs`). Una etiqueta en el último tramo o en uno de referencia corta no
  entra, igual que no entra su pérdida en el resto de patrones.
- **Qué fue cada tramo** (`leg_status`): manda la etiqueta del corredor; si no dice nada, el
  cálculo.

  | Etiqueta | Estado |
  | --- | --- |
  | Confirmación «Sí» | Error |
  | Confirmación «No» | Sin error, aunque el cálculo diga que sí |
  | Confirmación «Físico» | Físico |
  | Sin confirmación, con tipo | Error (el corredor le ha puesto tipo) |
  | Sin confirmación ni tipo (o sin etiqueta) | Lo que diga el cálculo; si es error, **sin revisar** |

- **Físico no es error.** Es tiempo perdido por el físico (cansancio, terreno), no un fallo de
  orientación, y no se entrena igual: no entra en el reparto ni en el número de errores, pero se
  cuenta aparte (`physical_legs`, `physical_loss_s`) para que no se pierda. Un error confirmado
  con tipo «Físico» de la taxonomía sí cuenta como error de ese tipo: lo ha dicho el corredor.
- **Reparto** (`ErrorTypes`): tramos (`legs`, n), errores y su pérdida (suma de `p_i`, s);
  errores por tipo (`by_type`, de más a menos errores, a igualdad más pérdida primero) y, dentro
  de cada tipo, por subtipo («sin subtipo» al final); errores **sin tipo** (`untyped`, con su
  pérdida), de los que `unreviewed` son los que solo propone el cálculo. Los tipos son claves de
  la taxonomía; la app les pone nombre.
- **Cruces:** el reparto sale entero (`total`), por cubo de duración del tramo de P7
  (`by_leg_length`, los seis cubos en orden, aunque estén vacíos), por formato (`by_format`:
  sprint, media, larga y sin formato, en ese orden) y por formato y cubo a la vez. Todo sale del
  núcleo: la app solo elige cuál enseñar.

Ejemplo de test (`crates/tramos-core/src/common_errors.rs`): tres carreras que cuentan con 9
tramos, 6 errores (175 s), 2 sin tipo (70 s, uno sin revisar) y un tramo físico de 8 s. Por tipo:
navegación 3 errores y 105 s (paralelo 2 y 45 s, pasarse 1 y 60 s) y ataque 1 y 0 s (sin
subtipo), porque un error marcado por el corredor puede no tener pérdida. Un error confirmado en el
último tramo y otro en un tramo de referencia corta no cuentan, ni una carrera sin rendimiento
habitual.

## ¿El cansancio anticipa el error? (P14)

«¿El cansancio anticipa el error?» (`docs/preguntas.md`, P14). Implementado en
`tramos_core::fatigue` (#34) con las métricas del FIT de cada tramo (`docs/metricas.md`) y las
etiquetas (`docs/taxonomia.md`). El comando `history` lo devuelve en `fatigue` y la pantalla lo
dibuja en la sección «Cansancio» (`docs/app.md`, "Vista histórica").

**Es un dato débil.** El pulso de muñeca llega con segundos de retraso, da saltos y en un tramo
corto apenas reacciona; además se mezclan pocas carreras. La pantalla lo avisa siempre y enseña
`n` en cada columna.

- **Mismas carreras y tramos:** las del histórico con el mismo filtro y, de cada una, sus tramos
  que cuentan (`pattern_legs`).
- **Qué fue cada tramo:** `leg_status` de P9: manda la etiqueta («No» deja limpio un error del
  cálculo) y lo marcado como **físico no es error ni limpio**: no entra en la deriva ni como
  tramo siguiente en «antes del error» (sí como anterior: su pulso vale igual).
- **Fase de carrera:** el tercio del tramo por número de tramos del recorrido (con el último):
  primero hasta `⌈L / 3⌉`, segundo hasta `⌈2L / 3⌉` y tercero el resto
  (`history::race_third`): 21 tramos → 1–7, 8–14 y 15–21; 10 → 1–4, 5–7 y 8–10. Generaliza el
  primer tercio de P11; por tramos y no por tiempo, que con picadas que faltan no se sabe.
- **Pulso de la carrera:** una carrera aporta pulso si al menos 3 (`MIN_LEGS`) de sus tramos que
  cuentan tienen pulso medio. Las demás (sin FIT, sin pulso o con pocos tramos con pulso) se
  cuentan en `races_without_heart_rate` y solo aportan esfuerzo.

### Valores relativos a cada carrera

El pulso cambia mucho de un día a otro (calor, formato, descanso) y la velocidad, con el
terreno. Mezclar el pulso en ppm de carreras distintas compararía días, no fases. Por eso las dos
medidas del pulso son **relativas a la mediana de su carrera**, como la velocidad de P8:

- `p_c`: mediana del pulso medio de los tramos que cuentan de la carrera (con o sin error).
- `r_c`: mediana de `pulso / velocidad en movimiento` de sus tramos **limpios**, si hay al menos
  3; si no, la carrera no aporta deriva.

Los ppm y la velocidad sin normalizar salen al lado, en la tabla, como referencia.

### Deriva (`drift`)

Por tercio, en los tramos limpios con pulso y velocidad de las carreras con `r_c`:

| Campo | Definición |
| --- | --- |
| `legs` | n. |
| `mean_relative_ratio` | Media de `(pulso / velocidad) / r_c` (1 = lo habitual en esa carrera). Si sube en el último tercio, el corredor necesita más pulso para ir igual de rápido: deriva cardiaca, cansancio. |
| `mean_heart_rate_bpm`, `mean_speed_mps` | Medias simples del pulso y la velocidad de esos tramos. |

Solo los limpios: un error baja la velocidad sin que sea cansancio. Media simple: cada tramo pesa
lo mismo (el pulso de cada tramo ya está ponderado por el tiempo).

### Antes del error (`before_error`, `before_clean`)

Como en P8, el «tramo anterior» es el anterior de la secuencia de tramos que cuentan (los
excluidos ni cuentan ni cortan) y el primero de cada carrera no tiene anterior. Cada tramo que
sigue a otro va, en **su** tercio, a `before_error` si es error o a `before_clean` si es limpio
(los físicos, a ninguno), con el pulso medio del tramo anterior:

| Campo | Definición |
| --- | --- |
| `legs` | n: tramos del grupo cuyo anterior tiene pulso. |
| `mean_relative_bpm` | Media de `pulso del anterior − p_c` (ppm; positivo = más alto de lo habitual en esa carrera). |
| `mean_heart_rate_bpm` | Media del pulso del anterior, sin normalizar. |

Se compara en la **misma fase** porque el pulso sube a lo largo de la carrera: si los errores se
concentraran al final, comparar todo junto daría más pulso antes del error sin que el pulso dijera
nada. Los errores sin pulso del anterior (carrera sin pulso o anterior sin sub-track) se cuentan
en `errors_without_heart_rate`.

### Esfuerzo percibido (`effort_error`, `effort_clean`, `effort_physical`)

El esfuerzo (1–10) que el corredor apunta en la etiqueta de un tramo (nivel 3), por tercio y
según el estado del tramo (error, limpio o físico): `legs` (tramos con esfuerzo, n) y
`mean_effort`. Es el del **propio tramo**, no el del anterior: es cuando el corredor lo nota, y lo
apunta sobre todo en los errores. No necesita FIT: cuentan todas las carreras.

### Ejemplo de test

`crates/tramos-core/src/fatigue.rs`, con `o` limpio y `x` error:

- **A** (10 tramos: tercios 1–4, 5–7 y 8–10), con FIT. Limpios 1, 2, 4, 5, 7 y 8 con
  pulso / velocidad 45, 50, 50, 50, 55 y 60 → `r_c` = 50; errores 3, 6 y 9. Pulsos 135, 150,
  158, 155, 160, 170, 165, 168 y 172 → `p_c` = 160.
- **B**, sin FIT: solo esfuerzo. **C**, con FIT: un físico, un error del cálculo marcado «No»,
  un tramo sin sub-track y solo dos limpios con velocidad (sin deriva). Pulsos 150, 160, 155 y
  170 → `p_c` = 157,5. **D**, sin rendimiento habitual: no cuenta.

Resultados:

- Deriva: 1.er tercio (0,9 + 1 + 1) / 3 = **96,7 %** (n = 3); 2.º (1,0 + 1,1) / 2 = **105 %**;
  3.º **120 %**.
- Antes del error / antes de limpio: 1.er tercio −10 ppm (n = 1) frente a (−25 − 2) / 2 =
  **−13,5 ppm** (n = 2); 2.º 0 frente a (−5 + 10 + 2,5 − 2,5) / 4 = **+1,25 ppm** (n = 4, con
  los dos de C); 3.º **+8** frente a **+5**. El tramo de C que sigue al físico no entra; su error
  tras el tramo sin sub-track y el error de B (sin FIT) van a `errors_without_heart_rate` = 2.
- Esfuerzo: 1.er tercio error 8, limpio 5 (marcado «No») y físico 9; 2.º errores (9 + 6) / 2 =
  **7,5** y limpio 4 (un error del cálculo marcado «No»); 3.º error 7. No cuentan el esfuerzo
  del último tramo ni el de un tramo de referencia corta.
