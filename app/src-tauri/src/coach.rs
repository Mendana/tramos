//! Atletas (#37, #119, `docs/app.md`, "Atletas"): quien entrena ve las carreras de un atleta
//! como las ve él, en solo lectura, a partir de los paquetes recibidos (`docs/paquete.md`).
//!
//! Los paquetes de un corredor se vuelcan en una base en memoria con la misma forma que la de su
//! app: cada carrera con sus originales del recorrido, su resultado vinculado a «su» persona, el
//! formato, las etiquetas, el track y sus umbrales. Así todas las vistas de corredor (lista,
//! carrera, mapa, histórico) funcionan sin cambios y recalculan con la versión del algoritmo de
//! esta app. Lo que no se puede volcar (paquetes con solo el resumen) va aparte.
//!
//! «Mis atletas» (#142): una tarjeta por atleta con lo principal de su histórico y las
//! novedades, lo recibido después de la última vez que se entró en él.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use thiserror::Error;
use tramos_core::event_kind::EventKind;
use tramos_core::group::{GroupComparison, GroupEntry, GroupRow, compare, group_row, group_total};
use tramos_core::group_compare::{
    CompareOptions, GroupsComparison, MemberRaces, MemberStats, RaceSelection, compare_groups,
    pool_side, races_of_both, sides,
};
use tramos_core::group_stats::{GroupStats, MemberAnalyses, group_stats};
use tramos_core::history::{HistoryFilter, HistoryStats};
use tramos_core::identify::{ResultRef, normalize_name};
use tramos_core::model::{Event, RaceResult};
use tramos_core::package::{RacePackage, RaceSummary, race_id};
use tramos_core::race_format::RaceFormat;
use tramos_store::{AthleteGroupId, ReceivedPackage, ReceivedRunner, ResultId, Store, StoreError};

use crate::history::{HistoryView, history_view, history_view_where};
use crate::import::{SELF_PERSON_KEY, stored_self_person};
use crate::races::RaceError;
use crate::settings;

/// Errores de la sección Atletas. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum CoachError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("no hay paquetes de ese corredor")]
    UnknownRunner,
    #[error("ningún atleta ha compartido esa carrera")]
    UnknownRace,
    #[error(transparent)]
    Race(#[from] RaceError),
}

/// Un atleta de la sección Atletas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoachRunner {
    pub runner_id: String,
    pub display_name: String,
    /// Carreras compartidas.
    pub races: usize,
    pub last_exported_at: DateTime<Utc>,
}

impl From<ReceivedRunner> for CoachRunner {
    fn from(r: ReceivedRunner) -> Self {
        Self {
            runner_id: r.runner_id,
            display_name: r.display_name,
            races: r.packages,
            last_exported_at: r.last_exported_at,
        }
    }
}

/// Corredores de los que hay paquetes, por nombre.
pub fn coach_runners(store: &Store) -> Result<Vec<CoachRunner>, CoachError> {
    Ok(store
        .received_runners()?
        .into_iter()
        .map(CoachRunner::from)
        .collect())
}

/// Una carrera compartida solo con el resumen: no se puede abrir.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SummaryRace {
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
    pub kind: EventKind,
    pub summary: RaceSummary,
    /// Ya estaba vista y el atleta la ha vuelto a exportar cambiada desde la última visita.
    pub updated: bool,
}

/// Lo que ve quien entrena de un atleta.
#[derive(Debug)]
pub struct RunnerView {
    pub runner: CoachRunner,
    /// Sus carreras con tramos, volcadas como en su app.
    pub store: Store,
    /// El `race_id` de cada resultado de `store`, para cruzar carreras entre corredores: el de
    /// la carrera volcada no vale, porque solo lleva las categorías del recorrido.
    pub race_ids: HashMap<i64, String>,
    /// Las compartidas solo con el resumen, de la más reciente a la más antigua.
    pub summary_only: Vec<SummaryRace>,
    /// Paquetes que no se han podido leer, con el motivo, en español.
    pub problems: Vec<String>,
    /// Los resultados de `store` de carreras ya vistas que se han actualizado desde la última
    /// visita ([`is_updated`]); vacío si [`runner_view`] no recibe esa visita.
    pub updated_results: Vec<i64>,
    /// La última visita de antes de entrar ahora (`None` si no había ninguna): con ella se
    /// vuelve a volcar sin perder las marcas de actualizadas.
    pub visited_before: Option<DateTime<Utc>>,
}

/// Lo que se manda a la interfaz de [`RunnerView`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunnerViewInfo {
    pub runner: CoachRunner,
    pub summary_only: Vec<SummaryRace>,
    pub problems: Vec<String>,
    /// Resultados (`result_id` de sus carreras) ya vistos y actualizados desde la última visita.
    pub updated_results: Vec<i64>,
}

impl RunnerView {
    pub fn info(&self) -> RunnerViewInfo {
        RunnerViewInfo {
            runner: self.runner.clone(),
            summary_only: self.summary_only.clone(),
            problems: self.problems.clone(),
            updated_results: self.updated_results.clone(),
        }
    }
}

/// Vuelca los paquetes del corredor `runner_id` en una base en memoria, sin marcar ninguno como
/// actualizado.
pub fn runner_view(main: &Store, runner_id: &str) -> Result<RunnerView, CoachError> {
    build_view(main, runner_id, None)
}

/// Como [`runner_view`], marcando las carreras ya vistas que se han actualizado desde
/// `visited_before`.
fn build_view(
    main: &Store,
    runner_id: &str,
    visited_before: Option<DateTime<Utc>>,
) -> Result<RunnerView, CoachError> {
    let runner = main
        .received_runners()?
        .into_iter()
        .find(|r| r.runner_id == runner_id)
        .map(CoachRunner::from)
        .ok_or(CoachError::UnknownRunner)?;
    let mut store = Store::open_in_memory()?;
    let name = if runner.display_name.trim().is_empty() {
        "Corredor"
    } else {
        runner.display_name.as_str()
    };
    let person = store.create_person(name, None)?;
    store.set_setting(SELF_PERSON_KEY, &person.0.to_string())?;

    let mut summary_only = Vec::new();
    let mut problems = Vec::new();
    let mut race_ids = HashMap::new();
    let mut updated_results = Vec::new();
    let mut latest: Option<RacePackage> = None;
    for received in main.received_packages_of(runner_id)? {
        let package = match RacePackage::parse(&received.content) {
            Ok(package) => package,
            Err(e) => {
                problems.push(e.to_string());
                continue;
            }
        };
        let updated = is_updated(&received, visited_before);
        match add_race(&mut store, person, &package, updated, &mut summary_only) {
            Ok(Some(result)) => {
                race_ids.insert(result.0, package.race.race_id.clone());
                if updated {
                    updated_results.push(result.0);
                }
            }
            Ok(None) => {}
            Err(e) => problems.push(format!(
                "{} {}: {e}",
                package.race.date,
                race_name(&package)
            )),
        }
        if latest
            .as_ref()
            .is_none_or(|l| l.exported_at < package.exported_at)
        {
            latest = Some(package);
        }
    }
    // Sus umbrales: los del paquete más reciente.
    if let Some(latest) = latest {
        store.set_setting(
            settings::ERROR_THRESHOLD_S_KEY,
            &latest.config.error_threshold_s.to_string(),
        )?;
        store.set_setting(
            settings::ERROR_THRESHOLD_PCT_KEY,
            &latest.config.error_threshold_pct.to_string(),
        )?;
    }
    summary_only.sort_by_key(|race| std::cmp::Reverse(race.date));
    Ok(RunnerView {
        runner,
        store,
        race_ids,
        summary_only,
        problems,
        updated_results,
        visited_before,
    })
}

fn race_name(package: &RacePackage) -> &str {
    package.race.name.as_deref().unwrap_or("carrera sin nombre")
}

