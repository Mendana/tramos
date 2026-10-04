# Datos y privacidad

## En la app

- Todo se guarda en local, en un SQLite por usuario. Los ficheros originales se conservan para
  poder recalcular.
- Al importar un .spl se descarta la fecha de nacimiento de los corredores.
- El contenido del .spl original guardado sí incluye las fechas de nacimiento (y los nombres de
  toda la prueba), así que los ficheros originales nunca salen de la base local: no van en los
  paquetes para la entrenadora ni a proveedores externos (ver `docs/almacenamiento.md`).
- Pulso y GPS se tratan como datos sensibles: cada corredor decide, carrera a carrera, qué comparte
  con la entrenadora (nada, agregados, tramos o track completo).
- La entrenadora solo lee; los datos de un corredor solo cambian desde su propia app.
- Cuando llegue el LLM, a proveedores externos solo se envían agregados anónimos.

## En el repositorio (es público)

- Prohibido subir datos reales: .spl sin anonimizar, FIT, GPX o capturas con nombres.
- `fixtures/` contiene solo datos anonimizados o sintéticos, generados con las herramientas de
  `tools/`.
- `fixtures/private/` está en `.gitignore`: ahí van los datos reales para pruebas locales.
  Los tests que los usan deben saltarse (no fallar) si la carpeta no existe, para que la CI pase.
