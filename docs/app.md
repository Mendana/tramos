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
| `preview_import(splPath, fitPath, identity)` | Primer paso de importar: lee los ficheros sin guardar nada. |
| `import_race(request)` | Segundo paso: guarda la carrera con lo que ha confirmado el usuario. |
| `list_races` | Carreras del usuario, de la más reciente a la más antigua, con su tiempo perdido. |
| `race_detail(resultId)` | Una carrera con la tabla de tramos del resultado. |

Los errores llegan a la interfaz como texto en español. La lógica está en
`app/src-tauri/src/import.rs`, `races.rs` y `settings.rs`, en Rust sin Tauri, y se prueba con los fixtures (`cargo test` en
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
     no (track de otra hora o de otra carrera), **no se guarda el track** y se muestra el error,
     con la sugerencia de desplazamiento si la hay. La carrera sí queda importada. Reimportar con
     otro FIT sustituye el track.
5. La lista de carreras se actualiza.

El análisis (tiempo perdido, tramos, métricas) no se guarda al importar: se calcula al mostrarlo
a partir de la carrera y el track guardados.

## Ajustes

Pantalla **Ajustes** (botón de la cabecera). Se guardan en la tabla `settings` de la base:

| Clave | Valor | Por defecto | Efecto |
| --- | --- | --- | --- |
| `lost_time.error_threshold_s` | Pérdida mínima en segundos para que un tramo sea error (≥ 0). | 15 | Inmediato: el análisis se calcula al mostrarlo, así que cambiarlo recalcula la lista y todas las vistas de carrera. |
| `lost_time.error_threshold_pct` | Pérdida mínima en % del tiempo esperado (≥ 0). | 10 | Igual. |
| `import.time_zone` | Zona horaria IANA de las horas del .spl (`Europe/Madrid`, `Atlantic/Canary`…). | `Europe/Madrid` | Solo en las carreras que se importen después: las ya guardadas tienen sus horas en UTC. Una carrera reimportada se reutiliza tal cual, así que para corregir su hora habría que borrarla antes (aún no se puede desde la app). |
| `self.si_card` | Tarjeta SI del usuario. | — | Rellena el formulario de importar. |
| `self.full_name` | Nombre y apellidos, tal y como los escribió. | — | Igual. |
| `self.person_id` | Id de la persona del usuario en `people` (no se edita). | — | A ella se vinculan sus resultados. |

Un valor guardado que no se entiende (número negativo, zona desconocida) se trata como si no
estuviera y toma el valor por defecto. El tiempo ideal sigue siendo la suma de referencias.

Con una zona horaria equivocada, el FIT no se solapa con la carrera y la alineación lo dice, con
la sugerencia de desplazamiento (`docs/alineacion.md`).

## Lista de carreras

Los resultados vinculados a la persona del usuario, de la carrera más reciente a la más antigua:
fecha, nombre de la carrera, categoría, puesto (o estado), formato, tiempo, tiempo perdido (con el
número de errores) y si tiene track del reloj. Una fila abre la vista de la carrera.

## Vista de carrera (P1)

- **Cabecera**: carrera, fecha, categoría, corredor, resultado, formato, balizas y clasificados
  del recorrido (con las categorías que lo comparten, si son varias).
- **Totales**: tiempo, tiempo perdido, tiempo sin errores, número de errores y rendimiento
  habitual. Aviso si la referencia es débil.
- **Tabla de tramos**: tramo, balizas (S = salida, M = meta), split, puesto en el tramo,
  referencia, IR, pérdida en segundos y en % y notas (error, último tramo, referencia corta). Los
  tramos con error van resaltados.

Los números salen de `tramos_core::runner_report::runner_report`, la misma función que usa
`tramos analizar` (`docs/cli.md`), sobre la carrera guardada: la tabla coincide con la de la CLI.
Los umbrales son los de los ajustes.

## Seguridad

La ventana tiene una CSP restrictiva (`docs/datos-y-privacidad.md`). Los permisos de la ventana
(`app/src-tauri/capabilities/default.json`) son los de `core:default` y `dialog:allow-open`, este
último solo para el diálogo de abrir ficheros.