/// Vuelca una carrera del paquete y devuelve su resultado; si solo trae el resumen, la apunta en
/// `summary_only`.
fn add_race(
    store: &mut Store,
    person: tramos_store::PersonId,
    package: &RacePackage,
    updated: bool,
    summary_only: &mut Vec<SummaryRace>,
) -> Result<Option<ResultId>, StoreError> {
    let Some(course) = &package.course else {
        summary_only.push(SummaryRace {
            date: package.race.date,
            name: package.race.name.clone(),
            format: package.race.format,
            kind: package.race.kind,
            summary: package.summary.clone(),
            updated,
        });
        return Ok(None);
    };
    let mut event = course.event.clone();
    event.name.clone_from(&package.race.name);
    event.date = package.race.date;
    // El corredor sale con su nombre visible, como en el selector. Los demás, con el nombre del
    // .spl; los paquetes de antes de #118 los traen vacíos y se numeran.
    let mut number = 0;
    for (c, class) in event.classes.iter_mut().enumerate() {
        for (r, result) in class.results.iter_mut().enumerate() {
            let runner = &mut result.runner;
            if c == course.result.class_index && r == course.result.result_index {
                runner.given_name.clone_from(&package.runner.display_name);
                runner.family_name.clear();
            } else if runner.given_name.trim().is_empty() && runner.family_name.trim().is_empty() {
                number += 1;
                runner.given_name = format!("Corredor {number}");
            }
        }
    }
    let saved = store.save_event(&event, None)?;
    let result = saved
        .results
        .get(course.result.class_index)
        .and_then(|class| class.get(course.result.result_index))
        .copied()
        .ok_or_else(|| {
            StoreError::InvalidData("el paquete apunta a un resultado que no está".into())
        })?;
    store.link_result(result, person)?;
    store.set_event_format(saved.id, package.race.format)?;
    store.set_event_kind(saved.id, package.race.kind)?;
    for tag in &package.tags {
        store.save_tag(result, tag.leg_index, &tag.taxonomy_version, &tag.tag)?;
    }
    if let Some(shared) = &package.track {
        store.save_track(result, &shared.track, None)?;
        store.set_manual_offset(result, shared.manual_offset_s)?;
    }
    Ok(Some(result))
}

/// Un corredor en la tabla del grupo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupRunnerRow {
    pub runner: CoachRunner,
    /// Es quien usa la app, con sus carreras propias («Incluirme», #119).
    pub is_self: bool,
    /// `None` si su histórico no se ha podido calcular (`problem` dice por qué).
    pub row: Option<GroupRow>,
    pub problem: Option<String>,
}

/// Vista de grupo (P15): una fila por corredor y todos contra todos, con el mismo filtro.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupView {
    /// Quien usa la app primero si se incluye; después, en el orden de `coach_runners`. Los
    /// índices de `comparison` son posiciones aquí.
    pub runners: Vec<GroupRunnerRow>,
    pub comparison: GroupComparison,
    /// Totales de todos los que salen (`group_total`, #120): sus carreras y tramos juntos.
    pub total: HistoryStats,
    /// Si las carreras propias cuentan en «Todos» (ajuste «Incluirme»).
    pub include_self: bool,
    /// El grupo de atletas al que se limita; `None` = todos.
    pub group: Option<i64>,
}

/// La vista de grupo con todos los atletas de los que hay paquetes o, con `group`, solo con los
/// miembros de ese grupo. Las carreras propias cuentan como un atleta más con `include_self` (sin
/// grupo) o si quien entrena es miembro del grupo. Cada uno se calcula como en su histórico
/// (`history_view`), con sus umbrales.
pub fn group_view(
    main: &mut Store,
    filter: &HistoryFilter,
    include_self: bool,
    group: Option<AthleteGroupId>,
) -> Result<GroupView, CoachError> {
    let members = match group {
        None => None,
        Some(id) => Some(
            main.athlete_groups()?
                .into_iter()
                .find(|g| g.id == id)
                .ok_or(StoreError::GroupNotFound(id.0))?
                .members,
        ),
    };
    let counts = |runner_id: &str| {
        members
            .as_ref()
            .is_none_or(|m| m.iter().any(|r| r == runner_id))
    };
    let mut runners = Vec::new();
    let mut entries = Vec::new();
    let me = own_runner(main)?.filter(|me| {
        if members.is_some() {
            counts(&me.runner_id)
        } else {
            include_self
        }
    });
    if let Some(me) = me {
        let race_ids = own_race_ids(main)?;
        let row = group_row_of(
            main,
            me,
            true,
            &race_ids,
            filter,
            runners.len(),
            &mut entries,
        );
        runners.push(row);
    }
    for runner in coach_runners(main)?
        .into_iter()
        .filter(|r| counts(&r.runner_id))
    {
        let view = runner_view(main, &runner.runner_id)?;
        let row = group_row_of(
            &view.store,
            runner,
            false,
            &view.race_ids,
            filter,
            runners.len(),
            &mut entries,
        );
        runners.push(row);
    }
    let stats: Vec<HistoryStats> = runners
        .iter()
        .filter_map(|r| r.row.as_ref().map(|row| row.stats))
        .collect();
    Ok(GroupView {
        runners,
        comparison: compare(&entries),
        total: group_total(&stats),
        include_self,
        group: group.map(|g| g.0),
    })
}

/// La fila del corredor que irá en la posición `index` del grupo, a partir de su base. Apunta
/// sus carreras en `entries` para cruzarlas con las de los demás.
fn group_row_of(
    store: &Store,
    runner: CoachRunner,
    is_self: bool,
    race_ids: &HashMap<i64, String>,
    filter: &HistoryFilter,
    index: usize,
    entries: &mut Vec<GroupEntry>,
) -> GroupRunnerRow {
    let (row, problem) = match history_view(store, filter) {
        Ok(h) => {
            for race in &h.races {
                let Some(race_id) = race_ids.get(&race.result_id) else {
                    continue;
                };
                entries.push(GroupEntry {
                    runner: index,
                    race_id: race_id.clone(),
                    date: race.date,
                    name: race.name.clone(),
                    format: race.format,
                    stats: race.stats,
                });
            }
            (Some(row_of(&h)), None)
        }
        Err(e) => (None, Some(e.to_string())),
    };
    GroupRunnerRow {
        runner,
        is_self,
        row,
        problem,
    }
}

/// La fila de la tabla del grupo de un histórico: la misma en Comparar atletas y en la tarjeta de
/// Mis atletas.
fn row_of(h: &HistoryView) -> GroupRow {
    group_row(&h.history, &h.by_leg_length, &h.by_slope, &h.common_errors)
}

/// Quien usa la app como corredor del grupo: su identificador de paquetes, su nombre y sus
/// carreras. `None` si aún no tiene ninguna.
pub fn own_runner(main: &mut Store) -> Result<Option<CoachRunner>, StoreError> {
    let Some(me) = stored_self_person(main)? else {
        return Ok(None);
    };
    let display_name = main
        .people()?
        .into_iter()
        .find(|p| p.id == me)
        .map(|p| p.display_name)
        .unwrap_or_default();
    Ok(Some(CoachRunner {
        runner_id: main.package_runner_id()?,
        display_name,
        races: main.person_results(me)?.len(),
        // Lo propio no llega por la carpeta: está siempre al día.
        last_exported_at: Utc::now(),
    }))
}

/// El `race_id` de cada carrera propia, para cruzarla con las de los atletas: el mismo que
/// llevaría su paquete.
pub(crate) fn own_race_ids(main: &Store) -> Result<HashMap<i64, String>, StoreError> {
    let mut ids = HashMap::new();
    let Some(me) = stored_self_person(main)? else {
        return Ok(ids);
    };
    for r in main.person_results(me)? {
        let (event_id, _) = main.result_ref(r.result)?;
        ids.insert(r.result.0, race_id(&main.load_event(event_id)?));
    }
    Ok(ids)
}

/// La base de un miembro de un grupo: la propia (con el `race_id` de cada carrera) o la volcada
/// de un atleta.
enum MemberBase {
    Own(HashMap<i64, String>),
    Athlete(Box<RunnerView>),
}

impl MemberBase {
    fn parts<'a>(&'a self, main: &'a Store) -> (&'a Store, &'a HashMap<i64, String>) {
        match self {
            Self::Own(race_ids) => (main, race_ids),
            Self::Athlete(view) => (&view.store, &view.race_ids),
        }
    }
}

