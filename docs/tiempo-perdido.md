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
  resultados del recorrido se analizan con ella: un no clasificado (baliza fallida, abandono)
  tiene `IR_i`, habitual, pérdidas y errores en los tramos que tenga. Un no presentado no tiene
  picadas con hora y sale sin números.
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
  en `Class::results`), no por `Runner::id`, que no es único.

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

El JSON redondea los flotantes a 6 decimales; el test de Rust
(`crates/tramos-core/tests/lost_time_baltanas.rs`) compara todo el informe con tolerancia absoluta
de 1e-6 en los flotantes y exactitud en el resto, ignorando `resumen`. Con `sum_of_best_splits`
no hay otro JSON: el test reconstruye el superman a partir de los splits del JSON del oráculo y
comprueba `ideal_elapsed_s` y `behind_ideal_s`, y los tests de Python hacen lo mismo con el
oráculo.
