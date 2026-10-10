# Datos y privacidad

## En la app

- Todo se guarda en local, en un SQLite por usuario. Los ficheros originales se conservan para
  poder recalcular.
- Al importar un .spl se descarta la fecha de nacimiento de los corredores.
- El contenido del .spl original guardado sí incluye las fechas de nacimiento (y los nombres de
  toda la prueba), así que los ficheros originales nunca salen de la base local: no van en los
  paquetes para la entrenadora ni a proveedores externos (ver `docs/almacenamiento.md`).
- Las personas (identidad de un corredor entre carreras) solo guardan lo que escribe el usuario:
  un nombre visible y notas. No llevan fecha de nacimiento ni se rellenan desde el .spl
  (ver `docs/almacenamiento.md`).
- Pulso y GPS se tratan como datos sensibles: cada corredor decide, carrera a carrera, qué comparte
  con la entrenadora (nada, agregados, tramos o track completo). Lo que lleva cada nivel está en
  `docs/paquete.md`. El paquete nunca lleva el .spl: con «tramos» va una copia reducida del
  recorrido (solo las categorías del corredor) con nombre, apellidos y club de cada corredor, que
  ya son públicos en los resultados de WinSplits (#118), pero sin dorsales, tarjetas ni sexo de
  nadie. El propio corredor va además con el nombre visible que escribió en la app y un
  identificador al azar.
- Los paquetes viajan por una **carpeta compartida** (Drive, OneDrive, Dropbox…, #36,
  `docs/paquete.md`): cualquiera con acceso a esa carpeta puede leerlos, y el proveedor de la
  sincronización los guarda en sus servidores. El modelo previsto (#140) es una carpeta madre de
  la entrenadora con una subcarpeta por atleta, compartida solo con ese atleta: cada uno deja
  los suyos en su subcarpeta y no ve los de los demás, y la entrenadora lee la carpeta madre
  entera. Si en cambio se comparte una carpeta común del grupo, todos los del grupo ven los
  paquetes de todos. Al dejar de compartir una carrera se borra su fichero de la
  carpeta, pero lo que la entrenadora ya hubiera importado sigue en su app.
- Quien entrena solo lee lo de sus atletas (`docs/app.md`, "Atletas"): mientras ve a un atleta, la
  app no tiene controles de edición y el núcleo rechaza cualquier cambio; los datos de un
  corredor solo cambian desde su propia app. Lo recibido se guarda en su base local y no sale de
  ella: si quien entrena también corre y comparte lo suyo, solo exporta sus propias carreras,
  nunca lo recibido de sus atletas, y no reimporta sus propios paquetes.
- Cuando llegue el LLM, a proveedores externos solo se envían agregados anónimos.
- La ventana de la app tiene una CSP restrictiva (`app/src-tauri/tauri.conf.json`), porque va a
  mostrar textos leídos de ficheros externos (nombres, clubes, categorías). Solo carga scripts,
  estilos e imágenes de la propia app, sin `unsafe-eval` ni `unsafe-inline`, y solo se comunica
  con el núcleo (IPC de Tauri: `ipc:` y `http://ipc.localhost`, este en Windows). La única
  excepción es el servidor de teselas del mapa, `https://tile.openstreetmap.org`, en `img-src` y
  `connect-src` (abajo); a ningún otro servidor externo se puede conectar. Los workers solo
  pueden venir de la propia app (`worker-src 'self'`). Tauri añade los hashes de los scripts y
  estilos en línea del HTML compilado.
  La CSP solo se aplica a la app compilada: en `npm run tauri dev` la página la sirve Vite
  directamente y Tauri no la inyecta (en escritorio no hace de proxy del servidor de desarrollo).
- **Mapa** (#20, `docs/app.md`). El track y las balizas se dibujan en local: no salen del
  equipo. Pero el fondo son teselas de OpenStreetMap que se piden a sus servidores
  (`tile.openstreetmap.org`) al abrir el mapa de una carrera, y **cada petición revela la zona y
  el zoom que se están mirando** (y la IP del equipo): quien gestione esos servidores puede
  deducir por dónde se ha corrido, aunque no el recorrido exacto ni el pulso. No se envía nada
  más (ni track, ni nombres, ni tiempos). Si una carrera no tiene track, no se pide ninguna
  tesela. Las teselas quedan en la caché del navegador del sistema según sus cabeceras.
- **Actualizaciones** (#40, `docs/distribucion.md`). Al arrancar, y al pulsar «Buscar
  actualizaciones» en Ajustes, la app pide `latest.json` a GitHub
  (`https://github.com/Mendana/tramos/releases/latest/download/latest.json`) y, si se acepta
  instalar, descarga el instalador de la misma release. La petición la hace el núcleo de Tauri
  (Rust), no la ventana, así que no pasa por la CSP. GitHub ve la IP del equipo y que se usa
  Tramos; no se envía ningún dato de carreras, ni nombres, ni la versión instalada en la URL.
- El enlace de la atribución de OSM se abre en el navegador del sistema; la app solo puede
  abrir URL de `https://www.openstreetmap.org/` y de los documentos del repositorio
  (`https://github.com/Mendana/tramos/blob/main/`), a los que enlaza la Ayuda. La Ayuda va dentro
  de la app y no pide nada a ningún servidor: solo esos enlaces, al pulsarlos, abren el navegador.

## En el repositorio (es público)

- Prohibido subir datos reales: .spl sin anonimizar, FIT, GPX o capturas con nombres.
- `fixtures/` contiene solo datos anonimizados o sintéticos, generados con las herramientas de
  `tools/`.
- `fixtures/private/` está en `.gitignore`: ahí van los datos reales para pruebas locales.
  Los tests que los usan deben saltarse (no fallar) si la carpeta no existe, para que la CI pase.
