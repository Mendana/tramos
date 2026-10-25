//! Carreras de los atletas (#163, `docs/app.md`, "Atletas"): quien entrena ve juntos a los
//! atletas que corrieron la misma carrera, la que tiene el mismo `race_id` en sus paquetes
//! (`docs/paquete.md`). Si quien entrena también la corrió, cuenta como uno más.
//!
//! Cada atleta se mide como en su vista de carrera: su base volcada (`coach::runner_view`), con
//! sus umbrales y sus etiquetas, o la propia. Los números de la carrera salen del núcleo
//! (`tramos_core::same_race`) y el mapa, de `race_map` de cada uno (`race_map::course_map`).
//! Con sus tracks, las opciones de ruta de cada tramo (#190, `tramos_core::route_choice`).

use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDate;
use serde::Serialize;
use tramos_core::identify::ResultRef;
use tramos_core::lost_time::LostTimeConfig;
use tramos_core::metrics::{LegMetrics, MetricsOptions, leg_metrics};
use tramos_core::model::Event;
use tramos_core::package::{RacePackage, RaceSummary, ShareLevel, race_id};
use tramos_core::race_format::RaceFormat;
use tramos_core::route_choice::{RouteChoice, RouteOptions, RouteTrack, route_choice};
use tramos_core::same_race::{SameRaceCourse, SameRaceInput, same_race};
use tramos_core::taxonomy::LegTag;
use tramos_store::{ResultId, Store, StoreError};

use crate::coach::{
    CoachError, CoachRunner, RunnerView, coach_runners, is_coach, own_race_ids, own_runner,
    runner_view,
};
use crate::race_map::{CourseMap, RaceMap, Segmented, course_map, race_map_and_legs};
use crate::settings;
use crate::tags::result_tags;

/// Un atleta que corrió una carrera.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaceRunner {
    pub runner: CoachRunner,
    /// Es quien usa la app, con su carrera propia.
    pub is_self: bool,
    /// Lo que compartió de ella; `None` en la propia, de la que está todo.
    pub level: Option<ShareLevel>,
}

/// Una carrera recibida en la lista de Carreras.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AthleteRaceRow {
    pub race_id: String,
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
    /// Quien usa la app primero, si la corrió; después, como en `coach_runners`.
    pub athletes: Vec<RaceRunner>,
}

/// Las carreras de las que hay paquetes, de la más reciente a la más antigua (a igual fecha, por
/// nombre), con los atletas que las corrieron. Solo lee lo necesario de cada paquete.
pub fn athlete_races(main: &mut Store) -> Result<Vec<AthleteRaceRow>, CoachError> {
    let me = own_runner(main)?;
    let own = match &me {
        Some(_) => own_race_ids(main)?,
        None => HashMap::new(),
    };
    let runners = coach_runners(main)?;
    let order: HashMap<&str, usize> = runners
        .iter()
        .enumerate()
        .map(|(i, r)| (r.runner_id.as_str(), i))
        .collect();

    let mut rows: BTreeMap<String, AthleteRaceRow> = BTreeMap::new();
    for race in main.received_races()? {
        let Some(&i) = order.get(race.runner_id.as_str()) else {
            continue;
        };
        let row = rows
            .entry(race.race_id.clone())
            .or_insert_with(|| AthleteRaceRow {
                race_id: race.race_id.clone(),
                date: race.date,
                name: None,
                format: None,
                athletes: Vec::new(),
            });
        // Nombre y formato: los del primero que los tenga (cada atleta elige su formato).
        if row.name.is_none() {
            row.name = race.name;
        }
        if row.format.is_none() {
            row.format = race.format;
        }
        row.athletes.push(RaceRunner {
            runner: runners[i].clone(),
            is_self: false,
            level: Some(race.level),
        });
    }
    let mut rows: Vec<AthleteRaceRow> = rows.into_values().collect();
    for row in &mut rows {
        row.athletes
            .sort_by_key(|a| order.get(a.runner.runner_id.as_str()).copied());
        if let Some(me) = &me
            && own.values().any(|id| *id == row.race_id)
        {
            row.athletes.insert(
                0,
                RaceRunner {
                    runner: me.clone(),
                    is_self: true,
                    level: None,
                },
            );
        }
    }
    rows.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.name.cmp(&b.name)));
    Ok(rows)
}