/// Compara dos grupos de atletas (#121, `docs/historico.md`, "Comparar grupos") con el filtro
/// del histórico y las opciones de la comparación. Los miembros de los que no hay paquetes (ni
/// son quien usa la app con carreras) no cuentan.
pub fn compare_athlete_groups(
    main: &mut Store,
    filter: &HistoryFilter,
    a: AthleteGroupId,
    b: AthleteGroupId,
    options: CompareOptions,
) -> Result<GroupsComparison, CoachError> {
    let groups = main.athlete_groups()?;
    let members = |id: AthleteGroupId| {
        groups
            .iter()
            .find(|g| g.id == id)
            .map(|g| g.members.clone())
            .ok_or(StoreError::GroupNotFound(id.0))
    };
    let sides = sides(&members(a)?, &members(b)?, options.overlap);
    let me = own_runner(main)?.map(|r| r.runner_id);
    let main: &Store = main;
    let known: BTreeSet<String> = coach_runners(main)?
        .into_iter()
        .map(|r| r.runner_id)
        .collect();

    // Cada miembro se carga una vez, aunque esté en los dos lados.
    let mut bases: BTreeMap<&str, MemberBase> = BTreeMap::new();
    for id in sides.a.iter().chain(&sides.b) {
        if bases.contains_key(id.as_str()) {
            continue;
        }
        let base = if me.as_deref() == Some(id.as_str()) {
            MemberBase::Own(own_race_ids(main)?)
        } else if known.contains(id) {
            MemberBase::Athlete(Box::new(runner_view(main, id)?))
        } else {
            continue;
        };
        bases.insert(id, base);
    }
    let mut views: BTreeMap<&str, HistoryView> = BTreeMap::new();
    for (id, base) in &bases {
        let (store, _) = base.parts(main);
        views.insert(id, history_view(store, filter)?);
    }

    // Con «solo las de los dos», cada uno se vuelve a calcular con esas carreras.
    let shared = match options.races {
        RaceSelection::All => None,
        RaceSelection::Shared => {
            let raced: BTreeMap<&str, Vec<String>> = views
                .iter()
                .map(|(id, view)| {
                    let (_, race_ids) = bases[id].parts(main);
                    let races = view
                        .races
                        .iter()
                        .filter(|r| r.stats.is_some())
                        .filter_map(|r| race_ids.get(&r.result_id).cloned())
                        .collect();
                    (*id, races)
                })
                .collect();
            let side = |ids: &[String]| -> Vec<MemberRaces<'_>> {
                ids.iter()
                    .filter_map(|id| raced.get_key_value(id.as_str()))
                    .map(|(id, races)| (*id, races.as_slice()))
                    .collect()
            };
            let allowed = races_of_both(&side(&sides.a), &side(&sides.b));
            for (id, base) in &bases {
                let (store, race_ids) = base.parts(main);
                let keep = |result: i64| race_ids.get(&result).is_some_and(|r| allowed.contains(r));
                views.insert(id, history_view_where(store, filter, &keep)?);
            }
            Some(allowed.len())
        }
    };

    let pooled = |ids: &[String]| {
        let members: Vec<MemberStats<'_>> = ids
            .iter()
            .filter_map(|id| views.get(id.as_str()))
            .map(|v| MemberStats {
                total: v.history.total,
                by_leg_length: &v.by_leg_length,
                by_slope: &v.by_slope.by_class,
                errors: &v.common_errors.total,
            })
            .collect();
        pool_side(&members)
    };
    Ok(compare_groups(
        options,
        pooled(&sides.a),
        pooled(&sides.b),
        sides.in_both.len(),
        shared,
    ))
}

/// Un atleta de quien entrena que compartió una carrera con los tramos (#164): su paquete, para
/// saber cuál de los resultados de esa carrera es el suyo.
#[derive(Debug, Clone)]
pub struct RaceAthlete {
    pub runner_id: String,
    pub display_name: String,
    package: RacePackage,
}

impl RaceAthlete {
    /// Su resultado dentro de `event`, que es la misma carrera vista desde otra copia (la propia
    /// de quien entrena o la reducida de otro paquete): cada paquete trae su copia, así que no
    /// vale la posición, se casa por la identidad del resultado.
    ///
    /// Identidad: categoría, nombre y apellidos (sin mayúsculas, tildes ni signos,
    /// `identify::normalize_name`) y, solo si con eso hay más de uno, el club. Dorsal y tarjeta no
    /// sirven: los paquetes los llevan a `null`. `None` si no hay resultado que case, si casa más
    /// de uno aunque se mire el club (mejor no marcar que marcar a otro) o si el paquete viene sin
    /// nombres (anterior a #118).
    pub fn result_in(&self, event: &Event) -> Option<ResultRef> {
        let course = self.package.course.as_ref()?;
        let own = course.result.get(&course.event)?;
        let class_name = &course.event.classes.get(course.result.class_index)?.name;
        let name = full_name_key(own);
        if name.is_empty() {
            return None;
        }
        let mut found: Vec<(ResultRef, &RaceResult)> = Vec::new();
        for (class_index, class) in event.classes.iter().enumerate() {
            if normalize_name(&class.name) != normalize_name(class_name) {
                continue;
            }
            for (result_index, result) in class.results.iter().enumerate() {
                if full_name_key(result) == name {
                    found.push((
                        ResultRef {
                            class_index,
                            result_index,
                        },
                        result,
                    ));
                }
            }
        }
        if found.len() > 1 {
            let club = club_key(own);
            found.retain(|(_, result)| club_key(result) == club);
        }
        match found.as_slice() {
            [(at, _)] => Some(*at),
            _ => None,
        }
    }
}

fn full_name_key(result: &RaceResult) -> String {
    normalize_name(&format!(
        "{} {}",
        result.runner.given_name, result.runner.family_name
    ))
}

fn club_key(result: &RaceResult) -> String {
    normalize_name(result.runner.club.as_deref().unwrap_or_default())
}

/// Los atletas que compartieron con los tramos la carrera `race_id` (el de `docs/paquete.md`:
/// el mismo para todos los que la corrieron), por nombre. Los paquetes que no se pueden leer o que
/// solo traen el resumen no cuentan. Para cruzar atletas en una carrera (#164, #163).
pub fn race_athletes(main: &Store, race_id: &str) -> Result<Vec<RaceAthlete>, CoachError> {
    let names: HashMap<String, String> = coach_runners(main)?
        .into_iter()
        .map(|r| (r.runner_id, r.display_name))
        .collect();
    let mut athletes: Vec<RaceAthlete> = main
        .received_packages_of_race(race_id)?
        .into_iter()
        .filter_map(|received| RacePackage::parse(&received.content).ok())
        .filter(|package| package.course.is_some())
        .map(|package| RaceAthlete {
            runner_id: package.runner.runner_id.clone(),
            display_name: names
                .get(&package.runner.runner_id)
                .cloned()
                .unwrap_or_else(|| package.runner.display_name.clone()),
            package,
        })
        .collect();
    athletes.sort_by(|a, b| {
        (a.display_name.to_lowercase(), &a.runner_id)
            .cmp(&(b.display_name.to_lowercase(), &b.runner_id))
    });
    Ok(athletes)
}

/// Un atleta propio que corrió la misma carrera que la que se está viendo: cuál de sus
/// resultados es el suyo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AthleteMark {
    pub result: ResultRef,
    pub runner_id: String,
    pub display_name: String,
}

