//! App de escritorio de Tramos. Expone el núcleo a la interfaz mediante comandos Tauri;
//! no reimplementa cálculos.

pub mod batch;
pub mod clock_offset;
pub mod coach;
pub mod groups;
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
use tramos_core::group_compare::{CompareOptions, GroupsComparison};
use tramos_core::history::HistoryFilter;
use tramos_core::identify::RunnerIdentity;
use tramos_core::loss_breakdown::RaceBreakdown;
use tramos_core::package::{ShareChoice, ShareLevel};
use tramos_core::race_format::RaceFormat;
use tramos_core::taxonomy::{LegTag, Taxonomy};
use tramos_store::{AthleteGroupId, SaveOutcome, Store};

use crate::batch::BatchSummary;
use crate::clock_offset::OffsetView;
use crate::coach::{AthleteCard, AthleteNews, CoachRunner, GroupView, RunnerView, RunnerViewInfo};
use crate::groups::{GroupFields, GroupsView};
use crate::history::HistoryView;
use crate::import::{ImportOutcome, ImportPreview, ImportRequest};
use crate::race_map::RaceMap;
use crate::races::{RaceDetail, RaceRow};
use crate::settings::{Role, Settings};
use crate::sharing::{RaceSharing, ReceiveReport, SeenFiles, ShareReport};
use crate::tags::TagView;

/// Nombre de la base de datos del usuario, en el directorio de datos de la app.
const DATABASE_FILE: &str = "tramos.sqlite";

/// Error de los comandos que modifican carreras mientras se ve a un atleta.
const READ_ONLY: &str = "mientras ves a un atleta no se puede modificar nada";

