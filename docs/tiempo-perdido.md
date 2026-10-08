# Tiempo perdido

Adaptación del método de WinSplits. Todo se calcula por **recorrido** (categorías con la misma
secuencia de balizas agrupadas), no por categoría. La agrupación (igualdad exacta de la
secuencia de balizas) está en `docs/modelo.md`, sección "Agrupación por recorrido".

## Definiciones

Para el tramo *i* de un recorrido:

- **Referencia** `ref_i`: media del 25 % más rápido de los splits válidos del tramo
  (redondeando hacia arriba, mínimo 1 corredor). Solo cuentan corredores clasificados
  (estado 0) con picada en ambos extremos del tramo.
- **Índice de rendimiento** `IR_i = ref_i / t_i`, donde `t_i` es el split del corredor.
  100 % = igual que la referencia; más alto = mejor.
- **Rendimiento habitual** del corredor en la carrera: mediana de sus `IR_i` ponderada por `ref_i`
  (WinSplits pondera por longitud; aquí no hay longitudes en el .spl).
- **Tiempo esperado** `esp_i = ref_i / habitual`.
- **Pérdida** `p_i = t_i − esp_i` (segundos, puede ser negativa) y `p_i / esp_i` (porcentaje).
- **Error**: el tramo es error si `p_i > umbral_segundos` **y** `p_i / esp_i > umbral_porcentaje`.
  Ambos umbrales son configurables. Valores iniciales: 15 s y 10 %.
- **Tiempo sin errores**: tiempo total menos la suma de pérdidas de los tramos con error.

## Exclusiones

- El último tramo (de la última baliza, normalmente la 100 o la 200, a meta) se calcula pero se
  excluye de los análisis de patrones.
- Tramos con referencia menor de 20 s: se excluyen de los análisis de patrones.
- Tramos sin picada en alguno de sus extremos: sin split, sin pérdida.

## Fiabilidad

- Con pocos corredores en el recorrido la referencia es frágil. Si el recorrido tiene menos de
  4 corredores válidos, se marca la carrera como "referencia débil" en la interfaz.
- La referencia de las preguntas históricas puede pasar a ser el propio histórico del corredor;
  eso queda fuera del MVP.

## Precisiones de implementación