/// Qué resultados de la carrera de `result_id` (de la base que se ve: la del atleta `viewed` o, si
/// no hay ninguno, la propia) son de otros atletas de quien entrena con paquete de la misma
/// carrera (#164). Para marcarlos en «Frente al grupo». Vacío si no entrena, si la carrera no
/// tiene `race_id` conocido o si nadie más la compartió con los tramos. El atleta que se está
/// viendo no se marca a sí mismo.
pub fn marked_athletes(
    main: &Store,
    viewed: Option<&RunnerView>,
    result_id: i64,
) -> Result<Vec<AthleteMark>, CoachError> {
    if !is_coach(main)? {
        return Ok(Vec::new());
    }
    let id = ResultId(result_id);
    let (store, race, skip) = match viewed {
        Some(view) => (
            &view.store,
            view.race_ids.get(&result_id).cloned(),
            Some(view.runner.runner_id.as_str()),
        ),
        None => {
            let (event_id, _) = main.result_ref(id)?;
            (main, Some(race_id(&main.load_event(event_id)?)), None)
        }
    };
    let Some(race) = race else {
        return Ok(Vec::new());
    };
    let (event_id, own) = store.result_ref(id)?;
    let event = store.load_event(event_id)?;
    let mut marks: Vec<AthleteMark> = Vec::new();
    for athlete in race_athletes(main, &race)? {
        if Some(athlete.runner_id.as_str()) == skip {
            continue;
        }
        let Some(at) = athlete.result_in(&event) else {
            continue;
        };
        if at != own && marks.iter().all(|m| m.result != at) {
            marks.push(AthleteMark {
                result: at,
                runner_id: athlete.runner_id,
                display_name: athlete.display_name,
            });
        }
    }
    Ok(marks)
}

/// ¿Es nueva una carrera que llegó por primera vez en `first_arrival` para quien entró en el
/// atleta por última vez en `last_seen`? Si nunca ha entrado, todo es nuevo. Cuenta la primera
/// llegada (`first_imported_at`), no la última vez que se sustituyó: un paquete que vuelve a
/// llegar cambiado (el atleta etiquetó un error, por ejemplo) de una carrera ya vista no es una
/// carrera nueva, sino una actualizada ([`is_updated`]).
pub fn is_new(first_arrival: DateTime<Utc>, last_seen: Option<DateTime<Utc>>) -> bool {
    last_seen.is_none_or(|seen| first_arrival > seen)
}

/// ¿Es una carrera ya vista que se ha actualizado desde `last_seen`? Llegó antes de esa visita
/// y su paquete se ha sustituido después. Sin visita previa, todo es nuevo y nada es actualizado.
pub fn is_updated(package: &ReceivedPackage, last_seen: Option<DateTime<Utc>>) -> bool {
    last_seen.is_some_and(|seen| {
        !is_new(package.first_imported_at, Some(seen)) && package.imported_at > seen
    })
}

/// Los paquetes del atleta de carreras que llegaron por primera vez después de `last_seen`.
fn new_packages(
    main: &Store,
    runner_id: &str,
    last_seen: Option<DateTime<Utc>>,
) -> Result<Vec<ReceivedPackage>, StoreError> {
    Ok(main
        .received_packages_of(runner_id)?
        .into_iter()
        .filter(|p| is_new(p.first_imported_at, last_seen))
        .collect())
}

/// Cuántas carreras del atleta, ya vistas, se han actualizado desde `last_seen`.
fn updated_count(
    main: &Store,
    runner_id: &str,
    last_seen: Option<DateTime<Utc>>,
) -> Result<usize, StoreError> {
    Ok(main
        .received_packages_of(runner_id)?
        .iter()
        .filter(|p| is_updated(p, last_seen))
        .count())
}

/// Una carrera nueva de un atleta.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NewRace {
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
    /// Cuándo llegó por primera vez.
    pub received_at: DateTime<Utc>,
}

/// Lo nuevo de un atleta desde la última vez que se entró en él.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AthleteNews {
    pub runner: CoachRunner,
    /// `None` si nunca se ha entrado en él.
    pub last_seen: Option<DateTime<Utc>>,
    /// De la más reciente a la más antigua (por fecha de la carrera). Los paquetes que no se
    /// pueden leer no salen: se ven al entrar en el atleta.
    pub new_races: Vec<NewRace>,
}

/// Las novedades de cada atleta del que hay paquetes, en el orden de [`coach_runners`]. Solo lee
/// los paquetes nuevos.
pub fn athlete_news(main: &Store) -> Result<Vec<AthleteNews>, CoachError> {
    let seen = settings::last_seen(main)?;
    let mut news = Vec::new();
    for runner in coach_runners(main)? {
        let last_seen = seen.get(&runner.runner_id).copied();
        let mut new_races: Vec<NewRace> = new_packages(main, &runner.runner_id, last_seen)?
            .into_iter()
            .filter_map(|received| {
                let package = RacePackage::parse(&received.content).ok()?;
                Some(NewRace {
                    date: package.race.date,
                    name: package.race.name,
                    format: package.race.format,
                    received_at: received.first_imported_at,
                })
            })
            .collect();
        new_races.sort_by(|a, b| b.date.cmp(&a.date).then(b.received_at.cmp(&a.received_at)));
        news.push(AthleteNews {
            runner,
            last_seen,
            new_races,
        });
    }
    Ok(news)
}

/// Entra en un atleta: lo vuelca como [`runner_view`] y apunta que se le ha visto ahora, así que
/// sus novedades se quedan a cero.
/// Las carreras ya vistas que se han actualizado desde la visita anterior salen marcadas
/// (`updated_results`, `SummaryRace::updated`) aunque la visita nueva deje sus novedades a cero.
pub fn enter_runner(main: &mut Store, runner_id: &str) -> Result<RunnerView, CoachError> {
    let before = settings::last_seen(main)?.get(runner_id).copied();
    let view = build_view(main, runner_id, before)?;
    settings::set_last_seen(main, runner_id, Utc::now())?;
    Ok(view)
}

/// Vuelve a volcar al atleta que se está viendo porque ha llegado algo suyo: sus novedades
/// siguen a cero y las marcas de actualizadas se calculan desde la visita de antes de entrar, no
/// desde ahora.
pub fn refresh_runner(main: &mut Store, view: &RunnerView) -> Result<RunnerView, CoachError> {
    let runner_id = view.runner.runner_id.as_str();
    let fresh = build_view(main, runner_id, view.visited_before)?;
    settings::set_last_seen(main, runner_id, Utc::now())?;
    Ok(fresh)
}

/// Carreras de la línea de evolución de la tarjeta.
pub const TREND_RACES: usize = 10;

/// Una carrera en la línea de evolución de un atleta.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrendPoint {
    pub date: NaiveDate,
    pub name: Option<String>,
    /// Su rendimiento habitual (IR de la carrera, 1 = 100 %).
    pub performance: f64,
}

/// La tarjeta de un atleta en Mis atletas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AthleteCard {
    pub runner: CoachRunner,
    /// Lo mismo que su fila en Comparar atletas sin filtros: carreras, IR medio, tasa de error,
    /// error más común… `None` si su histórico no se ha podido calcular (`problem` dice por qué).
    pub row: Option<GroupRow>,
    pub problem: Option<String>,
    /// El rendimiento de sus [`TREND_RACES`] últimas carreras con rendimiento, de la más antigua a
    /// la más reciente.
    pub trend: Vec<TrendPoint>,
    /// Fecha de su carrera más reciente, también de las compartidas solo con el resumen.
    pub last_race: Option<NaiveDate>,
    /// Los grupos de atletas en los que está.
    pub groups: Vec<i64>,
    /// Carreras que han llegado desde la última vez que se entró en él ([`athlete_news`]).
    pub new_races: usize,
    /// Carreras ya vistas que ha vuelto a exportar cambiadas desde esa visita: no cuentan como
    /// nuevas ([`is_updated`]).
    pub updated_races: usize,
}

