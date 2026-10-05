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
   GitHub. Mientras está en borrador no la ve nadie más.

Para probar el workflow sin publicar nada, se puede subir un tag de prueba con la versión actual
(por ejemplo `v0.1.0`) y, tras revisarla, borrar la release en borrador y el tag.

## Pendiente

- Firma del instalador (quitaría el aviso de SmartScreen): necesita un certificado.
- Actualizaciones automáticas: #40.
