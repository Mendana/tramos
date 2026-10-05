# Paquete por carrera

Unidad de intercambio corredor → entrenadora (#35): un fichero por corredor y carrera que el
corredor exporta y la entrenadora importa. Hoy viaja por una carpeta compartida (#36); el día que
haya servidor, el mismo paquete serviría de cuerpo de la petición. Implementado en
`tramos_core::package` (formato), `tramos-store` (paquetes recibidos) y la app (exportar e
importar, `app/src-tauri/src/package.rs`).

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
exportar. Cada nivel incluye todo lo del anterior:

| Nivel (`level`) | Qué lleva |
| --- | --- |
| `aggregates` | Carrera (`race_id`, fecha, nombre, formato), corredor (`runner_id` y el nombre visible que el corredor escribió para sí mismo en la app) y resumen (`summary`): categoría, estado, puesto, tiempo, tiempo perdido, errores, tiempo sin errores, rendimiento habitual y consistencia, calculados con los umbrales del corredor (`config`) y la versión del núcleo (`core_version`). |
| `legs` | Además, los **originales del recorrido** (`course`) y las **etiquetas** de los tramos (`tags`, con su versión de taxonomía). |
| `track` | Además, el **track del reloj** (`track`: puntos con hora, posición, altitud, pulso, cadencia y distancia) y el desfase manual, si lo hay. |

**Originales del recorrido.** Para que la app de la entrenadora pueda recalcular el tiempo perdido
con su propia versión del algoritmo hacen falta los splits de todo el recorrido (la referencia sale
de todos los corredores). El paquete no lleva el .spl, que nunca sale de la base local, sino una
copia **reducida y anonimizada** del modelo (`course.event`):

- Solo las categorías que comparten el recorrido del corredor.
- De cada corredor quedan el estado, el puesto y las picadas. Nombre y apellidos van vacíos y
  club, dorsal, tarjeta y sexo, a `null`, **también los del propio corredor** (su nombre es el
  visible de `runner`). Fechas de nacimiento no hay: el modelo nunca las tiene.
- `course.result` dice cuál de esos resultados es el del corredor.

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
  buena. Reimportar nunca duplica.
- Sustituir vale también para bajar de nivel: si el corredor reexporta con `aggregates` una
  carrera que había compartido con `track`, el track desaparece de la app de la entrenadora.
- Qué hace la app de la entrenadora con los paquetes (vistas, selector de corredor) es #37.
