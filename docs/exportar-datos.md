# Exportar los datos para análisis

Para analizar los datos fuera de la app (pandas, R, modelos de ML) la copia de seguridad no sirve:
es la base SQLite entera, con los ficheros originales. **Exportar datos para análisis** (#217, en
Ajustes, junto a la copia de seguridad) escribe en una carpeta que elige el usuario unas tablas
planas, limpias y documentadas:

| Fichero | Qué es |
| --- | --- |
| `races.csv` | Una fila por carrera propia. |
| `legs.csv` | Una fila por tramo de esas carreras: el formato natural para ML. |
| `schema.json` | Cada columna con su tipo, unidad, descripción y el documento de `docs/` donde se define. |
| `LEEME.md` | Lo mismo para una persona: cómo cargarlo y las columnas. |

La serie del track por segundo (`samples.csv`, para modelos de secuencias) no está en esta
versión del formato (#219).

Todo sale del núcleo (`tramos_core::export` arma las filas y escribe el CSV, el esquema y el
`LEEME.md`) con lo que la app ya calcula: el informe de cada carrera (`runner_report`, con las
etiquetas del corredor), las métricas de tramo y el rendimiento físico. La app lee la base y
llama al núcleo (`app/src-tauri/src/export.rs`, comando `export_analysis`); no recalcula nada, así
que los números son los que se ven en pantalla.

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

## Formato

- `schema.json` lleva la versión del formato, `format_version`: 1 hoy. Sube cuando se quita o cambia el significado de
  una columna; añadir columnas al final no la cambia, así que conviene leer las columnas por
  nombre y no por posición.
- **CSV en UTF-8**, sin BOM, separado por comas, con un `\n` al final de cada línea y la
  cabecera en la primera. Los campos con coma, comillas o saltos de línea van entre comillas
  dobles, con las comillas dobladas (RFC 4180).
- **Punto decimal** y sin separador de miles. Los números se escriben con toda su precisión (sin
  redondear): `0.3333333333333333`.
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
    }
  ]
}
```

`type` es `integer`, `number`, `string`, `boolean` o `date`. `unit` y `allowed_values` solo salen
cuando hay.

## Columnas

La lista es la de `RACE_COLUMNS` y `LEG_COLUMNS` en `crates/tramos-core/src/export.rs`, y un test
comprueba que este documento nombra todas. La última columna dice dónde se define cada número.

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


## Ejemplo con pandas

```python
import pandas as pd

races = pd.read_csv("races.csv", parse_dates=["date"])
legs = pd.read_csv("legs.csv")

# Cada tramo con los datos de su carrera.
data = legs.merge(races, on="race_id", suffixes=("", "_race"))

# Los tramos que cuentan, con su error revisado.
counted = data[~data["excluded_from_patterns"]]
print(counted.groupby("format")["is_error"].mean())

# Eficiencia y desacople de las carreras con FIT, por fecha.
print(races.loc[races["has_track"], ["date", "efficiency_m_per_beat", "decoupling_pct"]])
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
- **Sin coordenadas**: no hay posiciones GPS ni el track. Hay cifras que salen de él (distancias,
  velocidades, pulso, cadencia, subida y bajada), que son datos personales de por sí: dicen cómo
  corre una persona, y la fecha y la distancia de una carrera pueden identificarla para quien
  conozca la prueba.
- Los identificadores (`race_id`, `event_id`) son números de la base de datos de la app. No llevan
  datos de nadie.

El fichero se escribe en la carpeta que elige el usuario y la app no lo envía a ningún sitio. Si
la carpeta ya tiene un `races.csv`, un `legs.csv`, un `schema.json` o un `LEEME.md` anteriores,
los sustituye; los demás ficheros no se tocan.

## Pruebas

- `crates/tramos-core/src/export.rs`: filas calculadas a mano (con y sin track, con y sin
  etiqueta), escapado del CSV, el esquema, el `LEEME.md` y que este documento nombra todas las
  columnas.
- `app/src-tauri/src/export.rs`: guarda una carrera con el fixture anonimizado de Baltanás y su
  FIT sintético, exporta a una carpeta temporal y comprueba las columnas, las filas y que no sale
  ningún nombre.
