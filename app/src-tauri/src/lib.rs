//! App de escritorio de Tramos. Expone el núcleo a la interfaz mediante comandos Tauri;
//! no reimplementa cálculos.

pub mod import;

use std::sync::{Mutex, MutexGuard};

use tauri::Manager;
use tramos_core::identify::RunnerIdentity;
use tramos_store::Store;

use crate::import::{ImportOutcome, ImportPreview, ImportRequest, RaceRow};

/// Nombre de la base de datos del usuario, en el directorio de datos de la app.
const DATABASE_FILE: &str = "tramos.sqlite";

/// Estado compartido por los comandos: la base de datos local.
struct AppState {
    store: Mutex<Store>,
}

impl AppState {
    fn store(&self) -> Result<MutexGuard<'_, Store>, String> {
        self.store
            .lock()
            .map_err(|_| "la base de datos quedó bloqueada por un error anterior".to_string())
    }
}

/// Devuelve la versión del núcleo (`tramos_core::VERSION`).
#[tauri::command]
fn core_version() -> &'static str {
    tramos_core::VERSION
}

/// Identidad del usuario guardada (tarjeta y nombre), para rellenar el formulario de importar.
#[tauri::command]
fn stored_identity(state: tauri::State<'_, AppState>) -> Result<RunnerIdentity, String> {
    import::stored_identity(&*state.store()?).map_err(|e| e.to_string())
}

/// Primer paso de importar: lee los ficheros y propone corredor y formato, sin guardar nada.
#[tauri::command]
fn preview_import(
    state: tauri::State<'_, AppState>,
    spl_path: String,
    fit_path: Option<String>,
    identity: RunnerIdentity,
) -> Result<ImportPreview, String> {
    import::preview(&*state.store()?, &spl_path, fit_path.as_deref(), &identity)
        .map_err(|e| e.to_string())
}

/// Segundo paso de importar: guarda la carrera con lo que ha confirmado el usuario.
#[tauri::command]
fn import_race(
    state: tauri::State<'_, AppState>,
    request: ImportRequest,
) -> Result<ImportOutcome, String> {
    import::import(&mut *state.store()?, &request).map_err(|e| e.to_string())
}

/// Carreras del usuario, de la más reciente a la más antigua.
#[tauri::command]
fn list_races(state: tauri::State<'_, AppState>) -> Result<Vec<RaceRow>, String> {
    import::list_races(&*state.store()?).map_err(|e| e.to_string())
}

/// Arranca la app. Devuelve el error de Tauri en lugar de abortar.
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let store = Store::open(dir.join(DATABASE_FILE))?;
            app.manage(AppState {
                store: Mutex::new(store),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_version,
            stored_identity,
            preview_import,
            import_race,
            list_races
        ])
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_version_is_the_core_crate_version() {
        assert_eq!(core_version(), tramos_core::VERSION);
    }
}
