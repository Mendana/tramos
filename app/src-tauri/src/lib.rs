//! App de escritorio de Tramos. Expone el núcleo a la interfaz mediante comandos Tauri;
//! no reimplementa cálculos.

pub mod batch;
pub mod clock_offset;
pub mod coach;
pub mod history;
pub mod import;
pub mod package;
pub mod race_map;
pub mod races;
pub mod settings;
pub mod sharing;
pub mod tags;
pub mod zones;

use std::sync::{Mutex, MutexGuard};

use tauri::Manager;
use tramos_core::comparison::CourseComparison;
use tramos_core::history::HistoryFilter;
use tramos_core::identify::RunnerIdentity;
use tramos_core::loss_breakdown::RaceBreakdown;
use tramos_core::package::{ShareChoice, ShareLevel};
use tramos_core::race_format::RaceFormat;
use tramos_core::taxonomy::{LegTag, Taxonomy};
use tramos_store::{SaveOutcome, Store};

use crate::batch::BatchSummary;
use crate::clock_offset::OffsetView;
use crate::coach::{CoachRunner, GroupView, RunnerView, RunnerViewInfo};
use crate::history::HistoryView;
use crate::import::{ImportOutcome, ImportPreview, ImportRequest};
use crate::race_map::RaceMap;
use crate::races::{RaceDetail, RaceRow};
use crate::settings::{AppMode, Settings};
use crate::sharing::{RaceSharing, ReceiveReport, SeenFiles, ShareReport};
use crate::tags::TagView;

/// Nombre de la base de datos del usuario, en el directorio de datos de la app.
const DATABASE_FILE: &str = "tramos.sqlite";

/// Error de los comandos que modifican algo en modo entrenadora.
const READ_ONLY: &str = "en modo entrenadora no se puede modificar nada";

/// Estado compartido por los comandos: la base de datos local, los ficheros de la carpeta
/// compartida ya importados y, en modo entrenadora, el corredor que se está viendo.
struct AppState {
    store: Mutex<Store>,
    seen: Mutex<SeenFiles>,
    viewed: Mutex<Option<RunnerView>>,
}

impl AppState {
    fn store(&self) -> Result<MutexGuard<'_, Store>, String> {
        self.store
            .lock()
            .map_err(|_| "la base de datos quedó bloqueada por un error anterior".to_string())
    }

    fn viewed(&self) -> Result<MutexGuard<'_, Option<RunnerView>>, String> {
        self.viewed
            .lock()
            .map_err(|_| "el corredor quedó bloqueado por un error anterior".to_string())
    }

    /// Lee de la base que muestran las vistas de corredor: la del usuario o, en modo
    /// entrenadora, la del corredor que se está viendo (vacía si no hay ninguno).
    fn read<T, E: ToString>(&self, f: impl FnOnce(&Store) -> Result<T, E>) -> Result<T, String> {
        let coach = coach::is_coach(&*self.store()?).map_err(|e| e.to_string())?;
        if !coach {
            return f(&*self.store()?).map_err(|e| e.to_string());
        }
        let viewed = self.viewed()?;
        match viewed.as_ref() {
            Some(view) => f(&view.store),
            None => f(&Store::open_in_memory().map_err(|e| e.to_string())?),
        }
        .map_err(|e| e.to_string())
    }

    /// La base del usuario para modificarla. En modo entrenadora, error: solo lee.
    fn write(&self) -> Result<MutexGuard<'_, Store>, String> {
        let store = self.store()?;
        if coach::is_coach(&store).map_err(|e| e.to_string())? {
            return Err(READ_ONLY.to_string());
        }
        Ok(store)
    }
}

/// Tras cambiar algo de un resultado, lo exporta a la carpeta compartida si toca. Un fallo no
/// deshace el cambio: la vista de carrera lo muestra (`race_sharing`).
fn reshare(store: &mut Store, result_id: i64) {
    let _ = sharing::export_result(store, result_id);
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
    settings::save(&mut *state.store()?, &settings).map_err(|e| e.to_string())?;
    if settings.sharing.mode != AppMode::Coach {
        *state.viewed()? = None;
    }
    Ok(())
}