Lo que las definiciones de arriba no fijaban, decidido al implementarlas (#12) en
`tramos_core::lost_time` y en el oráculo `tools/reference/tiempo_perdido.py`:

- **Splits a partir de las picadas.** Los puntos del recorrido son salida (32736), balizas y meta
  (32752). Las picadas se emparejan en orden: para cada punto se busca la siguiente picada con su
  código a partir de la última emparejada; si no aparece, el punto queda sin hora y se sigue
  buscando desde el mismo sitio (las picadas que sobran se ignoran). `t_i` = hora del destino −
  hora del origen. Un split que no sea positivo (horas iguales o al revés) se trata como si no
  hubiera split.
- **Población.** La referencia solo usa clasificados (`RaceStatus::Ok`), pero **todos** los
  resultados del recorrido se analizan con ella: un no clasificado (baliza fallida, abandono,
  otro código de estado del .spl como el 5 o el 7) tiene `IR_i`, habitual, pérdidas y errores en
  los tramos que tenga. Un no presentado no tiene picadas con hora y sale sin números.
- **Tramo sin referencia** (ningún clasificado con split): sin `ref_i`, sin `IR_i` y sin pérdida
  para nadie.
- **25 %**: `ceil(n / 4)` de los `n` clasificados con split en ese tramo.
- **Rendimiento habitual**: entran todos los tramos con `IR_i`, **incluidos** el último y los de
  referencia corta (las exclusiones son solo para los análisis de patrones). Mediana ponderada:
  se ordenan los `IR_i` de menor a mayor (a igual valor, por número de tramo), se acumulan los
  pesos `ref_i` y es el primer `IR_i` con el que el acumulado **supera** la mitad del total; si
  lo **iguala** (tolerancia relativa 1e-9), la media de ese `IR_i` y el siguiente. Con pesos
  iguales es la mediana de siempre. Sin ningún `IR_i`, no hay habitual ni pérdidas.
- **Error**: las dos desigualdades son estrictas; `p_i / esp_i` se expresa en porcentaje
  (`loss_pct = 100 · p_i / esp_i`) y se compara con el umbral en porcentaje (10 = 10 %).
- **Tiempo total**: meta − salida (no la suma de splits). **Tiempo perdido** del corredor: suma de
  `p_i` de los tramos con error, de **todos** los tramos (también el último y los cortos).
  **Tiempo sin errores** = tiempo total − tiempo perdido.
- **Puesto por tramo**: 1 + número de clasificados con split estrictamente menor (empates con el
  mismo puesto: 1, 1, 3). A los no clasificados se les da el puesto que habrían tenido.
- **Diferencia acumulada respecto al tiempo ideal** (P4 de `docs/preguntas.md`): (hora del
  destino de *i* − salida) − tiempo ideal acumulado hasta el tramo *i*. Sin valor si falta la
  picada o si algún tramo hasta *i* no tiene ideal. Ver "Tiempo ideal" más abajo.
- **Exclusiones**: cada tramo lleva `is_last`, `short_reference` (`ref_i < 20` estricto) y
  `excluded_from_patterns` (cualquiera de las dos).
- **Referencia débil**: menos de 4 clasificados en el recorrido (todas sus categorías juntas).
- Los corredores se identifican por su posición (índice de categoría en `Event::classes` e índice
  en `Class::results`): el .spl no trae id de corredor.

## Tiempo ideal

El tiempo ideal acumulado hasta el tramo *i* es la suma, tramo a tramo, de un tiempo ideal por
tramo. Hay dos definiciones y se elige en la configuración (`ideal_time`):

| `ideal_time` | Ideal del tramo *j* | Ideal hasta *i* |
| --- | --- | --- |
| `sum_of_references` (**por defecto**) | `ref_j` | `Σ ref_j` (j ≤ i) |
| `sum_of_best_splits` | mejor split del tramo entre los clasificados con split (la misma población que la referencia) | `Σ min t_j` (j ≤ i): el "superman" de WinSplits |

- **Por qué la referencia por defecto**: es coherente con el resto del método (pérdidas y
  rendimiento se miden contra `ref_i`) y es más estable, porque un solo split muy rápido (o mal
  picado) no la mueve tanto.
- **Por qué existe la opción**: la gráfica clásica de WinSplits usa el "superman", que es lo que
  muchos corredores y entrenadores conocen; la entrenadora puede preferir una u otra. Con el
  superman casi todos van por detrás del ideal y el ganador rara vez acaba en 0.
- Solo cambian `ideal_elapsed_s` y `behind_ideal_s`; referencias, rendimiento, pérdidas y errores
  son iguales con las dos definiciones.
- Un tramo sin clasificados con split no tiene ideal con ninguna de las dos: el ideal acumulado
  queda sin valor desde ese tramo.

## Frente al grupo (P4)

`tramos_core::comparison::course_comparison(&Event, ResultRef, &LostTimeConfig)` da todos los
corredores del recorrido de un resultado (todas las categorías que lo comparten), para
superponerlos en las gráficas de P4. Sale del mismo `analyze_course` que el informe del
corredor, así que sus números coinciden con los de `runner_report`.

- `legs[]` (`index`, `from`, `to`) e `ideal_time_s` (tiempo ideal del recorrido entero, según
  `ideal_time`).
- `runners[]`: `result` (`class_index`, `result_index`), `is_self` (el resultado pedido),
  nombre, club, `class_name`, `status`, `place` (en su categoría), `course_place`, `total_s` y,
  una posición por tramo, `behind_ideal_s`, `loss_s` e `is_error`.
- **Puesto en el recorrido** (`course_place`): 1 + clasificados del recorrido con tiempo total
  estrictamente menor (empates con el mismo puesto). Solo los clasificados con tiempo.
- **Orden**: clasificados por tiempo total (los empates, en el orden de la carrera) y después el
  resto, en el orden de la carrera.
- Por la definición de la diferencia acumulada, la del ganador en meta es su tiempo total menos
  el tiempo ideal (criterio de aceptación de #23, comprobado con el oráculo sobre el fixture).

## Dónde gano y dónde pierdo (P5)

Sale de la pérdida (`tramos_core::gain_loss`, P5 de `docs/preguntas.md`):

- **Ganancia** del tramo *i*: `g_i = esp_i − t_i = −p_i` (segundos). Positiva = mejor que el
  rendimiento habitual. Una pérdida de 0 da ganancia 0 (no −0).
- **Ganancia acumulada** hasta *i*: suma de las `g_j` conocidas con j ≤ i. Un tramo sin pérdida
  (sin split o sin referencia) no tiene ganancia ni acumulada y no suma a los siguientes. Como la
  pérdida, cuenta **todos** los tramos, con error o sin él: es "voy tantos segundos por delante o
  por detrás de lo que me tocaba con mi rendimiento habitual".
- **Racha perdiendo**: dos o más tramos seguidos con `g_i < 0` estricto. Un tramo sin dato o con
  `g_i = 0` la corta. De cada racha se da el primer y el último tramo y los segundos perdidos
  (`Σ −g_i`, positivo).

Ejemplo: pérdidas 8, −2, 1,5, 20, 0,5, −4, 3 y 6 s → ganancias −8, 2, −1,5, −20, −0,5, 4, −3 y
−6 s; acumulada final −33 s; rachas 3–5 (22 s) y 7–8 (9 s). El tramo 1 pierde solo y no es racha.

Va en el informe del corredor (`tramos_core::runner_report`, el de `tramos analizar` y la app),
no en `analyze_event`: por tramo `gain_s` y `cumulative_gain_s`, y en los totales
`losing_streaks[]` con `first_leg`, `last_leg` y `loss_s`.

## Consistencia (P10)

¿Soy consistente? Cuánto varía el rendimiento de un tramo a otro dentro de una carrera
(`tramos_core::consistency`, P10 de `docs/preguntas.md`):

- **Consistencia** de la carrera: desviación típica de los `IR_i` ponderada por `ref_i`.
  Con `w_i = ref_i`, `m = Σ w_i·IR_i / Σ w_i` y `consistencia = √(Σ w_i·(IR_i − m)² / Σ w_i)`.
  Menor = más consistente; 0 = el mismo IR en todos los tramos. Se da como fracción, igual que el
  IR (0,083 = 8,3 puntos de IR; la app lo escribe «± 8,3 %»).
- **Tramos**: los de los análisis de patrones (`pattern_legs`): con IR, sin el último ni los de
  referencia corta (ver "Exclusiones"). El último tramo, un esprint hasta meta, y los muy cortos,
  donde un par de segundos mueven mucho el IR, darían una dispersión que no habla de cómo se ha
  corrido la carrera. Son además los mismos tramos que cuenta el histórico.
- **Desviación de población** (dividir entre `Σ w_i`, sin corrección de Bessel): los pesos son
  duraciones, no repeticiones, y la corrección no tiene sentido con ellos.
- **Alrededor de la media ponderada**, no del rendimiento habitual (que es una mediana): es la
  definición de siempre de la desviación típica y no depende de cómo se elige el habitual.
- Con **menos de 2 tramos** que cuenten, no hay valor (`null`): con uno saldría siempre 0.

Ejemplo: IR 1,0 (ref 60 s), 0,8 (ref 120 s) y 1,2 (ref 60 s). `m = (60 + 96 + 72) / 240 =
0,95`; varianza `(60·0,05² + 120·0,15² + 60·0,25²) / 240 = 6,6 / 240 = 0,0275`; consistencia
`√0,0275 = 0,1658` (± 16,6 %). Con pesos iguales, IR 0,9 y 1,1 dan 0,1.

Va en los totales del informe del corredor (`consistency`). En el histórico, cada grupo da la
media de la consistencia de sus carreras (`docs/historico.md`).

## ¿Lento o desorientado? (P2)

Versión inicial, heurística (`tramos_core::loss_breakdown`, P2 de `docs/preguntas.md`): reparte
la pérdida `p_i` de un tramo en **desvío**, **paradas** y **ritmo** con las métricas del FIT del
tramo (`docs/metricas.md`). Es la pregunta más interpretativa: los números son una estimación.

- `d_run` = distancia recorrida en el tramo (`distance_m`); `d_line` = línea recta entre las
  posiciones de las balizas (`straight_m`); `v_mov` = velocidad en movimiento
  (`moving_speed_mps`).
- **Relación habitual** `r0`: mediana simple de `d_run / d_line` (`distance_ratio`) en los
  tramos **sin error** de la carrera que cuentan (`pattern_legs`) y tienen relación. Nadie va en
  línea recta: `r0` es cuánto rodea el corredor normalmente en esa carrera. Con menos de 3 de
  esos tramos no hay `r0` y no se reparte nada.
- **Desvío** = `(d_run − r0 · d_line) / v_mov`: el tiempo que cuestan, a su velocidad en
  movimiento del tramo, los metros de más respecto a lo habitual. Negativo si el tramo fue más
  directo de lo habitual.
- **Paradas** = tiempo parado del tramo (`stopped_s`): velocidad < 0,5 m/s, sin los 5 s
  siguientes a la picada.
- **Ritmo** = el resto, `p_i − desvío − paradas`. Así las tres partes suman siempre `p_i`.
- **Tramos**: los que cuentan (`pattern_legs`), con pérdida, con sub-track cuyo número y balizas
  casan con la tabla de tramos y con `v_mov`. Los totales de la carrera (`errors`) suman solo los
  errores; los errores que no se pueden repartir se cuentan aparte (`errors_without_breakdown`).

**Las partes pueden salir negativas.** La pérdida se mide contra lo esperado con el rendimiento
habitual de toda la carrera, y el desvío, con la velocidad del propio tramo: si un rodeo se corre
más rápido de lo habitual, el desvío puede pasar de la pérdida y el ritmo sale negativo. En el
FIT sintético, el tramo 9 (rodeo de 1215 m frente a 250 m en línea recta y 30 s parado) pierde
284 s: desvío 297 s, paradas 30 s y ritmo −43 s. La lectura es «el rodeo y la parada explican
toda la pérdida».

**Parámetros**, todos aquí:

| Parámetro | Valor | Dónde |
| --- | --- | --- |
| Tramos sin error mínimos para `r0` | 3 | `loss_breakdown::MIN_CLEAN_LEGS` |
| Velocidad de parado | 0,5 m/s | `MetricsOptions::stop_speed_mps` |
| Segundos tras la picada que no son parada | 5 s | `MetricsOptions::punch_grace_s` |
| Hueco del track (su tiempo no es parada ni movimiento, y queda en el ritmo) | 10 s | `MetricsOptions::max_gap_s` |

Ejemplo (test de `crates/tramos-core/src/loss_breakdown.rs`): tramos sin error con relación 1,2,
1,1 y 1,4 → `r0 = 1,2`. Un error de 150 s con 600 m recorridos frente a 200 m en línea recta, a
3 m/s y 20 s parado: desvío (600 − 1,2 · 200) / 3 = **120 s**, paradas **20 s**, ritmo 150 − 140
= **10 s**. Un error de 40 s por la línea habitual y sin parar es todo ritmo.

Criterio de aceptación de #27 (`crates/tramos-core/tests/loss_breakdown.rs`): con el FIT
sintético, el tramo del rodeo atribuye más de la mitad de su pérdida a desvío y parada.

## Salida (`tramos_core::lost_time`)

`analyze_event(&Event, &LostTimeConfig) -> LostTimeReport` (o `analyze_course` para un
`CourseGroup`). Todo deriva `serde`; en JSON:

- `config`: `error_threshold_s` (15), `error_threshold_pct` (10) e `ideal_time`
  (`sum_of_references` por defecto, o `sum_of_best_splits`). Los campos ausentes toman el valor
  por defecto, así que una configuración guardada antes de existir `ideal_time` sigue valiendo.
- `courses[]` (orden de `group_by_course`): `course`, `classes`, `valid_runners`,
  `weak_reference`, `legs[]` y `runners[]`.
- `legs[]`: `index` (desde 1), `from`, `to`, `valid_splits`, `reference_count`, `reference_s`,
  `ideal_elapsed_s` (tiempo ideal acumulado hasta el final del tramo, según `ideal_time`),
  `is_last`, `short_reference`, `excluded_from_patterns`.
- `runners[]` (todos los resultados, categoría a categoría): `class_index`, `result_index`,
  `status`, `place`, `total_s`, `usual_performance` (1 = 100 %), `lost_time_s`, `error_count`,
  `time_without_errors_s` y `legs[]` con `index`, `split_s`, `elapsed_s`, `place`,
  `performance_index` (1 = 100 %), `expected_s`, `loss_s`, `loss_pct`, `is_error` y
  `behind_ideal_s`. Lo que no se puede calcular va a `null` (`is_error` a `false`).

## Ejemplo de test

Recorrido con 4 corredores y 2 tramos (salida → 31 → meta); splits del tramo 1: 60, 62, 70, 90 s
→ `ref_1 = 60` (25 % de 4 = 1 corredor). Un corredor con 90 s en el tramo 1 tiene `IR_1 = 0,667`.

Completo, con los splits del tramo 2 (A 66, B 60, C 75, D 60 s → `ref_2 = 60`), para el
corredor D (90 y 60 s):

- `IR_1 = 60/90 = 0,667`, `IR_2 = 60/60 = 1`. Pesos iguales (60 y 60): el acumulado del primero
  iguala la mitad, así que habitual = (0,667 + 1) / 2 = **0,833**.
- `esp_1 = esp_2 = 60 / 0,833 = 72 s`. `p_1 = 90 − 72 = 18 s` (25 %) → **error** (18 > 15 y
  25 % > 10 %). `p_2 = 60 − 72 = −12 s` (−16,7 %) → no.
- Tiempo total 150 s, tiempo perdido 18 s, tiempo sin errores **132 s**.
- Puesto en el tramo 1: 4.º; en el tramo 2 empata con B a 60 s: 1.º (A 3.º, C 4.º).
- Tiempo ideal: 60 y 120 s; D pasa a 90 y 150 s → diferencia acumulada +30 y +30 s. Aquí las
  dos definiciones coinciden, porque con 4 corredores la referencia es el mejor split.
- El tramo 2 es el último: se calcula, pero `excluded_from_patterns`.

Ejemplo de las dos definiciones del tiempo ideal: 5 corredores y 2 tramos; tramo 1 en 50, 80, 54,
70 y 60 s, tramo 2 en 30 s todos. `ref_1 = (50 + 54) / 2 = 52` (25 % de 5 = 2 corredores),
`ref_2 = 30`.

- `sum_of_references`: ideal 52 y 82 s. El de 80 s (80 y 110 s) va +28 y +28 s.
- `sum_of_best_splits`: ideal 50 y 80 s. El mismo corredor va +30 y +30 s.

Los tests del núcleo (`crates/tramos-core/src/lost_time.rs`) y del oráculo
(`tools/test_tiempo_perdido.py`) incluyen este caso y otros calculados a mano.

## Oráculo y test de paridad

`tools/reference/tiempo_perdido.py` implementa este documento en Python, de forma independiente
del núcleo, y genera `fixtures/spl/baltanas-anon.tiempo-perdido.expected.json` (umbrales por
defecto y tiempo ideal por defecto) con un `resumen` legible del recorrido de M-SEN para revisión
humana (revisado en el PR de #12):

```bash
python3 tools/reference/tiempo_perdido.py fixtures/spl/baltanas-anon.spl \
  > fixtures/spl/baltanas-anon.tiempo-perdido.expected.json
# otra definición del tiempo ideal: --ideal suma-mejores (por defecto, suma-referencias)
```

Para no pesar (~260 KB), el JSON va en forma compacta: los tramos de cada corredor son filas
cuyas columnas se nombran una sola vez en `runner_leg_columns`, y los corredores sin ningún split
(no presentados y similares) llevan `legs: []`, porque todos sus tramos tienen solo `index` e
`is_error: false` y el resto a `null`. El test de Rust lo devuelve a la forma del informe del
núcleo antes de comparar, así que se comparan todos los tramos.

El JSON redondea los flotantes a 6 decimales; el test de Rust
(`crates/tramos-core/tests/lost_time_baltanas.rs`) compara todo el informe con tolerancia absoluta
de 1e-6 en los flotantes y exactitud en el resto, ignorando `resumen`. Con `sum_of_best_splits`
no hay otro JSON: el test reconstruye el superman a partir de los splits del JSON del oráculo y
comprueba `ideal_elapsed_s` y `behind_ideal_s`, y los tests de Python hacen lo mismo con el
oráculo.
