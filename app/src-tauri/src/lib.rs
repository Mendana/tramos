//! App de escritorio de Tramos. Expone el núcleo a la interfaz mediante comandos Tauri;
//! no reimplementa cálculos.

pub mod batch;
pub mod clock_offset;
pub mod history;
pub mod import;
pub mod race_map;
pub mod races;
pub mod settings;
pub mod tags;

use std::sync::{Mutex, MutexGuard};

use tauri::Manager;
use tramos_core::comparison::CourseComparison;
use tramos_core::history::HistoryFilter;
use tramos_core::identify::RunnerIdentity;
use tramos_core::loss_breakdown::RaceBreakdown;
use tramos_core::race_format::RaceFormat;
use tramos_core::taxonomy::{LegTag, Taxonomy};
use tramos_store::Store;

use crate::batch::BatchSummary;
use crate::clock_offset::OffsetView;
use crate::history::HistoryView;
use crate::import::{ImportOutcome, ImportPreview, ImportRequest};
use crate::race_map::RaceMap;
use crate::races::{RaceDetail, RaceRow};
use crate::settings::Settings;
use crate::tags::TagView;

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

/// Ajustes del usuario (umbrales, zona horaria e identidad).
#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Result<Settings, String> {
    settings::load(&*state.store()?).map_err(|e| e.to_string())
}

/// Valida y guarda los ajustes; si alguno no vale, no se guarda ninguno.
#[tauri::command]
fn save_settings(state: tauri::State<'_, AppState>, settings: Settings) -> Result<(), String> {
    settings::save(&mut *state.store()?, &settings).map_err(|e| e.to_string())
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

/// Importa todas las carreras de una carpeta (.spl y FIT) y devuelve el resumen. Es asíncrono
/// para no bloquear la ventana mientras alinea los FIT, que con una temporada lleva un rato.
#[tauri::command]
async fn import_folder(
    state: tauri::State<'_, AppState>,
    folder_path: String,
) -> Result<BatchSummary, String> {
    batch::import_folder(&mut *state.store()?, &folder_path).map_err(|e| e.to_string())
}

/// Carreras del usuario, de la más reciente a la más antigua, con su tiempo perdido.
#[tauri::command]
fn list_races(state: tauri::State<'_, AppState>) -> Result<Vec<RaceRow>, String> {
    races::list_races(&*state.store()?).map_err(|e| e.to_string())
}

/// Una carrera del usuario con su tabla de tramos.
#[tauri::command]
fn race_detail(state: tauri::State<'_, AppState>, result_id: i64) -> Result<RaceDetail, String> {
    races::race_detail(&*state.store()?, result_id).map_err(|e| e.to_string())
}

/// Corredores del recorrido de un resultado, para compararse con ellos (P4).
#[tauri::command]
fn race_comparison(
    state: tauri::State<'_, AppState>,
    result_id: i64,
) -> Result<CourseComparison, String> {
    races::race_comparison(&*state.store()?, result_id).map_err(|e| e.to_string())
}

/// Cambia el formato de la carrera de un resultado (`null` = sin formato).
#[tauri::command]
fn set_race_format(
    state: tauri::State<'_, AppState>,
    result_id: i64,
    format: Option<RaceFormat>,
) -> Result<(), String> {
    races::set_race_format(&mut *state.store()?, result_id, format).map_err(|e| e.to_string())
}

/// ¿Lento o desorientado? (P2) de un resultado; `null` sin track.
#[tauri::command]
fn race_breakdown(
    state: tauri::State<'_, AppState>,
    result_id: i64,
) -> Result<Option<RaceBreakdown>, String> {
    races::race_breakdown(&*state.store()?, result_id).map_err(|e| e.to_string())
}

/// Desfase entre el reloj y el cronometraje de un resultado: el calculado, su confianza, la
/// sugerencia de ±1/2 h y el fijado a mano; `null` sin track.
#[tauri::command]
fn race_offset(
    state: tauri::State<'_, AppState>,
    result_id: i64,
) -> Result<Option<OffsetView>, String> {
    clock_offset::race_offset(&*state.store()?, result_id).map_err(|e| e.to_string())
}

/// Fija el desfase de un resultado a mano (`null` = automático) y devuelve cómo queda.
#[tauri::command]
fn set_race_offset(
    state: tauri::State<'_, AppState>,
    result_id: i64,
    offset_s: Option<f64>,
) -> Result<OffsetView, String> {
    clock_offset::set_race_offset(&mut *state.store()?, result_id, offset_s)
        .map_err(|e| e.to_string())
}

/// Mapa de un resultado: track coloreado por ritmo o pulso, tramos y balizas.
#[tauri::command]
fn race_map(state: tauri::State<'_, AppState>, result_id: i64) -> Result<RaceMap, String> {
    race_map::race_map(&*state.store()?, result_id).map_err(|e| e.to_string())
}

/// Taxonomía de errores con la que se etiqueta (`docs/taxonomia.md`).
#[tauri::command]
fn taxonomy() -> Result<Taxonomy, String> {
    tags::taxonomy().map_err(|e| e.to_string())
}

/// Etiquetas de los tramos de un resultado.
#[tauri::command]
fn leg_tags(state: tauri::State<'_, AppState>, result_id: i64) -> Result<Vec<TagView>, String> {
    tags::leg_tags(&*state.store()?, result_id).map_err(|e| e.to_string())
}

/// Guarda la etiqueta de un tramo; vacía, la borra (`null`).
#[tauri::command]
fn save_leg_tag(
    state: tauri::State<'_, AppState>,
    result_id: i64,
    leg_index: usize,
    tag: LegTag,
) -> Result<Option<TagView>, String> {
    tags::save_leg_tag(&mut *state.store()?, result_id, leg_index, tag).map_err(|e| e.to_string())
}

/// Histórico de las carreras del usuario por formato (P6), con filtros de fechas y formato.
#[tauri::command]
fn history(
    state: tauri::State<'_, AppState>,
    filter: HistoryFilter,
) -> Result<HistoryView, String> {
    history::history_view(&*state.store()?, &filter).map_err(|e| e.to_string())
}

/// Arranca la app. Devuelve el error de Tauri en lugar de abortar.
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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
            get_settings,
            save_settings,
            preview_import,
            import_race,
            import_folder,
            list_races,
            race_detail,
            race_comparison,
            race_breakdown,
            set_race_format,
            race_offset,
            set_race_offset,
            race_map,
            taxonomy,
            leg_tags,
            save_leg_tag,
            history
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
