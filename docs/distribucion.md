# Distribución

Los usuarios tienen sobre todo Windows y el desarrollo se hace en Linux: los instaladores de los
dos sistemas se generan en la CI (`.github/workflows/release.yml`, #39 y, Linux, #206).

## Instalar (usuarios)

1. Abre la última versión en [Releases](https://github.com/Mendana/tramos/releases).
2. Descarga el instalador `Tramos_X.Y.Z_x64-setup.exe` (o el `.msi`, si prefieres ese formato).
3. Ábrelo. Mientras el instalador no esté firmado, **Windows SmartScreen** avisa de que la
   aplicación no es de un editor reconocido: pulsa «Más información» y luego «Ejecutar de todas
   formas». Es lo esperado con una app de código abierto sin certificado de firma; el instalador
   sale de la CI de este repositorio a partir del código del tag.
4. Si el equipo no tiene WebView2 (Windows 10 antiguo), el instalador lo descarga.

En **Linux** (#206), de la misma release:

- **AppImage** (`Tramos_X.Y.Z_amd64.AppImage`), el recomendado: vale para casi cualquier
  distribución x86-64 sin instalar nada. Dale permiso de ejecución (`chmod +x`) y ábrelo; para
  tenerlo en el menú, una herramienta como Gear Lever o AppImageLauncher. Necesita FUSE 2
  (`libfuse2` en Debian y Ubuntu, `fuse2` en Arch). Se compila en Ubuntu 22.04, así que pide una
  glibc de esa época o más nueva.
- **`.deb`** (Debian, Ubuntu y derivadas) y **`.rpm`** (Fedora, openSUSE): se instalan con el
  gestor de paquetes y traen sus dependencias (WebKitGTK).

Los datos (la base de datos local con tus carreras) no están en la carpeta de la app sino en el
directorio de datos del usuario, así que instalar una versión nueva encima no los borra.

Solo hace falta instalar a mano la primera vez: desde la 0.2.0, las versiones siguientes llegan
solas (abajo, "Actualizaciones").

## Actualizaciones (#40)

La app usa el plugin de actualizaciones de Tauri (`tauri-plugin-updater`) contra las releases de
GitHub.

- **Cuándo busca.** Al arrancar, en segundo plano (no en `npm run tauri dev`), y cuando se pulsa
  «Buscar actualizaciones» en Ajustes, apartado «Acerca de Tramos». Al arrancar, si no hay red o falla, no
  dice nada; con el botón, enseña el error.
- **Qué consulta.** `https://github.com/Mendana/tramos/releases/latest/download/latest.json`
  (`plugins.updater.endpoints` de `tauri.conf.json`). `latest` es la última release **publicada**
  que no es pre-versión: los borradores no los ve nadie, así que una versión no llega a los
  usuarios hasta que se publica a mano (paso 5 de abajo).
- **Qué hace si la hay.** Un aviso arriba, en todas las pantallas: «Hay una versión nueva…», con
  **Instalar y reiniciar** y **Más tarde** (el aviso vuelve en el próximo arranque). Nada se
  descarga sin pulsar el botón. Al pulsarlo descarga el instalador, enseña el progreso y lo abre
  en modo `passive` (`plugins.updater.windows.installMode`): una ventana con la barra de progreso,
  sin preguntas. El instalador cierra la app y la vuelve a abrir con la versión nueva. Los datos
  no se tocan.
- **En Linux** la app sabe con qué se instaló (el plugin lo lleva compilado) y busca en
  `latest.json` lo suyo: `linux-x86_64-appimage` (o `linux-x86_64`), `-deb` o `-rpm`. El AppImage
  se sustituye a sí mismo, sin pedir nada; el `.deb` y el `.rpm` los instala el sistema y piden la
  contraseña de administrador (`pkexec`). Después la app se reinicia sola (`relaunch` en
  `app/src/updates.tsx`).
- **Qué trae.** El aviso y el apartado de Ajustes llevan un **Qué trae** desplegable con las notas
  de la versión nueva (#160): el campo `notes` de `latest.json`, que el plugin entrega como
  `body` de la actualización. Son la sección de esa versión del `CHANGELOG.md` (abajo, "Notas de
  versión"). Si la versión no trae notas, no sale el desplegable.
- **Firma.** Cada instalador va firmado con la clave de actualizaciones (minisign, no es un
  certificado de Windows: no quita el aviso de SmartScreen). La app lleva la clave pública
  (`plugins.updater.pubkey`) y rechaza cualquier instalador que no esté firmado con la privada.
  La privada y su contraseña están en los secretos del repositorio
  (`TAURI_SIGNING_PRIVATE_KEY` y `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`), que el workflow pasa a
  `tauri-action`; con ellas (y `bundle.createUpdaterArtifacts`) genera las firmas `.sig` y sube
  `latest.json` a la release, con una entrada por sistema e instalador.
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

## Notas de versión (#160)

`CHANGELOG.md`, en la raíz, cuenta en español y en llano lo que verá el usuario en cada versión
(no detalles técnicos). Una sección por versión con el título exacto `## X.Y.Z`, la más nueva
arriba, solo con párrafos y listas de `- ` (es lo único que pinta la app: `ReleaseNotes` en
`app/src/help/markdown.tsx`, sin HTML). Los cambios que aún no tienen versión van en
`## Sin publicar`; las instrucciones de mantenimiento están al principio del propio fichero.

El workflow de release (`tools/changelog_seccion.py`, con tests en
`tools/test_changelog_seccion.py`) saca la sección de la versión del tag y:

- la deja en la descripción de la release **delante** de las instrucciones de instalación, al
  crear el borrador (`gh api`);
- y la pone como `releaseBody` de `tauri-action`, que la copia tal cual en el `notes` de
  `latest.json`: lo que enseña la app antes de instalar.

Si el `CHANGELOG.md` no tiene sección para la versión del tag, o está vacía, el workflow falla
antes de compilar.

## Datos del instalador

Para que Windows no enseñe vacíos el editor y el copyright (en «Aplicaciones instaladas» y en las
propiedades del `.exe`), `app/src-tauri/tauri.conf.json` fija en `bundle`:

| Campo | Valor |
| --- | --- |
| `publisher` | Mendaña |
| `copyright` | © 2026 Mendaña. Licencia MIT |
| `homepage` | `https://github.com/Mendana/tramos` |
| `category` | `Sports` |
| `shortDescription` / `longDescription` | Descripción de la app, en español. |

`authors` de `app/src-tauri/Cargo.toml` también es «Mendaña». Son datos de la app, no secretos: no
dependen de la clave de actualizaciones ni del certificado de firma. Al cambiar de año, se
actualiza el año del copyright.

## Publicar una versión (desarrollo)

1. Escribe las notas: en `CHANGELOG.md`, cambia `## Sin publicar` por `## X.Y.Z` (revisa que lo
   que cuenta esté en llano y que no falte nada de lo que ha cambiado desde la versión anterior) y
   deja arriba un `## Sin publicar` vacío. Va en el mismo PR que sube la versión.
2. Sube la versión en los cuatro sitios, con el mismo número: `version` de `Cargo.toml` (workspace),
   `app/src-tauri/Cargo.toml`, `app/src-tauri/tauri.conf.json` y `app/package.json`. El MSI solo
   admite versiones numéricas (`0.2.0`; una pre-versión, como mucho numérica: `0.2.0-1`). El
   test `version_matches_manifest` de `crates/tramos-core/src/lib.rs` también lleva el número, y
   los `Cargo.lock` y `package-lock.json` cambian con él (`cargo check` y
   `npm install --package-lock-only`).
3. Mergea ese cambio en `main` y crea el tag desde `main`:

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

4. El workflow **Release**:
   - **Release en borrador**: comprueba que el tag coincide con la versión de `tauri.conf.json` y
     que el `CHANGELOG.md` tiene sección para ella (si no, falla sin compilar) y crea la **release
     en borrador** con las notas de la versión delante de las instrucciones de instalación. Si se
     repite con el mismo tag, reutiliza su borrador; si la release ya está publicada, falla.
   - **Instaladores**: compila con la acción oficial `tauri-apps/tauri-action` y sube a ese
     borrador (por su `releaseId`), primero en `windows-latest` (el `.exe` de NSIS y el `.msi`) y
     después en `ubuntu-22.04` (AppImage, `.deb` y `.rpm`). Van **uno detrás de otro**
     (`max-parallel: 1`): cada uno descarga `latest.json`, le añade su sistema y lo vuelve a subir,
     y a la vez el último borraría lo del otro.
5. Revisa la release (lee las notas; descarga e instala el `.exe` en un Windows y, si puedes, abre
   el AppImage en un Linux) y publícala a mano desde GitHub. Mientras está en borrador no la ve
   nadie más. Al publicarla, las instalaciones que ya existen la encuentran en su siguiente
   arranque. Comprueba antes que la release trae `latest.json` (con las notas en `notes` y las
   entradas `windows-x86_64` y `linux-x86_64`) y un `.sig` por instalador: sin ellos, nadie se
   actualiza.

Para probar el workflow sin publicar nada: en una rama, pon una versión de prueba en los cuatro
sitios (por ejemplo `0.4.1-1`, numérica por el MSI) con su sección en el `CHANGELOG.md`, sube el
tag `v0.4.1-1` desde esa rama y, tras revisar el borrador, bórralo junto con el tag. Con la
versión de una release ya publicada el workflow falla, para no tocarla.

## Pendiente

- Firma del instalador (quitaría el aviso de SmartScreen): necesita un certificado.
