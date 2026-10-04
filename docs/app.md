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
- **Estructura**: barra lateral con Carreras, Importar y Ajustes; el contenido, centrado hasta
  1080 px. Por debajo de 860 px de ancho la barra lateral pasa arriba. La ventana abre a
  1180 × 780 (mínimo 760 × 520).

## Gráficas

Cada análisis se enseña en un **panel desplegable** (`app/src/charts/ChartPanel.tsx`) bajo la
vista de carrera (o, más adelante, en la vista histórica): título, número de casos en los que se
apoya, una frase que explica cómo leerlo y un selector **Gráfica / Tabla**. La tabla es la vista
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
  columna opcional y tooltip con el valor delante y el detalle detrás.
- `LineChart`: una línea de 2 px con un velo del 10 % hasta el 0, marcadores opcionales con un
  anillo del color de la superficie y un cursor vertical que se ajusta al punto más cercano.
- `common.tsx`: tamaño, rejilla con marcas, tooltip (al lado de la marca, para no taparla) y
  leyenda (cuadrado para barras, raya para líneas). Con dos o más series siempre hay leyenda.
- Dos medidas de escala distinta nunca comparten eje: van en paneles separados.

Colores de las gráficas (`tokens.css`): `--chart-series-1` es el acento, validado con la guía de
visualización contra la superficie de cada modo (en oscuro, un tono más oscuro que el acento de
la interfaz para quedar en la banda de luminosidad). `--chart-grid`, `--chart-axis` y
`--chart-reference` son la rejilla, la línea del 0 y la línea de referencia. `--chart-error`
(naranja de baliza) marca los tramos con error y `--chart-muted` (gris) el resto; validados igual,
se distinguen también con daltonismo.

Paneles de la vista de carrera:

| Panel | Qué enseña | Casos |
| --- | --- | --- |
| Pérdida por tramo (P1), abierto de entrada | Pérdida de cada tramo en segundos, hacia arriba si pierde y hacia abajo si gana; los tramos con error en naranja y el resto en gris. | Tramos con pérdida. |
| Pérdida acumulada (P3) | Tiempo perdido sumado tramo a tramo desde la salida: solo suben los tramos con error, marcados con un punto. Acaba en el tiempo perdido de la carrera. | Errores y tramos. |
| Rendimiento por tramo | IR de cada tramo como columna, con la línea del 100 % (la referencia). | Tramos con IR. |

Los paneles van entre las cifras destacadas y la tabla de tramos, y salen de los mismos tramos que
la tabla: sus valores coinciden con ella.

## Seguridad

La ventana tiene una CSP restrictiva (`docs/datos-y-privacidad.md`). Los permisos de la ventana
(`app/src-tauri/capabilities/default.json`) son los de `core:default` y `dialog:allow-open`, este
último solo para el diálogo de abrir ficheros.
