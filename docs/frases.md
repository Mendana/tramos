# Resumen en frases

Unas pocas frases arriba del histórico (pantalla Estadísticas) y de cada carrera (pestaña
Resumen) que dicen lo importante en español llano, para quien no es de datos (#126). Por
ejemplo: «Fallas más en los tramos largos (de 4 a 8 min): el 40 % son error, frente al 15 % de
media».

Implementado en `tramos_core::insights`. La app las recibe hechas en los comandos `history` y
`race_detail` (`docs/app.md`) y no calcula nada: solo las enseña y enlaza.

## Principios

- **Sobre los análisis que ya existen.** Las reglas no recalculan nada: comparan con un umbral los
  números que ya dan P1, P2, P5, P6, P7, P8, P9, P11 y P13 (`docs/preguntas.md`), con sus
  definiciones (`docs/historico.md`, `docs/tiempo-perdido.md`). Por eso las frases del histórico
  respetan los filtros: salen de los mismos análisis, con el mismo filtro.
- **Como mucho 3 frases**, las más importantes (`MAX_INSIGHTS`, decisión de #126).
- **Con pocos datos la frase sale, pero con aviso** («con pocas carreras», decisión de #126). Cada
  regla tiene dos mínimos: por debajo del **mínimo** no sale; entre el mínimo y el **suficiente**
  sale marcada (`few_data`) con su aviso (`caveat`).
- **Cada frase enlaza a la vista que la justifica** (`target`): el análisis que la enseña entero.
- **Los umbrales entran**: «al menos 10 puntos» incluye justo 10 (con una tolerancia de 10⁻⁹
  para que el redondeo binario no deje fuera el caso justo).

## Salida (`Insight`)

| Campo | Qué es |
| --- | --- |
| `rule` | Identificador de la regla (`leg_length`, `clean_race`…; tablas de abajo). |
| `text` | La frase, con sus números redondeados (% sin decimales, tiempos `m:ss`). |
| `few_data` | `true` si sale con pocos datos. |
| `caveat` | El aviso que va junto a la frase si `few_data`: «con pocas carreras», «con pocos tramos», «con pocos errores» o «referencia débil». `null` si no. |
| `target` | La vista que la justifica (abajo, "Destinos"). |

## Cómo se eligen las 3

1. Cada regla da como mucho una frase.
2. Primero las frases con datos suficientes y después las de pocos datos.
3. Dentro de cada grupo, por **prioridad** de la regla (1 = la más importante).
4. Se quedan las 3 primeras. Sin ninguna, la app no pinta el bloque.

La prioridad es fija y está pensada para lo que más se puede entrenar: dónde y cómo se falla
antes que cuánto frena el terreno o el descanso.

## Umbrales comunes

| Constante | Valor | Qué es |
| --- | --- | --- |
| `SUFFICIENT_RACES` | 5 | Por debajo, la frase del histórico lleva «con pocas carreras». |
| `MIN_LEGS` | 5 | Tramos mínimos de un grupo (cubo, clase de desnivel, tramos tras un error, tramos de entrada) para que salga. |
| `SUFFICIENT_LEGS` | 10 | Por debajo, «con pocos tramos» (o «con pocos errores»). Coincide con el mínimo de la vista de grupo (P15). |
| `RATE_GAP` y `RATE_RATIO` | 10 puntos y 1,5 veces | Una tasa de error es «claramente mayor» que otra si cumple las dos: 12 % frente a 2 % sí; 45 % frente a 35 %, no (un 29 % más). |
| `PERFORMANCE_GAP` | 10 puntos | Diferencia de IR para decir que se rinde peor. |

En el histórico, el aviso de carreras va primero: si hay menos de 5 carreras, el aviso es «con
pocas carreras» aunque también falten tramos.

## Reglas del histórico

`history_insights(&HistoryAnalyses)`, con los análisis del comando `history` (todos con el mismo
filtro) y la taxonomía para el nombre de los tipos de error.

| Prioridad | `rule` | Se apoya en | Sale si | Mínimo | Suficiente | Destino |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `leg_length` | P7, `by_leg_length` y la tasa de error del total | La tasa del peor cubo de duración es claramente mayor que la del total. | El cubo con ≥ 5 tramos (a igual tasa, el más corto). | 5 carreras y 10 tramos en el cubo. | `leg_length` |
| 2 | `common_error` | P9, `common_errors.total` | El tipo con más errores se lleva ≥ 40 % de los errores **con tipo**. | 3 errores con tipo. | 5 carreras y 10 errores con tipo («con pocos errores»). | `common_errors` |
| 3 | `after_error` | P8, `after_error` | La tasa tras un error es claramente mayor que tras un tramo limpio. | 5 tramos tras un error y 5 tras uno limpio. | 5 carreras y 10 tramos tras un error («con pocos errores»). | `after_error` |
| 4 | `loss_breakdown` | P2, `loss_breakdown.errors` | Una parte (desvío, paradas o ritmo) es ≥ 50 % de la pérdida de los errores repartidos. | 3 errores repartidos y pérdida > 0. | 5 carreras con track y 10 errores repartidos («con pocos errores»). | `loss_breakdown` |
| 5 | `slope` | P13, `by_slope` | El IR de la peor clase (subida o bajada) está ≥ 10 puntos por debajo del llano. | 5 tramos en la clase y en llano (a igual IR, la subida). | 5 carreras con track y 10 tramos en las dos clases. | `slope` |
| 6 | `format` | P6, `history.by_format` | La tasa de error del peor formato es claramente mayor que la del mejor. | Dos formatos con ≥ 2 carreras cada uno (con filtro de formato no sale; las carreras sin formato no cuentan). | 5 carreras en los dos. | `formats` |
| 7 | `days_off` | P11, `days_off` y el IR medio del total | El IR de entrada (tres primeros tramos) del peor cubo de **más de 7 días** está ≥ 10 puntos por debajo del IR medio del total (la misma referencia que el panel). | 2 carreras y 5 tramos de entrada en el cubo. | 5 carreras en el cubo. | `days_off` |

Textos (los números entre llaves salen de cada caso):

- `leg_length`: «Fallas más en los tramos {cortos | medios | largos} ({de 4 a 8 min}): el {40 %}
  son error, frente al {15 %} de media.» Cortos hasta 1 min, largos desde 4 min.
- `common_error`: «Tu error más común: {navegación} ({5} de tus {10} errores con tipo).»
- `after_error`: «Un error suele traer otro: tras fallar, fallas el {40 %} de los tramos; tras un
  tramo limpio, el {15 %}.»
- `loss_breakdown`, según la parte:
  - desvío: «Cuando fallas, sobre todo te desvías: el {60 %} del tiempo de tus errores es por
    correr de más.»
  - paradas: «Cuando fallas, sobre todo te paras: el {67 %} del tiempo de tus errores es tiempo
    parado.»
  - ritmo: «Cuando fallas, sobre todo vas más lento: el {83 %} del tiempo de tus errores es
    ritmo, no desvío ni paradas.»
- `slope`: «Las {subidas | bajadas} te frenan: rindes al {72 %} {en subida | en bajada} y al
  {90 %} en llano.»
- `format`: «Fallas más en {media} que en {sprint}: el {25 %} de tus tramos son error, frente al
  {10 %}.»
- `days_off`: «Tras {entre 15 y 30 | más de 30} días sin competir entras peor en mapa: rindes al
  {80 %} en los tres primeros tramos, frente a tu {92 %} de media.»

**Por qué** solo los cubos de más de 7 días en `days_off`: la pregunta es por el descanso; entrar
peor compitiendo seguido no es lo que pregunta P11. **Por qué** errores *con tipo* en
`common_error`: si la mitad de los errores no tiene tipo, «el 30 % de tus errores» escondería que
casi todos los que tienen tipo son del mismo; la frase dice sobre cuántos se apoya. P10
(consistencia) y P14 (cansancio, dato débil) no tienen frase por ahora.

## Reglas de la carrera

`race_insights(&RunnerReport)`, con el informe de la vista de carrera (P1 y P5).

**Mínimo de todas**: la carrera tiene rendimiento habitual y al menos 3 tramos con pérdida
(`MIN_RACE_LEGS`); si no, ninguna frase. **Pocos datos**: con referencia débil (pocos
clasificados en el recorrido, `docs/tiempo-perdido.md`), «referencia débil»; si no, con menos de 8
tramos con pérdida (`SUFFICIENT_RACE_LEGS`), «con pocos tramos». El aviso es de la carrera: lo
llevan todas sus frases.

| Prioridad | `rule` | Se apoya en | Sale si | Destino |
| --- | --- | --- | --- | --- |
| 1 | `clean_race` | P1, `error_count` | Ningún error. | `legs` |
| 2 | `concentrated_loss` | P1, los tramos con error y `lost_time_s` | El tramo con error más caro se lleva ≥ 50 % del tiempo perdido y hay al menos 2 errores; o, si no, los dos más caros, y hay al menos 3 errores. Con un solo error no sale: sería obvio. | `legs` |
| 3 | `losing_streak` | P5, `losing_streaks` | La racha perdiendo más cara tiene ≥ 3 tramos y ≥ 30 s. | `gain_loss` |
| 4 | `errors_by_third` | P1, los tramos con error y los tercios de `history::race_third` | Al menos 3 errores y ≥ 2/3 de ellos en el mismo tercio (por número de tramos del recorrido, como P11 y P14). | `legs` |

Textos:

- `clean_race`: «Carrera limpia: no fallaste en ningún tramo.»
- `concentrated_loss`: «Un solo tramo, el {4}, se llevó el {75 %} del tiempo perdido ({1:30} de
  {2:00}).» o «Dos tramos, el {3} y el {7}, se llevaron el {69 %} del tiempo perdido ({1:50} de
  {2:40}).»
- `losing_streak`: «Del tramo {2} al {4} encadenaste {3} tramos perdiendo tiempo: {0:37} en
  total.»
- `errors_by_third`: «La mayoría de tus errores ({3} de {4}) llegaron {al principio de la carrera
  (primer tercio) | en la mitad de la carrera (segundo tercio) | al final de la carrera (último
  tercio)}.»

## Destinos

`target` es el análisis que justifica la frase. La app sabe dónde está cada uno:

| `target` | Pantalla | Pestaña |
| --- | --- | --- |
| `formats` | Estadísticas | Resumen |
| `leg_length`, `common_errors`, `slope`, `loss_breakdown` | Estadísticas | ¿Dónde fallo? |
| `days_off` | Estadísticas | ¿Cómo evoluciono? |
| `after_error` | Estadísticas | Cabeza y piernas |
| `legs` | Carrera | Tramos |
| `gain_loss` | Carrera | Análisis |

## Tests

En `crates/tramos-core/src/insights.rs`, con datos sintéticos y números calculados a mano: cada
regla tiene un test que la dispara, otro que no y otro con pocos datos. Además: que salen como
mucho 3 y por prioridad, que las de pocos datos van detrás, que los umbrales entran y la forma
JSON. En la app (`app/src-tauri`), que el comando `history` da las frases de sus análisis con el
mismo filtro (y ninguna si el filtro no deja carreras) y que `race_detail` da las de su informe.

Ejemplos (los de los tests):

- `leg_length`: total 15 de 100 tramos con error (15 %); cubo de 4–8 min, 8 de 20 (40 %): 25
  puntos más y 2,7 veces → sale. Con 4 de 20 (20 %), 5 puntos → no sale. Con 4 de 6 en
  30 s–1 min (67 %) → sale «con pocos tramos».
- `concentrated_loss`: errores de 60, 50, 30 y 20 s (2:40). El más caro es el 37,5 %; los dos más
  caros, 110 s, el 68,75 % → «Dos tramos, el 3 y el 7, se llevaron el 69 %…». Cinco errores de
  40 s: los dos más caros son el 40 % → no sale.
- `errors_by_third`: 12 tramos (tercios 1–4, 5–8 y 9–12) con errores en 2, 9, 10 y 12 → 3 de 4 al
  final → sale. Uno en cada tercio → no sale.