/// Un atleta en la vista de una carrera.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaceAthleteRow {
    #[serde(flatten)]
    pub athlete: RaceRunner,
    /// Su resumen: recalculado con sus umbrales si compartió los tramos; el de su paquete si solo
    /// compartió el resumen. `None` si no se ha podido leer ni analizar (`problem`).
    pub summary: Option<RaceSummary>,
    /// Su recorrido, en `AthleteRaceView::courses`. `None` sin tramos.
    pub course: Option<usize>,
    /// Puesto en su recorrido, juntando sus categorías.
    pub course_place: Option<usize>,
    /// Su resultado en su base (la volcada del atleta o la propia), para abrir su carrera.
    /// `None` sin tramos.
    pub result_id: Option<i64>,
    /// Tiene track y sale en el mapa.
    pub on_map: bool,
    /// Por qué no se ha podido leer o analizar su carrera, o por qué su track no sale en el mapa.
    pub problem: Option<String>,
}

/// Un recorrido de la carrera, con sus atletas y su mapa.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaceCourseView {
    /// Los atletas (`runner`) son posiciones en `AthleteRaceView::athletes`.
    #[serde(flatten)]
    pub course: SameRaceCourse,
    /// Los tracks de sus atletas que lo compartieron; `None` si no hay ninguno.
    pub map: Option<CourseMap>,
    /// Los tramos en los que sus atletas con track cogieron rutas distintas, en orden de tramo.
    pub routes: Vec<RouteChoice>,
}

/// Una carrera con todos los atletas que la corrieron.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AthleteRaceView {
    pub race_id: String,
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
    /// Quien usa la app primero, si la corrió; después, como en `coach_runners`.
    pub athletes: Vec<RaceAthleteRow>,
    /// Los recorridos, del que tiene más atletas al que menos (`same_race`).
    pub courses: Vec<RaceCourseView>,
}

/// De dónde salen los tramos de un atleta: su resultado en la base propia o en la volcada.
#[derive(Clone, Copy)]
enum Source {
    Own(ResultId),
    Athlete(usize, ResultId),
}

/// Lo que necesita el núcleo de un atleta, leído de su base.
struct Loaded {
    athlete: usize,
    event: Event,
    at: ResultRef,
    config: LostTimeConfig,
    tags: Vec<(usize, LegTag)>,
}

fn load(store: &Store, result: ResultId, athlete: usize) -> Result<Loaded, StoreError> {
    let (event_id, at) = store.result_ref(result)?;
    Ok(Loaded {
        athlete,
        event: store.load_event(event_id)?,
        at,
        config: settings::lost_time_config(store)?,
        tags: result_tags(store, result)?,
    })
}