/// Qué falla en unas zonas del mapa (lo que impide guardarlas) y los avisos sobre sus colores,
/// para enseñarlos mientras se editan.
#[tauri::command]
fn check_zones(zones: zones::Zones) -> Vec<String> {
    match zones.problem() {
        Some(problem) => vec![format!("No se pueden guardar: {problem}.")],
        None => zones.warnings(),
    }
}

/// Si ya se ha elegido el modo (corredor o entrenadora). Al instalar, no: la app lo pregunta.
#[tauri::command]
fn mode_chosen(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    settings::mode_chosen(&*state.store()?).map_err(|e| e.to_string())
}

/// Elige el modo al empezar, sin tocar los demás ajustes.
#[tauri::command]
fn choose_mode(state: tauri::State<'_, AppState>, mode: AppMode) -> Result<(), String> {
    settings::choose_mode(&mut *state.store()?, mode).map_err(|e| e.to_string())?;
    *state.viewed()? = None;
    Ok(())
}

/// En modo entrenadora, corredores de los que hay paquetes.
#[tauri::command]
fn coach_runners(state: tauri::State<'_, AppState>) -> Result<Vec<CoachRunner>, String> {
    coach::coach_runners(&*state.store()?).map_err(|e| e.to_string())
}

/// En modo entrenadora, vista de grupo (P15): una fila por corredor y todos contra todos.
#[tauri::command]
fn group_view(
    state: tauri::State<'_, AppState>,
    filter: HistoryFilter,
) -> Result<GroupView, String> {
    let store = state.store()?;
    if !coach::is_coach(&store).map_err(|e| e.to_string())? {
        return Err("la vista de grupo es del modo entrenadora".to_string());
    }
    coach::group_view(&store, &filter).map_err(|e| e.to_string())
}

/// En modo entrenadora, el corredor que se está viendo, sin volver a volcarlo.
#[tauri::command]
fn viewed_runner(state: tauri::State<'_, AppState>) -> Result<Option<RunnerViewInfo>, String> {
    Ok(state.viewed()?.as_ref().map(RunnerView::info))
}

/// En modo entrenadora, elige el corredor que se ve (`null` = ninguno). A partir de ahí las
/// vistas de corredor muestran sus carreras. Devuelve lo que no sale en ellas.
#[tauri::command]
fn view_runner(
    state: tauri::State<'_, AppState>,
    runner_id: Option<String>,
) -> Result<Option<RunnerViewInfo>, String> {
    let store = state.store()?;
    if !coach::is_coach(&store).map_err(|e| e.to_string())? {
        return Err("solo en modo entrenadora se ven otros corredores".to_string());
    }
    let view = runner_id
        .map(|id| coach::runner_view(&store, &id))
        .transpose()
        .map_err(|e| e.to_string())?;
    let info = view.as_ref().map(RunnerView::info);
    *state.viewed()? = view;
    Ok(info)
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
    let mut store = state.write()?;
    let outcome = import::import(&mut store, &request).map_err(|e| e.to_string())?;
    reshare(&mut store, outcome.result_id);
    Ok(outcome)
}

/// Importa todas las carreras de una carpeta (.spl y FIT) y devuelve el resumen. Es asíncrono
/// para no bloquear la ventana mientras alinea los FIT, que con una temporada lleva un rato.
#[tauri::command]
async fn import_folder(
    state: tauri::State<'_, AppState>,
    folder_path: String,
) -> Result<BatchSummary, String> {
    let mut store = state.write()?;
    let summary = batch::import_folder(&mut store, &folder_path).map_err(|e| e.to_string())?;
    let _ = sharing::share_all(&mut store);
    Ok(summary)
}

/// Carreras del usuario, de la más reciente a la más antigua, con su tiempo perdido.
#[tauri::command]
fn list_races(state: tauri::State<'_, AppState>) -> Result<Vec<RaceRow>, String> {
    state.read(races::list_races)
}

/// Una carrera del usuario con su tabla de tramos.
#[tauri::command]
fn race_detail(state: tauri::State<'_, AppState>, result_id: i64) -> Result<RaceDetail, String> {
    state.read(|s| races::race_detail(s, result_id))
}

