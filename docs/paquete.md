# Paquete por carrera

Unidad de intercambio corredor → entrenadora (#35): un fichero por corredor y carrera que el
corredor exporta y la entrenadora importa. Hoy viaja por una carpeta compartida (#36); el día que
haya servidor, el mismo paquete serviría de cuerpo de la petición. Implementado en
`tramos_core::package` (formato), `tramos-store` (paquetes recibidos) y la app (exportar e
importar, `app/src-tauri/src/package.rs`; carpeta compartida, `sharing.rs`).

## Fichero

- JSON en UTF-8. Nombre: `tramos-<race_id>-<runner_id>.json` (`package_file_name`), para que el
  mismo corredor y la misma carrera caigan siempre en el mismo fichero.
- `format`: siempre `"tramos-paquete"`. `version`: versión del formato, hoy **1**. Una app que
  recibe una versión mayor que la que conoce la rechaza (`UnsupportedVersion`) en vez de leerla
  a medias. Los cambios compatibles (campos nuevos opcionales) no suben la versión.

## Identificadores estables

- **`runner_id`**: identifica al corredor que exporta, no a una persona del .spl. Se genera una
  vez por base de datos (32 cifras hexadecimales al azar) y se guarda en los ajustes
  (`Store::package_runner_id`). No sale del nombre ni de la tarjeta, así que no revela nada y no
  cambia si el corredor corrige su nombre. Si el corredor empieza con una base nueva, tendrá otro
  `runner_id`.
- **`race_id`**: identifica la carrera y es **el mismo para todos los corredores** que la
  importaron, para poder cruzarlos (P15). Son las 32 primeras cifras hexadecimales del SHA-256 de
  la estructura de la carrera: fecha y, por cada categoría en orden de nombre, su nombre y las
  balizas de su recorrido (`race_id`). No usa tiempos ni nombres de corredores, así que un .spl
  descargado otra vez con tiempos corregidos da el mismo `race_id`; uno con categorías o
  recorridos distintos, otro.
- El paquete se identifica por el par **(`runner_id`, `race_id`)**.

## Nivel de permiso

El corredor decide, carrera a carrera, qué comparte (`docs/datos-y-privacidad.md`). «Nada» es no
exportar (`ShareChoice::Nothing`, `none`). Cada nivel incluye todo lo del anterior:

| Nivel (`level`) | Qué lleva |
| --- | --- |
| `aggregates` | Carrera (`race_id`, fecha, nombre, formato), corredor (`runner_id` y el nombre visible que el corredor escribió para sí mismo en la app) y resumen (`summary`): categoría, estado, puesto, tiempo, tiempo perdido, errores, tiempo sin errores, rendimiento habitual y consistencia, calculados con los umbrales del corredor (`config`) y la versión del núcleo (`core_version`). |
| `legs` | Además, los **originales del recorrido** (`course`) y las **etiquetas** de los tramos (`tags`, con su versión de taxonomía). |
| `track` | Además, el **track del reloj** (`track`: puntos con hora, posición, altitud, pulso, cadencia y distancia) y el desfase manual, si lo hay. |

**Originales del recorrido.** Para que la app de la entrenadora pueda recalcular el tiempo perdido
con su propia versión del algoritmo hacen falta los splits de todo el recorrido (la referencia sale
de todos los corredores). El paquete no lleva el .spl, que nunca sale de la base local, sino una
copia **reducida** del modelo (`course.event`):

- Solo las categorías que comparten el recorrido del corredor.
- De cada corredor (también del propio) quedan nombre, apellidos, club, estado, puesto y picadas:
  lo mismo que publican los resultados de WinSplits, para que la entrenadora reconozca a los
  rivales y a sus otros atletas (#118). Dorsal, tarjeta y sexo van a `null`. Fechas de nacimiento
  no hay: el modelo nunca las tiene.
- `course.result` dice cuál de esos resultados es el del corredor.

Los paquetes de antes de #118 traen nombre y apellidos vacíos y club `null`. Es el mismo formato
(los campos ya existían), así que `version` sigue siendo 1 y la app destino lee los dos: a quien
venga sin nombre lo llama «Corredor 1», «Corredor 2»… Un paquete viejo se rehace con nombres en
cuanto el corredor vuelve a exportar (al guardar los Ajustes con carpeta, o al cambiar algo de
esa carrera).

Con un paquete `legs` o `track`, la app destino recalcula a partir de `course` con su versión del
algoritmo; el `summary` es lo que vio el corredor. Con `aggregates` solo tiene el `summary`.

## Reglas de lectura (`RacePackage::parse`)

- `format` distinto, JSON inválido o `version` mayor que la conocida: error.
- El contenido tiene que cuadrar con el nivel: `aggregates` sin `course` ni `track`; `legs` con
  `course` y sin `track`; `track` con los dos. Un `course.result` que no apunta a ningún resultado
  también es error.

## Importar (`tramos-store`, tabla `received_packages`)

- Se guarda el paquete entero (el JSON tal cual) con su par de identificadores, nivel, versión e
  instante de exportación.
- **Reexportar sustituye al anterior.** Como mucho hay un paquete por (`runner_id`, `race_id`):
  importar otro del mismo par lo sustituye si es igual de reciente o más (`exported_at`), y se
  ignora si es más antiguo, para que una copia vieja que llegue tarde por la carpeta no pise la
  buena. Uno idéntico al guardado no cambia nada. Reimportar nunca duplica.
- Sustituir vale también para bajar de nivel: si el corredor reexporta con `aggregates` una
  carrera que había compartido con `track`, el track desaparece de la app de la entrenadora.
- Qué hace la app de quien entrena con los paquetes (Mis atletas, novedades y vistas en solo
  lectura) está en `docs/app.md`, "Atletas" (#37, #119).

## Carpeta compartida (#36)

Sin servidor: el corredor y quien le entrena comparten carpetas sincronizadas (Drive, OneDrive,
Dropbox…) con esta estructura (#140): quien entrena crea una **carpeta madre** y dentro una
**subcarpeta por atleta**, compartida solo con ese atleta. Cada atleta elige en su app su
subcarpeta y exporta ahí; quien entrena elige la carpeta madre y su app lee los paquetes de ella
y de todas sus subcarpetas. Así un atleta no ve lo de los demás y quien entrena no tiene que
añadir una carpeta por atleta. Cada app la tiene en sus ajustes (`sharing.folder`, `docs/app.md`, "Ajustes") junto
con dos casillas independientes (#119): **«Compartir mis carreras»** (`sharing.share_own`)
exporta y **«Entreno a otros atletas»** (`athletes.enabled`) recibe. Quien entrena y también
corre marca las dos y usa **la misma carpeta** para las dos cosas (la carpeta madre: sus
paquetes quedan en su raíz y al leer se saltan). Implementado en
`app/src-tauri/src/sharing.rs`.

**Qué se comparte de cada carrera.** Lo elegido para esa carrera en la vista de carrera
(tabla `result_sharing`, `docs/almacenamiento.md`) o, si no se ha elegido nada, el ajuste
`sharing.default_choice`, que por defecto es `legs` (tramos y etiquetas, sin pulso ni GPS). Si
se elige `track` para una carrera sin track, se comparten los tramos.

**Exportar.** Si comparte lo suyo y hay carpeta, cada carrera del usuario se exporta sola
(`export_result`) cuando cambia algo de lo que lleva el paquete: al importarla (una o una carpeta
entera), al etiquetar un tramo, al cambiar el formato o el desfase del reloj y al cambiar qué se
comparte de ella. Al guardar los ajustes con carpeta se exportan todas (`share_all`), porque los
umbrales cambian el resumen.

- Se escribe primero un temporal oculto (`.tramos-….json.tmp`) y se renombra, para que quien lea
  la carpeta nunca encuentre un paquete a medias.
- Si el fichero ya está igual salvo el instante de exportación, **no se reescribe**: así la
  sincronización no lo vuelve a subir y quien entrena no lo vuelve a leer.
- Si la carrera pasa a «nada», **se borra su fichero** de la carpeta. Lo que quien entrena ya
  hubiera importado se queda en su app.
- Un fallo al exportar (carpeta que no está, sin permiso) no deshace el cambio que lo provocó: la
  vista de carrera lo muestra y se reintenta al abrirla o al volver a cambiar algo.
- Una carrera que no se puede analizar (sin picadas) no se exporta.

**Recibir.** Si entrena y hay carpeta, la app importa los paquetes de la
carpeta (`receive`) al arrancar, al guardar los ajustes y **cada minuto** mientras está abierta.
No usa avisos del sistema de ficheros: las carpetas sincronizadas no siempre los dan bien y
mirar cada minuto basta.

- Lee la carpeta y **sus subcarpetas, hasta 3 niveles por debajo** (carpeta madre, atleta y, por
  ejemplo, temporada); lo que esté más hondo no se lee. No entra en carpetas ocultas (las que
  empiezan por `.`) ni **sigue enlaces simbólicos**, ni a carpetas ni a ficheros. Una subcarpeta
  que no se puede leer se anota entre los problemas y no impide leer el resto; si no se puede leer
  la carpeta elegida, es un error.
- Solo lee los ficheros `tramos-*.json`; el resto se ignora. Tampoco lee los suyos, estén en la
  subcarpeta que estén: los que terminan en su propio `runner_id`
  (`tramos-<race_id>-<runner_id>.json`), que son las carreras propias que exporta esta misma app.
- **Ficheros repetidos.** Un paquete se identifica por (`runner_id`, `race_id`), no por su
  ruta. Si hay varios ficheros del mismo corredor y carrera (una copia en otra subcarpeta, un
  paquete que un atleta ha movido), se queda **el exportado más recientemente** (`exported_at`, en
  segundos), sea cual sea su carpeta y el orden en que se lean; los demás cuentan como «sin cambios». Con el
  mismo `exported_at` y distinto contenido, el último en orden de ruta sustituye al anterior.
- Recuerda (por ruta completa) la fecha de modificación y el tamaño de cada fichero importado y no vuelve a leer los
  que no cambian (un paquete con track puede pesar varios MB). Al reabrir la app los lee todos
  una vez; los que no aportan nada salen como «sin cambios».
- Un fichero que no se puede importar (JSON roto, versión más nueva) no impide importar los demás
  y se vuelve a intentar la vez siguiente.
- Si no entrena, no se importa nada de la carpeta, aunque haya paquetes de otros.