/// Una tarjeta por atleta del que hay paquetes, en el orden de [`coach_runners`]. Cada uno se
/// calcula como en su histórico sin filtros, con sus umbrales.
pub fn athlete_cards(main: &Store) -> Result<Vec<AthleteCard>, CoachError> {
    let seen = settings::last_seen(main)?;
    let groups = main.athlete_groups()?;
    let mut cards = Vec::new();
    for runner in coach_runners(main)? {
        let view = runner_view(main, &runner.runner_id)?;
        let last_seen = seen.get(&runner.runner_id).copied();
        let new_races = new_packages(main, &runner.runner_id, last_seen)?.len();
        let updated_races = updated_count(main, &runner.runner_id, last_seen)?;
        let (row, problem, trend, last_race) =
            match history_view(&view.store, &HistoryFilter::default()) {
                Ok(h) => {
                    // `races` va de la más reciente a la más antigua.
                    let mut trend: Vec<TrendPoint> = h
                        .races
                        .iter()
                        .filter_map(|race| {
                            Some(TrendPoint {
                                date: race.date,
                                name: race.name.clone(),
                                performance: race.stats?.mean_performance?,
                            })
                        })
                        .take(TREND_RACES)
                        .collect();
                    trend.reverse();
                    let last = h.races.first().map(|r| r.date);
                    (Some(row_of(&h)), None, trend, last)
                }
                Err(e) => (None, Some(e.to_string()), Vec::new(), None),
            };
        let last_race = last_race.max(view.summary_only.first().map(|r| r.date));
        cards.push(AthleteCard {
            groups: groups
                .iter()
                .filter(|g| g.members.contains(&runner.runner_id))
                .map(|g| g.id.0)
                .collect(),
            runner,
            row,
            problem,
            trend,
            last_race,
            new_races,
            updated_races,
        });
    }
    Ok(cards)
}

/// Un miembro en las estadísticas de un grupo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupStatsMember {
    pub runner: CoachRunner,
    /// Es quien usa la app, con sus carreras propias.
    pub is_self: bool,
    /// Sus carreras que cuentan con el filtro (las que tienen números).
    pub races: usize,
    /// Si su histórico no se ha podido calcular, por qué: entonces no cuenta.
    pub problem: Option<String>,
}

/// Estadísticas de un grupo de atletas (#145): los análisis de Estadísticas con todos sus
/// miembros juntos.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupStatsView {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub color: String,
    /// Quien usa la app primero, si es miembro y tiene carreras; después, como en
    /// `coach_runners`.
    pub members: Vec<GroupStatsMember>,
    /// Miembros de los que ya no hay carreras (ni son quien usa la app con carreras): no
    /// cuentan.
    pub missing: usize,
    pub stats: GroupStats,
}

/// Las estadísticas del grupo `group` con el filtro del histórico (`docs/historico.md`,
/// "Estadísticas de un grupo"): el histórico de cada miembro, con sus umbrales, como en sus
/// Estadísticas (`history_view`), y todos juntos con `tramos_core::group_stats`.
pub fn athlete_group_stats(
    main: &mut Store,
    filter: &HistoryFilter,
    group: AthleteGroupId,
) -> Result<GroupStatsView, CoachError> {
    let group = main
        .athlete_groups()?
        .into_iter()
        .find(|g| g.id == group)
        .ok_or(StoreError::GroupNotFound(group.0))?;
    let is_member = |runner_id: &str| group.members.iter().any(|m| m == runner_id);
    let mut loaded: Vec<(CoachRunner, bool, Result<HistoryView, RaceError>)> = Vec::new();
    if let Some(me) = own_runner(main)?.filter(|me| is_member(&me.runner_id)) {
        let view = history_view(main, filter);
        loaded.push((me, true, view));
    }
    for runner in coach_runners(main)?
        .into_iter()
        .filter(|r| is_member(&r.runner_id))
    {
        let base = runner_view(main, &runner.runner_id)?;
        let view = history_view(&base.store, filter);
        loaded.push((runner, false, view));
    }
    let analyses: Vec<MemberAnalyses<'_>> = loaded
        .iter()
        .filter_map(|(_, _, view)| view.as_ref().ok())
        .map(|v| MemberAnalyses {
            history: &v.history,
            by_leg_length: &v.by_leg_length,
            by_slope: &v.by_slope,
            loss_breakdown: &v.loss_breakdown,
            after_error: &v.after_error,
            common_errors: &v.common_errors,
            fatigue: &v.fatigue,
            days_off: &v.days_off,
        })
        .collect();
    let stats = group_stats(filter, &analyses);
    let members: Vec<GroupStatsMember> = loaded
        .iter()
        .map(|(runner, is_self, view)| GroupStatsMember {
            runner: runner.clone(),
            is_self: *is_self,
            races: view.as_ref().map_or(0, |v| v.history.total.races),
            problem: view.as_ref().err().map(ToString::to_string),
        })
        .collect();
    Ok(GroupStatsView {
        id: group.id.0,
        missing: group.members.len().saturating_sub(members.len()),
        name: group.name,
        description: group.description,
        color: group.color,
        members,
        stats,
    })
}