/// La carrera `race_id` con todos los atletas que la corrieron. Si nadie la ha compartido,
/// [`CoachError::UnknownRace`].
pub fn athlete_race(main: &mut Store, race_id: &str) -> Result<AthleteRaceView, CoachError> {
    let me = own_runner(main)?;
    let own_result = match &me {
        Some(_) => own_race_ids(main)?
            .into_iter()
            .filter(|(_, id)| id == race_id)
            .map(|(result, _)| result)
            .min(),
        None => None,
    };
    let main: &Store = main;
    let runners = coach_runners(main)?;
    let mut packages: Vec<(usize, RacePackage)> = Vec::new();
    let mut unreadable: Vec<(usize, String)> = Vec::new();
    for received in main.received_packages_of_race(race_id)? {
        let Some(i) = runners
            .iter()
            .position(|r| r.runner_id == received.runner_id)
        else {
            continue;
        };
        match RacePackage::parse(&received.content) {
            Ok(package) => packages.push((i, package)),
            Err(e) => unreadable.push((i, e.to_string())),
        }
    }
    if packages.is_empty() && unreadable.is_empty() {
        return Err(CoachError::UnknownRace);
    }
    packages.sort_by_key(|(i, _)| *i);
    unreadable.sort_by_key(|(i, _)| *i);

    let first = packages.first().map(|(_, p)| &p.race);
    let mut view = AthleteRaceView {
        race_id: race_id.to_string(),
        date: first.map_or_else(NaiveDate::default, |r| r.date),
        name: packages.iter().find_map(|(_, p)| p.race.name.clone()),
        format: packages.iter().find_map(|(_, p)| p.race.format),
        athletes: Vec::new(),
        courses: Vec::new(),
    };

    // Las filas y de dónde sale cada una.
    let mut sources: Vec<Option<Source>> = Vec::new();
    let mut views: Vec<RunnerView> = Vec::new();
    let row = |runner: CoachRunner, is_self, level, summary, problem| RaceAthleteRow {
        athlete: RaceRunner {
            runner,
            is_self,
            level,
        },
        summary,
        course: None,
        course_place: None,
        result_id: None,
        on_map: false,
        problem,
    };
    if let (Some(me), Some(result)) = (&me, own_result) {
        view.athletes.push(row(me.clone(), true, None, None, None));
        sources.push(Some(Source::Own(ResultId(result))));
    }
    for (i, package) in &packages {
        let runner = runners[*i].clone();
        if package.course.is_none() {
            let summary = Some(package.summary.clone());
            view.athletes
                .push(row(runner, false, Some(package.level), summary, None));
            sources.push(None);
            continue;
        }
        let base = runner_view(main, &runner.runner_id)?;
        let result = base
            .race_ids
            .iter()
            .filter(|(_, id)| *id == race_id)
            .map(|(result, _)| *result)
            .min();
        let problem = result.is_none().then(|| {
            base.problems
                .first()
                .cloned()
                .unwrap_or_else(|| "su carrera no se ha podido abrir".to_string())
        });
        view.athletes.push(row(
            runner,
            false,
            Some(package.level),
            Some(package.summary.clone()),
            problem,
        ));
        sources.push(result.map(|r| Source::Athlete(views.len(), ResultId(r))));
        views.push(base);
    }
    for (i, problem) in unreadable {
        view.athletes
            .push(row(runners[i].clone(), false, None, None, Some(problem)));
        sources.push(None);
    }

    let store_of = |source: Source| match source {
        Source::Own(result) => (main, result),
        Source::Athlete(v, result) => (&views[v].store, result),
    };
    let mut loaded = Vec::new();
    for (athlete, source) in sources.iter().enumerate() {
        let Some(source) = *source else { continue };
        let (store, result) = store_of(source);
        view.athletes[athlete].result_id = Some(result.0);
        loaded.push(load(store, result, athlete)?);
    }
    let inputs: Vec<SameRaceInput<'_>> = loaded
        .iter()
        .map(|l| SameRaceInput {
            event: &l.event,
            result: l.at,
            config: l.config,
            tags: &l.tags,
        })
        .collect();
    let race = same_race(&inputs);

    // Del núcleo, posiciones de la entrada; aquí, de `athletes`.
    for (input, runner) in race.runners.iter().enumerate() {
        let row = &mut view.athletes[loaded[input].athlete];
        match runner {
            Some(r) => {
                row.summary = Some(r.summary.clone());
                row.course = Some(r.course);
                row.course_place = r.course_place;
            }
            None => row.problem = Some("su carrera no se puede analizar".to_string()),
        }
    }
    for mut course in race.courses {
        for r in &mut course.runners {
            r.runner = loaded[r.runner].athlete;
        }
        for leg in &mut course.shared_errors {
            for e in &mut leg.errors {
                e.runner = loaded[e.runner].athlete;
            }
        }
        let mut maps = Vec::new();
        let mut tracks = Vec::new();
        for r in &course.runners {
            let Some(source) = sources[r.runner] else {
                continue;
            };
            let (store, result) = store_of(source);
            match race_map_and_legs(store, result.0).map_err(|e| e.to_string()) {
                Ok((RaceMap::Ready(map), legs)) => {
                    maps.push((r.runner, *map));
                    if let Some(legs) = legs {
                        let metrics = leg_metrics(
                            &legs.track,
                            &legs.segmentation,
                            &MetricsOptions::default(),
                        )
                        .unwrap_or_default();
                        tracks.push((r.runner, legs, metrics));
                    }
                }
                Ok((RaceMap::NoTrack, _)) => {}
                Ok((RaceMap::NotAligned { message }, _)) | Err(message) => {
                    view.athletes[r.runner].problem = Some(message);
                }
            }
        }
        let routes = route_choices(&course, &tracks);
        for (athlete, _) in &maps {
            view.athletes[*athlete].on_map = true;
        }
        view.courses.push(RaceCourseView {
            course,
            map: course_map(maps),
            routes,
        });
    }
    if view.date == NaiveDate::default()
        && let Some(own) = own_result
    {
        let (event_id, _) = main.result_ref(ResultId(own))?;
        view.date = main.load_event(event_id)?.date;
    }
    Ok(view)
}

