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
use tramos_core::group::{GroupComparison, GroupEntry, GroupRow, compare, group_row, group_total};
use tramos_core::group_compare::{
    CompareOptions, GroupsComparison, MemberRaces, MemberStats, RaceSelection, compare_groups,
    pool_side, races_of_both, sides,
};
use tramos_core::group_stats::{GroupStats, MemberAnalyses, group_stats};
use tramos_core::history::{HistoryFilter, HistoryStats};
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

/// ¿Es nuevo un paquete recibido en `received_at` para quien entró en el atleta por última vez en
/// `last_seen`? Si nunca ha entrado, todo es nuevo. Un paquete que vuelve a llegar cambiado (el
/// atleta etiquetó un error, por ejemplo) se recibe otra vez y también cuenta.
pub fn is_new(received_at: DateTime<Utc>, last_seen: Option<DateTime<Utc>>) -> bool {
    last_seen.is_none_or(|seen| received_at > seen)
}

/// Los paquetes del atleta recibidos después de `last_seen`.
fn new_packages(
    main: &Store,
    runner_id: &str,
    last_seen: Option<DateTime<Utc>>,
) -> Result<Vec<ReceivedPackage>, StoreError> {
    Ok(main
        .received_packages_of(runner_id)?
        .into_iter()
        .filter(|p| is_new(p.imported_at, last_seen))
        .collect())
}

/// Una carrera nueva de un atleta.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NewRace {
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
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
                    received_at: received.imported_at,
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
pub fn enter_runner(main: &mut Store, runner_id: &str) -> Result<RunnerView, CoachError> {
    let view = runner_view(main, runner_id)?;
    settings::set_last_seen(main, runner_id, Utc::now())?;
    Ok(view)
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
    /// Carreras recibidas desde la última vez que se entró en él ([`athlete_news`]).
    pub new_races: usize,
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
    fn a_package_is_new_if_it_arrived_after_the_last_visit() {
        let at = |s: &str| s.parse::<DateTime<Utc>>().unwrap();
        let received = at("2026-10-05T10:00:00Z");
        // Nunca visto: todo es nuevo.
        assert!(is_new(received, None));
        assert!(is_new(received, Some(at("2026-10-05T09:59:59Z"))));
        // Recibido justo al entrar o antes: ya visto.
        assert!(!is_new(received, Some(received)));
        assert!(!is_new(received, Some(at("2026-10-06T08:00:00Z"))));
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

        // Lo que llega después vuelve a contar: otra carrera y la misma reexportada cambiada.
        std::thread::sleep(std::time::Duration::from_millis(5));
        let mut changed = package.clone();
        changed.exported_at += chrono::Duration::seconds(1);
        changed.tags.clear();
        assert_eq!(
            coach.save_received_package(&changed).unwrap(),
            tramos_store::SaveOutcome::Replaced
        );
        assert_eq!(new_count(&coach, &runner_id), (1, 1));
        let mut other = package.clone();
        other.race.race_id = "otra".into();
        coach.save_received_package(&other).unwrap();
        assert_eq!(new_count(&coach, &runner_id), (2, 2));

        // Otro atleta no cuenta para este, y entrar en él no toca lo de este.
        let mut someone = package.clone();
        someone.runner.runner_id = "otro".into();
        coach.save_received_package(&someone).unwrap();
        enter_runner(&mut coach, "otro").unwrap();
        assert_eq!(new_count(&coach, "otro"), (0, 0));
        assert_eq!(new_count(&coach, &runner_id), (2, 2));
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
}