/// Corredores del recorrido de un resultado, para compararse con ellos (P4).
#[tauri::command]
fn race_comparison(
    state: tauri::State<'_, AppState>,
    result_id: i64,
) -> Result<CourseComparison, String> {
    state.read(|s| races::race_comparison(s, result_id))
}

/// Cambia el formato de la carrera de un resultado (`null` = sin formato).
#[tauri::command]
fn set_race_format(
    state: tauri::State<'_, AppState>,
    result_id: i64,
    format: Option<RaceFormat>,
) -> Result<(), String> {
    let mut store = state.write()?;
    races::set_race_format(&mut store, result_id, format).map_err(|e| e.to_string())?;
    reshare(&mut store, result_id);
    Ok(())
}

/// ¿Lento o desorientado? (P2) de un resultado; `null` sin track.
#[tauri::command]
fn race_breakdown(
    state: tauri::State<'_, AppState>,
    result_id: i64,
) -> Result<Option<RaceBreakdown>, String> {
    state.read(|s| races::race_breakdown(s, result_id))
}

/// Desfase entre el reloj y el cronometraje de un resultado: el calculado, su confianza, la
/// sugerencia de ±1/2 h y el fijado a mano; `null` sin track.
#[tauri::command]
fn race_offset(
    state: tauri::State<'_, AppState>,
    result_id: i64,
) -> Result<Option<OffsetView>, String> {
    state.read(|s| clock_offset::race_offset(s, result_id))
}

/// Fija el desfase de un resultado a mano (`null` = automático) y devuelve cómo queda.
#[tauri::command]
fn set_race_offset(
    state: tauri::State<'_, AppState>,
    result_id: i64,
    offset_s: Option<f64>,
) -> Result<OffsetView, String> {
    let mut store = state.write()?;
    let view = clock_offset::set_race_offset(&mut store, result_id, offset_s)
        .map_err(|e| e.to_string())?;
    reshare(&mut store, result_id);
    Ok(view)
}

/// Mapa de un resultado: track coloreado por ritmo o pulso, tramos y balizas.
#[tauri::command]
fn race_map(state: tauri::State<'_, AppState>, result_id: i64) -> Result<RaceMap, String> {
    state.read(|s| race_map::race_map(s, result_id))
}

/// Exporta el paquete de una carrera del usuario a una carpeta (`docs/paquete.md`) con el nivel
/// de permiso elegido. Devuelve la ruta del fichero.
#[tauri::command]
fn export_race_package(
    state: tauri::State<'_, AppState>,
    result_id: i64,
    level: ShareLevel,
    folder_path: String,
) -> Result<String, String> {
    package::export_package(&mut *state.write()?, result_id, level, &folder_path)
        .map_err(|e| e.to_string())
}

/// Importa el paquete de otro corredor: `created`, `replaced`, `unchanged` o `ignored_older`.
#[tauri::command]
fn import_race_package(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<&'static str, String> {
    let outcome =
        package::import_package(&mut *state.store()?, &path).map_err(|e| e.to_string())?;
    Ok(match outcome {
        SaveOutcome::Created => "created",
        SaveOutcome::Replaced => "replaced",
        SaveOutcome::Unchanged => "unchanged",
        SaveOutcome::IgnoredOlder => "ignored_older",
    })
}

/// Exporta la carrera a la carpeta compartida si hace falta y dice cómo queda.
#[tauri::command]
fn race_sharing(state: tauri::State<'_, AppState>, result_id: i64) -> Result<RaceSharing, String> {
    sharing::race_sharing(&mut *state.store()?, result_id).map_err(|e| e.to_string())
}

/// Cambia lo que se comparte de una carrera (`null` = lo de por defecto) y dice cómo queda.
#[tauri::command]
fn set_race_sharing(
    state: tauri::State<'_, AppState>,
    result_id: i64,
    choice: Option<ShareChoice>,
) -> Result<RaceSharing, String> {
    sharing::set_race_sharing(&mut *state.write()?, result_id, choice).map_err(|e| e.to_string())
}