/// ¿Entrena a otros atletas? (ajuste «Entreno a otros atletas», #119).
pub fn is_coach(store: &Store) -> Result<bool, StoreError> {
    Ok(settings::load(store)?.sharing.coach)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::history_view;
    use crate::package::race_package;
    use crate::race_map::tests::imported;
    use crate::races::{list_races, race_detail};
    use crate::tags::{leg_tags, save_leg_tag};
    use tramos_core::history::HistoryFilter;
    use tramos_core::package::ShareLevel;
    use tramos_core::taxonomy::{Confirmation, LegTag};

    /// El corredor etiqueta un tramo y fija sus umbrales; la entrenadora recibe su paquete.
    fn received(level: ShareLevel, with_fit: bool) -> (Store, i64, Store, String) {
        let (mut runner, result_id) = imported(with_fit);
        let mut s = settings::load(&runner).unwrap();
        s.error_threshold_s = 20.0;
        s.error_threshold_pct = 12.0;
        settings::save(&mut runner, &s).unwrap();
        save_leg_tag(
            &mut runner,
            result_id,
            3,
            LegTag {
                confirmation: Some(Confirmation::Error),
                note: Some("nota".into()),
                ..LegTag::default()
            },
        )
        .unwrap();
        let package = race_package(&mut runner, result_id, level).unwrap();
        let mut coach = Store::open_in_memory().unwrap();
        coach.save_received_package(&package).unwrap();
        (runner, result_id, coach, package.runner.runner_id)
    }

    #[test]
    fn the_coach_sees_the_race_as_the_runner_does() {
        let (runner, result_id, coach, runner_id) = received(ShareLevel::Track, true);
        let runners = coach_runners(&coach).unwrap();
        assert_eq!(runners.len(), 1);
        assert_eq!(runners[0].races, 1);

        let view = runner_view(&coach, &runner_id).unwrap();
        assert!(view.summary_only.is_empty() && view.problems.is_empty());
        let rows = list_races(&view.store).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].has_track);
        assert_eq!(
            view.race_ids.get(&rows[0].result_id),
            Some(&coach.received_packages().unwrap()[0].race_id)
        );

        // La misma tabla de tramos, con los umbrales del corredor.
        let mine = race_detail(&runner, result_id).unwrap();
        let theirs = race_detail(&view.store, rows[0].result_id).unwrap();
        assert_eq!(theirs.config, mine.config);
        assert_eq!(theirs.report.lost_time, mine.report.lost_time);
        assert_eq!(theirs.format, mine.format);
        assert_eq!(theirs.given_name, runners[0].display_name);

        // Las etiquetas y el track viajan.
        let tags = leg_tags(&view.store, rows[0].result_id).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].tag.note.as_deref(), Some("nota"));
        assert_eq!(
            view.store.load_track(ResultId(rows[0].result_id)).unwrap(),
            runner.load_track(ResultId(result_id)).unwrap()
        );

        // Y el histórico sale igual (salvo los identificadores, que son de otra base).
        let filter = HistoryFilter::default();
        let without_ids = |store: &Store| {
            let mut races = history_view(store, &filter).unwrap().races;
            for race in &mut races {
                race.result_id = 0;
            }
            races
        };
        assert_eq!(without_ids(&view.store), without_ids(&runner));
    }

    #[test]
    fn a_training_arrives_as_a_training() {
        let (mut runner, result_id) = imported(false);
        crate::races::set_race_kind(&mut runner, result_id, EventKind::Training).unwrap();
        let mut coach = Store::open_in_memory().unwrap();
        for level in [ShareLevel::Legs, ShareLevel::Aggregates] {
            let mut package = race_package(&mut runner, result_id, level).unwrap();
            assert_eq!(package.race.kind, EventKind::Training);
            if level == ShareLevel::Aggregates {
                // Otro atleta, para tener los dos casos a la vez.
                package.runner.runner_id = "solo-resumen".into();
            }
            coach.save_received_package(&package).unwrap();
        }
        let runners = coach_runners(&coach).unwrap();
        let with_legs = runners
            .iter()
            .find(|r| r.runner_id != "solo-resumen")
            .unwrap();
        let view = runner_view(&coach, &with_legs.runner_id).unwrap();
        assert_eq!(
            list_races(&view.store).unwrap()[0].kind,
            EventKind::Training
        );
        let view = runner_view(&coach, "solo-resumen").unwrap();
        assert_eq!(view.summary_only[0].kind, EventKind::Training);
    }

    /// Nombres de los corredores de la única carrera que ve la entrenadora.
    fn course_names(view: &RunnerView) -> Vec<String> {
        let rows = list_races(&view.store).unwrap();
        let (event_id, _) = view.store.result_ref(ResultId(rows[0].result_id)).unwrap();
        let event = view.store.load_event(event_id).unwrap();
        event
            .classes
            .iter()
            .flat_map(|c| c.results.iter())
            .map(|r| format!("{} {}", r.runner.given_name, r.runner.family_name))
            .collect()
    }

    #[test]
    fn the_others_on_the_course_keep_their_names() {
        let (runner, result_id, coach, runner_id) = received(ShareLevel::Legs, false);
        let view = runner_view(&coach, &runner_id).unwrap();
        let names = course_names(&view);
        let (event_id, at) = runner.result_ref(ResultId(result_id)).unwrap();
        let original = runner.load_event(event_id).unwrap();
        // Otro de su categoría, que seguro que va en el paquete.
        let someone = original.classes[at.class_index]
            .results
            .iter()
            .enumerate()
            .find(|(i, r)| *i != at.result_index && !r.runner.family_name.is_empty())
            .map(|(_, r)| r)
            .unwrap();
        let full = format!(
            "{} {}",
            someone.runner.given_name, someone.runner.family_name
        );
        assert!(names.contains(&full), "{full}");
        assert!(names.iter().all(|n| !n.starts_with("Corredor ")));
        assert!(
            view.store
                .load_track(ResultId(list_races(&view.store).unwrap()[0].result_id))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn an_old_package_without_names_is_numbered() {
        let (mut runner, result_id) = imported(false);
        let mut package = race_package(&mut runner, result_id, ShareLevel::Legs).unwrap();
        let course = package.course.as_mut().unwrap();
        for r in course
            .event
            .classes
            .iter_mut()
            .flat_map(|c| c.results.iter_mut())
        {
            r.runner.given_name.clear();
            r.runner.family_name.clear();
            r.runner.club = None;
        }
        let mut coach = Store::open_in_memory().unwrap();
        coach.save_received_package(&package).unwrap();
        let view = runner_view(&coach, &package.runner.runner_id).unwrap();
        let names = course_names(&view);
        assert!(names.iter().any(|n| n.trim() == "Corredor 1"));
        assert!(names.iter().all(|n| !n.trim().is_empty()));
    }

    #[test]
    fn a_summary_only_race_is_listed_apart() {
        let (runner, result_id, coach, runner_id) = received(ShareLevel::Aggregates, false);
        let view = runner_view(&coach, &runner_id).unwrap();
        assert!(list_races(&view.store).unwrap().is_empty());
        assert_eq!(view.summary_only.len(), 1);
        let mine = race_detail(&runner, result_id).unwrap();
        assert_eq!(
            view.summary_only[0].summary.lost_time_s,
            mine.report.lost_time.lost_time_s
        );
    }

    #[test]
    fn the_group_crosses_the_runners_in_the_same_race() {
        let (_, _, mut coach, runner_id) = received(ShareLevel::Legs, false);
        // Otro corredor del mismo recorrido, con su propio paquete.
        let mut other = RacePackage::parse(&coach.received_packages().unwrap()[0].content).unwrap();
        other.runner.runner_id = "otro".into();
        other.runner.display_name = "Berta".into();
        if let Some(course) = other.course.as_mut() {
            course.result.result_index = 0;
        }
        coach.save_received_package(&other).unwrap();

        let group = group_view(&mut coach, &HistoryFilter::default(), false, None).unwrap();
        let names: Vec<&str> = group
            .runners
            .iter()
            .map(|r| r.runner.display_name.as_str())
            .collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"Berta"));
        assert!(group.runners.iter().all(|r| r.row.is_some() && !r.is_self));
        assert_eq!(group.comparison.shared_races.len(), 1);
        assert_eq!(group.comparison.shared_races[0].results.len(), 2);
        assert_eq!(group.comparison.head_to_head.len(), 2);
        let mine = group
            .runners
            .iter()
            .position(|r| r.runner.runner_id == runner_id)
            .unwrap();
        let pair = group
            .comparison
            .head_to_head
            .iter()
            .find(|p| p.runner == mine)
            .unwrap();
        assert_eq!(pair.races, 1);
        assert_eq!(pair.better + pair.worse, 1);
    }

    #[test]
    fn with_include_self_the_own_races_count_as_one_more_athlete() {
        // Un atleta comparte su carrera; quien entrena corrió la misma y la tiene importada.
        let (_, _, athlete_coach, runner_id) = received(ShareLevel::Legs, false);
        let package = &athlete_coach.received_packages().unwrap()[0];
        let (mut both, _) = imported(false);
        both.save_received_package(&RacePackage::parse(&package.content).unwrap())
            .unwrap();
        let filter = HistoryFilter::default();

        let without = group_view(&mut both, &filter, false, None).unwrap();
        assert!(!without.include_self);
        assert_eq!(without.runners.len(), 1);
        assert_eq!(without.runners[0].runner.runner_id, runner_id);
        // Sola, ninguna carrera compartida.
        assert!(without.comparison.shared_races.is_empty());

        let with = group_view(&mut both, &filter, true, None).unwrap();
        assert!(with.include_self);
        assert_eq!(with.runners.len(), 2);
        let me = &with.runners[0];
        assert!(me.is_self && me.row.is_some());
        assert_eq!(me.runner.runner_id, both.package_runner_id().unwrap());
        assert_eq!(me.runner.races, 1);
        // La carrera propia se cruza con la del atleta: es la misma.
        assert_eq!(with.comparison.shared_races.len(), 1);
        assert_eq!(with.comparison.shared_races[0].results.len(), 2);
        assert_eq!(with.comparison.head_to_head.len(), 2);
    }

    #[test]
    fn including_oneself_without_races_adds_nobody() {
        let (_, _, mut coach, _) = received(ShareLevel::Legs, false);
        let group = group_view(&mut coach, &HistoryFilter::default(), true, None).unwrap();
        assert_eq!(group.runners.len(), 1);
        assert!(!group.runners[0].is_self);
    }

    #[test]
    fn a_race_is_new_if_it_first_arrived_after_the_last_visit() {
        let at = |s: &str| s.parse::<DateTime<Utc>>().unwrap();
        let received = at("2026-10-05T10:00:00Z");
        // Nunca visto: todo es nuevo.
        assert!(is_new(received, None));
        assert!(is_new(received, Some(at("2026-10-05T09:59:59Z"))));
        // Recibido justo al entrar o antes: ya visto.
        assert!(!is_new(received, Some(received)));
        assert!(!is_new(received, Some(at("2026-10-06T08:00:00Z"))));
    }

    /// Cuántas carreras actualizadas (ya vistas) marca la tarjeta del atleta `runner_id`.
    fn updated_count_of(coach: &Store, runner_id: &str) -> usize {
        athlete_cards(coach)
            .unwrap()
            .iter()
            .find(|c| c.runner.runner_id == runner_id)
            .unwrap()
            .updated_races
    }

    /// Cuántas carreras nuevas hay del atleta `runner_id`, según las novedades y su tarjeta.
    fn new_count(coach: &Store, runner_id: &str) -> (usize, usize) {
        let news = athlete_news(coach).unwrap();
        let cards = athlete_cards(coach).unwrap();
        let of_news = news
            .iter()
            .find(|n| n.runner.runner_id == runner_id)
            .unwrap();
        let of_card = cards
            .iter()
            .find(|c| c.runner.runner_id == runner_id)
            .unwrap();
        (of_news.new_races.len(), of_card.new_races)
    }

    #[test]
    fn the_new_races_count_drops_when_entering_the_athlete() {
        let (_, _, mut coach, runner_id) = received(ShareLevel::Legs, false);
        let package = RacePackage::parse(&coach.received_packages().unwrap()[0].content).unwrap();

        // Nunca se ha entrado en él: su carrera es nueva, con su fecha y su nombre.
        let news = athlete_news(&coach).unwrap();
        assert_eq!(news.len(), 1);
        assert_eq!(news[0].last_seen, None);
        assert_eq!(news[0].new_races.len(), 1);
        assert_eq!(news[0].new_races[0].date, package.race.date);
        assert_eq!(news[0].new_races[0].name, package.race.name);
        assert_eq!(new_count(&coach, &runner_id), (1, 1));

        // Al entrar, baja a cero.
        let view = enter_runner(&mut coach, &runner_id).unwrap();
        assert_eq!(list_races(&view.store).unwrap().len(), 1);
        assert!(athlete_news(&coach).unwrap()[0].last_seen.is_some());
        assert_eq!(new_count(&coach, &runner_id), (0, 0));

        // La misma carrera reexportada cambiada no es nueva: sale como actualizada.
        std::thread::sleep(std::time::Duration::from_millis(5));
        let mut changed = package.clone();
        changed.exported_at += chrono::Duration::seconds(1);
        changed.tags.clear();
        assert_eq!(
            coach.save_received_package(&changed).unwrap(),
            tramos_store::SaveOutcome::Replaced
        );
        assert_eq!(new_count(&coach, &runner_id), (0, 0));
        assert_eq!(updated_count_of(&coach, &runner_id), 1);
        // Otra carrera sí es nueva.
        let mut other = package.clone();
        other.race.race_id = "otra".into();
        coach.save_received_package(&other).unwrap();
        assert_eq!(new_count(&coach, &runner_id), (1, 1));
        assert_eq!(updated_count_of(&coach, &runner_id), 1);

        // Otro atleta no cuenta para este, y entrar en él no toca lo de este.
        let mut someone = package.clone();
        someone.runner.runner_id = "otro".into();
        coach.save_received_package(&someone).unwrap();
        enter_runner(&mut coach, "otro").unwrap();
        assert_eq!(new_count(&coach, "otro"), (0, 0));
        assert_eq!(new_count(&coach, &runner_id), (1, 1));
    }

    #[test]
    fn an_updated_race_is_marked_on_entering_and_not_counted_as_new() {
        let (_, _, mut coach, runner_id) = received(ShareLevel::Legs, false);
        let package = RacePackage::parse(&coach.received_packages().unwrap()[0].content).unwrap();
        // Primera visita: nada estaba visto antes, así que nada sale como actualizado.
        let first = enter_runner(&mut coach, &runner_id).unwrap();
        assert!(first.updated_results.is_empty() && first.visited_before.is_none());

        // Ya vista, la carrera vuelve cambiada: no es nueva y la tarjeta la cuenta aparte.
        std::thread::sleep(std::time::Duration::from_millis(5));
        let mut changed = package.clone();
        changed.exported_at += chrono::Duration::seconds(1);
        changed.tags.clear();
        coach.save_received_package(&changed).unwrap();
        assert_eq!(new_count(&coach, &runner_id), (0, 0));
        assert_eq!(updated_count_of(&coach, &runner_id), 1);

        // Al entrar sale marcada y las cuentas vuelven a cero.
        let view = enter_runner(&mut coach, &runner_id).unwrap();
        assert!(view.visited_before.is_some());
        let results: Vec<i64> = list_races(&view.store)
            .unwrap()
            .iter()
            .map(|r| r.result_id)
            .collect();
        assert_eq!(results.len(), 1);
        assert_eq!(view.updated_results, results);
        assert_eq!(view.info().updated_results, results);
        assert_eq!(updated_count_of(&coach, &runner_id), 0);

        // Si llega algo mientras se ve, al volcar de nuevo la marca se conserva.
        let again = refresh_runner(&mut coach, &view).unwrap();
        assert_eq!(again.updated_results.len(), 1);
        assert_eq!(updated_count_of(&coach, &runner_id), 0);
        // Y la siguiente visita ya no la marca.
        let next = enter_runner(&mut coach, &runner_id).unwrap();
        assert!(next.updated_results.is_empty());
    }

    #[test]
    fn a_package_is_updated_only_if_it_was_seen_and_replaced_later() {
        let at = |s: &str| s.parse::<DateTime<Utc>>().unwrap();
        let package = |first: &str, last: &str| ReceivedPackage {
            runner_id: "a".into(),
            race_id: "r".into(),
            level: ShareLevel::Legs,
            format_version: 1,
            exported_at: at(last),
            imported_at: at(last),
            first_imported_at: at(first),
            content: String::new(),
        };
        let seen = Some(at("2026-10-05T12:00:00Z"));
        // Llegó antes de la visita y se sustituyó después: actualizada, y no nueva.
        let updated = package("2026-10-05T10:00:00Z", "2026-10-06T10:00:00Z");
        assert!(is_updated(&updated, seen));
        assert!(!is_new(updated.first_imported_at, seen));
        // Llegó después de la visita: nueva, aunque se haya sustituido (no es actualizada).
        let new = package("2026-10-05T13:00:00Z", "2026-10-06T10:00:00Z");
        assert!(!is_updated(&new, seen));
        assert!(is_new(new.first_imported_at, seen));
        // Vista y sin cambios desde entonces.
        assert!(!is_updated(
            &package("2026-10-05T10:00:00Z", "2026-10-05T11:00:00Z"),
            seen
        ));
        // Sin visita previa todo es nuevo y nada es actualizado.
        assert!(!is_updated(&updated, None));
    }

    #[test]
    fn the_card_has_the_same_numbers_as_the_row_in_compare_athletes() {
        let (_, _, mut coach, runner_id) = received(ShareLevel::Legs, false);
        let mut other = RacePackage::parse(&coach.received_packages().unwrap()[0].content).unwrap();
        other.runner.runner_id = "otro".into();
        other.runner.display_name = "Berta".into();
        if let Some(course) = other.course.as_mut() {
            course.result.result_index = 0;
        }
        coach.save_received_package(&other).unwrap();
        let juniors = coach
            .create_athlete_group("Juveniles", "", "#2563eb")
            .unwrap();
        coach.add_athlete_group_member(juniors, &runner_id).unwrap();

        let group = group_view(&mut coach, &HistoryFilter::default(), false, None).unwrap();
        let cards = athlete_cards(&coach).unwrap();
        assert_eq!(cards.len(), 2);
        for card in &cards {
            let row = group
                .runners
                .iter()
                .find(|r| r.runner.runner_id == card.runner.runner_id)
                .unwrap();
            assert_eq!(card.row, row.row);
            assert!(card.row.is_some() && card.problem.is_none());
            assert_eq!(card.runner, row.runner);
            // Una carrera: la línea es un punto, su IR, que es también el IR medio.
            let stats = card.row.as_ref().unwrap().stats;
            assert_eq!(card.trend.len(), 1);
            assert_eq!(Some(card.trend[0].performance), stats.mean_performance);
            assert_eq!(Some(card.trend[0].date), card.last_race);
        }
        let mine = cards
            .iter()
            .find(|c| c.runner.runner_id == runner_id)
            .unwrap();
        assert_eq!(mine.groups, vec![juniors.0]);
        let berta = cards.iter().find(|c| c.runner.runner_id == "otro").unwrap();
        assert!(berta.groups.is_empty());
    }

    #[test]
    fn the_trend_keeps_the_last_races_from_oldest_to_newest() {
        let (_, _, mut coach, runner_id) = received(ShareLevel::Legs, false);
        let package = RacePackage::parse(&coach.received_packages().unwrap()[0].content).unwrap();
        // La misma carrera en 12 fechas distintas: la línea se queda con las 10 últimas.
        for days in 1..12 {
            let mut copy = package.clone();
            copy.race.race_id = format!("copia-{days}");
            copy.race.date = package.race.date + chrono::Duration::days(days);
            coach.save_received_package(&copy).unwrap();
        }
        let cards = athlete_cards(&coach).unwrap();
        let card = cards
            .iter()
            .find(|c| c.runner.runner_id == runner_id)
            .unwrap();
        assert_eq!(card.row.as_ref().unwrap().stats.races, 12);
        assert_eq!(card.trend.len(), TREND_RACES);
        let dates: Vec<NaiveDate> = card.trend.iter().map(|p| p.date).collect();
        let expected: Vec<NaiveDate> = (2..12)
            .map(|days| package.race.date + chrono::Duration::days(days))
            .collect();
        assert_eq!(dates, expected);
        assert_eq!(card.last_race, dates.last().copied());
    }

    #[test]
    fn an_unknown_runner_is_an_error() {
        let coach = Store::open_in_memory().unwrap();
        assert!(coach_runners(&coach).unwrap().is_empty());
        assert!(matches!(
            runner_view(&coach, "nadie"),
            Err(CoachError::UnknownRunner)
        ));
    }

    /// Otro atleta de la misma carrera: el paquete de `base` con otro `runner_id` y otro de los
    /// resultados de su categoría (con nombre) como suyo. Es lo que mandaría su app: la misma
    /// carrera, su propia copia reducida.
    fn package_of_another(
        base: &RacePackage,
        runner_id: &str,
        name: &str,
        nth: usize,
    ) -> RacePackage {
        let mut package = base.clone();
        package.runner.runner_id = runner_id.into();
        package.runner.display_name = name.into();
        let course = package.course.as_mut().unwrap();
        let class = &course.event.classes[course.result.class_index];
        let named: Vec<usize> = (0..class.results.len())
            .filter(|&i| i != course.result.result_index)
            .filter(|&i| !class.results[i].runner.family_name.is_empty())
            .collect();
        course.result.result_index = named[nth];
        package
    }

    fn enable_coaching(store: &mut Store) {
        let mut s = settings::load(store).unwrap();
        s.sharing.coach = true;
        settings::save(store, &s).unwrap();
    }

    /// El único resultado de la única carrera de la base.
    fn only_result(store: &Store) -> i64 {
        list_races(store).unwrap()[0].result_id
    }

    #[test]
    fn two_athletes_of_the_same_race_mark_each_other() {
        let (_, _, mut coach, first_id) = received(ShareLevel::Legs, false);
        let first = RacePackage::parse(&coach.received_packages().unwrap()[0].content).unwrap();
        let second = package_of_another(&first, "otro", "Berta", 0);
        assert_eq!(second.race.race_id, first.race.race_id);
        coach.save_received_package(&second).unwrap();
        enable_coaching(&mut coach);

        // El nombre del .spl de cada uno, para comprobar que se marca a quien toca.
        let spl_name = |p: &RacePackage| {
            let course = p.course.as_ref().unwrap();
            let r = course.result.get(&course.event).unwrap();
            format!("{} {}", r.runner.given_name, r.runner.family_name)
        };
        let marked_name = |view: &RunnerView, mark: &AthleteMark| {
            let (event_id, _) = view
                .store
                .result_ref(ResultId(only_result(&view.store)))
                .unwrap();
            let event = view.store.load_event(event_id).unwrap();
            let r = mark.result.get(&event).unwrap();
            format!("{} {}", r.runner.given_name, r.runner.family_name)
        };

        let view_first = runner_view(&coach, &first_id).unwrap();
        let marks =
            marked_athletes(&coach, Some(&view_first), only_result(&view_first.store)).unwrap();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].runner_id, "otro");
        assert_eq!(marks[0].display_name, "Berta");
        assert_eq!(marked_name(&view_first, &marks[0]), spl_name(&second));

        let view_second = runner_view(&coach, "otro").unwrap();
        let marks =
            marked_athletes(&coach, Some(&view_second), only_result(&view_second.store)).unwrap();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].runner_id, first_id);
        assert_eq!(marked_name(&view_second, &marks[0]), spl_name(&first));
    }

    #[test]
    fn the_coach_sees_her_athletes_marked_in_her_own_race() {
        let (_, _, athletes, _) = received(ShareLevel::Legs, false);
        let base = RacePackage::parse(&athletes.received_packages().unwrap()[0].content).unwrap();
        // Quien entrena corrió la misma carrera y la tiene importada; sus atletas son otros dos.
        let (mut coach, own_result) = imported(false);
        coach
            .save_received_package(&package_of_another(&base, "ana", "Ana", 0))
            .unwrap();
        coach
            .save_received_package(&package_of_another(&base, "berta", "Berta", 1))
            .unwrap();
        enable_coaching(&mut coach);

        let marks = marked_athletes(&coach, None, own_result).unwrap();
        let names: Vec<&str> = marks.iter().map(|m| m.display_name.as_str()).collect();
        assert_eq!(names, ["Ana", "Berta"]);
        assert_ne!(marks[0].result, marks[1].result);
        // Ninguno es ella misma.
        let (_, own) = coach.result_ref(ResultId(own_result)).unwrap();
        assert!(marks.iter().all(|m| m.result != own));
    }

    #[test]
    fn without_other_athletes_in_the_race_nothing_is_marked() {
        // Sola en su carrera: la pestaña queda como hoy.
        let (_, _, mut coach, runner_id) = received(ShareLevel::Legs, false);
        enable_coaching(&mut coach);
        let view = runner_view(&coach, &runner_id).unwrap();
        let result = only_result(&view.store);
        assert!(
            marked_athletes(&coach, Some(&view), result)
                .unwrap()
                .is_empty()
        );

        // Quien entrena corrió la misma carrera pero el atleta no es otro resultado suyo.
        let (mut own, own_result) = imported(false);
        own.save_received_package(
            &RacePackage::parse(&coach.received_packages().unwrap()[0].content).unwrap(),
        )
        .unwrap();
        enable_coaching(&mut own);
        // El atleta es el mismo resultado que el de quien entrena: no se marca a sí misma.
        assert!(marked_athletes(&own, None, own_result).unwrap().is_empty());

        // Un paquete de otra carrera no cuenta.
        let mut other_race =
            RacePackage::parse(&coach.received_packages().unwrap()[0].content).unwrap();
        other_race.runner.runner_id = "otro".into();
        other_race.race.race_id = "otra-carrera".into();
        coach.save_received_package(&other_race).unwrap();
        let view = runner_view(&coach, &runner_id).unwrap();
        assert!(
            marked_athletes(&coach, Some(&view), only_result(&view.store))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn only_the_coach_marks_athletes() {
        let (_, _, mut coach, first_id) = received(ShareLevel::Legs, false);
        let first = RacePackage::parse(&coach.received_packages().unwrap()[0].content).unwrap();
        coach
            .save_received_package(&package_of_another(&first, "otro", "Berta", 0))
            .unwrap();
        let view = runner_view(&coach, &first_id).unwrap();
        // Sin «Entreno a otros atletas».
        assert!(
            marked_athletes(&coach, Some(&view), only_result(&view.store))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_package_without_names_or_with_a_summary_only_is_not_matched() {
        let (_, _, mut coach, first_id) = received(ShareLevel::Legs, false);
        let first = RacePackage::parse(&coach.received_packages().unwrap()[0].content).unwrap();
        // Anterior a #118: sin nombres no hay identidad que casar.
        let mut old = package_of_another(&first, "viejo", "Viejo", 0);
        for r in old
            .course
            .as_mut()
            .unwrap()
            .event
            .classes
            .iter_mut()
            .flat_map(|c| c.results.iter_mut())
        {
            r.runner.given_name.clear();
            r.runner.family_name.clear();
            r.runner.club = None;
        }
        // Solo el resumen: no hay recorrido.
        let mut summary = package_of_another(&first, "resumen", "Resumen", 1);
        summary.level = ShareLevel::Aggregates;
        summary.course = None;
        coach.save_received_package(&old).unwrap();
        coach.save_received_package(&summary).unwrap();
        enable_coaching(&mut coach);
        let view = runner_view(&coach, &first_id).unwrap();
        assert!(
            marked_athletes(&coach, Some(&view), only_result(&view.store))
                .unwrap()
                .is_empty()
        );
    }
}
