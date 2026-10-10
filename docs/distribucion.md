# Distribución

Los usuarios tienen Windows y el desarrollo se hace en Linux: el instalador se genera en la CI
(`.github/workflows/release.yml`, #39).

## Instalar (usuarios)

1. Abre la última versión en [Releases](https://github.com/Mendana/tramos/releases).
2. Descarga el instalador `Tramos_X.Y.Z_x64-setup.exe` (o el `.msi`, si prefieres ese formato).
3. Ábrelo. Mientras el instalador no esté firmado, **Windows SmartScreen** avisa de que la
   aplicación no es de un editor reconocido: pulsa «Más información» y luego «Ejecutar de todas
   formas». Es lo esperado con una app de código abierto sin certificado de firma; el instalador
   sale de la CI de este repositorio a partir del código del tag.
4. Si el equipo no tiene WebView2 (Windows 10 antiguo), el instalador lo descarga.

Los datos (la base de datos local con tus carreras) no están en la carpeta de la app sino en el
directorio de datos del usuario, así que instalar una versión nueva encima no los borra.

Solo hace falta instalar a mano la primera vez: desde la 0.2.0, las versiones siguientes llegan
solas (abajo, "Actualizaciones").

## Actualizaciones (#40)

La app usa el plugin de actualizaciones de Tauri (`tauri-plugin-updater`) contra las releases de
GitHub.

- **Cuándo busca.** Al arrancar, en segundo plano (no en `npm run tauri dev`), y cuando se pulsa
  «Buscar actualizaciones» en Ajustes, apartado «Versión». Al arrancar, si no hay red o falla, no
  dice nada; con el botón, enseña el error.
- **Qué consulta.** `https://github.com/Mendana/tramos/releases/latest/download/latest.json`
  (`plugins.updater.endpoints` de `tauri.conf.json`). `latest` es la última release **publicada**
  que no es pre-versión: los borradores no los ve nadie, así que una versión no llega a los
  usuarios hasta que se publica a mano (paso 4 de abajo).
- **Qué hace si la hay.** Un aviso arriba, en todas las pantallas: «Hay una versión nueva…», con
  **Instalar y reiniciar** y **Más tarde** (el aviso vuelve en el próximo arranque). Nada se
  descarga sin pulsar el botón. Al pulsarlo descarga el instalador, enseña el progreso y lo abre
  en modo `passive` (`plugins.updater.windows.installMode`): una ventana con la barra de progreso,
  sin preguntas. El instalador cierra la app y la vuelve a abrir con la versión nueva. Los datos
  no se tocan.
- **Firma.** Cada instalador va firmado con la clave de actualizaciones (minisign, no es un
  certificado de Windows: no quita el aviso de SmartScreen). La app lleva la clave pública
  (`plugins.updater.pubkey`) y rechaza cualquier instalador que no esté firmado con la privada.
  La privada y su contraseña están en los secretos del repositorio
  (`TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), que el workflow pasa a
  `tauri-action`; con ellas (y `bundle.createUpdaterArtifacts`) genera las firmas `.sig` y sube
  `latest.json` a la release.
- **La clave privada no se puede perder.** Si se pierde, las instalaciones existentes no aceptarán
  ninguna versión firmada con otra clave: habría que reinstalar a mano en todos los equipos. Hay
  una copia fuera del equipo de desarrollo.
- **Compilar en local** el instalador (`npm run tauri build`) necesita la clave en el entorno,
  porque `createUpdaterArtifacts` firma los instaladores:

  ```bash
  export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/tramos.key)"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD='…'
  ```

  La CI no lo necesita: compila con `--no-bundle`.

## Publicar una versión (desarrollo)

1. Sube la versión en los cuatro sitios, con el mismo número: `version` de `Cargo.toml` (workspace),
   `app/src-tauri/Cargo.toml`, `app/src-tauri/tauri.conf.json` y `app/package.json`. El MSI solo
   admite versiones numéricas (`0.2.0`; una pre-versión, como mucho numérica: `0.2.0-1`).
2. Mergea ese cambio en `main` y crea el tag desde `main`:

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. El workflow **Release** comprueba que el tag coincide con la versión de `tauri.conf.json`
   (si no, falla sin compilar), compila en `windows-latest` con la acción oficial
   `tauri-apps/tauri-action` y deja una **release en borrador** con el `.exe` (NSIS) y el `.msi`.
4. Revisa la release (descarga e instala el `.exe` en un Windows) y publícala a mano desde
   GitHub. Mientras está en borrador no la ve nadie más. Al publicarla, las instalaciones que ya
   existen la encuentran en su siguiente arranque. Comprueba antes que la release trae
   `latest.json` y los `.sig`: sin ellos, nadie se actualiza.

Para probar el workflow sin publicar nada, se puede subir un tag de prueba con la versión actual
(por ejemplo `v0.1.0`) y, tras revisarla, borrar la release en borrador y el tag.

## Pendiente

- Firma del instalador (quitaría el aviso de SmartScreen): necesita un certificado.