/// Exporta todas las carreras del usuario a la carpeta compartida.
#[tauri::command]
fn share_all(state: tauri::State<'_, AppState>) -> Result<ShareReport, String> {
    sharing::share_all(&mut *state.write()?).map_err(|e| e.to_string())
}

/// En modo entrenadora, importa los paquetes nuevos de la carpeta compartida.
#[tauri::command]
fn receive_packages(state: tauri::State<'_, AppState>) -> Result<ReceiveReport, String> {
    let mut seen = state
        .seen
        .lock()
        .map_err(|_| "la carpeta compartida quedó bloqueada por un error anterior".to_string())?;
    let store = &mut *state.store()?;
    let report = sharing::receive(store, &mut seen).map_err(|e| e.to_string())?;
    // Si ha llegado algo, el corredor que se está viendo se vuelve a volcar.
    if report.created + report.replaced > 0 {
        let mut viewed = state.viewed()?;
        if let Some(id) = viewed.as_ref().map(|v| v.runner.runner_id.clone()) {
            *viewed = Some(coach::runner_view(store, &id).map_err(|e| e.to_string())?);
        }
    }
    Ok(report)
}

/// Taxonomía de errores con la que se etiqueta (`docs/taxonomia.md`).
#[tauri::command]
fn taxonomy() -> Result<Taxonomy, String> {
    tags::taxonomy().map_err(|e| e.to_string())
}

/// Etiquetas de los tramos de un resultado.
#[tauri::command]
fn leg_tags(state: tauri::State<'_, AppState>, result_id: i64) -> Result<Vec<TagView>, String> {
    state.read(|s| tags::leg_tags(s, result_id))
}

/// Guarda la etiqueta de un tramo; vacía, la borra (`null`).
#[tauri::command]
fn save_leg_tag(
    state: tauri::State<'_, AppState>,
    result_id: i64,
    leg_index: usize,
    tag: LegTag,
) -> Result<Option<TagView>, String> {
    let mut store = state.write()?;
    let saved =
        tags::save_leg_tag(&mut store, result_id, leg_index, tag).map_err(|e| e.to_string())?;
    reshare(&mut store, result_id);
    Ok(saved)
}

/// Histórico de las carreras del usuario por formato (P6), con filtros de fechas y formato.
#[tauri::command]
fn history(
    state: tauri::State<'_, AppState>,
    filter: HistoryFilter,
) -> Result<HistoryView, String> {
    state.read(|s| history::history_view(s, &filter))
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
                seen: Mutex::new(SeenFiles::new()),
                viewed: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_version,
            get_settings,
            save_settings,
            check_zones,
            mode_chosen,
            choose_mode,
            coach_runners,
            view_runner,
            viewed_runner,
            group_view,
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
            export_race_package,
            import_race_package,
            race_sharing,
            set_race_sharing,
            share_all,
            receive_packages,
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

    fn state(store: Store) -> AppState {
        AppState {
            store: Mutex::new(store),
            seen: Mutex::new(SeenFiles::new()),
            viewed: Mutex::new(None),
        }
    }

    #[test]
    fn in_coach_mode_nothing_can_be_modified_and_the_viewed_runner_is_read() {
        let (mut runner, result_id) = crate::race_map::tests::imported(false);
        let package = package::race_package(&mut runner, result_id, ShareLevel::Legs).unwrap();
        let mut coach = Store::open_in_memory().unwrap();
        settings::choose_mode(&mut coach, AppMode::Coach).unwrap();
        coach.save_received_package(&package).unwrap();
        let state = state(coach);

        assert_eq!(state.write().err().as_deref(), Some(READ_ONLY));
        // Sin corredor elegido, las vistas no ven nada.
        assert!(state.read(races::list_races).unwrap().is_empty());
        let view = coach::runner_view(&state.store().unwrap(), &package.runner.runner_id).unwrap();
        *state.viewed().unwrap() = Some(view);
        assert_eq!(state.read(races::list_races).unwrap().len(), 1);
    }

    #[test]
    fn a_runner_reads_and_writes_their_own_database() {
        let (store, _) = crate::race_map::tests::imported(false);
        let state = state(store);
        assert!(state.write().is_ok());
        assert_eq!(state.read(races::list_races).unwrap().len(), 1);
    }
}
