# App de escritorio

`app/` es la app Tauri 2 + React. La interfaz solo llama a comandos de `app/src-tauri/src/lib.rs`,
que a su vez llaman al núcleo (`tramos-core`) y al almacenamiento (`tramos-store`): no calcula
nada por su cuenta.

## Base de datos

Una base SQLite por usuario (`docs/almacenamiento.md`), `tramos.sqlite`, en el directorio de
datos de la app que da Tauri para el identificador `io.github.mendana.tramos`:

| Sistema | Ruta |
| --- | --- |
| Linux | `~/.local/share/io.github.mendana.tramos/` |
| Windows | `%APPDATA%\io.github.mendana.tramos\` |
| macOS | `~/Library/Application Support/io.github.mendana.tramos/` |

Se abre al arrancar (aplicando las migraciones pendientes) y la comparten todos los comandos.

## Comandos

| Comando | Qué hace |
| --- | --- |
| `core_version` | Versión del núcleo. |
| `get_settings` / `save_settings(settings)` | Ajustes del usuario (abajo). Guardar valida todos y, si alguno no vale, no guarda ninguno. |
| `check_zones(zones)` | Unas zonas del mapa mientras se editan (#96): por qué no se pueden guardar o, si se puede, los avisos sobre sus colores, en español. |
| `hidden_panels` / `set_hidden_panels(ids)` | Paneles de análisis ocultos (#130), por identificador. Se leen y guardan en la base propia también mientras se ve a un atleta, así que valen para todos. |
| `role_chosen` / `choose_role(role)` | Si ya se ha dicho cómo se usa la app y decirlo sin tocar los demás ajustes: `runner` (corre), `coach` (entrena) o `both` (las dos cosas; abajo, "Atletas"). |
| `coach_runners` | Si entrena, los atletas de los que hay paquetes, por nombre visible: identificador, nombre, cuántas carreras y el instante de su paquete más reciente. |
| `view_runner(runnerId)` | Elige el atleta que se ve (`null` = volver a lo propio): vuelca sus paquetes y, a partir de ahí, las vistas de corredor muestran sus carreras en solo lectura. Devuelve lo que no sale en ellas: las carreras compartidas solo con el resumen y los paquetes que no se han podido leer. Elegir un atleta sin entrenar da error. |
| `group_view(filter, group)` | Si entrena, la vista de grupo (P15, `docs/historico.md`): una fila por atleta de los que hay paquetes (o por qué no se ha podido calcular), todos contra todos en las carreras compartidas y los totales de todos juntos (`total`, #120), con el filtro del histórico. Con `group` (un grupo de atletas), solo sus miembros: quien usa la app sale si es miembro. Sin grupo, con `athletes.include_self`, las carreras propias cuentan como un atleta más (la primera fila, `is_self`). Si no entrena, error. |
| `set_include_self(include)` | Guarda «Incluirme» (`athletes.include_self`) de la vista de grupo. |
| `athlete_groups` | Si entrena, sus grupos de atletas (#120: identificador, nombre, descripción, color y `runner_id` de los miembros) y a quién puede meter en ellos: quien usa la app (con su `package_runner_id`, si tiene carreras) y los atletas de los que hay paquetes. |
| `create_athlete_group(group)` / `update_athlete_group(id, group)` | Crea un grupo vacío (devuelve su identificador) o cambia uno: `group` = `{name, description, color}` (`docs/almacenamiento.md`, nombre no vacío y color `#rrggbb`). |
| `delete_athlete_group(id)` | Borra un grupo; sus atletas y sus paquetes siguen ahí. |
| `set_athlete_group_member(id, runnerId, member)` | Mete (`true`) o saca (`false`) a un atleta de un grupo. |
| `compare_athlete_groups(filter, a, b, options)` | Compara dos grupos de atletas (#121, `docs/historico.md`, "Comparar grupos") con el filtro del histórico. `options` = `{overlap, races}`: quien está en los dos cuenta en los dos (`count_in_both`) o en ninguno (`exclude`); entran solo las carreras de los dos grupos (`shared`) o todas (`all`). Devuelve cada lado junto (atletas, cifras, P7, P13 y tipos de error), cuántos están en los dos, cuántas carreras entran y las diferencias A − B. |
| `athlete_group_stats(filter, id)` | Estadísticas de un grupo de atletas (#145, `docs/historico.md`, "Estadísticas de un grupo") con el filtro del histórico: el nombre, la descripción y el color del grupo, sus miembros (quien usa la app primero, si es miembro y tiene carreras; cada uno con sus carreras que cuentan o por qué no cuenta), cuántos miembros ya no tienen carreras y los análisis de Estadísticas con todos juntos (`stats`: por formato y total, P7, P13, P2, P8, P9, P11 y P14; sin consistencia). Si no entrena, error. |
| `viewed_runner` | Lo mismo que `view_runner` del atleta que se está viendo, sin volver a volcarlo; `null` = lo propio. |
| `preview_import(splPath, fitPath, identity)` | Primer paso de importar: lee los ficheros sin guardar nada. |
| `import_race(request)` | Segundo paso: guarda la carrera con lo que ha confirmado el usuario. |
| `import_folder(folderPath)` | Importa todas las carreras de una carpeta, cada una con su FIT, y devuelve el resumen (abajo, "Importar una carpeta"). Es asíncrono: no bloquea la ventana mientras alinea. |
| `list_races` | Carreras del usuario, de la más reciente a la más antigua, con su tiempo perdido, su rendimiento habitual (`usual_performance`) y cuántos tramos con error quedan sin revisar (`unreviewed_count`: sin respuesta Sí, No o Físico en su etiqueta, como el contador de la vista de carrera). |
| `race_detail(resultId)` | Una carrera con la tabla de tramos del resultado y su resumen en frases (`insights`: como mucho tres, `tramos_core::insights::race_insights` sobre el mismo informe; `docs/frases.md`). |
| `set_race_format(resultId, format)` | Cambia el formato de la carrera del resultado (`sprint`, `middle`, `long` o `null` = sin formato). Es de la carrera entera. |
| `race_comparison(resultId)` | Corredores del recorrido del resultado, para compararse con ellos (P4): `course_comparison` del núcleo con los umbrales de los ajustes. |
| `race_breakdown(resultId)` | ¿Lento o desorientado? (P2): `tramos_core::loss_breakdown::race_breakdown` con las métricas del track guardado, alineado y troceado como en `race_map` (con su desfase manual si lo tiene; `docs/tiempo-perdido.md`). `null` sin track o si no se puede alinear ni trocear. |
| `race_offset(resultId)` | Desfase entre el reloj y el cronometraje del resultado (abajo, "Reloj y cronometraje"): el calculado con su confianza y avisos, el error si no se puede alinear, la sugerencia de ±1/2 h y el fijado a mano. `null` sin track. |
| `set_race_offset(resultId, offsetS)` | Fija el desfase a mano (segundos; `null` vuelve al automático) y devuelve lo mismo que `race_offset`. Antes comprueba que con él la carrera cae en el track (`align_with_offset`); si no, da el error y no guarda nada. Sin track, error. |
| `export_race_package(resultId, level, folderPath)` | Exporta el paquete de una carrera del usuario (`docs/paquete.md`) con el nivel `aggregates`, `legs` o `track` a la carpeta, con su nombre de fichero (exportar otra vez la sobrescribe). Solo los resultados vinculados a la persona del usuario. Devuelve la ruta. Sin interfaz: la app exporta sola a la carpeta compartida (abajo). |
| `import_race_package(path)` | Importa el paquete de otro corredor: `created`, `replaced` (sustituye al de ese corredor y carrera), `unchanged` (era idéntico) o `ignored_older` (ya había uno más reciente). Sin interfaz: quien entrena recibe por la carpeta compartida. |
| `race_sharing(resultId)` | Qué se comparte de una carrera del usuario y cómo está en la carpeta compartida (`docs/paquete.md`, "Carpeta compartida"): si se puede compartir (comparte lo suyo, con carpeta y la carrera es suya), lo elegido para ella, lo de por defecto, el nivel con el que está en la carpeta y, si no se ha podido exportar, por qué. Antes la exporta si hace falta. |
| `set_race_sharing(resultId, choice)` | Cambia lo que se comparte de una carrera (`none`, `aggregates`, `legs`, `track`; `null` = lo de por defecto), la exporta o la quita de la carpeta y devuelve lo mismo que `race_sharing`. |
| `share_all` | Exporta todas las carreras del usuario a la carpeta compartida: cuántas se han escrito, cuántas ya estaban igual, cuántas no se comparten y las que han fallado. Si no comparte lo suyo o no hay carpeta, no hace nada. |
| `receive_packages` | Si entrena, importa los paquetes nuevos o cambiados de la carpeta compartida, salvo los suyos: cuántos nuevos, sustituidos y sin cambios, los ficheros que no se han podido leer y cuántos paquetes y atletas hay guardados. Si no entrena, no importa nada. |
| `taxonomy` | La taxonomía de errores con la que se etiqueta (`tramos_core::taxonomy`, `docs/taxonomia.md`). |
| `leg_tags(resultId)` | Etiquetas de los tramos del resultado, por tramo, con la versión de la taxonomía y los instantes de creación y última modificación. |
| `save_leg_tag(resultId, legIndex, tag)` | Guarda la etiqueta de un tramo (desde 1, también el último) y devuelve la guardada; una etiqueta vacía borra la del tramo y devuelve `null`. Antes la normaliza (nota sin espacios en los extremos, causas ordenadas y sin repetir) y comprueba que el tramo existe en el recorrido y que la etiqueta encaja en la taxonomía. |
| `race_map(resultId)` | El mapa del resultado: track por tramos coloreado por ritmo y pulso, balizas y escalas (abajo, "Mapa"). |
| `history(filter)` | Histórico de las carreras del usuario por formato (P6): `tramos_core::history` con los umbrales de los ajustes. `filter` = `{from, to, format}` (fechas `AAAA-MM-DD` incluidas y formato; `null` no filtra). Devuelve además cuántas carreras tiene el usuario sin filtrar, la fecha de la primera y la última, una fila por carrera que pasa el filtro (`races`), la pérdida según duración del tramo (P7, `by_leg_length`: `tramos_core::leg_length`) y la pérdida según desnivel (P13, `by_slope`: `tramos_core::slope` con el umbral por defecto; para cada carrera con track, el track guardado se alinea (con su desfase manual si lo tiene) y se trocea como en `race_map` y sus métricas son las de `tramos_core::metrics::leg_metrics`), y los errores más comunes (P9, `common_errors`: `tramos_core::common_errors` con las etiquetas guardadas de cada carrera); además, el cansancio (P14, `fatigue`: `tramos_core::fatigue` con las métricas del track y las etiquetas de cada carrera), y el resumen en frases (`insights`: como mucho tres, `tramos_core::insights::history_insights` sobre esos mismos análisis y con el mismo filtro; `docs/frases.md`). |

El núcleo de la app decide qué base leen las vistas (#119): los comandos que leen las vistas de
corredor (`list_races`, `race_detail`, `race_comparison`, `race_breakdown`, `race_offset`,
`race_map`, `leg_tags` y `history`) leen las carreras del atleta que se está viendo o, si no se ve
a ninguno, las propias. Mientras se ve a un atleta, los que tocan carreras (`import_race`,
`import_folder`, `set_race_format`, `set_race_offset`, `save_leg_tag`, `race_sharing`,
`set_race_sharing`, `share_all` y `export_race_package`) dan error: «mientras ves a un atleta no
se puede modificar nada», porque sus `result_id` son de otra base y no deben tocar la propia. Los
ajustes del usuario (`save_settings`, los paneles ocultos e «Incluirme») sí se pueden cambiar:
son suyos, se vea a quien se vea. `view_runner(null)` vuelve a lo propio y todo es editable otra
vez.

Los errores llegan a la interfaz como texto en español. La lógica está en
`app/src-tauri/src/import.rs`, `batch.rs`, `races.rs`, `race_map.rs`, `clock_offset.rs`, `history.rs`, `settings.rs`, `tags.rs`, `package.rs`, `sharing.rs` y `coach.rs`, en Rust sin Tauri, y se prueba con los fixtures (`cargo test` en
`app/src-tauri`).

## Importar una carrera

1. **Ficheros**: el usuario arrastra a la ventana el .spl y, si lo tiene, el .fit, o los elige
   con el diálogo del sistema (`tauri-plugin-dialog`). Se distinguen por la extensión.
2. **Identidad**: tarjeta SI y nombre y apellidos, rellenados con los de la última importación.
3. **Revisar** (`preview_import`): lee y valida los dos ficheros (las horas del .spl, en la zona
   horaria de los ajustes), identifica al corredor
   (`docs/identificacion.md`) y sugiere el formato (`docs/modelo.md`, "Formato de carrera").
   - Un resultado: se propone. Si casa por tarjeta pero no por nombre, se pide comprobarlo.
   - Varios: el usuario elige.
   - Ninguno (o sin identidad): se listan todos los resultados con un filtro; con más de 50
     coincidencias se pide acotar.
   - Si ese .spl ya se importó, se avisa de que la carrera no se duplicará.
4. **Importar** (`import_race`), con el resultado y el formato elegidos:
   - Se leen y validan los ficheros otra vez antes de guardar nada: si alguno no vale, no se
     guarda nada.
   - Se guarda el .spl original. Si ya había una carrera enlazada a ese mismo fichero (mismo
     SHA-256), se reutiliza: **reimportar no duplica**. Si no, se guarda la carrera entera.
   - Se fija el formato elegido (si eligió uno).
   - El resultado se vincula con la **persona del usuario** (`docs/almacenamiento.md`,
     Personas). La primera vez se crea con el nombre que escribió (o «Yo» si lo dejó vacío), nunca
     con datos del .spl. Si el resultado ya estaba vinculado a otra persona, no se cambia y se avisa.
   - Se guardan la tarjeta y el nombre en los ajustes para la próxima vez (un campo vacío no
     borra el que había).
   - Con FIT, se alinea con las picadas del resultado (`docs/alineacion.md`). Si se puede, se
     guardan el FIT original y el track, y se muestran el desfase, la confianza y los avisos. Si
     el track no se solapa pero lo haría desplazado ±1 o ±2 h (hora mal convertida), **también se
     guarda**, sin desfase, y el aviso dice que el desplazamiento se aplica en la vista de la
     carrera ("Reloj y cronometraje"). Si no se solapa de ninguna manera (FIT de otra carrera),
     **no se guarda el track** y se muestra el error. La carrera sí queda importada. Reimportar
     con otro FIT sustituye el track y vuelve al desfase automático; con el mismo FIT, conserva
     el desfase manual.
5. La lista de carreras se actualiza.

El análisis (tiempo perdido, tramos, métricas) no se guarda al importar: se calcula al mostrarlo
a partir de la carrera y el track guardados.

## Importar una carpeta

Para cargar de una vez la temporada pasada (#41). En la pantalla **Importar**, el control
*Una carrera / Una carpeta* de la cabecera (`ImportScreen.tsx`; la carpeta, en
`BatchImportPanel.tsx`). La lógica está en `app/src-tauri/src/batch.rs` y reutiliza la de
importar una carrera: no calcula nada por su cuenta.

1. **Carpeta**: el usuario la elige con el diálogo del sistema. Se buscan los `.spl` y los `.fit`
   (por la extensión, sin distinguir mayúsculas) en ella y en sus subcarpetas, sin las ocultas
   (las que empiezan por `.`) ni seguir enlaces a carpetas. El resto de ficheros se ignora.
2. **Identidad**: la de los ajustes (tarjeta SI y nombre). Sin ninguna de las dos (o con un
   nombre que se queda vacío al normalizar) no se importa nada y se pide rellenarla en Mi perfil.
3. **Cada .spl** se lee con la zona horaria de los ajustes y se busca al corredor
   (`docs/identificacion.md`). **Solo se importa si sale un único resultado que casa** (por
   tarjeta y nombre, o por lo que haya configurado). Si casa por tarjeta pero no por nombre, si
   hay varios o si no hay ninguno, la carrera **no se importa** y el resumen dice que se importe
   sola, donde se puede elegir: en la importación por lotes no se adivina, y aún no se puede
   borrar una carrera desde la app. Un .spl ilegible tampoco se importa, y una copia exacta de
   otro de la carpeta se importa una sola vez.
4. **Cada .fit** se lee entero. Su **intervalo** va del primer al último punto con posición. Un
   FIT que no se puede leer o sin puntos con posición, o una copia exacta de otro, no se usa.
5. **Emparejar por fecha y hora** (`batch::pair_fits`):
   - La **ventana** de una carrera es la de la alineación (`docs/alineacion.md`, "Ventana de la
     carrera", `tramos_core::alignment::race_window`): de la salida a la meta del corredor (o su
     primera y última picada con hora), en UTC, ampliada **60 s por cada lado**, el desfase
     máximo que busca la alineación. Un FIT que no la toca no se podría alinear. Si el resultado
     no tiene horas, la carrera se importa sin FIT.
   - Un FIT **coincide** con una carrera si su intervalo se solapa con la ventana (más de 0 s).
   - Cada carrera se queda con el FIT **que más se solapa** con ella y cada FIT va a una sola
     carrera: se reparten de mayor a menor solape (a igual solape, primero la carrera y el FIT
     que van antes en el orden de la carpeta). Así, un rodaje de calentamiento grabado aparte,
     que solo toca el principio de la ventana, pierde frente al FIT de la carrera.
   - **Empate**: si al tocarle a una carrera hay otro FIT libre que se solapa con ella lo mismo
     (menos de 1 s de diferencia, por ejemplo el mismo entrenamiento exportado dos veces), la
     carrera se importa **sin FIT** y se avisa: se importa sola eligiendo el FIT. Esos FIT
     pueden ir aún a otra carrera.
   - Los FIT que se quedan sin carrera salen en el resumen: «coincide con X, pero otro FIT
     encajaba mejor», «hay otro FIT de la misma hora que X» o «no coincide con ninguna carrera
     en la que te haya encontrado».
6. **Importar** cada carrera, de la más antigua a la más reciente, con `import::import`, como en
   "Importar una carrera": se vincula a la persona del usuario y, con FIT, se alinea con sus
   picadas; si no se alinea, el track no se guarda y se dice por qué. El formato es el
   sugerido. Si una carrera no se puede importar (por ejemplo, el fichero ha desaparecido entre
   medias), se dice y se sigue con las demás.
   - **Reimportar no duplica**: un .spl ya importado reutiliza su carrera (sale como «ya estaba
     importada»), no cambia el formato (el usuario lo puede haber corregido) y, si ahora tiene
     FIT, se lo añade o sustituye, como al reimportar una a una. Importar dos veces la misma
     carpeta deja lo mismo.
   - Si el FIT emparejado no es de un deporte a pie (`Track::sport`, `docs/formato-fit.md`), se
     importa igual pero se avisa: el reloj pudo quedarse en otro modo.

**Resumen** (`BatchSummary`):

| Campo | Qué es |
| --- | --- |
| `races` | Un elemento por .spl, de la carrera más antigua a la más reciente (los ilegibles, al final): ruta, nombre y fecha de la carrera, `status` (`imported`, `already_imported` o `not_imported`), `result_id` si se ha importado, el FIT emparejado (`fit`: ruta, `track_saved`, desfase y confianza) y `messages`, los avisos o el motivo de no importarla. |
| `unpaired_fits` | Los FIT sin carrera: ruta, `reason` (`no_race`: válido pero sin carrera; `invalid`: ilegible o sin posiciones; `duplicate`: copia de otro) y `message`. |
| `warnings` | Problemas de la carpeta: subcarpetas que no se pueden leer, nombres de fichero ilegibles. |

La interfaz lo enseña en tres bloques, pensados para quien no es de datos, con cuatro cifras
arriba (importadas, con reloj, sin pareja y con avisos):

- **Importadas**: las carreras nuevas, con su fecha, su nombre y el FIT con el que se han
  emparejado (o «Sin FIT»). Una fila abre la carrera.
- **Sin pareja**: las carreras importadas sin FIT y los FIT válidos que no se han usado, con el
  motivo.
- **Con avisos**: las carreras con algún aviso (no importadas, ya importadas, FIT que no se ha
  podido alinear o de otro deporte, empate de FIT), los FIT ilegibles o repetidos y los
  problemas de la carpeta.

## Ajustes

Dos pantallas de la barra lateral guardan el mismo formulario (`SettingsView.tsx`, #127):

- **Mi perfil**: quién eres (nombre y apellidos, tarjeta SI) y qué compartes («Compartir mis
  carreras», la carpeta compartida y qué se comparte por defecto de cada carrera).
- **Ajustes**: tramo con error (umbrales), hora de las carreras (zona horaria), colores del mapa,
  paneles de análisis y entrenar («Entreno a otros atletas» y, con ella, la carpeta compartida),
  con un índice a la izquierda que salta a cada apartado.

Se guardan en la tabla `settings` de la base:

| Clave | Valor | Por defecto | Efecto |
| --- | --- | --- | --- |
| `lost_time.error_threshold_s` | Pérdida mínima en segundos para que un tramo sea error (≥ 0). | 15 | Inmediato: el análisis se calcula al mostrarlo, así que cambiarlo recalcula la lista y todas las vistas de carrera. |
| `lost_time.error_threshold_pct` | Pérdida mínima en % del tiempo esperado (≥ 0). | 10 | Igual. |
| `import.time_zone` | Zona horaria IANA de las horas del .spl (`Europe/Madrid`, `Atlantic/Canary`…). | `Europe/Madrid` | Solo en las carreras que se importen después: las ya guardadas tienen sus horas en UTC. Una carrera reimportada se reutiliza tal cual, así que para corregir su hora habría que borrarla antes (aún no se puede desde la app). |
| `self.si_card` | Tarjeta SI del usuario. | — | Rellena el formulario de importar. |
| `self.full_name` | Nombre y apellidos, tal y como los escribió. | — | Igual. |
| `self.person_id` | Id de la persona del usuario en `people` (no se edita). | — | A ella se vinculan sus resultados. |
| `sharing.share_own` | «Compartir mis carreras»: `true` o `false`. | `true` | Exporta las carreras propias a la carpeta compartida (`docs/paquete.md`, "Carpeta compartida"). |
| `athletes.enabled` | «Entreno a otros atletas»: `true` o `false`. | `false` | Importa los paquetes de los atletas de la carpeta compartida y añade la sección Atletas (abajo). |
| `athletes.include_self` | «Incluirme» en la vista de grupo: `true` o `false`. | `false` | Las carreras propias cuentan como un atleta más en la tabla, el cara a cara y las carreras compartidas. Se cambia en la vista de grupo, sin «Guardar». |
| `sharing.mode` | Modo de antes de #119: `runner` o `coach`. Ya no se escribe. | — | Solo si faltan los dos de arriba: `coach` es entrenar sin compartir lo propio; `runner`, al revés. |
| `sharing.folder` | Carpeta compartida (sincronizada con Drive, OneDrive, Dropbox…). Tiene que existir. La misma para compartir y para recibir. | — | Sin carpeta no se comparte nada. Al guardar con carpeta, se exportan todas las carreras propias (si las comparte) y se buscan paquetes nuevos (si entrena). |
| `sharing.default_choice` | Qué se comparte de una carrera si no se ha elegido nada para ella: `none`, `aggregates`, `legs` o `track`. | `legs` | Se puede cambiar en cada carrera (vista de carrera). |
| `package_runner_id` | Identificador al azar del corredor en los paquetes (no se edita, `docs/paquete.md`). | — | — |
| `map.pace_zones` | Zonas de ritmo del mapa (#96), en JSON: `limits` (s/km, de menor a mayor) y `colors` (`#rrggbb`, uno más que límites). Vacío = ninguna. | — | Sin zonas, el mapa colorea por cuantiles de cada carrera. Con ellas, por las zonas del usuario (ver "Mapa"). |
| `map.heart_rate_zones` | Igual, para el pulso (ppm). | — | Igual. |
| `ui.hidden_panels` | Paneles de análisis ocultos (#130), en JSON: lista ordenada de identificadores (`app/src/panels.tsx`). `[]` = todos a la vista. | Rachas limpias, pulso antes del error y esfuerzo percibido | Inmediato: el panel desaparece de Estadísticas o de la carrera. No pasa por «Guardar». |

Un valor guardado que no se entiende (número negativo, zona desconocida, zonas que no valen) se
trata como si no estuviera y toma el valor por defecto. El tiempo ideal sigue siendo la suma de
referencias.

**Colores del mapa** (#96, `app/src/ZoneEditor.tsx`, `app/src-tauri/src/zones.rs`). Para ritmo
y para pulso, «Por cuantiles de cada carrera» (lo de siempre) o «Mis zonas»:

- Cada zona tiene un color y, menos la primera, un límite «desde» (ritmo en min/km, «5:30»;
  pulso en ppm). Al elegir «Mis zonas» se proponen 5 (pulso: 120, 140, 160 y 175; ritmo: 4:30,
  5:30, 6:30 y 8:00) con los primeros colores de `ZONE_COLORS`, y se pueden añadir hasta 10 y
  quitar hasta dejar 2.
- **No se guardan** (`save_settings` da error y no guarda nada) si los límites no van de menor a
  mayor sin repetirse, alguno no es un número mayor que 0, sobra o falta un límite o un color no
  es `#rrggbb`.
- **Avisos**, que no impiden guardar, mientras se editan (`check_zones`): un color con contraste
  menor que 2:1 (WCAG) con el fondo de las teselas (`#f2efe9`) «se ve poco sobre el mapa», y dos
  zonas seguidas a menos de 9 de distancia en OKLab (× 100) «tienen colores muy parecidos». Son
  los umbrales que cumple la escala azul por defecto (contraste desde 2,18; tonos seguidos a entre
  9,5 y 10,4). Los colores propuestos los cumplen todos, también seguidos (un test lo comprueba).

Con una zona horaria equivocada, el FIT no se solapa con la carrera y la alineación lo dice, con
la sugerencia de desplazamiento (`docs/alineacion.md`).

## Atletas (#37, #119)

Quien entrena ve todo lo de cada atleta como si fuera él, sin poder modificar nada
(`docs/datos-y-privacidad.md`). No hay modos excluyentes: las funciones de corredor (Inicio, Mis
carreras, Estadísticas, Importar, etiquetar y compartir) están siempre, y entrenar añade la
sección **Atletas**. Quien entrena y también corre usa las dos cosas a la vez.

- **Primera vez.** La app pregunta «¿Cómo vas a usar la app?»: «Corro», «Entreno» o «Las dos
  cosas» (`choose_role`). Corro comparte lo propio y no entrena; Entreno, al revés; las dos
  cosas, ambas. Una base de antes que ya tiene carreras se toma por quien corre y no pregunta;
  una con el modo de antes `coach` entrena y no comparte lo propio. Se cambia después en Mi
  perfil («Compartir mis carreras») y en Ajustes («Entreno a otros atletas»). Al elegir Entreno o
  las dos cosas se abren los Ajustes, porque sin carpeta compartida no le llega nada.
- **Recibir.** Los paquetes llegan por la misma carpeta compartida en la que se exportan los
  propios (`docs/paquete.md`, "Carpeta compartida"). Los suyos no vuelven a entrar.
- **Barra lateral.** Debajo de «Lo mío», el bloque **Atletas**: un selector «Ver a» con los
  atletas de los que hay paquetes (nombre visible y número de carreras; sin elegir, «Elige un
  atleta…»), y, mientras se ve a uno, sus **Carreras** y **Estadísticas**; **Comparar
  atletas**, la vista de grupo; y **Grupos**. Las entradas de «Lo mío» (y Mi perfil) vuelven siempre a lo
  propio.
- **Qué se está viendo.** Las vistas de un atleta llevan arriba una franja «Estás viendo a
  Nombre. Solo lectura: no se puede etiquetar ni cambiar nada.» con «Volver a lo mío», que
  vuelve a Inicio. Las migas de pan llevan su nombre delante. En la barra lateral se marca su
  entrada de Atletas, no la de «Lo mío».
- **Ver.** Los paquetes del atleta se vuelcan en una base en memoria con la forma de la de su
  app (`coach::runner_view`): cada carrera con los originales del recorrido, su resultado
  vinculado a «su» persona, el formato, las etiquetas, el track con su desfase manual y los
  umbrales de su paquete más reciente. Así **todas las vistas de corredor** (lista, carrera con
  P2 y P4, mapa e histórico) salen tal cual, recalculadas con la versión del algoritmo de esta
  app. En la comparación con el grupo (P4), el atleta sale con su nombre visible y los demás,
  con el nombre y apellidos de los resultados; los de paquetes anteriores a #118, que vienen sin
  nombres, como «Corredor 1», «Corredor 2»… Cuando llega algo nuevo de ese atleta, se vuelve a
  volcar. Cambiar de atleta, o volver a lo propio, empieza de cero el botón de volver y los
  filtros de la lista.
- **Solo resumen.** Las carreras compartidas con `aggregates` no traen tramos: salen aparte en la
  lista («Solo con el resumen»), con fecha, carrera, categoría, resultado, tiempo y tiempo
  perdido, y no se pueden abrir ni entran en el histórico.
- **Grupos** (#120). Pantalla del bloque Atletas para organizarlos en grupos (por ejemplo,
  «Juveniles» o «Equipo de relevos»). Cada grupo, una tarjeta con su color, nombre, descripción y
  una casilla por atleta (también «Nombre (tú)», si tiene carreras propias) para meterlo o
  sacarlo al momento. Un atleta puede estar en varios grupos y sigue en ellos aunque cambie su
  nombre visible, porque van por el `runner_id` de sus paquetes; si ya no hay paquetes suyos, se
  dice cuántos son. «Nuevo grupo» y el lápiz de cada tarjeta abren el formulario: nombre
  (obligatorio), descripción y uno de 8 colores (el nuevo toma el primero libre). Desde el
  lápiz, «Borrar…» pide confirmar y avisa de que sus atletas y sus carreras se quedan.
  «Estadísticas del grupo» (desactivado sin miembros) abre las estadísticas de ese grupo (abajo).
  Con dos grupos o más, arriba, «Comparar grupos».
- **Estadísticas del grupo** (#145, desde Grupos; migas «Grupos / Estadísticas del grupo»;
  `GroupStatsScreen.tsx`). Las pestañas y los paneles de Estadísticas con todos los miembros
  juntos, cada uno con sus umbrales (`docs/historico.md`, "Estadísticas de un grupo"). Arriba, el
  nombre y la descripción del grupo y los filtros del histórico; debajo, sus miembros con sus
  carreras (los que no cuentan, con el motivo), y las cifras del grupo: carreras (y de cuántos
  atletas), IR medio, tasa de error y pérdida por tramo. Mismas pestañas que Estadísticas:
  - Resumen: la tabla por formato (sin la columna de consistencia) y sus tres paneles;
  - ¿Dónde falla?: duración del tramo, tipos de error, desnivel y ¿lento o desorientado?;
  - ¿Cómo evoluciona?: días sin competir, con una nota de por qué no sale la consistencia;
  - Cabeza y piernas: después de fallar y cansancio.

  Son los mismos componentes y los mismos identificadores de panel que en Estadísticas, así que
  ocultar un panel lo oculta en las dos pantallas. No salen la consistencia, la lista de carreras
  ni el resumen en frases. La pestaña no se recuerda al salir. Si ningún miembro tiene carreras,
  se dice; si ninguna pasa los filtros, se ofrece quitarlos.
- **Comparar grupos** (#121, desde Grupos; migas «Grupos / Comparar grupos»). Un grupo A frente
  a otro B (de entrada, los dos primeros), con dos opciones (`docs/historico.md`, "Comparar
  grupos"): «Si alguien está en los dos» (cuenta en los dos o se deja fuera) y «Carreras» (las
  de los dos grupos o todas), y los filtros del histórico. Un aviso dice cuántas carreras entran
  y qué pasa con quien está en los dos. Debajo:
  - una tabla con atletas, carreras, tramos, IR medio, tasa de error y pérdida media de cada
    grupo, y la diferencia A − B en puntos;
  - tres paneles que se pueden ocultar como los demás (`groups-error-types`,
    `groups-leg-length` y `groups-slope`): la parte de los errores de cada tipo (los 6 más
    comunes entre los dos; elegirlos solo ordena lo que da el núcleo), la tasa de error según la
    duración del tramo y el IR medio en subida, llano y bajada. A y B llevan siempre los dos
    primeros colores de serie, no los de los grupos, que podrían ser iguales.
- **Comparar atletas** (P15). La vista de grupo, con los filtros del histórico
  (`docs/historico.md`, "Vista de grupo (P15)"), un selector **Grupo** («Todos los atletas» o
  uno de los grupos; cambiarlo no entra en «volver») y, con «Todos», la casilla
  **«Incluirme»** (desmarcada por defecto, se guarda en `athletes.include_self`), con la que las
  carreras propias cuentan como un atleta más («Nombre (tú)», la primera fila). Con un grupo,
  salen sus miembros, quien usa la app solo si está en él, y su nombre y descripción arriba. Si
  ninguno tiene carreras con esos filtros, se dice.
  - **Atletas**: una fila por atleta con carreras, IR medio, tasa de error, pérdida media
    (%), su error más común (tipo y parte de sus errores), la duración de tramo con más tasa de
    error (si tiene al menos 10 tramos) e IR en subida, llano y bajada. Un clic en la fila abre
    sus carreras (en la propia, las tuyas). Con dos o más, una última fila de **totales**
    («Todos» o «Total de Grupo»): carreras, IR medio, tasa de error y pérdida media de todos
    juntos (`docs/historico.md`, "Vista de grupo (P15)").
  - **Cara a cara**: tabla de todos contra todos. En cada celda, cuántas carreras compartidas
    tuvo el de la fila más IR que el de la columna y cuántas menos («2–1», en verde si más, en
    rojo si menos) y, debajo, la diferencia media de IR en puntos. «—» sin carreras en común.
  - **Carreras compartidas**: las que han corrido al menos dos, de la más reciente a la más
    antigua, con los atletas de más a menos IR y sus errores.
- **Solo lectura.** Mientras se ve a un atleta no hay ningún control de edición ni de
  etiquetado: el formato como etiqueta en vez de desplegable, sin selector de qué se comparte, el
  desfase del reloj sin campos ni botones y las etiquetas de los tramos como texto (si fue error,
  el contexto y la nota) en vez de botones y lápiz. Además, el núcleo rechaza cualquier cambio en
  las carreras (arriba, "Comandos"). Al volver a lo propio todo es editable.

## Inicio

Primera pantalla del corredor (`Home.tsx`, #127): qué hay nuevo y qué queda por hacer. Los
análisis van en Estadísticas.

- **Saludo** con el nombre de pila de Mi perfil.
- **Tu última carrera**: nombre, fecha, categoría, formato, tiempo con el resultado, tiempo
  perdido con los errores y rendimiento habitual, con «Tu media» (el IR medio del histórico sin
  filtros) debajo. «Ver carrera» la abre y, si tiene errores sin revisar, «Revisar n errores»
  también. Con track, a la derecha, una miniatura del recorrido (`TrackThumb.tsx`): el track y
  las balizas de `race_map` dibujados en SVG, sin teselas.
- **Pendiente**: las carreras con errores sin revisar (`unreviewed_count`, las 5 más recientes y
  cuántas más hay), con «Revisar», y cuántas carreras no tienen el FIT del reloj, con «Importar»
  (reimportar una carrera con su FIT le añade el track). Sin nada pendiente, lo dice.
- **Tu rendimiento**: línea con el rendimiento habitual de las 10 últimas carreras, de la más
  antigua a la más reciente, y un enlace a Estadísticas.
- Sin carreras, invita a importar la primera.

Inicio es siempre de lo propio: con «Entreno» y sin carreras propias, invita a importar.

## Lista de carreras

Los resultados vinculados a la persona del usuario, de la carrera más reciente a la más antigua:
fecha, nombre de la carrera, categoría, puesto (o estado), formato, tiempo, tiempo perdido (con el
número de errores) y si tiene track del reloj. Una fila abre la vista de la carrera.

Pantalla **Mis carreras** (#128). Encima de la tabla, una barra (`raceFilter.ts`) que solo elige y
ordena las filas de `list_races`, sin calcular nada:

- **Buscar**: palabras que tienen que estar todas en el nombre de la carrera o la categoría, sin
  distinguir mayúsculas ni tildes.
- **Formato**: todas, sprint, media, larga o sin formato.
- **Temporada**: los años con carreras.
- **Solo con track**.
- **Orden**: más recientes (lo normal), más tiempo perdido o mejor rendimiento habitual. Las
  carreras sin ese valor van al final.
- **Quitar filtros** (sale con algún filtro puesto) vuelve a la lista entera y deja el orden.

25 carreras por página, con «1–25 de 60» y los botones de página debajo. Cambiar un filtro vuelve a
la primera página. Los filtros y la página siguen al volver de una carrera; se reinician al pasar
a ver a un atleta o volver a lo propio. En cada fila, el icono de etiqueta en naranja avisa de errores
sin revisar (`unreviewed_count`). Con filtros que no deja ninguna, se ofrece quitarlos.

## Vista de carrera (P1)

- **Cabecera**: carrera, fecha, categoría, corredor y resultado. A la derecha, el **formato**
  en un desplegable (sprint, media, larga o sin formato): se sugiere al importar y aquí se
  puede corregir (#97). El cambio se guarda al momento y mueve la carrera de grupo en la vista
  histórica. Si compartes lo tuyo y hay carpeta compartida, al lado, **qué se comparte** de la
  carrera con quien te entrena: «por defecto» (el ajuste), nada, resumen, tramos o track completo
  (#36). Al abrir la carrera y al cambiarlo se exporta si hace falta; si falla, se avisa con el
  motivo, y si se pide el track de una carrera sin track, se dice que van los tramos.
- **Totales**: tiempo, tiempo perdido, tiempo sin errores, número de errores y rendimiento
  habitual, con la consistencia de la carrera debajo («Consistencia ± 23 %», P10,
  `docs/tiempo-perdido.md`): van juntos porque son el centro y la dispersión del IR. Aviso si la
  referencia es débil.
- **Tabla de tramos**: tramo, balizas (S = salida, M = meta), split, puesto en el tramo,
  referencia, IR, pérdida en segundos y en % y notas (último tramo, referencia corta, tipo de
  error etiquetado) con la etiqueta del tramo. Los tramos con error van resaltados.

Los números salen de `tramos_core::runner_report::runner_report`, la misma función que usa
`tramos analizar` (`docs/cli.md`), sobre la carrera guardada: la tabla coincide con la de la CLI.
Los umbrales son los de los ajustes.

Debajo de los totales, **pestañas** (#129, `Tabs` en `ui.tsx`; con las flechas del teclado se pasa
de una a otra). La pestaña abierta es parte de la pantalla: «volver» regresa a ella, y cambiar de
pestaña no cuenta como otra pantalla. Desde Inicio, «Revisar» abre directamente Tramos.

- **Resumen** (la de entrada):
  - arriba, el **resumen en frases** de la carrera (#126, `docs/frases.md`): como mucho tres
    frases, cada una un enlace a la pestaña que la justifica (Tramos o Análisis). Las de pocos
    datos llevan su aviso escrito al lado («con pocos tramos», «referencia débil»), no solo un
    color. Sin frases no se pinta el bloque. También al ver a un atleta;
  - si quedan errores sin revisar, un aviso con «Revisar ahora», que lleva a Tramos;
  - el panel de pérdida por tramo (P1);
  - «Dónde más perdiste»: los tres tramos con más pérdida, con su split y su referencia y, con
    track, «Ver en el mapa», que lo selecciona y abre Mapa;
  - con track, el recorrido en miniatura (`TrackSvg`) y «Abrir el mapa».
- **Tramos**: la tabla de tramos con el etiquetado, la casilla «Solo errores» (los propuestos y
  los etiquetados como error) y el contador «n de m propuestos revisados». La pestaña lleva un
  contador con los que quedan sin revisar.
- **Mapa**: «Reloj y cronometraje» plegado encima, el mapa y, a su derecha, la lista de tramos
  con su split y su pérdida; un clic en uno lo selecciona.
- **Frente al grupo**: P4.
- **Análisis**: pérdida acumulada (P3), dónde gano y dónde pierdo (P5), rendimiento por tramo y
  ¿lento o desorientado? (P2).

En las pestañas, los paneles están siempre abiertos, sin desplegable (`PanelsOpen` en
`ChartPanel.tsx`).

Las filas de la tabla se pueden seleccionar (clic, o Intro o espacio con el foco): el tramo
seleccionado se resalta a la vez en la tabla, en la lista de tramos y en el mapa, y sigue
seleccionado al cambiar de pestaña. Otro clic en el mismo lo quita.

### Reloj y cronometraje (#68)

Tarjeta plegable encima del mapa, en la pestaña Mapa, solo si la carrera tiene track. Plegada es
una línea: el título, si es automático o fijado a mano y el desfase en uso en palabras («tu reloj
va 7,1 s adelantado»). Se despliega sola, con una píldora «Revisar», si la alineación ha fallado o
la confianza es baja (`ClockOffset.tsx`, comandos
`race_offset` y `set_race_offset`, lógica en `clock_offset.rs`). Una frase explica qué es el
desfase (la diferencia de hora entre el reloj y el cronometraje, con la que se sabe dónde estaba
el corredor al picar cada baliza) y cuándo tocarlo: si en el mapa las balizas no caen donde
estaban.

- **Cifras**: el desfase calculado (`align`) con su confianza (o «pocas balizas útiles: se toma
  0», o «no se ha podido calcular») y el **en uso**, explicado en palabras («tu reloj va 7,1 s
  adelantado»). Una píldora dice si es automático o fijado a mano. Con confianza baja (< 0,5)
  sale un aviso que invita a revisarlo en el mapa; los avisos de la alineación, debajo.
- **Sugerencia de ±1/2 h**: si la alineación automática no encaja porque el track se solaparía
  desplazado horas enteras (`suggested_shift_s`, `docs/alineacion.md`), se explica (cambio de
  horario o zona horaria mal elegida) y un botón «Aplicar -1 h» fija el desfase sugerido: las
  picadas se desplazan esas horas, se estima con `align` el desfase fino que queda y se fija la
  suma (`suggested_offset_s`), así que no hace falta afinarlo a mano.
- **A mano**: un campo en segundos (con coma o punto; positivo si el reloj va adelantado),
  relleno con el desfase en uso, y «Aplicar». Si con ese desfase la carrera no cae en el track,
  o no es un número de un día como mucho, sale el error y no se guarda.
- **Volver al automático** (solo con uno fijado): borra el manual.

Se guarda con el track (`tracks.manual_offset_s`, `docs/almacenamiento.md`). Todo lo que sale del
track lo usa, porque pasa por el mismo sitio (`race_map::aligned_legs`): las balizas y los tramos
del mapa, P2 en la vista de carrera y P13, P2 y P8 en el histórico. Al cambiarlo, la vista vuelve
a pedir el mapa y P2. La tabla de tramos no cambia: sale de los splits del .spl, no del track.

### Etiquetar errores (#30)

Los tres niveles de `docs/taxonomia.md`, sin ninguno obligatorio:

- **Confirmar (nivel 1)**: en la columna de notas de cada tramo propuesto (los que el cálculo
  marca como error) hay tres botones, **Sí**, **No** y **Físico**. Un clic guarda la respuesta;
  otro clic en la marcada la quita. Los tramos ya etiquetados también los muestran. Encima de la
  tabla, una frase explica qué hacer y un contador dice cuántos propuestos están revisados.
- **Tipo y contexto (niveles 2 y 3)**: el lápiz de cada fila abre, debajo de ella, el formulario
  completo: ¿hubo error?, tipo y subtipo, causas (varias), parte del tramo, segundos que cree haber
  perdido, esfuerzo (1–10) y nota. Se guarda con «Guardar»; «Quitar etiqueta» la borra. Con
  «No» se ocultan el tipo y el subtipo y no se guardan. El lápiz está en todos los tramos, así que
  sirve también para añadir un error en un tramo no propuesto. Abrirlo selecciona el tramo en el
  mapa.
- El tipo y el subtipo etiquetados salen en las notas de la fila. Una clave que ya no esté en la
  taxonomía se muestra tal cual.
- Los botones de la fila no la seleccionan (ni con clic ni con teclado).

## Mapa

En la pestaña Mapa de la vista de carrera (#20, #129). Sin mapa de orientación en el MVP: la ruta se
pinta sobre OpenStreetMap con **MapLibre GL JS** (BSD-3), que se carga aparte (`lazy`) al abrir
una carrera. Componente: `app/src/MapView.tsx`; tipos: `app/src/mapApi.ts`; estilos:
`app/src/styles/map.css`.

**Comando `race_map(resultId)`** (`app/src-tauri/src/race_map.rs`). La interfaz no calcula
nada: recibe el track ya troceado y clasificado. Devuelve, según `status`:

| `status` | Cuándo | La vista enseña |
| --- | --- | --- |
| `no_track` | La carrera se importó sin FIT. | Un estado vacío: «Sin track del reloj», con la sugerencia de reimportarla con el FIT. |
| `not_aligned` | Hay track, pero no se puede alinear o segmentar: la hora estaba mal convertida al importar (se guarda con la sugerencia de desplazamiento) y aún no se ha corregido el desfase. | El error, en `message`, y que se corrige en «Reloj y cronometraje». |
| `ready` | Lo normal. | El mapa. |

Con `ready`:

| Campo | Qué es |
| --- | --- |
| `bounds` | `[oeste, sur, este, norte]` de todos los tramos, para encuadrar. |
| `legs` | Un tramo por par de picadas consecutivas (`docs/segmentacion.md`): `index`, `from`, `to`, `coordinates` (de baliza a baliza), `bounds` y `missing`. |
| `pieces` | Trozos del track en orden, con su tramo (`leg`), `coordinates`, `pace_class` y `heart_rate_class`. |
| `controls` | Balizas situadas: `position` (0 = salida; el tramo *n* acaba en la baliza *n*), `code`, `role` (`start`, `control`, `finish`), `coordinate` e `in_gap`. |
| `pace` / `heart_rate` | Escalas, según `kind`: `quantiles` con `edges`, los 6 límites de las 5 clases, de menor a mayor (s/km y ppm), o `zones` con las zonas del usuario (`limits` y `colors`, como en Ajustes). `heart_rate` es `null` si el track no trae pulso en al menos la mitad del tiempo de carrera, tenga zonas o no. |
| `warnings` | Avisos de la alineación y balizas que no se pueden situar, en español. |

Coordenadas `[longitud, latitud]` como en GeoJSON, redondeadas a 6 decimales (~10 cm).

Cómo se calcula:

1. Se alinea el track guardado con las picadas (`docs/alineacion.md`), con el desfase fijado a
   mano si lo tiene (`align_with_offset`), y se trocea en tramos (`docs/segmentacion.md`), con
   las opciones por defecto. Solo se pinta de la salida a la meta.
2. **Balizas**: la posición de la segmentación, es decir, dónde estaba el corredor en el instante
   de cada picada alineada. Las que no tienen posición (picada sin hora o fuera del track) no
   salen y lo dice un aviso; las que caen en un hueco del track salen con trazo discontinuo.
3. **Ritmo** de cada intervalo entre dos puntos de un tramo: distancia recorrida en la ventana
   del intervalo ampliada 5 s a cada lado (recortada al track), entre la duración de la ventana.
   La distancia es la de `tramos_core::metrics::interval_distance_m` (la del reloj o, si no, la
   del GPS, `docs/metricas.md`) acumulada sobre el track entero, así que la ventana cruza sin
   saltos el límite entre tramos. Por debajo de 0,5 m/s (parado) o más lento de 20:00 min/km, el
   ritmo es 20:00 min/km. Un intervalo de más de 10 s es un **hueco**: sin ritmo ni pulso.
4. **Pulso** de un intervalo: la media de sus dos extremos, si los dos lo tienen.
5. **Clases**. Con zonas del usuario (Ajustes, #96), la clase es la zona: el número de límites
   que alcanza el valor, así que un valor justo en un límite va a la zona de arriba (con 120, 140
   y 160: 119 es la zona 0, 120 la 1 y 160 la 3). Sirven para comparar entre carreras. Sin zonas,
   5 clases por cuantiles ponderados por la duración de los intervalos (cada tono ocupa más o
   menos el mismo tiempo de carrera); la clase de un valor es el número de límites interiores que
   supera. En los dos casos, 0 es lo más rápido (o el pulso más bajo). Con cuantiles, el mapa
   enseña dónde fue el corredor más despacio *en esa carrera*, sin depender de su forma ni del
   terreno. Las zonas son las de los ajustes de la base que se mira: en la vista de un atleta
   recibido no hay, porque el paquete no las lleva, y sale por cuantiles.
6. Los intervalos seguidos del mismo tramo con las mismas clases se juntan en un trozo: con el
   FIT sintético, 376 trozos para unos 1 500 puntos (unos 100 kB de JSON).

Lo que se ve:

- **Track** con un borde blanco, coloreado por **ritmo** o, si hay pulso, por **pulso** (control
  segmentado *Ritmo / Pulso*; de entrada, ritmo). Escala secuencial de un solo tono (azul, de
  claro a oscuro, `--map-seq-1…5`), validada con la guía de visualización: luminosidad monótona,
  saltos visibles entre tonos y el más claro a más de 2:1 sobre el fondo del mapa. Las teselas
  de OSM son claras también en modo oscuro, así que estos colores no cambian con el modo; la
  leyenda se pinta sobre una tira del color del mapa (`--map-paper`) para que los tonos se vean
  igual. Los huecos (y, con pulso, los trozos sin pulso) van en gris discontinuo. Con zonas, cada
  trozo lleva el color de su zona.
- **Leyenda** bajo el mapa: los cinco tonos, los cuatro límites entre clases (min/km o ppm) y
  los extremos «Más rápido / Más lento» (o «Más bajo / Más alto»). Con zonas, «· tus zonas» y
  cada color con su rango («< 120», «120–140», «≥ 160»); el color va en el atributo `fill` de un
  SVG, porque la CSP no deja estilos en línea.
- **Balizas** como en un mapa de orientación, en el magenta del recorrido: triángulo en la
  salida, círculo con el número de orden en cada baliza (el código, en la etiqueta accesible) y
  doble círculo en la meta.
- **Tramo seleccionado**: halo magenta, el resto del track atenuado, sus dos balizas más
  marcadas y, bajo el mapa, su resumen (balizas, split y pérdida). Si no se ve entero, el mapa se
  encuadra en él (sin animación con `prefers-reduced-motion`). Clic en un tramo del mapa (la zona
  de clic es más ancha que la línea) lo selecciona y resalta su fila; otro clic lo quita.
- **Botones** propios para acercar, alejar y ver toda la carrera (los de MapLibre traen iconos en
  `data:`, que la CSP no deja cargar). Sin rotación ni inclinación.

Los tramos del mapa y los de la tabla se emparejan por número. Para un corredor clasificado son
los mismos; con picadas que no casan con el recorrido (baliza fallida) el mapa sigue las picadas
y la tabla el recorrido (`docs/segmentacion.md`).

**Teselas de OpenStreetMap** (`https://tile.openstreetmap.org/{z}/{x}/{y}.png`), según su
[política de uso](https://operations.osmfoundation.org/policies/tiles/):

- Atribución siempre visible, «© OpenStreetMap contributors», con enlace a
  `https://www.openstreetmap.org/copyright`. El enlace se abre en el navegador del sistema con
  `tauri-plugin-opener`, al que la ventana solo deja abrir URL de `https://www.openstreetmap.org/`
  y de los documentos del repositorio (los de la Ayuda, abajo).
- Solo se piden las teselas de lo que se mira, al moverse por el mapa: nada de descargas
  masivas ni de precarga. Zoom máximo 19 (el de OSM). MapLibre no vuelve a pedir las teselas
  caducadas mientras el mapa está abierto (`refreshExpiredTiles: false`) y la caché del
  navegador respeta las cabeceras del servidor.
- Es un servicio gratuito para un uso moderado. Si el grupo creciera mucho, habría que pasar a
  un proveedor de teselas con clave o a uno propio.
- Pedir teselas revela la zona que se mira (`docs/datos-y-privacidad.md`).

## Vista histórica (P6)

Pantalla **Estadísticas** de la barra lateral (`HistoryScreen.tsx`): todas las carreras del usuario
agregadas por formato. Las definiciones (qué carreras y tramos cuentan, IR medio, tasa de error,
pérdida media por tramo, carreras sin formato) están en `docs/historico.md`.

- **Filtros**: desde y hasta (fechas incluidas) y formato (todos, sprint, media, larga). Cambiar
  uno vuelve a pedir el histórico. «Quitar filtros» los borra.
- **Resumen en frases** (#126, `docs/frases.md`): debajo de los filtros y encima de las cifras,
  como mucho tres frases que resumen lo importante con los filtros puestos («Fallas más en los
  tramos largos…»). Cada una es un enlace a la pestaña de su análisis (`docs/frases.md`,
  "Destinos"). Las de pocos datos llevan su aviso escrito al lado («con pocas carreras»…), no
  solo un color. Sin frases no se pinta el bloque. También al ver a un atleta.
- **Cifras** del total: carreras, IR medio (con la consistencia media debajo, P10), tasa de
  error y pérdida media por tramo.
- **Tabla por formato**: sprint, media y larga (aunque no tengan carreras) y, si hay, las
  carreras sin formato, más la fila del total: carreras, tramos que cuentan, errores, IR medio,
  tasa de error, pérdida media por tramo en segundos y en % y consistencia media. Debajo del título, qué tramos
  cuentan y los umbrales de error.
- **Carreras** (#98): las que entran con los filtros, de la más reciente a la más antigua, con
  fecha, nombre, formato, categoría y sus números (IR, tramos que cuentan, errores, tasa de error
  y pérdida por tramo). Las que no cuentan lo dicen. Una fila abre la carrera.
- **Gráficas por formato** (pestaña Resumen): paneles de IR medio (con la línea del 100 %), tasa de error y pérdida
  media por tramo en % (los segundos no se comparan entre formatos; la tabla del panel da los
  dos).
- **Estados vacíos**: sin carreras importadas, invita a importar; con carreras pero ninguna con
  esos filtros, ofrece quitarlos. Si las fechas están al revés, se avisa. Las carreras sin
  números (`races_without_data`) se mencionan en un aviso.

Debajo de las cifras, **pestañas por pregunta** (#130), con los mismos filtros. La última pestaña
mirada se recuerda mientras la app está abierta:

| Pestaña | Qué tiene |
| --- | --- |
| Resumen | La tabla por formato, las gráficas por formato y las carreras que entran. |
| ¿Dónde fallo? | Por duración del tramo (P7), tipos de error (P9), por desnivel (P13) y ¿lento o desorientado? (P2). |
| ¿Cómo evoluciono? | Consistencia (P10) y días sin competir (P11). |
| Cabeza y piernas | Después de fallar (P8) y cansancio (P14). |

Los paneles están abiertos, en rejilla de dos columnas (una por debajo de 1100 px), y los avisos y
recuentos de cada análisis ocupan la fila entera.

**Ocultar paneles** (`app/src/panels.tsx`):

- Cada panel tiene un aspa para ocultarlo.
- «Personalizar», junto a las pestañas, abre un cuadro con todos los paneles, también los de la
  vista de carrera, con una casilla cada uno y «Enseñar todos».
- En Ajustes, «Paneles de análisis» dice cuántos hay ocultos y tiene los mismos dos botones.
- Se guarda al momento en `ui.hidden_panels`. De entrada están ocultos los que menos se miran:
  rachas limpias, pulso antes del error y esfuerzo percibido.
- Si se ocultan todos los paneles de un análisis, también desaparecen sus avisos.

Los análisis que se apoyan en el histórico usan los mismos filtros y, si cuentan tramos, los
mismos (`pattern_legs`).

- **Por duración del tramo (P7)** (`LegLengthPanel.tsx`): panel «Pérdida según duración del
  tramo». Una columna por cubo de referencia (20–30 s, 30–60 s, 1–2 min, 2–4 min, 4–8 min y
  ≥ 8 min, `docs/historico.md`) con la **tasa de error**, la línea de la tasa de error del total
  («Tu media») y, bajo cada etiqueta, `n` (los tramos del cubo), porque un cubo con pocos tramos
  es poco fiable. Va la tasa en la gráfica porque es la respuesta directa a «¿fallo más en los
  tramos largos o en los cortos?» y `n` es su denominador; la tabla añade la pérdida media por
  tramo en % y en segundos. Los cubos vacíos salen sin columna y con `n = 0`.
- **Consistencia (P10)** (`ConsistencyPanel.tsx`): panel «Consistencia por carrera». Línea con
  la consistencia de cada carrera que la tiene (`races[].stats.mean_consistency`), de la más
  antigua a la más reciente, con un marcador por carrera; la descripción da la media con esos
  filtros y la tabla, fecha, carrera, formato y consistencia. Las carreras sin valor no salen.
- **Días sin competir (P11)** (`DaysOffPanel.tsx`): dos paneles con los cuatro cubos de días
  desde la carrera anterior (≤ 7, 8–14, 15–30 y > 30, `docs/historico.md`) y, bajo cada
  etiqueta, sus carreras. «IR al entrar en mapa»: IR medio de los tres primeros tramos frente al
  IR medio del total («Tu IR medio»). «Errores al principio de la carrera»: tasa de error del
  primer tercio frente a la del total («Tu media»). Son dos paneles porque son dos medidas de
  escala distinta (nada de dos ejes). Comparten la tabla, con todas las cifras y sus `n`. El
  contador del panel dice cuántas carreras no tienen anterior.
- **Por desnivel (P13)** (`SlopePanel.tsx`): una línea con cuántas carreras aportan tramos (las
  que tienen FIT, de las que pasan el filtro), cuántos tramos se han clasificado, cuántos no se
  pueden clasificar y cuántos son de carreras sin FIT (`by_slope`, `docs/historico.md`). Debajo,
  dos paneles con una columna por clase (subida, llano y bajada) y `n` bajo cada etiqueta:
  «IR medio según desnivel», con la línea del 100 % de la referencia, porque es la respuesta
  directa a «¿me frena el desnivel?», y «Tasa de error según desnivel». Las descripciones dan la
  regla de clasificación con el umbral aplicado. Las dos tablas dan, por clase, tramos, errores,
  IR medio y tasa de error. Una clase vacía sale sin columna y con `n = 0`. El umbral aún no se
  puede cambiar desde Ajustes: se usa el valor por defecto del núcleo (4 m por cada 100 m).
- **¿Lento o desorientado? (P2)** (`BreakdownPanel.tsx`): panel «De qué está hecha la pérdida de
  tus errores». Una columna por parte (desvío, paradas y ritmo) con su % de la pérdida de todos
  los errores repartidos (`loss_breakdown`, `docs/historico.md`); el tooltip y la tabla dan los
  segundos. La descripción dice cuántos errores no se pueden repartir (sin FIT o sin datos
  suficientes).
- **Después de fallar (P8)** (`AfterErrorPanel.tsx`). Dos paneles, los dos con la tasa de error
  del total como referencia («Tu media») y `n` bajo cada columna:
  - «¿Un error trae otro?»: la tasa de error del tramo siguiente a uno limpio, a un error, a un
    error acelerando y a un error sin acelerar.
  - «Rachas limpias»: la tasa de error según los tramos limpios seguidos previos.

  Las definiciones están en `docs/historico.md`. Las tablas dan tramos, errores y tasa.
- **Errores más comunes (P9)** (`CommonErrorsPanel.tsx`), detrás de la duración del tramo
  porque se cruza con ella: panel «Tipos de error» con dos desplegables, **formato** (todos o
  uno de los que tienen tramos) y **duración del tramo** (todas o un cubo de P7), que eligen
  cuál de los repartos de `common_errors` se enseña. Una columna por tipo con su % de los errores
  y `n` debajo, de más a menos, y una gris al final con los errores sin tipo. La descripción dice
  cuántos errores no tienen tipo, cuántos están sin revisar (y que se etiquetan en la tabla de
  tramos de cada carrera) y, aparte, los tramos marcados como físico con su pérdida, que no
  cuentan como error. La tabla da tipos y subtipos con errores, % y pérdida, la fila de sin tipo,
  el total y la de físico.
- **Cansancio (P14)** (`FatiguePanel.tsx`), la última. Empieza con un **aviso de dato débil**
  (el pulso de muñeca llega con retraso, da saltos y en un tramo corto apenas reacciona) y una
  línea con cuántas carreras aportan pulso (las que tienen FIT con pulso). Tres paneles, todos
  por tercio de carrera y con `n` bajo cada columna o grupo:
  - «Deriva del pulso»: una columna por tercio con pulso / velocidad de los tramos limpios
    frente a lo habitual en cada carrera, como diferencia en % (0 = lo habitual; con el 100 % de
    base, una deriva de unos pocos % no se vería). La tabla da el cociente en % (100 % = lo
    habitual), el pulso medio y el ritmo en movimiento.
  - «Pulso antes del error»: columnas agrupadas (antes de un error y antes de un tramo limpio)
    con el pulso del tramo anterior en ppm sobre la mediana de su carrera; debajo, `n` de cada
    serie. La descripción dice cuántos errores no tienen pulso del anterior. La tabla da también
    el pulso en ppm.
  - «Esfuerzo percibido»: columnas agrupadas (errores, limpios y físico) con el esfuerzo medio
    apuntado en las etiquetas; sin ninguno, lo dice.

  Las definiciones están en `docs/historico.md`, "¿El cansancio anticipa el error? (P14)".

## Ayuda (#125)

Ayuda dentro de la app, que funciona sin conexión: una página por pantalla y por pestaña, y páginas
de conceptos (tiempo perdido, IR, tipos de error, ¿lento o desorientado?, zonas del mapa, reloj y
desfase, formatos, compartir y atletas). Código en `app/src/help/`.

- **Abrirla.** «Ayuda», en el bloque «Cuenta» de la barra lateral, abre la
  portada. El botón «?» a la derecha de la cabecera abre la página de la pantalla abierta y, en
  la vista de carrera y en Estadísticas, la de la pestaña abierta (`views.ts`). En la propia
  ayuda no sale. La ayuda es una pantalla más: entra en «volver» y en las migas («Ayuda / Tiempo
  perdido»), y no lleva delante el nombre del atleta que se esté viendo.
- **Pantalla** (`HelpScreen.tsx`): a la izquierda, el índice de todas las páginas por bloques
  (Ayuda, Pantallas, Una carrera, Estadísticas y Conceptos); a la derecha, la página. Un enlace
  a otra página la abre como otra pantalla, desde arriba.
- **Páginas**: un fichero Markdown por página, `app/src/help/<id>.md`, que empieza por
  `# Título`. Se importan con `?raw` en `pages.ts`, así que van dentro del bundle.
- **Contenido**: para quien no es de datos, con un ejemplo inventado por concepto (nunca datos
  reales: ni nombres, ni lugares, ni fechas, ni tiempos de carreras de verdad) y, al final, el
  enlace al documento de `docs/` con el detalle técnico. Explica en llano lo que definen los
  documentos de `docs/`, que son la fuente de verdad: no los contradice.
- **Markdown**: renderizador propio y mínimo (`markdown.tsx`), sin dependencias: títulos (`#`,
  `##`, `###`), párrafos, listas de un nivel (`-` y `1.`), tablas, citas (`>`, que se pintan como
  un recuadro «Ejemplo»), negrita, cursiva, código y enlaces. Construye elementos de React y lo
  que no entiende sale como texto: no hay HTML crudo ni estilos en línea (CSP).
- **Enlaces**: `otra-pagina.md` abre esa página de la ayuda;
  `https://github.com/Mendana/tramos/blob/main/...` se abre en el navegador del sistema (abajo,
  "Seguridad"); cualquier otro destino sale como texto.

**Que no se quede vieja.**

- Regla en `CLAUDE.md`: quien cambia una vista actualiza su página en el mismo PR, y quien añade
  una vista o una pestaña, le añade página.
- `tsc` (y con él `npm run build`, que corre en la CI) falla si una pantalla o pestaña no tiene
  página: en `views.ts`, el mapa vista → página es un `Record<Screen["kind"], HelpPageId>`, y los
  de las pestañas, `Record<RaceTab, HelpPageId>` y `Record<HistoryTab, HelpPageId>`. Y como las
  páginas se importan estáticamente, si falta un fichero el build no lo resuelve y falla.
- Al compilar, el plugin `helpCheck` de `app/vite.config.ts` comprueba cada `.md` de
  `app/src/help/`: que está registrado en `pages.ts`, que empieza por `# Título`, que no lleva
  HTML ni listas anidadas y que sus enlaces van a una página que existe o a un fichero del
  repositorio que existe. Si algo falla, el build falla con la lista.

**Añadir una página**:

1. Crea `app/src/help/<id>.md` (minúsculas, cifras y guiones), empezando por `# Título`.
2. En `pages.ts`, impórtala con `?raw`, añade `<id>` a `HelpPageId` y a `HELP_PAGES` con su
   bloque del índice (el orden de `HELP_PAGES` es el del índice).
3. Si es la de una pantalla o pestaña nueva, apúntala en `views.ts` (`tsc` lo pide).
4. Enlázala desde las páginas relacionadas y pasa `npm run build`.

## Diseño

Base visual común a todas las pantallas (#88), en CSS propio y sin librerías de componentes. La
CSP no deja inyectar estilos en tiempo de ejecución y no hay fuentes externas: se usan las del
sistema.

- **Variables** (`app/src/styles/tokens.css`): colores, tipografía, espaciado (múltiplos de 4),
  radios y sombras, con modo claro y oscuro según el sistema (`prefers-color-scheme`). Ninguna
  pantalla usa colores ni medidas sueltas.
- **Paleta** inspirada en el mapa de orientación: el **magenta** de los recorridos es el acento
  (navegación, botón principal, selección) y el **naranja** de la baliza marca los errores (filas
  de tramo con error, pérdidas, cifras). Las ganancias van en verde.
- **Estilos base** (`styles/base.css`): documento, títulos, botones (`btn`, `btn-primary`,
  `btn-ghost`, `btn-lg`), campos (`field`, `field-label`, `field-hint`, `input`) y utilidades.
- **Componentes** (`styles/components.css` y `src/ui.tsx`): estructura con barra lateral, tarjetas,
  avisos (`Notice`: información, aviso, error, éxito), píldoras, cifras destacadas (`Stat`),
  tablas (números tabulares a la derecha, filas clicables, tramos con error resaltados), zona para
  soltar ficheros, lista de opciones, control segmentado, secciones de formulario y estado vacío.
  Los iconos son SVG en línea en `ui.tsx`; el de la app es una baliza.
- **Estructura** (#127, #119, boceto en `docs/bocetos/navegacion.html`): barra lateral oscura
  (`--side-*`) con bloques: Inicio; «Lo mío», con Mis carreras (y un contador de errores sin
  revisar de lo propio), Estadísticas e Importar; si entrena, «Atletas», con el selector, las
  Carreras y Estadísticas del atleta que se ve, Comparar atletas y Grupos; «Cuenta», con Mi
  perfil, Ajustes y Ayuda.

  Encima del contenido, una cabecera fija con el botón de volver (a la pantalla anterior, hasta
  30), las migas de pan («Mis carreras / Nombre de la carrera»; al ver a un atleta, con su
  nombre delante) y, a la derecha, el botón «?» de la ayuda (abajo, "Ayuda"). Al pie de la barra
  lateral, la versión del núcleo; si entrena y hay carpeta compartida, también cuántos paquetes
  y de cuántos atletas ha recibido (y cuántos ficheros no ha podido leer). El contenido, centrado hasta
  1160 px. Por debajo de 860 px de ancho la barra lateral pasa arriba, sin los títulos de los
  bloques. La ventana abre a
  1180 × 780 (mínimo 760 × 520).

## Gráficas

Cada análisis se enseña en un **panel** (`app/src/charts/ChartPanel.tsx`), siempre abierto en las
pestañas de Estadísticas y de la vista de carrera, y con un aspa para ocultarlo (#130): título,
número de casos en los que se apoya, una frase que explica cómo leerlo y un selector **Gráfica / Tabla**. La tabla es la vista
accesible de la gráfica: todo valor que se ve al pasar el ratón está también en ella.

**Sin librería de gráficas** (#21): son componentes propios en SVG y React (`app/src/charts/`).

- La CSP no deja inyectar estilos en tiempo de ejecución, que es lo que hacen varias librerías.
  Aquí los estilos van en `styles/charts.css` y los colores son las variables de diseño, así que
  el modo oscuro sale solo.
- Las gráficas que piden las preguntas (barras, barras con signo, líneas, puntos) son pocas y
  sencillas. Con componentes propios siguen al pie de la letra las reglas de la guía de
  visualización: columnas de 24 px como mucho con el extremo redondeado y la base recta desde la
  línea del 0, rejilla y ejes en líneas finas y tenues, texto siempre en los colores de texto y
  nunca en el de la serie, y tooltip por columna con una zona activa más grande que la columna,
  también con el teclado.
- Si algún día hace falta algo que no merezca la pena dibujar a mano, la alternativa sería una
  librería que pinte en SVG con atributos de React (por ejemplo, Recharts, MIT).

Piezas:

- `scale.ts`: escala lineal y dominio «redondo» con sus marcas, que siempre incluye el 0.
- `ColumnChart`: una serie de columnas (también negativas), línea de referencia opcional, color por
  columna opcional y tooltip con el valor delante y el detalle detrás. Opcionalmente, una línea de
  datos en la **misma unidad y escala** (p. ej. un acumulado; sigue siendo un solo eje), franjas
  de fondo resaltadas (p. ej. rachas) y una segunda línea de etiqueta en el eje X (p. ej. el
  número de casos de cada columna).
- `LineChart`: una línea de 2 px con un velo del 10 % hasta el 0, marcadores opcionales con un
  anillo del color de la superficie y un cursor vertical que se ajusta al punto más cercano.
- `MultiLineChart`: varias líneas sobre los mismos puntos, la destacada (el corredor) más
  gruesa, etiqueta directa al final de cada línea y tooltip con el valor de cada serie.
- `GroupedColumnChart`: columnas agrupadas, una por serie y punto, con 2 px de separación, y
  una segunda línea de etiqueta opcional en el eje X (p. ej. el número de casos de cada grupo).
- `common.tsx`: tamaño, rejilla con marcas, tooltip (al lado de la marca, para no taparla) y
  leyenda (cuadrado para barras, raya para líneas). Con dos o más series siempre hay leyenda.
- Dos medidas de escala distinta nunca comparten eje: van en paneles separados.

Colores de las gráficas (`tokens.css`): `--chart-series-1` es el acento, validado con la guía de
visualización contra la superficie de cada modo (en oscuro, un tono más oscuro que el acento de
la interfaz para quedar en la banda de luminosidad). `--chart-grid`, `--chart-axis` y
`--chart-reference` son la rejilla, la línea del 0 y la línea de referencia. `--chart-error`
(naranja de baliza) marca los tramos con error y `--chart-muted` (gris) el resto; validados igual,
se distinguen también con daltonismo. Para ganar o perder (P5), el par divergente `--chart-gain` (azul)
y `--chart-loss` (el mismo naranja), validado igual; la dirección de la columna también lo dice.
Para varios corredores (P4), la paleta categórica `--chart-series-1` a `--chart-series-4`
(magenta para el corredor; azul, verde y ámbar para los compañeros, en ese orden fijo), validada
en los dos modos entre todas las parejas. `--chart-line-neutral` es una línea de datos en tinta neutra y `--chart-highlight` el fondo de una
franja resaltada.

Paneles de la vista de carrera:

| Panel | Qué enseña | Casos |
| --- | --- | --- |
| Pérdida por tramo (P1), en la pestaña Resumen | Pérdida de cada tramo en segundos, hacia arriba si pierde y hacia abajo si gana; los tramos con error en naranja y el resto en gris. | Tramos con pérdida. |
| Pérdida acumulada (P3) | Tiempo perdido sumado tramo a tramo desde la salida: solo suben los tramos con error, marcados con un punto. Acaba en el tiempo perdido de la carrera. | Errores y tramos. |
| Dónde gano y dónde pierdo (P5) | Ganancia de cada tramo frente a lo esperado (`gain_s`, arriba gano en azul, abajo pierdo en naranja), la línea del acumulado (`cumulative_gain_s`) y una franja por cada racha de dos o más tramos seguidos perdiendo (`losing_streaks`). Todo sale del núcleo (`docs/tiempo-perdido.md`). | Tramos con ganancia y rachas. |
| Rendimiento por tramo | IR de cada tramo como columna, con la línea del 100 % (la referencia). | Tramos con IR. |
| ¿Lento o desorientado? (P2) | Cada error de los tramos que cuentan con tres columnas, desvío, paradas y ritmo (colores de serie 1–3, en ese orden), que suman su pérdida; la descripción resume el total de los errores en segundos y en %. La tabla da todos los tramos repartidos y la relación habitual `r0`. Sin FIT, o con menos de 3 tramos sin error con track, el panel lo dice en lugar de la gráfica. Lo pide aparte (`race_breakdown`). | Errores repartidos. |

Bajo «Frente al grupo» (P4, `GroupComparison.tsx`), el corredor junto a compañeros elegidos de
su mismo recorrido (hasta 3, de entrada el ganador del recorrido). Las fichas de arriba eligen a
los compañeros y hacen de leyenda; la elección vale para los dos paneles:

| Panel | Qué enseña | Casos |
| --- | --- | --- |
| Diferencia con el tiempo ideal | Diferencia acumulada respecto al tiempo ideal tras cada tramo, una línea por corredor; hacia abajo es por detrás, como en la gráfica clásica de WinSplits. | Corredores. |
| Pérdida por tramo comparada | Pérdida de cada tramo frente a lo esperado con el rendimiento habitual de cada uno, en columnas agrupadas. | Corredores. |

Los paneles van en las pestañas Resumen (P1), Análisis y Frente al grupo, y salen de los mismos
tramos que la tabla: sus valores coinciden con ella.

## Seguridad

La ventana tiene una CSP restrictiva (`docs/datos-y-privacidad.md`). Los permisos de la ventana
(`app/src-tauri/capabilities/default.json`) son los de `core:default`, `dialog:allow-open`, este
solo para el diálogo de abrir ficheros o elegir una carpeta (la carpeta la recorre Rust: la
ventana no tiene permisos de sistema de ficheros), y `opener:allow-open-url` limitado a
`https://www.openstreetmap.org/*`, para el enlace de la atribución del mapa, y a
`https://github.com/Mendana/tramos/blob/main/*`, para los enlaces de la Ayuda a los documentos
técnicos. Los dos se abren en el navegador del sistema, no en la ventana.

MapLibre y la CSP:

- La CSS de MapLibre va en el bundle (`import` en `MapView.tsx`), así que `style-src 'self'`
  basta. MapLibre no inserta hojas de estilo: solo cambia propiedades de estilo de sus elementos
  desde JavaScript (posición de los marcadores, tamaño del lienzo), que la CSP no bloquea. Las
  balizas son elementos propios con clases de `map.css`, sin estilos en línea.
- Su worker se empaqueta como un fichero más de la app (`?worker&url` de Vite, en formato ES) y
  se le pasa con `setWorkerUrl`: `worker-src 'self'`, sin `blob:`.
- Las teselas: `img-src` y `connect-src` admiten `https://tile.openstreetmap.org` (MapLibre las
  descarga con `fetch` y las decodifica en un `ImageBitmap`).
- No se usan los controles de MapLibre con iconos en `data:` (zoom, brújula): `img-src` no
  admite `data:`.
