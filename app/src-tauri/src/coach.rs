//! Modo entrenadora (#37, `docs/app.md`, "Modo entrenadora"): ver las carreras de un corredor
//! como las ve él, en solo lectura, a partir de los paquetes recibidos (`docs/paquete.md`).
//!
//! Los paquetes de un corredor se vuelcan en una base en memoria con la misma forma que la de su
//! app: cada carrera con sus originales del recorrido, su resultado vinculado a «su» persona, el
//! formato, las etiquetas, el track y sus umbrales. Así todas las vistas de corredor (lista,
//! carrera, mapa, histórico) funcionan sin cambios y recalculan con la versión del algoritmo de
//! esta app. Lo que no se puede volcar (paquetes con solo el resumen) va aparte.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use thiserror::Error;
use tramos_core::package::{RacePackage, RaceSummary};
use tramos_core::race_format::RaceFormat;
use tramos_store::{ReceivedRunner, Store, StoreError};

use crate::import::SELF_PERSON_KEY;
use crate::settings::{self, AppMode};

/// Errores del modo entrenadora. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum CoachError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("no hay paquetes de ese corredor")]
    UnknownRunner,
}

/// Un corredor en el selector de la entrenadora.
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

/// Lo que ve la entrenadora de un corredor.
#[derive(Debug)]
pub struct RunnerView {
    pub runner: CoachRunner,
    /// Sus carreras con tramos, volcadas como en su app.
    pub store: Store,
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
    let mut latest: Option<RacePackage> = None;
    for received in main.received_packages_of(runner_id)? {
        let package = match RacePackage::parse(&received.content) {
            Ok(package) => package,
            Err(e) => {
                problems.push(e.to_string());
                continue;
            }
        };
        if let Err(e) = add_race(&mut store, person, &package, &mut summary_only) {
            problems.push(format!(
                "{} {}: {e}",
                package.race.date,
                race_name(&package)
            ));
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
        summary_only,
        problems,
    })
}

fn race_name(package: &RacePackage) -> &str {
    package.race.name.as_deref().unwrap_or("carrera sin nombre")
}

/// Vuelca una carrera del paquete; si solo trae el resumen, la apunta en `summary_only`.
fn add_race(
    store: &mut Store,
    person: tramos_store::PersonId,
    package: &RacePackage,
    summary_only: &mut Vec<SummaryRace>,
) -> Result<(), StoreError> {
    let Some(course) = &package.course else {
        summary_only.push(SummaryRace {
            date: package.race.date,
            name: package.race.name.clone(),
            format: package.race.format,
            summary: package.summary.clone(),
        });
        return Ok(());
    };
    let mut event = course.event.clone();
    event.name.clone_from(&package.race.name);
    event.date = package.race.date;
    // Los originales van sin nombres: el corredor lleva el suyo y los demás, un número.
    let mut number = 0;
    for (c, class) in event.classes.iter_mut().enumerate() {
        for (r, result) in class.results.iter_mut().enumerate() {
            if c == course.result.class_index && r == course.result.result_index {
                result
                    .runner
                    .given_name
                    .clone_from(&package.runner.display_name);
            } else {
                number += 1;
                result.runner.given_name = format!("Corredor {number}");
            }
            result.runner.family_name.clear();
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
    Ok(())
}

/// ¿Está la app en modo entrenadora?
pub fn is_coach(store: &Store) -> Result<bool, StoreError> {
    Ok(settings::load(store)?.sharing.mode == AppMode::Coach)
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
    use tramos_store::ResultId;

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
    fn the_others_on_the_course_are_numbered() {
        let (_, _, coach, runner_id) = received(ShareLevel::Legs, false);
        let view = runner_view(&coach, &runner_id).unwrap();
        let rows = list_races(&view.store).unwrap();
        let (event_id, _) = view.store.result_ref(ResultId(rows[0].result_id)).unwrap();
        let event = view.store.load_event(event_id).unwrap();
        let names: Vec<&str> = event
            .classes
            .iter()
            .flat_map(|c| c.results.iter().map(|r| r.runner.given_name.as_str()))
            .collect();
        assert!(names.contains(&"Corredor 1"));
        assert!(names.iter().all(|n| !n.is_empty()));
        assert!(
            view.store
                .load_track(ResultId(rows[0].result_id))
                .unwrap()
                .is_none()
        );
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
    fn an_unknown_runner_is_an_error() {
        let coach = Store::open_in_memory().unwrap();
        assert!(coach_runners(&coach).unwrap().is_empty());
        assert!(matches!(
            runner_view(&coach, "nadie"),
            Err(CoachError::UnknownRunner)
        ));
    }
}
