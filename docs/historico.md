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

Los tests están en `crates/tramos-core/src/history.rs`; los del comando, que comprueban que el
histórico sale de los mismos números que la vista de carrera, en `app/src-tauri/src/history.rs`.

## Para los análisis que vienen

P7, P10, P11 y P13 se apoyan en este histórico: mismas carreras, mismo filtro y, cuando cuentan
tramos, los mismos `pattern_legs`. Cada uno añade sus números al resultado del comando y sus
paneles a la pantalla.