/// Estado compartido por los comandos: la base de datos local, los ficheros de la carpeta
/// compartida ya importados y, si entrena, el atleta que se está viendo (#119).
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

    /// Lee de la base que muestran las vistas de corredor: la del atleta que se está viendo o,
    /// si no se ve a ninguno, la propia. Los cerrojos, siempre en el mismo orden: base y atleta.
    fn read<T, E: ToString>(&self, f: impl FnOnce(&Store) -> Result<T, E>) -> Result<T, String> {
        let store = self.store()?;
        let viewed = self.viewed()?;
        match viewed.as_ref() {
            Some(view) => f(&view.store),
            None => f(&store),
        }
        .map_err(|e| e.to_string())
    }

    /// La base propia para lo que solo hace quien entrena (vista de grupo, grupos de atletas).
    /// Si no entrena, error.
    fn coach_store(&self) -> Result<MutexGuard<'_, Store>, String> {
        let store = self.store()?;
        if !coach::is_coach(&store).map_err(|e| e.to_string())? {
            return Err("esto es para quien entrena a otros atletas".to_string());
        }
        Ok(store)
    }

    /// La base propia para modificar sus carreras. Mientras se ve a un atleta, error: sus
    /// `result_id` son de otra base y no deben tocar la propia. Los ajustes del usuario (también
    /// los paneles ocultos e «Incluirme») no pasan por aquí: son suyos, se vea a quien se vea.
    fn write(&self) -> Result<MutexGuard<'_, Store>, String> {
        let store = self.store()?;
        if self.viewed()?.is_some() {
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
    if !settings.sharing.coach {
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

/// Paneles de análisis ocultos (#130). Son del usuario de la app: valen también al ver a un
/// atleta.
#[tauri::command]
fn hidden_panels(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    settings::hidden_panels(&*state.store()?).map_err(|e| e.to_string())
}

/// Guarda los paneles de análisis ocultos; una lista vacía los enseña todos.
#[tauri::command]
fn set_hidden_panels(state: tauri::State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    settings::set_hidden_panels(&mut *state.store()?, &ids).map_err(|e| e.to_string())
}

/// Tema de la interfaz (#143): `system`, `light` o `dark`.
#[tauri::command]
fn theme(state: tauri::State<'_, AppState>) -> Result<settings::Theme, String> {
    settings::theme(&*state.store()?).map_err(|e| e.to_string())
}

/// Guarda el tema de la interfaz. Se aplica al momento, sin pasar por `save_settings`.
#[tauri::command]
fn set_theme(state: tauri::State<'_, AppState>, theme: settings::Theme) -> Result<(), String> {
    settings::set_theme(&mut *state.store()?, theme).map_err(|e| e.to_string())
}

/// Si ya se ha dicho cómo se usa la app (corre, entrena o las dos cosas). Al instalar, no: la
/// app lo pregunta.
#[tauri::command]
fn role_chosen(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    settings::role_chosen(&*state.store()?).map_err(|e| e.to_string())
}

/// Elige cómo se usa la app al empezar, sin tocar los demás ajustes.
#[tauri::command]
fn choose_role(state: tauri::State<'_, AppState>, role: Role) -> Result<(), String> {
    settings::choose_role(&mut *state.store()?, role).map_err(|e| e.to_string())?;
    *state.viewed()? = None;
    Ok(())
}

/// Si ya se ha visto el recorrido guiado de la primera vez (#141). Es de quien usa la app: se lee
/// en la base propia también mientras se ve a un atleta.
#[tauri::command]
fn tour_seen(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    settings::tour_seen(&*state.store()?).map_err(|e| e.to_string())
}

/// Guarda que se ha visto (o saltado) el recorrido guiado.
#[tauri::command]
fn set_tour_seen(state: tauri::State<'_, AppState>, seen: bool) -> Result<(), String> {
    settings::set_tour_seen(&mut *state.store()?, seen).map_err(|e| e.to_string())
}

/// Si entrena, atletas de los que hay paquetes.
#[tauri::command]
fn coach_runners(state: tauri::State<'_, AppState>) -> Result<Vec<CoachRunner>, String> {
    coach::coach_runners(&*state.store()?).map_err(|e| e.to_string())
}

/// Si entrena, lo nuevo de cada atleta desde la última vez que se entró en él (#142).
#[tauri::command]
fn athlete_news(state: tauri::State<'_, AppState>) -> Result<Vec<AthleteNews>, String> {
    coach::athlete_news(&*state.coach_store()?).map_err(|e| e.to_string())
}

/// Si entrena, las tarjetas de Mis atletas (#142): una por atleta.
#[tauri::command]
fn athlete_cards(state: tauri::State<'_, AppState>) -> Result<Vec<AthleteCard>, String> {
    coach::athlete_cards(&*state.coach_store()?).map_err(|e| e.to_string())
}

/// Si entrena, vista de grupo (P15): una fila por atleta y todos contra todos; con «Incluirme»,
/// también las carreras propias. Con `group`, solo los miembros de ese grupo de atletas.
#[tauri::command]
fn group_view(
    state: tauri::State<'_, AppState>,
    filter: HistoryFilter,
    group: Option<i64>,
) -> Result<GroupView, String> {
    let mut store = state.coach_store()?;
    let include_self = settings::include_self(&store).map_err(|e| e.to_string())?;
    coach::group_view(&mut store, &filter, include_self, group.map(AthleteGroupId))
        .map_err(|e| e.to_string())
}

/// Si entrena, sus grupos de atletas y a quién puede meter en ellos.
#[tauri::command]
fn athlete_groups(state: tauri::State<'_, AppState>) -> Result<GroupsView, String> {
    groups::groups_view(&mut *state.coach_store()?).map_err(|e| e.to_string())
}

/// Crea un grupo de atletas vacío y devuelve su identificador.
#[tauri::command]
fn create_athlete_group(
    state: tauri::State<'_, AppState>,
    group: GroupFields,
) -> Result<i64, String> {
    groups::create(&mut *state.coach_store()?, &group).map_err(|e| e.to_string())
}

/// Cambia el nombre, la descripción y el color de un grupo de atletas.
#[tauri::command]
fn update_athlete_group(
    state: tauri::State<'_, AppState>,
    id: i64,
    group: GroupFields,
) -> Result<(), String> {
    groups::update(&mut *state.coach_store()?, id, &group).map_err(|e| e.to_string())
}

/// Borra un grupo de atletas; sus atletas y sus paquetes siguen ahí.
#[tauri::command]
fn delete_athlete_group(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    groups::delete(&mut *state.coach_store()?, id).map_err(|e| e.to_string())
}

/// Compara dos grupos de atletas (#121) con el filtro del histórico y las opciones: qué pasa con
/// quien está en los dos y qué carreras entran.
#[tauri::command]
fn compare_athlete_groups(
    state: tauri::State<'_, AppState>,
    filter: HistoryFilter,
    a: i64,
    b: i64,
    options: CompareOptions,
) -> Result<GroupsComparison, String> {
    coach::compare_athlete_groups(
        &mut *state.coach_store()?,
        &filter,
        AthleteGroupId(a),
        AthleteGroupId(b),
        options,
    )
    .map_err(|e| e.to_string())
}

/// Mete (`member = true`) o saca a un atleta de un grupo.
#[tauri::command]
fn set_athlete_group_member(
    state: tauri::State<'_, AppState>,
    id: i64,
    runner_id: String,
    member: bool,
) -> Result<(), String> {
    groups::set_member(&mut *state.coach_store()?, id, &runner_id, member)
        .map_err(|e| e.to_string())
}

/// Guarda si las carreras propias cuentan en la vista de grupo («Incluirme»).
#[tauri::command]
fn set_include_self(state: tauri::State<'_, AppState>, include: bool) -> Result<(), String> {
    settings::set_include_self(&mut *state.store()?, include).map_err(|e| e.to_string())
}

/// El atleta que se está viendo, sin volver a volcarlo; `null` = lo propio.
#[tauri::command]
fn viewed_runner(state: tauri::State<'_, AppState>) -> Result<Option<RunnerViewInfo>, String> {
    Ok(state.viewed()?.as_ref().map(RunnerView::info))
}

/// Elige el atleta que se ve; `null` vuelve a lo propio. A partir de ahí las vistas de
/// corredor muestran sus carreras, en solo lectura, y sus novedades se quedan a cero. Devuelve lo
/// que no sale en ellas.
#[tauri::command]
fn view_runner(
    state: tauri::State<'_, AppState>,
    runner_id: Option<String>,
) -> Result<Option<RunnerViewInfo>, String> {
    let mut store = state.store()?;
    if runner_id.is_some() && !coach::is_coach(&store).map_err(|e| e.to_string())? {
        return Err("solo quien entrena ve las carreras de otros atletas".to_string());
    }
    let view = runner_id
        .map(|id| coach::enter_runner(&mut store, &id))
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

/// Exporta la carrera a la carpeta compartida si hace falta y dice cómo queda. Es de una
/// carrera propia: mientras se ve a un atleta, error.
#[tauri::command]
fn race_sharing(state: tauri::State<'_, AppState>, result_id: i64) -> Result<RaceSharing, String> {
    sharing::race_sharing(&mut *state.write()?, result_id).map_err(|e| e.to_string())
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

/// Si entrena, importa los paquetes nuevos de la carpeta compartida (no los propios).
#[tauri::command]
fn receive_packages(state: tauri::State<'_, AppState>) -> Result<ReceiveReport, String> {
    let mut seen = state
        .seen
        .lock()
        .map_err(|_| "la carpeta compartida quedó bloqueada por un error anterior".to_string())?;
    let store = &mut *state.store()?;
    let report = sharing::receive(store, &mut seen).map_err(|e| e.to_string())?;
    // Si ha llegado algo, el corredor que se está viendo se vuelve a volcar; como se está
    // viendo, lo suyo no queda como novedad.
    if report.created + report.replaced > 0 {
        let mut viewed = state.viewed()?;
        if let Some(id) = viewed.as_ref().map(|v| v.runner.runner_id.clone()) {
            *viewed = Some(coach::enter_runner(store, &id).map_err(|e| e.to_string())?);
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
            hidden_panels,
            set_hidden_panels,
            theme,
            set_theme,
            role_chosen,
            choose_role,
            coach_runners,
            athlete_news,
            athlete_cards,
            view_runner,
            viewed_runner,
            tour_seen,
            set_tour_seen,
            group_view,
            set_include_self,
            athlete_groups,
            create_athlete_group,
            update_athlete_group,
            delete_athlete_group,
            set_athlete_group_member,
            compare_athlete_groups,
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
    fn while_viewing_an_athlete_nothing_can_be_modified_and_their_races_are_read() {
        let (mut athlete, athlete_result) = crate::race_map::tests::imported(false);
        let package =
            package::race_package(&mut athlete, athlete_result, ShareLevel::Legs).unwrap();
        // Quien entrena también corre: tiene su carrera y recibe la del atleta.
        let (mut both, own_result) = crate::race_map::tests::imported(true);
        settings::choose_role(&mut both, Role::Both).unwrap();
        both.save_received_package(&package).unwrap();
        let state = state(both);

        // Sin atleta elegido, lo propio, editable.
        assert!(state.write().is_ok());
        let own = state.read(races::list_races).unwrap();
        assert_eq!(
            (own.len(), own[0].result_id, own[0].has_track),
            (1, own_result, true)
        );

        let view = coach::runner_view(&state.store().unwrap(), &package.runner.runner_id).unwrap();
        *state.viewed().unwrap() = Some(view);
        let theirs = state.read(races::list_races).unwrap();
        assert_eq!(theirs.len(), 1);
        assert!(!theirs[0].has_track);
        // Cada comando que modifica una carrera da error, también con un `result_id` que
        // existe en la base propia.
        let tag = tramos_core::taxonomy::LegTag::default();
        let errors = [
            state.write().err(),
            state
                .write()
                .and_then(|mut s| {
                    tags::save_leg_tag(&mut s, own_result, 1, tag.clone())
                        .map_err(|e| e.to_string())
                })
                .err(),
            state
                .write()
                .and_then(|mut s| {
                    races::set_race_format(&mut s, own_result, None).map_err(|e| e.to_string())
                })
                .err(),
            state
                .write()
                .and_then(|mut s| {
                    clock_offset::set_race_offset(&mut s, own_result, Some(1.0))
                        .map_err(|e| e.to_string())
                })
                .err(),
            state
                .write()
                .and_then(|mut s| {
                    sharing::race_sharing(&mut s, own_result).map_err(|e| e.to_string())
                })
                .err(),
        ];
        for error in errors {
            assert_eq!(error.as_deref(), Some(READ_ONLY));
        }
        assert!(
            tags::leg_tags(&state.store().unwrap(), own_result)
                .unwrap()
                .is_empty()
        );

        // Volver a lo propio: otra vez editable.
        *state.viewed().unwrap() = None;
        assert!(state.write().is_ok());
        assert_eq!(
            state.read(races::list_races).unwrap()[0].result_id,
            own_result
        );
    }

    #[test]
    fn a_runner_reads_and_writes_their_own_database() {
        let (store, _) = crate::race_map::tests::imported(false);
        let state = state(store);
        assert!(state.write().is_ok());
        assert_eq!(state.read(races::list_races).unwrap().len(), 1);
    }
}