/// Las opciones de ruta de cada tramo de `course` con los tracks de sus atletas (`athlete`, sus
/// tramos y sus métricas), cada uno con su split y su error de la vista de la carrera.
fn route_choices(
    course: &SameRaceCourse,
    tracks: &[(usize, Segmented, Vec<LegMetrics>)],
) -> Vec<RouteChoice> {
    course
        .legs
        .iter()
        .enumerate()
        .filter_map(|(position, leg)| {
            let inputs: Vec<RouteTrack<'_>> = tracks
                .iter()
                .filter_map(|(athlete, legs, metrics)| {
                    let runner = course.runners.iter().find(|r| r.runner == *athlete)?;
                    let same =
                        |index, from, to| index == leg.index && from == leg.from && to == leg.to;
                    let sub = legs
                        .segmentation
                        .legs
                        .iter()
                        .find(|l| same(l.index, l.from, l.to))?;
                    let measured = metrics
                        .iter()
                        .find(|m| same(m.index, m.from, m.to))
                        .and_then(|m| m.track.as_ref());
                    Some(RouteTrack {
                        runner: *athlete,
                        points: &sub.track.as_ref()?.points,
                        split_s: runner.split_s.get(position).copied().flatten(),
                        is_error: runner.is_error.get(position).copied().unwrap_or(false),
                        distance_m: measured.map(|m| m.distance_m),
                        ascent_m: measured.and_then(|m| m.ascent_m),
                    })
                })
                .collect();
            route_choice(
                leg.index,
                leg.from,
                leg.to,
                &inputs,
                &RouteOptions::default(),
            )
        })
        .collect()
}

/// Para el botón «Ver con los demás atletas» de una carrera: su `race_id` y cuántos atletas más
/// la corrieron.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RaceTogether {
    pub race_id: String,
    /// Los demás atletas de los que hay paquete de esa carrera y, si se ve a un atleta, quien usa
    /// la app si también la corrió.
    pub others: usize,
}

