//! Atletas (#37, #119, `docs/app.md`, "Atletas"): quien entrena ve las carreras de un atleta
//! como las ve él, en solo lectura, a partir de los paquetes recibidos (`docs/paquete.md`).
//!
//! Los paquetes de un corredor se vuelcan en una base en memoria con la misma forma que la de su
//! app: cada carrera con sus originales del recorrido, su resultado vinculado a «su» persona, el
//! formato, las etiquetas, el track y sus umbrales. Así todas las vistas de corredor (lista,
//! carrera, mapa, histórico) funcionan sin cambios y recalculan con la versión del algoritmo de
//! esta app. Lo que no se puede volcar (paquetes con solo el resumen) va aparte.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use thiserror::Error;
use tramos_core::group::{GroupComparison, GroupEntry, GroupRow, compare, group_row};
use tramos_core::history::HistoryFilter;
use tramos_core::package::{RacePackage, RaceSummary, race_id};
use tramos_core::race_format::RaceFormat;
use tramos_store::{ReceivedRunner, ResultId, Store, StoreError};

use crate::history::history_view;
use crate::import::{SELF_PERSON_KEY, stored_self_person};
use crate::settings;

/// Errores de la sección Atletas. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum CoachError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("no hay paquetes de ese corredor")]
    UnknownRunner,
}

/// Un atleta en el selector de la sección Atletas.
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
    pub summary: RaceSummary,
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
}

/// Lo que se manda a la interfaz de [`RunnerView`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunnerViewInfo {
    pub runner: CoachRunner,
    pub summary_only: Vec<SummaryRace>,
    pub problems: Vec<String>,
}

impl RunnerView {
    pub fn info(&self) -> RunnerViewInfo {
        RunnerViewInfo {
            runner: self.runner.clone(),
            summary_only: self.summary_only.clone(),
            problems: self.problems.clone(),
        }
    }
}

/// Vuelca los paquetes del corredor `runner_id` en una base en memoria.
pub fn runner_view(main: &Store, runner_id: &str) -> Result<RunnerView, CoachError> {
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
    let mut latest: Option<RacePackage> = None;
    for received in main.received_packages_of(runner_id)? {
        let package = match RacePackage::parse(&received.content) {
            Ok(package) => package,
            Err(e) => {
                problems.push(e.to_string());
                continue;
            }
        };
        match add_race(&mut store, person, &package, &mut summary_only) {
            Ok(Some(result)) => {
                race_ids.insert(result.0, package.race.race_id.clone());
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
    summary_only: &mut Vec<SummaryRace>,
) -> Result<Option<ResultId>, StoreError> {
    let Some(course) = &package.course else {
        summary_only.push(SummaryRace {
            date: package.race.date,
            name: package.race.name.clone(),
            format: package.race.format,
            summary: package.summary.clone(),
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
    /// Si las carreras propias cuentan (ajuste «Incluirme»).
    pub include_self: bool,
}

/// La vista de grupo con todos los corredores de los que hay paquetes y, con `include_self`,
/// las carreras propias como un atleta más. Cada uno se calcula como en su histórico
/// (`history_view`), con sus umbrales.
pub fn group_view(
    main: &mut Store,
    filter: &HistoryFilter,
    include_self: bool,
) -> Result<GroupView, CoachError> {
    let mut runners = Vec::new();
    let mut entries = Vec::new();
    if include_self && let Some(me) = own_runner(main)? {
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
    for runner in coach_runners(main)? {
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
    Ok(GroupView {
        runners,
        comparison: compare(&entries),
        include_self,
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
            (
                Some(group_row(
                    &h.history,
                    &h.by_leg_length,
                    &h.by_slope,
                    &h.common_errors,
                )),
                None,
            )
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

/// Quien usa la app como corredor del grupo: su identificador de paquetes, su nombre y sus
/// carreras. `None` si aún no tiene ninguna.
fn own_runner(main: &mut Store) -> Result<Option<CoachRunner>, StoreError> {
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
fn own_race_ids(main: &Store) -> Result<HashMap<i64, String>, StoreError> {
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

        let group = group_view(&mut coach, &HistoryFilter::default(), false).unwrap();
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

        let without = group_view(&mut both, &filter, false).unwrap();
        assert!(!without.include_self);
        assert_eq!(without.runners.len(), 1);
        assert_eq!(without.runners[0].runner.runner_id, runner_id);
        // Sola, ninguna carrera compartida.
        assert!(without.comparison.shared_races.is_empty());

        let with = group_view(&mut both, &filter, true).unwrap();
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
        let group = group_view(&mut coach, &HistoryFilter::default(), true).unwrap();
        assert_eq!(group.runners.len(), 1);
        assert!(!group.runners[0].is_self);
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
}
