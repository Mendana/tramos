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
| `stored_identity` | Tarjeta SI y nombre del usuario guardados, para rellenar el formulario. |
| `preview_import(splPath, fitPath, identity)` | Primer paso de importar: lee los ficheros sin guardar nada. |
| `import_race(request)` | Segundo paso: guarda la carrera con lo que ha confirmado el usuario. |
| `list_races` | Carreras del usuario, de la más reciente a la más antigua. |

Los errores llegan a la interfaz como texto en español. La lógica está en
`app/src-tauri/src/import.rs`, en Rust sin Tauri, y se prueba con los fixtures (`cargo test` en
`app/src-tauri`).

## Importar una carrera

1. **Ficheros**: el usuario arrastra a la ventana el .spl y, si lo tiene, el .fit, o los elige
   con el diálogo del sistema (`tauri-plugin-dialog`). Se distinguen por la extensión.
2. **Identidad**: tarjeta SI y nombre y apellidos, rellenados con los de la última importación.
3. **Revisar** (`preview_import`): lee y valida los dos ficheros, identifica al corredor
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
   - Se guardan la tarjeta y el nombre para la próxima vez.
   - Con FIT, se alinea con las picadas del resultado (`docs/alineacion.md`). Si se puede, se
     guardan el FIT original y el track, y se muestran el desfase, la confianza y los avisos. Si
     no (track de otra hora o de otra carrera), **no se guarda el track** y se muestra el error,
     con la sugerencia de desplazamiento si la hay. La carrera sí queda importada. Reimportar con
     otro FIT sustituye el track.
5. La lista de carreras se actualiza.

El análisis (tiempo perdido, tramos, métricas) no se guarda al importar: se calcula al mostrarlo
a partir de la carrera y el track guardados.

## Ajustes que usa

| Clave | Valor |
| --- | --- |
| `self.person_id` | Id de la persona del usuario en `people`. |
| `self.si_card` | Su tarjeta SI. |
| `self.full_name` | Su nombre y apellidos, tal y como los escribió. |

## Lista de carreras

Los resultados vinculados a la persona del usuario, de la carrera más reciente a la más antigua:
fecha, nombre de la carrera, categoría, puesto (o estado), formato y si tiene track del reloj.

## Seguridad

La ventana tiene una CSP restrictiva (`docs/datos-y-privacidad.md`). Los permisos de la ventana
(`app/src-tauri/capabilities/default.json`) son los de `core:default` y `dialog:allow-open`, este
último solo para el diálogo de abrir ficheros.
