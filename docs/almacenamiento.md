# Almacenamiento local

Cada usuario tiene una base de datos SQLite en local (`crates/tramos-store`). Guarda el modelo
de `docs/modelo.md`, los resultados del análisis, las etiquetas y los ajustes. SQLite va
compilado dentro de la app (`rusqlite` con la feature `bundled`), así que no depende del sistema.

## API

| Función | Qué hace |
| --- | --- |
| `Store::open(ruta)` / `Store::open_in_memory()` | Abre o crea la base, activa las claves foráneas y aplica las migraciones pendientes. |
| `save_source_file(tipo, ruta, &[u8]) -> SourceFileId` | Guarda un fichero original con su contenido y su SHA-256. Si ya hay uno con la misma huella, devuelve su id sin duplicarlo. |
| `load_source_file(SourceFileId) -> SourceFile` | Tipo, ruta de origen, SHA-256, contenido e instante de importación. |
| `find_source_file(&[u8]) -> Option<SourceFileId>` | Fichero ya guardado con el mismo contenido (mismo SHA-256), sin guardar nada. |
| `save_event(&Event, Option<SourceFileId>) -> SavedEvent` | Guarda una carrera completa en una transacción, enlazada a su .spl si se indica. Devuelve su `EventId` y los `ResultId` por categoría y en orden. |
| `load_event(EventId) -> Event` | Carga la carrera exactamente como se guardó. |
| `result_ids(EventId)` | Los `ResultId` de una carrera ya guardada, como en `SavedEvent`. |
| `result_ref(ResultId) -> (EventId, ResultRef)` | Carrera de un resultado y su posición en ella (índices de categoría y de resultado), para buscarlo en el `Event` que da `load_event`. |
| `event_source_file(EventId)` | El fichero original enlazado a la carrera, si lo tiene. |
| `event_by_source_file(SourceFileId) -> Option<EventId>` | La primera carrera guardada (menor id) enlazada a ese fichero. Sirve para no duplicar una carrera al reimportar su .spl (`docs/app.md`). |
| `event_format(EventId)` / `set_event_format(EventId, Option<RaceFormat>)` | Formato de la carrera (`sprint`, `middle`, `long`); `None` si no se ha fijado o para borrarlo. Carrera inexistente: `EventNotFound`. |
| `event_start(EventId) -> Option<DateTime<Utc>>` | Inicio de la carrera para ordenar las del mismo día: la primera picada con hora de cualquiera de sus resultados (ver [Personas](#personas)). |
| `save_track(ResultId, &Track, Option<SourceFileId>)` / `load_track(ResultId)` | Track del reloj de un resultado, enlazado a su FIT si se indica. Guardar otra vez sustituye el anterior. |
| `track_source_file(ResultId)` | El fichero original enlazado al track, si lo tiene. |
| `setting(clave)` / `set_setting(clave, valor)` | Ajustes clave-valor. |
| `create_person(nombre, notas) -> PersonId` | Crea una persona (ver [Personas](#personas)). El nombre no puede estar vacío (`EmptyPersonName`). |
| `people() -> Vec<Person>` | Todas las personas, en orden de creación: id, nombre visible, notas e instante de creación. |
| `update_person(PersonId, nombre, notas)` | Sustituye el nombre visible y las notas (`None` las borra). Mismas reglas del nombre que al crear; persona inexistente: `PersonNotFound`. No cambia el instante de creación ni los vínculos. |
| `delete_person(PersonId)` | Borra una persona y desvincula sus resultados; no borra ninguno. |
| `link_result(ResultId, PersonId)` / `unlink_result(ResultId)` | Vincula un resultado con una persona o lo desvincula. Un resultado ya vinculado a otra persona da `ResultAlreadyLinked`. |
| `result_person(ResultId) -> Option<PersonId>` | Persona a la que está vinculado un resultado, si lo está. |
| `tags(ResultId) -> Vec<StoredTag>` | Etiquetas de los tramos de un resultado, por tramo: el contenido (`tramos_core::taxonomy::LegTag`, con las causas ordenadas por clave), la versión de la taxonomía y los instantes de creación y última modificación. Resultado inexistente: `ResultNotFound`. |
| `save_tag(ResultId, tramo, versión, &LegTag) -> Option<StoredTag>` | Guarda la etiqueta de un tramo (desde 1; 0 es `InvalidLegIndex`), sustituyendo la anterior con sus causas pero conservando su instante de creación. Una etiqueta vacía borra la del tramo y devuelve `None`. No comprueba las claves contra la taxonomía ni que el tramo exista: eso lo hace quien llama (`docs/taxonomia.md`). |
| `delete_tag(ResultId, tramo)` | Borra la etiqueta de un tramo; si no la tenía no hace nada. |
| `person_results(PersonId) -> Vec<PersonResult>` | Resultados de una persona por fecha de carrera: id del resultado y de la carrera, fecha, nombre, inicio y formato de la carrera (si los hay), categoría, estado, puesto y si el resultado tiene track. |

El análisis guardado (`legs`) aún no tiene API: de momento solo existe su tabla.

## Convenciones

- **Instantes**: `INTEGER` con los **milisegundos desde la época Unix en UTC**. Las columnas se
  llaman `*_epoch_ms` para no confundirlas con duraciones. Es compacto (un track tiene miles de
  puntos), se ordena y se resta directamente en SQL y sobra para el .spl y el FIT, que van al
  segundo. Para que la ida y vuelta sea exacta, guardar un instante con precisión por debajo del
  milisegundo (o un segundo intercalar) es un error (`SubMillisecondInstant`) en vez de redondear
  en silencio.
- **Fechas locales** (día de la carrera): `TEXT` `AAAA-MM-DD`.
- **Duraciones**: `REAL` en segundos (`*_s`). Los cocientes (`performance_index`, `loss_ratio`)
  van en fracción: `0.10` es un 10 %.
- **Reales**: SQLite guarda NaN como `NULL`, así que guardar un NaN es un error (`NotANumber`).
- **Orden** de las listas del modelo (categorías, resultados, picadas, balizas, puntos): columna
  `position`, desde 0.
- **Enumerados**: `TEXT` en `snake_case`, como en el JSON del modelo, con `CHECK`.
- **Integridad**: claves foráneas activas (`PRAGMA foreign_keys = ON`). Borrar una carrera borra
  en cascada sus recorridos, categorías, corredores, resultados, picadas, tracks, tramos y
  etiquetas; las personas se quedan, sin esos resultados.

## Tablas

| Tabla | Qué guarda | Columnas clave |
| --- | --- | --- |
| `events` | Carreras. | `name`, `date`, `source_file_id` (el .spl, opcional), `format` (`sprint`, `middle`, `long` u opcional). |
| `courses` | Recorridos: uno por secuencia de balizas distinta dentro de la carrera. Las categorías con el mismo recorrido comparten fila, porque el tiempo perdido se calcula por recorrido. | `event_id`. |
| `course_controls` | Balizas de cada recorrido, sin salida ni meta. | `course_id`, `position`, `code`. |
| `classes` | Categorías. | `event_id`, `position`, `source_id` (id del fichero), `name`, `short_name`, `course_id`. |
| `runners` | Corredores tal y como aparecen en una carrera (uno por resultado). **No hay columna de fecha de nacimiento.** | `event_id`, nombre, apellidos, `club`, `bib`, `si_card`, `sex`. |
| `results` | Resultado de un corredor en una categoría. | `class_id`, `position`, `runner_id`, `status`, `status_code` (solo para `unknown`), `place`, `person_id` (opcional). |
| `people` | Personas: identidad de un corredor entre carreras. Solo lo que escribe el usuario; **sin fecha de nacimiento**. | `display_name`, `notes` (opcional), `created_at_epoch_ms`. |
| `punches` | Picadas en orden, de la salida a la meta. | `result_id`, `position`, `code`, `time_epoch_ms` (`NULL` si no hay hora). |
| `tracks` | Track del reloj: como mucho uno por resultado. | `result_id`, `source_file_id` (el FIT, opcional), `sport` (`Track::sport`, opcional). |
| `track_points` | Puntos del track. | `track_id`, `position`, `time_epoch_ms`, `lat`, `lon`, `altitude_m`, `heart_rate_bpm`, `cadence_spm`, `distance_m`. |
| `legs` | Tramos de un resultado con lo que calcula el análisis (`docs/tiempo-perdido.md`). | `result_id`, `leg_index` (desde 1), `from_code`, `to_code`, `split_s`, `reference_s`, `performance_index`, `expected_s`, `loss_s`, `loss_ratio`, `is_error`, `algorithm_version`. |
| `tags` | Etiqueta del corredor sobre un tramo (`docs/taxonomia.md`). | `result_id`, `leg_index`, `taxonomy_version`; nivel 1 `confirmation` (`error`, `no_error`, `physical`); nivel 2 `error_type`, `error_subtype`; nivel 3 `leg_part` (`start`, `middle`, `attack`), `perceived_loss_s`, `effort` (1–10), `note`; `created_at_epoch_ms`, `updated_at_epoch_ms`. |
| `tag_causes` | Causas percibidas de una etiqueta (varias). | `tag_id`, `cause`. |
| `source_files` | Ficheros originales importados, con su contenido. **Nunca sale de la base local.** | `kind` (`spl`, `fit`), `path` (informativa), `sha256` (única), `size_bytes`, `content`, `imported_at_epoch_ms`. |
| `settings` | Ajustes clave-valor (umbrales, preferencias…). | `key`, `value`. |

Notas:

- `runners` no tiene id de origen: el .spl no trae ninguno (`docs/formato-spl.md`). Hasta la
  versión 3 había una columna `source_id` con el campo `0x80`, que es la longitud del registro;
  la migración 4 la quita. Los resultados se identifican por `results.id` o por
  `(class_id, position)`.
- `legs`: los resultados del análisis son todos opcionales (`NULL`), porque un tramo sin picada en
  un extremo no tiene split ni pérdida. `algorithm_version` es obligatoria: cuando cambia, los
  tramos de la carrera se recalculan enteros. La referencia está repetida en cada corredor del
  recorrido; a cambio, las consultas de patrones no necesitan uniones.
- `tags` apunta a `(result_id, leg_index)` y no a `legs.id`, para que las etiquetas sobrevivan a un
  recálculo de los tramos. Hay como mucho una etiqueta por tramo y ningún nivel es obligatorio.
  Tipos, subtipos y causas son claves del fichero de taxonomía, que vive fuera del código; por eso
  no tienen `CHECK`, y cada etiqueta guarda la `taxonomy_version` con la que se escribió por
  última vez (las claves que lleva son de esa versión).

## Migraciones

- Scripts SQL en `crates/tramos-store/migrations/NNNN_descripcion.sql`, embebidos en el binario
  con `include_str!` y listados en orden en `migrations.rs`.
- La versión aplicada se guarda en `PRAGMA user_version`: tras la migración *n* (desde 1) vale *n*.
  Una base nueva vale 0.
- Al abrir se aplican las pendientes en **una sola transacción**: si alguna falla no se aplica
  ninguna. Con el esquema al día no se hace nada.
- Si la base tiene una versión mayor que la que conoce la app (la abrió una versión más nueva), se
  rechaza con `SchemaTooNew` en vez de tocarla.
- Una migración publicada no se edita nunca: los cambios van en un script nuevo al final.
  Los scripts no abren ni cierran transacciones.

| Versión | Script | Cambio |
| --- | --- | --- |
| 1 | `0001_initial.sql` | Esquema inicial. |
| 2 | `0002_people.sql` | Tabla `people` y columna `results.person_id` (nula en los resultados que ya había). |
| 3 | `0003_track_sport.sql` | Columna `tracks.sport` (nula en los tracks que ya había). |
| 4 | `0004_drop_runner_source_id.sql` | Quita `runners.source_id` (el `0x80` del .spl, que no es un id). El resto de cada corredor se conserva. |
| 5 | `0005_event_format.sql` | Columna `events.format` (nula en las carreras que ya había). |

## Personas

`runners` es por carrera: una fila por resultado, tal y como aparece en cada .spl. Para buscar
patrones carrera tras carrera, el usuario agrupa en una **persona** (`people`) los resultados que
sabe que son de la misma persona (él mismo y, más adelante, sus compañeros).

- **Vínculo**: columna `results.person_id`, nula si el resultado no es de nadie conocido. Así un
  resultado pertenece **como mucho a una persona**, sin tabla intermedia. Va en `results` y no en
  `runners` porque son uno a uno y todo lo demás (tracks, tramos, etiquetas) cuelga del resultado.
- **Borrar una persona** desvincula sus resultados (`ON DELETE SET NULL`); no borra ninguno.
- **Vincular un resultado ya vinculado**: con la misma persona no hace nada; con otra es un error
  (`ResultAlreadyLinked`) y el vínculo no cambia. Reasignar es explícito: `unlink_result` y luego
  `link_result`. Así un vínculo hecho a mano no se pisa sin querer (por ejemplo, desde una
  asignación automática futura). `unlink_result` sobre un resultado sin persona no hace nada.
- **Orden de `person_results`**: por fecha de la carrera (`events.date`); con la misma fecha, por
  inicio de la carrera (`event_start`, también en `PersonResult::event_start`); luego por orden
  de guardado y, dentro de una carrera, por el orden de categorías y resultados. Las carreras sin
  inicio (ninguna picada con hora) van detrás de las demás de su día.
- **Inicio de una carrera**: la primera picada con hora de cualquiera de sus resultados, que en la
  práctica es la primera salida. Se calcula en la consulta, sin columna nueva. No se usa el
  inicio del FIT porque solo lo tienen algunos resultados, y una carrera ordenaría distinto según
  quién lo haya subido. Tampoco se usan las fechas de la cabecera del .spl (`0x22`/`0x24`): en
  los ficheros conocidos son de la tarde, después de la carrera, así que parecen de creación o
  subida del fichero (`docs/formato-spl.md`). Es el inicio de la carrera, no el del resultado:
  dos resultados de la misma carrera comparten valor.
- Nada impide vincular a una persona dos resultados de la misma carrera.
- **Qué guarda una persona**: el nombre visible y unas notas, ambos escritos por el usuario, y el
  instante de creación. No se rellena con datos del .spl y no tiene fecha de nacimiento
  (`docs/datos-y-privacidad.md`). El nombre se guarda tal cual; solo se rechaza vacío o en blanco.
- Decidir automáticamente quién es quién queda fuera de esta API: el núcleo propone qué
  resultado de una carrera es del usuario (`tramos_core::identify`, `docs/identificacion.md`),
  pero aquí el vínculo lo decide siempre quien llama.

## Ficheros originales

Se guarda **el contenido completo** (`content`, `BLOB`), junto con el tipo, la ruta de origen
(solo informativa), la huella SHA-256, el tamaño y el instante de importación:

- Así se puede volver a leer el fichero si cambia un importador, aunque el usuario haya borrado o
  movido el original del disco.
- `sha256` es `UNIQUE`: importar dos veces el mismo fichero (aunque venga de otra ruta) no
  duplica el contenido. `save_source_file` calcula la huella y, si ya existe, devuelve el id del
  fichero guardado, con la ruta y el tipo de la primera importación. Un `CHECK` asegura que
  `size_bytes = length(content)`.
- Las carreras y los tracks enlazan su fichero con `events.source_file_id` y
  `tracks.source_file_id`.

**Privacidad.** Aunque el modelo descarta la fecha de nacimiento al importar, el contenido del
.spl guardado la incluye, junto con los nombres de todos los corredores de la prueba. El FIT lleva
GPS y pulso. Por eso `source_files` **nunca sale de la base local**: no va en los paquetes para la
entrenadora ni a proveedores externos (`docs/datos-y-privacidad.md`). Lo que se comparta se saca
de las tablas del modelo, no de los ficheros originales.
