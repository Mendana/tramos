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
  con la entrenadora (nada, agregados, tramos o track completo).
- La entrenadora solo lee; los datos de un corredor solo cambian desde su propia app.
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
- El enlace de la atribución de OSM se abre en el navegador del sistema; la app solo puede
  abrir URL de `https://www.openstreetmap.org/`.

## En el repositorio (es público)

- Prohibido subir datos reales: .spl sin anonimizar, FIT, GPX o capturas con nombres.
- `fixtures/` contiene solo datos anonimizados o sintéticos, generados con las herramientas de
  `tools/`.
- `fixtures/private/` está en `.gitignore`: ahí van los datos reales para pruebas locales.
  Los tests que los usan deben saltarse (no fallar) si la carpeta no existe, para que la CI pase.
