# Exportar los datos para análisis

Para analizar los datos fuera de la app (pandas, R, modelos de ML) la copia de seguridad no sirve:
es la base SQLite entera, con los ficheros originales. **Exportar datos para análisis** (#217, en
Ajustes, junto a la copia de seguridad) escribe en una carpeta que elige el usuario unas tablas
planas, limpias y documentadas:

| Fichero | Qué es |
| --- | --- |
| `races.csv` | Una fila por carrera propia. |
| `legs.csv` | Una fila por tramo de esas carreras: el formato natural para ML. |
| `samples.csv` | **Opcional** (#219): la serie del track de cada carrera con FIT, un instante por segundo, para modelos de secuencias. |
| `schema.json` | Cada columna con su tipo, unidad, descripción y el documento de `docs/` donde se define. |
| `LEEME.md` | Lo mismo para una persona: cómo cargarlo y las columnas. |

Todo sale del núcleo (`tramos_core::export` arma las filas y escribe el CSV, el esquema y el
`LEEME.md`) con lo que la app ya calcula: el informe de cada carrera (`runner_report`, con las
etiquetas del corredor), las métricas de tramo, el rendimiento físico y la serie de la carrera
(`race_series`). La app lee la base y llama al núcleo (`app/src-tauri/src/export.rs`, comando
`export_analysis`); no recalcula nada, así que los números son los que se ven en pantalla.

## Qué se exporta

- Las carreras propias: los resultados vinculados a la persona del usuario. Las que no tomó la
  salida (`did_not_start`) no se exportan: no hay nada que analizar, y tampoco cuentan como «carrera
  anterior» (`docs/historico.md`, "Días sin competir (P11)").
- Todas, sin filtros: la exportación no sigue el filtro del histórico.
- Con los umbrales de error de Ajustes, las etiquetas del corredor y las zonas de pulso que haya
  en ese momento. Exportar de nuevo después de cambiarlos da otros números en las columnas que
  dependen de ellos (`is_error`, `lost_time_s`, `heart_rate_load`…).
- Los mismos `race_id` mientras la carrera siga en la base de datos: son los identificadores de
  la app. Sirven para unir dos exportaciones distintas.
- `samples.csv` solo si se marca **Incluir la serie de cada carrera** al exportar (desmarcada por
  defecto: ocupa mucho más que las otras tablas), y sus coordenadas solo si además se marca
  **Incluir coordenadas** (también desmarcada). Sin marcar nada, la exportación son los cuatro
  ficheros de siempre.

## Formato

- `schema.json` lleva la versión del formato, `format_version`: 1 hoy. Sube cuando se quita o cambia el significado de
  una columna; añadir columnas al final no la cambia, así que conviene leer las columnas por
  nombre y no por posición.
- **CSV en UTF-8**, sin BOM, separado por comas, con un `\n` al final de cada línea y la
  cabecera en la primera. Los campos con coma, comillas o saltos de línea van entre comillas
  dobles, con las comillas dobladas (RFC 4180).
- **Punto decimal** y sin separador de miles. Los números de `races.csv` y `legs.csv` se escriben
  con toda su precisión (sin redondear): `0.3333333333333333`. Los de `samples.csv` van
  redondeados para que el fichero no pese de más (abajo).
- **Fechas** en ISO 8601, `AAAA-MM-DD`: la fecha local de la carrera.
- **Sí o no**: `true` o `false`.
- **Vacío para lo que falta** (sin FIT, sin pulso, sin etiqueta, sin split…). Un vacío no es un
  cero: pandas lo lee como `NaN`.
- Columnas en inglés y en `snake_case`. Las categorías (`kind`, `format`, `status`…) también.
  Las duraciones, en segundos; las unidades de cada columna, abajo y en el esquema.

### `schema.json`

```json
{
  "format_version": 1,
  "conventions": { "encoding": "UTF-8, sin BOM", "decimal_separator": ".", "...": "..." },
  "tables": [
    {
      "file": "races.csv",
      "description": "Una fila por carrera propia (sin las no presentadas).",
      "columns": [
        { "name": "kind", "type": "string", "description": "...", "source": "docs/historico.md",
          "allowed_values": ["race", "training"] },
        { "name": "total_s", "type": "number", "unit": "s", "description": "...",
          "source": "docs/tiempo-perdido.md" }
      ]
    },
    { "file": "samples.csv", "description": "...", "step_s": 1.0, "columns": ["..."] }
  ]
}
```

`type` es `integer`, `number`, `string`, `boolean` o `date`. `unit` y `allowed_values` solo salen
cuando hay. El esquema describe solo lo que se ha exportado: `samples.csv` solo está si se pidió, y
con `lat` y `lon` solo si se pidieron las coordenadas. `step_s` (solo en las series) son los
segundos entre dos filas.

## Columnas

La lista es la de `RACE_COLUMNS`, `LEG_COLUMNS` y `SAMPLE_COLUMNS` en
`crates/tramos-core/src/export.rs`, y un test comprueba que este documento nombra todas. La última columna dice dónde se define cada número.

### `races.csv`

Las columnas de `track_duration_s` en adelante van vacías si la carrera no tiene FIT guardado o no se
ha podido alinear con las picadas. Qué mide cada una, y cuándo sale vacía, está en
`docs/rendimiento-fisico.md`.

| Columna | Tipo | Unidad | Qué es | Definición |
| --- | --- | --- | --- | --- |
| `race_id` | integer |  | Identificador estable de la carrera propia: el del resultado en la base de datos de la app. No cambia al recalcular ni al renombrar. Es la clave que une con legs.csv. | `docs/exportar-datos.md` |
| `event_id` | integer |  | Identificador de la prueba en la base de datos: dos carreras propias con el mismo event_id son de la misma prueba. | `docs/exportar-datos.md` |
| `date` | date |  | Día de la carrera (fecha local). | `docs/exportar-datos.md` |
| `kind` | string |  | Competición (race) o entrenamiento (training). | `docs/historico.md` |
| `format` | string |  | Formato: sprint, middle o long. Vacío si no se ha fijado. | `docs/historico.md` |
| `status` | string |  | Estado en la clasificación. Los no presentados no se exportan. | `docs/exportar-datos.md` |
| `legs` | integer |  | Tramos del recorrido, contando el último (el de la última baliza a meta). | `docs/tiempo-perdido.md` |
| `total_s` | number | s | Tiempo de salida a meta. Vacío sin tiempo. | `docs/tiempo-perdido.md` |
| `place` | integer |  | Puesto en la categoría. Solo los clasificados lo tienen. | `docs/exportar-datos.md` |
| `classified` | integer |  | Clasificados de la categoría con tiempo, el corredor incluido: el «de 18» del puesto. | `docs/tiempo-perdido.md` |
| `course_runners` | integer |  | Corredores con tiempo válido en el recorrido (todas las categorías que lo comparten): con ellos se calcula la referencia de cada tramo. | `docs/tiempo-perdido.md` |
| `lost_time_s` | number | s | Tiempo perdido en los tramos con error (errores revisados). | `docs/tiempo-perdido.md` |
| `error_count` | integer |  | Tramos con error (errores revisados: manda la etiqueta del corredor). | `docs/tiempo-perdido.md` |
| `time_without_errors_s` | number | s | Tiempo total menos el tiempo perdido en los errores. | `docs/tiempo-perdido.md` |
| `place_without_errors` | integer |  | Puesto que habría tenido en su categoría con su tiempo sin errores. | `docs/tiempo-perdido.md` |
| `usual_performance` | number | ratio | Rendimiento habitual (IR medio de la carrera): mediana de los IR de los tramos ponderada por la referencia. 1 = 100 %, igual que la referencia; más es mejor. | `docs/tiempo-perdido.md` |
| `consistency` | number | ratio | Consistencia (P10): desviación típica de los IR de los tramos que cuentan. 1 = 100 puntos de IR; menos es más regular. Vacío con menos de 2 tramos que cuenten. | `docs/tiempo-perdido.md` |
| `days_since_previous_race` | integer | días | Días desde la carrera anterior en la que el corredor tomó la salida, sea cual sea su tipo y formato. Vacío en la primera. | `docs/historico.md` |
| `has_track` | boolean |  | La carrera tiene el track del reloj (FIT) guardado y se ha podido alinear. Sin él, las columnas físicas van vacías. | `docs/metricas.md` |
| `track_duration_s` | number | s | Suma de las duraciones de los tramos con track. | `docs/rendimiento-fisico.md` |
| `track_distance_m` | number | m | Distancia recorrida en los tramos con track. | `docs/rendimiento-fisico.md` |
| `ascent_m` | number | m | Subida acumulada con la altitud suavizada. Vacío sin altitud. | `docs/rendimiento-fisico.md` |
| `descent_m` | number | m | Bajada acumulada (positiva). Vacío sin altitud. | `docs/rendimiento-fisico.md` |
| `moving_s` | number | s | Tiempo en movimiento. | `docs/rendimiento-fisico.md` |
| `stopped_s` | number | s | Tiempo parado, sin los primeros segundos de cada tramo. | `docs/rendimiento-fisico.md` |
| `gap_s` | number | s | Tiempo en huecos del track. | `docs/rendimiento-fisico.md` |
| `attack_s` | number | s | Tiempo en las zonas de ataque de las balizas. | `docs/rendimiento-fisico.md` |
| `speed_kind` | string |  | Velocidad con la que se comparan el traslado y sus mitades: flat (equivalente en llano) si el track tiene altitud, moving (real) si no. | `docs/rendimiento-fisico.md` |
| `transit_s` | number | s | Tiempo de traslado: en movimiento y fuera de las zonas de ataque. | `docs/rendimiento-fisico.md` |
| `transit_distance_m` | number | m | Distancia de traslado. | `docs/rendimiento-fisico.md` |
| `transit_speed_mps` | number | m/s | Velocidad real en el traslado. | `docs/rendimiento-fisico.md` |
| `transit_flat_speed_mps` | number | m/s | Velocidad equivalente en llano en el traslado. Vacío sin altitud. | `docs/rendimiento-fisico.md` |
| `transit_heart_rate_bpm` | number | ppm | Pulso medio en el traslado. Vacío con pulso en menos de la mitad del tiempo. | `docs/rendimiento-fisico.md` |
| `efficiency_m_per_beat` | number | m/latido | Eficiencia: metros por latido en el traslado, con la velocidad de speed_kind. Sube si va más rápido con el mismo pulso. | `docs/rendimiento-fisico.md` |
| `cadence_spm` | number | pasos/min | Cadencia media en el traslado. | `docs/rendimiento-fisico.md` |
| `stride_m` | number | m | Zancada: metros reales por paso en el traslado. | `docs/rendimiento-fisico.md` |
| `heart_rate_bpm` | number | ppm | Pulso medio de la salida a la meta, parado incluido. | `docs/rendimiento-fisico.md` |
| `heart_rate_max_bpm` | number | ppm | Pulso máximo con una media móvil de ±5 s. | `docs/rendimiento-fisico.md` |
| `decoupling_pct` | number | % | Desacople: caída de la eficiencia de la primera mitad del traslado a la segunda. Positivo: la segunda mitad cuesta más pulso por metro. Vacío si alguna mitad no llega a 2 min. | `docs/rendimiento-fisico.md` |
| `speed_change_pct` | number | % | Cambio de la velocidad (speed_kind) de la primera mitad a la segunda. | `docs/rendimiento-fisico.md` |
| `heart_rate_change_bpm` | number | ppm | Pulso de la segunda mitad menos el de la primera. | `docs/rendimiento-fisico.md` |
| `stride_change_pct` | number | % | Cambio de la zancada de la primera mitad a la segunda. | `docs/rendimiento-fisico.md` |
| `vam_m_per_h` | number | m/h | VAM: metros subidos por hora en los trozos de traslado con un 5 % de pendiente o más. Vacío con menos de 1 min de subida. | `docs/rendimiento-fisico.md` |
| `climb_s` | number | s | Tiempo de traslado en esos trozos de subida. | `docs/rendimiento-fisico.md` |
| `heart_rate_load` | number |  | Carga: minutos en cada zona de pulso de Ajustes por su número (1, 2, 3…), sumados. Solo se puede comparar entre carreras con las mismas zonas. Vacío sin zonas definidas o sin pulso. | `docs/rendimiento-fisico.md` |


### `legs.csv`

Una fila por cada tramo del recorrido, también los que no cuentan en los patrones
(`excluded_from_patterns`: el último, los de referencia corta, los sin split). Para modelar, lo
normal es quedarse con `excluded_from_patterns == false`, que son los que usan el histórico y todos
los análisis de la app.

- `is_error` es el **error revisado**: el que cuenta en toda la app (`docs/tiempo-perdido.md`,
  "Errores revisados"). Si el corredor ha etiquetado el tramo, manda su etiqueta; si no, el
  cálculo. `calculated_error` es lo que propuso el cálculo con los umbrales de Ajustes, diga lo que
  diga la etiqueta. Para entrenar «¿hay un error?», `calculated_error` es una entrada natural y
  `is_error` el objetivo, sobre todo en los tramos que el corredor ha revisado (`tag_confirmation`
  no vacío).
- `tag_confirmation`, `error_type`, `error_subtype` y `effort` salen de la etiqueta del corredor
  (`docs/taxonomia.md`). Las causas, la parte del tramo, la pérdida percibida y la nota no se
  exportan. La nota es texto libre y puede llevar nombres.
- `previous_leg_error` es el error revisado del tramo anterior **del recorrido**. No es la medida
  de «después de fallar» del histórico (`docs/historico.md`, "Después de fallar (P8)"), que salta
  los tramos que no cuentan.
- Las columnas FIT (`distance_m`…`attack_s`) son las métricas de tramo de `docs/metricas.md`, y
  van vacías en las carreras sin track y en los tramos sin sub-track.

| Columna | Tipo | Unidad | Qué es | Definición |
| --- | --- | --- | --- | --- |
| `race_id` | integer |  | Carrera del tramo: el race_id de races.csv. | `docs/exportar-datos.md` |
| `leg` | integer |  | Número del tramo, desde 1 (el que sale del triángulo de salida). | `docs/tiempo-perdido.md` |
| `legs` | integer |  | Tramos del recorrido, contando el último. | `docs/tiempo-perdido.md` |
| `position` | number | ratio | Posición relativa en la carrera: leg / legs. El último tramo vale 1. | `docs/historico.md` |
| `third` | integer |  | Tercio de la carrera por número de tramos: 1, 2 o 3. | `docs/historico.md` |
| `is_last` | boolean |  | Es el último tramo (de la última baliza a meta). | `docs/tiempo-perdido.md` |
| `excluded_from_patterns` | boolean |  | No cuenta en los análisis de patrones: el último tramo, los de referencia corta (menos de 20 s) y los sin split. | `docs/tiempo-perdido.md` |
| `split_s` | number | s | Tiempo del tramo. Vacío si falta alguna picada. | `docs/tiempo-perdido.md` |
| `reference_s` | number | s | Referencia del tramo: media del 25 % más rápido de los splits válidos del recorrido. Es un agregado, no el resultado de nadie. | `docs/tiempo-perdido.md` |
| `reference_count` | integer |  | Corredores en los que se basa la referencia. | `docs/tiempo-perdido.md` |
| `performance_index` | number | ratio | IR del tramo: referencia / split. 1 = igual que la referencia; más es mejor. | `docs/tiempo-perdido.md` |
| `expected_s` | number | s | Tiempo esperado: referencia / rendimiento habitual de la carrera. | `docs/tiempo-perdido.md` |
| `loss_s` | number | s | Pérdida: split menos esperado. Puede ser negativa. | `docs/tiempo-perdido.md` |
| `loss_pct` | number | % | Pérdida entre el tiempo esperado, en tanto por ciento. | `docs/tiempo-perdido.md` |
| `is_error` | boolean |  | Error revisado: lo que dice la etiqueta del corredor y, sin ella, el cálculo. Es el que cuenta en toda la app. | `docs/tiempo-perdido.md` |
| `calculated_error` | boolean |  | Error propuesto por el cálculo (los umbrales de Ajustes), diga lo que diga la etiqueta. | `docs/tiempo-perdido.md` |
| `tag_confirmation` | string |  | Respuesta del corredor en la etiqueta: error (sí), no_error (no) o physical (se perdió tiempo por el físico). Vacío sin respuesta. | `docs/taxonomia.md` |
| `error_type` | string |  | Tipo de error de la etiqueta (clave de la taxonomía). Vacío sin tipo. | `docs/taxonomia.md` |
| `error_subtype` | string |  | Subtipo de error de la etiqueta (clave de la taxonomía). | `docs/taxonomia.md` |
| `effort` | integer | 1-10 | Esfuerzo percibido de la etiqueta, de 1 a 10. | `docs/taxonomia.md` |
| `distance_m` | number | m | FIT: distancia recorrida en el tramo. Vacío sin track del tramo. | `docs/metricas.md` |
| `straight_m` | number | m | FIT: línea recta entre las dos balizas. | `docs/metricas.md` |
| `distance_ratio` | number | ratio | FIT: distancia / línea recta. Vacío si la línea recta mide menos de 1 m. | `docs/metricas.md` |
| `moving_speed_mps` | number | m/s | FIT: velocidad en movimiento. | `docs/metricas.md` |
| `flat_speed_mps` | number | m/s | FIT: velocidad equivalente en llano. Vacío sin altitud. | `docs/metricas.md` |
| `moving_s` | number | s | FIT: tiempo en movimiento. | `docs/metricas.md` |
| `stopped_s` | number | s | FIT: tiempo parado. | `docs/metricas.md` |
| `ascent_m` | number | m | FIT: subida acumulada. Vacío sin altitud. | `docs/metricas.md` |
| `descent_m` | number | m | FIT: bajada acumulada (positiva). Vacío sin altitud. | `docs/metricas.md` |
| `heart_rate_bpm` | number | ppm | FIT: pulso medio. Vacío sin pulso. | `docs/metricas.md` |
| `cadence_spm` | number | pasos/min | FIT: cadencia media. | `docs/metricas.md` |
| `attack_s` | number | s | FIT: tiempo de ataque, de la entrada en la zona de la baliza a la picada. | `docs/metricas.md` |
| `days_since_previous_race` | integer | días | Contexto: días desde la carrera anterior (el mismo valor que en races.csv). | `docs/historico.md` |
| `previous_leg_error` | boolean |  | Contexto: el tramo anterior fue error revisado. Vacío en el primer tramo o si el anterior no tiene pérdida. | `docs/historico.md` |

### `samples.csv`

La serie de la carrera de `docs/rendimiento-fisico.md` ("Serie de la carrera"), tal cual la da el
núcleo: la misma que dibuja la pestaña Físico, pero con un instante **cada segundo** (`SAMPLES_STEP_S`;
la pestaña usa 5 s). Un segundo es lo que piden los modelos de secuencias y sale a unas 3.600 filas
(unos 200 KB) por hora de carrera; con 100 carreras de una hora, unos 20 MB. Más fino no tiene sentido:
el FIT del reloj es, como mucho, a 1 Hz.

- Una carrera entra si tiene FIT guardado y se ha podido alinear con las picadas (las que en
  `races.csv` tienen `has_track` a `true`), con una fila por instante, de la salida a la meta. El
  último instante es la meta y puede quedar a menos de un segundo del anterior. Las demás carreras
  no tienen filas. El fichero lleva siempre la cabecera, aunque no haya ninguna fila.
- Los vacíos son los de la serie: sin altitud, pulso o cadencia, esas columnas van vacías, y en un
  **hueco** del track (`state` = `gap`) solo hay `t_s`, `distance_m` y `state`. El estado es el de
  `race_physical` y la tabla de abajo cuenta el resto.
- `leg` es el `leg` de `legs.csv`, así que se une por `race_id` y `leg`.
- **Redondeo**, para que el fichero no pese de más: `t_s` a milisegundos, `distance_m` y
  `altitude_m` a centímetros, las velocidades y `stride_m` a milímetros, `heart_rate_bpm` y
  `cadence_spm` a décimas, `lat` y `lon` a 6 decimales (unos 0,1 m).
- **Coordenadas**: `lat` y `lon` (grados) solo salen si se marca **Incluir coordenadas**. Son la
  posición del track en el instante, interpolada en línea recta entre los dos puntos del FIT que
  lo rodean (`tramos_core::physical_series::series_positions`, igual que la segmentación con las
  picadas), y van vacías en los huecos. **Dicen por dónde ha corrido el usuario**, y con la fecha de
  `races.csv`, dónde estaba: es el dato más delicado de la exportación (ver "Privacidad").
- El orden de las filas es el de `races.csv` (de la carrera más antigua a la más reciente) y, dentro
  de cada carrera, el del tiempo.

| `race_id` | integer |  | Carrera del instante: el race_id de races.csv. | `docs/exportar-datos.md` |
| `t_s` | number | s | Segundos desde la salida (la picada de salida), cada segundo; el último instante es la meta y puede quedar a menos de un segundo del anterior. | `docs/rendimiento-fisico.md` |
| `distance_m` | number | m | Metros recorridos desde la salida. En un hueco del track sigue contando la línea recta. | `docs/rendimiento-fisico.md` |
| `altitude_m` | number | m | Altitud suavizada. Vacío sin altitud y en los huecos. | `docs/rendimiento-fisico.md` |
| `speed_mps` | number | m/s | Velocidad real: la distancia recorrida en ±5 s. Vacío en los huecos. | `docs/rendimiento-fisico.md` |
| `flat_speed_mps` | number | m/s | Velocidad equivalente en llano, según la pendiente de la zona. Vacío sin altitud o sin pendiente medible. | `docs/rendimiento-fisico.md` |
| `heart_rate_bpm` | number | ppm | Pulso: media de las muestras a ±5 s. Vacío sin pulso. | `docs/rendimiento-fisico.md` |
| `cadence_spm` | number | pasos/min | Cadencia: media de las muestras a ±5 s. Vacío sin cadencia. | `docs/rendimiento-fisico.md` |
| `stride_m` | number | m | Zancada: velocidad real entre cadencia. Vacío sin cadencia y parado. | `docs/rendimiento-fisico.md` |
| `leg` | integer |  | Tramo del instante: el leg de legs.csv. Vacío en los huecos. | `docs/rendimiento-fisico.md` |
| `state` | string |  | Qué hace el corredor: traslado (transit), zona de ataque de la baliza (attack), parado (stopped) o sin datos (gap). | `docs/rendimiento-fisico.md` |
| `lat` | number | grados | Latitud del instante, interpolada en el track. Solo con las coordenadas. Vacío en los huecos. | `docs/rendimiento-fisico.md` |
| `lon` | number | grados | Longitud del instante, interpolada en el track. Solo con las coordenadas. Vacío en los huecos. | `docs/rendimiento-fisico.md` |

## Ejemplo con pandas

```python
import pandas as pd

races = pd.read_csv("races.csv", parse_dates=["date"])
legs = pd.read_csv("legs.csv")
samples = pd.read_csv("samples.csv")  # solo si se exportó la serie

# Cada tramo con los datos de su carrera.
data = legs.merge(races, on="race_id", suffixes=("", "_race"))

# Los tramos que cuentan, con su error revisado.
counted = data[~data["excluded_from_patterns"]]
print(counted.groupby("format")["is_error"].mean())

# Eficiencia y desacople de las carreras con FIT, por fecha.
print(races.loc[races["has_track"], ["date", "efficiency_m_per_beat", "decoupling_pct"]])

# La serie de una carrera como secuencia, sin los huecos.
one = samples[(samples["race_id"] == 7) & (samples["state"] != "gap")]
print(one[["t_s", "speed_mps", "heart_rate_bpm"]].describe())
```

Las columnas `true`/`false` se leen como booleanas; las enteras con vacíos (`place`, `effort`…)
salen como `float` con `NaN`.

## Privacidad

El objetivo es que el fichero se pueda llevar a un cuaderno, a una herramienta de ML o a un
servicio externo sin arrastrar datos de otras personas (`docs/datos-y-privacidad.md`).

- **Sin nombres de nadie**: ni el del usuario, ni los de otros corredores, ni clubes, ni el de la
  prueba, ni la categoría. La nota de las etiquetas no se exporta.
- **Sin resultados de los demás corredores**: ni sus tiempos, ni sus puestos de tramo. Solo salen
  agregados que la app ya usa: la referencia del tramo (`reference_s`, la media del 25 % más
  rápido) con cuántos corredores la forman, los corredores del recorrido (`course_runners`) y los
  clasificados de la categoría (`classified`).
- **Sin coordenadas por defecto**: no hay posiciones GPS ni el track. Hay cifras que salen de él
  (distancias, velocidades, pulso, cadencia, subida y bajada), que son datos personales de por sí:
  dicen cómo corre una persona, y la fecha y la distancia de una carrera pueden identificarla para
  quien conozca la prueba. La serie de `samples.csv` (si se marca) lleva esas mismas cifras
  instante a instante, y es más identificable que los resúmenes.
- **Coordenadas, solo si se piden**: con **Incluir coordenadas**, `samples.csv` lleva `lat` y `lon`
  de cada segundo. Es la ruta exacta, y con la fecha de `races.csv` dice dónde estaba la persona;
  un terreno de orientación puede ser un sitio concreto y conocido. La pantalla de exportar y el
  `LEEME.md` lo avisan. Quien lo comparta o lo suba a un servicio externo debe hacerlo sabiendo
  eso. Sin esa casilla (y sin la de la serie), ningún fichero lleva coordenadas.
- Los identificadores (`race_id`, `event_id`) son números de la base de datos de la app. No llevan
  datos de nadie.

El fichero se escribe en la carpeta que elige el usuario y la app no lo envía a ningún sitio. Si
la carpeta ya tiene un `races.csv`, un `legs.csv`, un `schema.json` o un `LEEME.md` anteriores,
los sustituye; los demás ficheros no se tocan.

Con una excepción: si esta vez **no** se pide la serie y la carpeta ya tiene un `samples.csv`, la
app lo **borra** y lo dice en la pantalla. Es un fichero con el nombre del formato, de una
exportación anterior, y dejarlo junto al `schema.json` nuevo (que ya no lo cuenta) sería dejar
datos que parecen de esta exportación y no lo son; si llevaba coordenadas, sería además dejarlas
donde el usuario ya había decidido no sacarlas. Quien quiera conservar una exportación con la serie
debe elegir otra carpeta.

## Pruebas

- `crates/tramos-core/src/export.rs`: filas calculadas a mano (con y sin track, con y sin
  etiqueta, las de la serie con y sin coordenadas), escapado del CSV, el esquema, el `LEEME.md` y
  que este documento nombra todas las columnas, también las de `samples.csv`.
- `crates/tramos-core/src/physical_series.rs`: las posiciones de la serie, calculadas a mano.
- `app/src-tauri/src/export.rs`: guarda una carrera con el fixture anonimizado de Baltanás y su
  FIT sintético, exporta a una carpeta temporal y comprueba las columnas, las filas y que no sale
  ningún nombre; con y sin la serie, con y sin coordenadas (sin la casilla no sale ninguna) y el
  borrado de un `samples.csv` anterior.