/// Los demás atletas que corrieron la carrera del resultado `result_id` de la base que se ve: la
/// del atleta `viewed` o, si es `None`, la propia. `None` si no entrena o si la carrera no tiene
/// `race_id` (un resultado que no es de un paquete).
pub fn race_together(
    main: &mut Store,
    viewed: Option<&RunnerView>,
    result_id: i64,
) -> Result<Option<RaceTogether>, CoachError> {
    if !is_coach(main)? {
        return Ok(None);
    }
    let id = match viewed {
        Some(view) => match view.race_ids.get(&result_id) {
            Some(id) => id.clone(),
            None => return Ok(None),
        },
        None => {
            let (event_id, _) = main.result_ref(ResultId(result_id))?;
            race_id(&main.load_event(event_id)?)
        }
    };
    let me_too = match viewed {
        Some(_) if own_runner(main)?.is_some() => own_race_ids(main)?.values().any(|r| *r == id),
        _ => false,
    };
    let others = main
        .received_races()?
        .iter()
        .filter(|r| r.race_id == id)
        .filter(|r| viewed.is_none_or(|v| v.runner.runner_id != r.runner_id))
        .count()
        + usize::from(me_too);
    Ok(Some(RaceTogether {
        race_id: id,
        others,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::race_package;
    use crate::race_map::race_map;
    use crate::race_map::tests::imported;
    use tramos_core::package::{PackageInput, PackageRunner, build};
    use tramos_core::taxonomy::{Confirmation, LegTag};

    /// Quien entrena, que también corre (`with_self`), y los paquetes de una atleta con track
    /// («Ana») y otro con solo los tramos («Beto»), los dos del mismo resultado del fixture, y
    /// una tercera («Carla») en otro recorrido de la misma carrera.
    struct Setup {
        coach: Store,
        race_id: String,
    }

    fn coach_store(with_self: bool) -> Store {
        let mut coach = if with_self {
            imported(false).0
        } else {
            Store::open_in_memory().unwrap()
        };
        let mut s = settings::load(&coach).unwrap();
        s.sharing.coach = true;
        settings::save(&mut coach, &s).unwrap();
        coach
    }

    fn setup(with_self: bool) -> Setup {
        let (mut runner, result_id) = imported(true);
        crate::tags::save_leg_tag(
            &mut runner,
            result_id,
            2,
            LegTag {
                confirmation: Some(Confirmation::Error),
                ..LegTag::default()
            },
        )
        .unwrap();
        let mut ana = race_package(&mut runner, result_id, ShareLevel::Track).unwrap();
        ana.runner.runner_id = "ana".into();
        ana.runner.display_name = "Ana".into();
        let mut beto = race_package(&mut runner, result_id, ShareLevel::Legs).unwrap();
        beto.runner.runner_id = "beto".into();
        beto.runner.display_name = "Beto".into();
        // Carla corrió la primera categoría, que es otro recorrido.
        let (event_id, at) = runner.result_ref(ResultId(result_id)).unwrap();
        let event = runner.load_event(event_id).unwrap();
        let other_class = (0..event.classes.len())
            .find(|&c| event.classes[c].course != event.classes[at.class_index].course)
            .unwrap();
        let carla = build(PackageInput {
            level: ShareLevel::Legs,
            exported_at: ana.exported_at,
            runner: PackageRunner {
                runner_id: "carla".into(),
                display_name: "Carla".into(),
            },
            event: &event,
            at: ResultRef {
                class_index: other_class,
                result_index: 0,
            },
            format: None,
            kind: tramos_core::event_kind::EventKind::Race,
            config: LostTimeConfig::default(),
            tags: Vec::new(),
            track: None,
        })
        .unwrap();
        let mut coach = coach_store(with_self);
        for p in [&ana, &beto, &carla] {
            coach.save_received_package(p).unwrap();
        }
        Setup {
            coach,
            race_id: ana.race.race_id,
        }
    }

    fn names(athletes: &[RaceRunner]) -> Vec<&str> {
        athletes
            .iter()
            .map(|a| a.runner.display_name.as_str())
            .collect()
    }

    #[test]
    fn the_list_has_one_row_per_race_with_its_athletes() {
        let Setup { mut coach, race_id } = setup(false);
        // Otra carrera, solo de Beto y solo con el resumen.
        let first =
            RacePackage::parse(&coach.received_packages_of("beto").unwrap()[0].content).unwrap();
        let mut older = first.clone();
        older.race.race_id = "otra".into();
        older.race.date = first.race.date - chrono::Duration::days(7);
        older.level = ShareLevel::Aggregates;
        older.course = None;
        older.tags.clear();
        coach.save_received_package(&older).unwrap();

        let rows = athlete_races(&mut coach).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].race_id, race_id);
        assert_eq!(rows[0].date, first.race.date);
        assert_eq!(rows[0].name, first.race.name);
        assert_eq!(names(&rows[0].athletes), ["Ana", "Beto", "Carla"]);
        let levels: Vec<_> = rows[0].athletes.iter().map(|a| a.level).collect();
        assert_eq!(
            levels,
            [
                Some(ShareLevel::Track),
                Some(ShareLevel::Legs),
                Some(ShareLevel::Legs)
            ]
        );
        assert_eq!(rows[1].race_id, "otra");
        assert_eq!(names(&rows[1].athletes), ["Beto"]);
        assert!(rows.iter().flat_map(|r| &r.athletes).all(|a| !a.is_self));
    }

    #[test]
    fn whoever_coaches_and_ran_it_counts_as_one_more() {
        let Setup { mut coach, race_id } = setup(true);
        let rows = athlete_races(&mut coach).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].athletes[0].is_self);
        assert_eq!(rows[0].athletes[0].level, None);
        assert_eq!(rows[0].athletes.len(), 4);

        let view = athlete_race(&mut coach, &race_id).unwrap();
        assert_eq!(view.athletes.len(), 4);
        let me = &view.athletes[0];
        assert!(me.athlete.is_self);
        // Es la misma carrera y el mismo resultado, con los mismos umbrales (los de por defecto):
        // el mismo resumen que Ana, salvo el tramo 2, que ella marcó como error y el cálculo no.
        let mine = me.summary.clone().unwrap();
        let ana = view.athletes[1].summary.clone().unwrap();
        assert_eq!(ana.error_count, mine.error_count + 1);
        assert_eq!(
            tramos_core::package::RaceSummary {
                error_count: ana.error_count,
                ..mine
            },
            ana
        );
        assert_eq!(me.course, Some(0));
        let own = crate::races::list_races(&coach).unwrap();
        assert_eq!(me.result_id, Some(own[0].result_id));
        assert!(!me.on_map);
    }

    #[test]
    fn the_race_puts_together_the_athletes_of_each_course() {
        let Setup { mut coach, race_id } = setup(false);
        let view = athlete_race(&mut coach, &race_id).unwrap();
        assert_eq!(view.race_id, race_id);
        let athletes: Vec<&str> = view
            .athletes
            .iter()
            .map(|a| a.athlete.runner.display_name.as_str())
            .collect();
        assert_eq!(athletes, ["Ana", "Beto", "Carla"]);
        assert!(view.athletes.iter().all(|a| a.problem.is_none()));
        assert!(view.athletes.iter().all(|a| a.result_id.is_some()));

        // Ana y Beto, en el mismo recorrido; Carla, sola en el suyo.
        assert_eq!(view.courses.len(), 2);
        let shared = &view.courses[0].course;
        let runners: Vec<usize> = shared.runners.iter().map(|r| r.runner).collect();
        assert_eq!(runners, [0, 1]);
        assert_eq!(view.athletes[0].course, Some(0));
        assert_eq!(view.athletes[2].course, Some(1));
        assert_eq!(view.courses[1].course.runners[0].runner, 2);

        // Cada uno con lo de su vista de carrera: el resumen recalculado es el de su paquete.
        for athlete in &view.athletes {
            let received = coach
                .received_packages_of(&athlete.athlete.runner.runner_id)
                .unwrap();
            let package = RacePackage::parse(&received[0].content).unwrap();
            assert_eq!(athlete.summary.as_ref(), Some(&package.summary));
        }

        // Los dos tienen el tramo 2 como error (Ana y Beto lo etiquetaron igual).
        let leg2 = shared.shared_errors.iter().find(|e| e.leg == 2).unwrap();
        let who: Vec<usize> = leg2.errors.iter().map(|e| e.runner).collect();
        assert_eq!(who, [0, 1]);
        assert!(leg2.errors.iter().all(|e| e.reviewed));

        // El mapa: solo Ana compartió el track; Beto no sale y no es un problema.
        let map = view.courses[0].map.as_ref().unwrap();
        let on_map: Vec<usize> = map.tracks.iter().map(|t| t.athlete).collect();
        assert_eq!(on_map, [0]);
        assert!(view.athletes[0].on_map && !view.athletes[1].on_map);
        let alone = match race_map(
            &runner_view(&coach, "ana").unwrap().store,
            view.athletes[0].result_id.unwrap(),
        )
        .unwrap()
        {
            RaceMap::Ready(map) => *map,
            other => panic!("se esperaba un mapa: {other:?}"),
        };
        // Con un solo track, las balizas son las suyas.
        assert_eq!(map.controls, alone.controls);
        assert_eq!(map.tracks[0].legs, alone.legs);
        assert!(view.courses[1].map.is_none());
        // Con un solo track no hay rutas que comparar.
        assert!(view.courses.iter().all(|c| c.routes.is_empty()));
    }

    #[test]
    fn a_race_with_a_single_athlete_opens_the_same() {
        let Setup { mut coach, .. } = setup(false);
        let package =
            RacePackage::parse(&coach.received_packages_of("carla").unwrap()[0].content).unwrap();
        let mut alone = package.clone();
        alone.race.race_id = "sola".into();
        coach.save_received_package(&alone).unwrap();
        let view = athlete_race(&mut coach, "sola").unwrap();
        assert_eq!(view.athletes.len(), 1);
        assert_eq!(view.courses.len(), 1);
        assert_eq!(view.courses[0].course.runners.len(), 1);
        assert!(view.courses[0].course.shared_errors.is_empty());

        assert!(matches!(
            athlete_race(&mut coach, "ninguna"),
            Err(CoachError::UnknownRace)
        ));
    }

    #[test]
    fn a_summary_only_athlete_is_in_the_table_but_not_in_the_charts() {
        let Setup { mut coach, race_id } = setup(false);
        let mut beto =
            RacePackage::parse(&coach.received_packages_of("beto").unwrap()[0].content).unwrap();
        beto.exported_at += chrono::Duration::seconds(1);
        beto.level = ShareLevel::Aggregates;
        beto.course = None;
        beto.tags.clear();
        coach.save_received_package(&beto).unwrap();

        let view = athlete_race(&mut coach, &race_id).unwrap();
        let row = &view.athletes[1];
        assert_eq!(row.athlete.level, Some(ShareLevel::Aggregates));
        assert_eq!(row.summary.as_ref(), Some(&beto.summary));
        assert_eq!((row.course, row.result_id), (None, None));
        let runners: Vec<usize> = view.courses[0]
            .course
            .runners
            .iter()
            .map(|r| r.runner)
            .collect();
        assert_eq!(runners, [0]);
        // Sin Beto, el tramo 2 ya no lo fallaron varios.
        assert!(view.courses[0].course.shared_errors.is_empty());
    }

    #[test]
    fn the_button_counts_the_other_athletes_of_the_race() {
        let Setup { mut coach, race_id } = setup(true);
        let own = crate::races::list_races(&coach).unwrap()[0].result_id;
        // Desde la propia: los tres atletas.
        let mine = race_together(&mut coach, None, own).unwrap().unwrap();
        assert_eq!(
            mine,
            RaceTogether {
                race_id: race_id.clone(),
                others: 3,
            }
        );
        // Desde la de Ana: Beto, Carla y quien entrena.
        let ana = runner_view(&coach, "ana").unwrap();
        let result = *ana.race_ids.keys().next().unwrap();
        let from_ana = race_together(&mut coach, Some(&ana), result)
            .unwrap()
            .unwrap();
        assert_eq!(from_ana.others, 3);
        assert_eq!(from_ana.race_id, race_id);

        // Sin entrenar, no hay botón.
        let mut s = settings::load(&coach).unwrap();
        s.sharing.coach = false;
        settings::save(&mut coach, &s).unwrap();
        assert_eq!(race_together(&mut coach, None, own).unwrap(), None);
    }

    #[test]
    fn a_race_nobody_else_ran_has_no_one_else() {
        let mut coach = coach_store(true);
        let own = crate::races::list_races(&coach).unwrap()[0].result_id;
        let only = race_together(&mut coach, None, own).unwrap().unwrap();
        assert_eq!(only.others, 0);
        assert!(athlete_races(&mut coach).unwrap().is_empty());
    }
}
