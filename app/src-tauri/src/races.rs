//! Carreras del usuario: la lista y la vista de una carrera con su tabla de tramos.
//!
//! El tiempo perdido no se guarda: se calcula al pedirlo con [`runner_report`], la misma función
//! que usa `tramos analizar`, a partir de la carrera guardada (`docs/app.md`).

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use chrono::NaiveDate;
use serde::Serialize;
use thiserror::Error;
use tramos_core::comparison::{CourseComparison, course_comparison};
use tramos_core::loss_breakdown::{RaceBreakdown, race_breakdown as breakdown};
use tramos_core::lost_time::LostTimeConfig;
use tramos_core::model::{Event, RaceStatus};
use tramos_core::race_format::RaceFormat;
use tramos_core::runner_report::{RunnerReport, runner_report};
use tramos_store::{EventId, ResultId, Store, StoreError};

use crate::import::stored_self_person;
use crate::race_map::stored_leg_metrics;
use crate::settings;

/// Errores al consultar las carreras. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum RaceError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("el resultado {0} no aparece en el análisis de su recorrido")]
    NotAnalyzed(i64),
    #[error(transparent)]
    Slope(#[from] tramos_core::slope::SlopeError),
}

/// Una carrera del usuario en la lista.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaceRow {
    pub event_id: i64,
    pub result_id: i64,
    pub date: NaiveDate,
    pub name: Option<String>,
    pub class_name: String,
    pub status: RaceStatus,
    pub place: Option<u16>,
    pub format: Option<RaceFormat>,
    pub has_track: bool,
    /// Tiempo de salida a meta.
    pub total_s: Option<f64>,
    /// Tiempo perdido en los tramos con error.
    pub lost_time_s: Option<f64>,
    pub error_count: usize,
}

/// Una carrera del usuario con su tabla de tramos.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaceDetail {
    pub event_id: i64,
    pub result_id: i64,
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
    pub class_name: String,
    pub given_name: String,
    pub family_name: String,
    pub club: Option<String>,
    pub si_card: Option<u32>,
    pub status: RaceStatus,
    pub place: Option<u16>,
    /// Umbrales y tiempo ideal con los que se ha calculado.
    pub config: LostTimeConfig,
    pub report: RunnerReport,
}

/// Carreras del usuario (los resultados vinculados a su persona), de la más reciente a la más
/// antigua, con su tiempo perdido. Vacía si aún no ha importado ninguna.
pub fn list_races(store: &Store) -> Result<Vec<RaceRow>, RaceError> {
    let Some(person) = stored_self_person(store)? else {
        return Ok(Vec::new());
    };
    let config = settings::lost_time_config(store)?;
    let mut events: HashMap<EventId, Event> = HashMap::new();
    let mut rows = Vec::new();
    for r in store.person_results(person)? {
        let (event_id, at) = store.result_ref(r.result)?;
        // Varios resultados de la misma carrera la cargan una sola vez.
        let event = match events.entry(event_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(store.load_event(event_id)?),
        };
        let lost = runner_report(event, at, &config).map(|report| report.lost_time);
        rows.push(RaceRow {
            event_id: r.event.0,
            result_id: r.result.0,
            date: r.event_date,
            name: r.event_name,
            class_name: r.class_name,
            status: r.status,
            place: r.place,
            format: r.event_format,
            has_track: r.has_track,
            total_s: lost.as_ref().and_then(|l| l.total_s),
            lost_time_s: lost.as_ref().and_then(|l| l.lost_time_s),
            error_count: lost.as_ref().map_or(0, |l| l.error_count),
        });
    }
    rows.reverse();
    Ok(rows)
}

/// Una carrera con la tabla de tramos del resultado `result_id`.
pub fn race_detail(store: &Store, result_id: i64) -> Result<RaceDetail, RaceError> {
    let result = ResultId(result_id);
    let (event_id, at) = store.result_ref(result)?;
    let event = store.load_event(event_id)?;
    let config = settings::lost_time_config(store)?;
    let report = runner_report(&event, at, &config).ok_or(RaceError::NotAnalyzed(result_id))?;
    let (Some(class), Some(result)) = (event.classes.get(at.class_index), at.get(&event)) else {
        return Err(RaceError::NotAnalyzed(result_id));
    };
    Ok(RaceDetail {
        event_id: event_id.0,
        result_id,
        date: event.date,
        name: event.name.clone(),
        format: store.event_format(event_id)?,
        class_name: class.name.clone(),
        given_name: result.runner.given_name.clone(),
        family_name: result.runner.family_name.clone(),
        club: result.runner.club.clone(),
        si_card: result.runner.si_card,
        status: result.status,
        place: result.place,
        config,
        report,
    })
}

/// Cambia el formato de la carrera del resultado `result_id`; `None` lo deja sin asignar. Vale
/// para toda la carrera (el formato es de la carrera, no del resultado).
pub fn set_race_format(
    store: &mut Store,
    result_id: i64,
    format: Option<RaceFormat>,
) -> Result<(), RaceError> {
    let (event_id, _) = store.result_ref(ResultId(result_id))?;
    store.set_event_format(event_id, format)?;
    Ok(())
}

/// Corredores del recorrido del resultado `result_id`, para compararse con ellos (P4).
pub fn race_comparison(store: &Store, result_id: i64) -> Result<CourseComparison, RaceError> {
    let (event_id, at) = store.result_ref(ResultId(result_id))?;
    let event = store.load_event(event_id)?;
    let config = settings::lost_time_config(store)?;
    course_comparison(&event, at, &config).ok_or(RaceError::NotAnalyzed(result_id))
}

/// ¿Lento o desorientado? (P2) del resultado `result_id`: la pérdida de sus tramos repartida en
/// desvío, paradas y ritmo. `None` sin track (o si ya no se puede alinear ni trocear).
pub fn race_breakdown(store: &Store, result_id: i64) -> Result<Option<RaceBreakdown>, RaceError> {
    let result = ResultId(result_id);
    let (event_id, at) = store.result_ref(result)?;
    let event = store.load_event(event_id)?;
    let config = settings::lost_time_config(store)?;
    let report = runner_report(&event, at, &config).ok_or(RaceError::NotAnalyzed(result_id))?;
    let race_result = at.get(&event).ok_or(RaceError::NotAnalyzed(result_id))?;
    let metrics = stored_leg_metrics(store, result, race_result)?;
    Ok(metrics.map(|m| breakdown(&report.lost_time, &m)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{ImportRequest, import, preview};
    use tramos_core::identify::{ResultRef, RunnerIdentity};
    use tramos_core::importers::spl;

    fn spl_path() -> String {
        format!(
            "{}/../../fixtures/spl/baltanas-anon.spl",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn identity() -> RunnerIdentity {
        RunnerIdentity {
            si_card: Some(143),
            full_name: Some("N143 Apellido143".into()),
        }
    }

    /// Importa el fixture de Baltanás como el corredor de la tarjeta 143.
    fn imported() -> (Store, i64) {
        let mut store = Store::open_in_memory().unwrap();
        let p = preview(&store, &spl_path(), None, &identity()).unwrap();
        let outcome = import(
            &mut store,
            &ImportRequest {
                spl_path: spl_path(),
                fit_path: None,
                result: p.candidates[0].result,
                format: p.suggested_format,
                identity: identity(),
            },
        )
        .unwrap();
        (store, outcome.result_id)
    }

    /// Criterio de aceptación de #19: la tabla es la de `tramos analizar` (que usa la misma
    /// función sobre el .spl leído directamente), también después de guardar y cargar.
    #[test]
    fn detail_matches_the_analysis_of_the_file() {
        let (store, result_id) = imported();
        let detail = race_detail(&store, result_id).unwrap();

        let event = spl::read(&std::fs::read(spl_path()).unwrap()).unwrap();
        let at = ResultRef {
            class_index: 9,
            result_index: 15,
        };
        assert_eq!(event.classes[9].name, "M-SEN");
        let expected = runner_report(&event, at, &LostTimeConfig::default()).unwrap();
        assert_eq!(detail.report, expected);
        assert_eq!(detail.report.lost_time.legs.len(), 21);
        assert_eq!(
            (detail.class_name.as_str(), detail.place, detail.si_card),
            ("M-SEN", Some(16), Some(143))
        );
        assert_eq!(detail.format, Some(RaceFormat::Sprint));
        assert_eq!(detail.config, LostTimeConfig::default());
    }

    #[test]
    fn list_shows_lost_time() {
        let (store, result_id) = imported();
        let detail = race_detail(&store, result_id).unwrap();
        let rows = list_races(&store).unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.result_id, result_id);
        assert_eq!(row.total_s, detail.report.lost_time.total_s);
        assert_eq!(row.lost_time_s, detail.report.lost_time.lost_time_s);
        assert_eq!(row.error_count, detail.report.lost_time.error_count);
        assert!(row.total_s.is_some() && row.lost_time_s.is_some());
    }

    /// Criterio de aceptación de #17: cambiar el umbral cambia los tramos marcados como error.
    #[test]
    fn thresholds_from_settings_change_the_errors() {
        let (mut store, result_id) = imported();
        let errors = |store: &Store| {
            let detail = race_detail(store, result_id).unwrap();
            let marked = detail
                .report
                .lost_time
                .legs
                .iter()
                .filter(|l| l.is_error)
                .count();
            (detail.report.lost_time.error_count, marked)
        };
        let (before, marked) = errors(&store);
        assert!(before > 0);
        assert_eq!(before, marked);

        let mut lenient = settings::load(&store).unwrap();
        lenient.error_threshold_s = 600.0;
        settings::save(&mut store, &lenient).unwrap();
        assert_eq!(errors(&store), (0, 0));
        assert_eq!(
            race_detail(&store, result_id)
                .unwrap()
                .config
                .error_threshold_s,
            600.0
        );
        assert_eq!(list_races(&store).unwrap()[0].error_count, 0);

        let mut strict = lenient;
        strict.error_threshold_s = 0.0;
        strict.error_threshold_pct = 0.0;
        settings::save(&mut store, &strict).unwrap();
        assert!(errors(&store).0 > before);
    }

    /// La comparación sale del mismo análisis que la vista de carrera: el propio corredor
    /// aparece con los mismos números, junto al resto de su recorrido.
    #[test]
    fn comparison_includes_self_with_the_detail_numbers() {
        let (store, result_id) = imported();
        let detail = race_detail(&store, result_id).unwrap();
        let comparison = race_comparison(&store, result_id).unwrap();
        let selves: Vec<_> = comparison.runners.iter().filter(|r| r.is_self).collect();
        assert_eq!(selves.len(), 1);
        let me = selves[0];
        let legs = &detail.report.lost_time.legs;
        assert_eq!(me.total_s, detail.report.lost_time.total_s);
        assert_eq!(
            me.behind_ideal_s,
            legs.iter().map(|l| l.behind_ideal_s).collect::<Vec<_>>()
        );
        assert_eq!(comparison.legs.len(), legs.len());
        assert_eq!(
            comparison.runners.len(),
            detail.report.course.valid_runners
                + comparison
                    .runners
                    .iter()
                    .filter(|r| r.course_place.is_none())
                    .count()
        );
        assert_eq!(comparison.runners[0].course_place, Some(1));
    }

    /// Criterio de aceptación de #97: cambiar el formato mueve la carrera de grupo en el
    /// histórico, y se ve en la lista y en la vista de carrera.
    #[test]
    fn changing_the_format_moves_the_race_in_the_history() {
        use crate::history::history_view;
        use tramos_core::history::HistoryFilter;

        let (mut store, result_id) = imported();
        let races_in = |store: &Store, format: Option<RaceFormat>| {
            history_view(store, &HistoryFilter::default())
                .unwrap()
                .history
                .by_format
                .iter()
                .find(|g| g.format == format)
                .map_or(0, |g| g.stats.races)
        };
        assert_eq!(races_in(&store, Some(RaceFormat::Sprint)), 1);

        set_race_format(&mut store, result_id, Some(RaceFormat::Long)).unwrap();
        assert_eq!(races_in(&store, Some(RaceFormat::Sprint)), 0);
        assert_eq!(races_in(&store, Some(RaceFormat::Long)), 1);
        assert_eq!(
            race_detail(&store, result_id).unwrap().format,
            Some(RaceFormat::Long)
        );
        assert_eq!(
            list_races(&store).unwrap()[0].format,
            Some(RaceFormat::Long)
        );

        // Sin formato: va al grupo de las carreras sin formato.
        set_race_format(&mut store, result_id, None).unwrap();
        assert_eq!(races_in(&store, Some(RaceFormat::Long)), 0);
        assert_eq!(races_in(&store, None), 1);
        assert_eq!(race_detail(&store, result_id).unwrap().format, None);
    }

    #[test]
    fn missing_result_is_an_error() {
        let store = Store::open_in_memory().unwrap();
        assert!(matches!(
            race_detail(&store, 42),
            Err(RaceError::Store(StoreError::ResultNotFound(42)))
        ));
        assert!(matches!(
            race_comparison(&store, 42),
            Err(RaceError::Store(StoreError::ResultNotFound(42)))
        ));
        let mut store = store;
        assert!(matches!(
            set_race_format(&mut store, 42, None),
            Err(RaceError::Store(StoreError::ResultNotFound(42)))
        ));
        assert!(list_races(&store).unwrap().is_empty());
    }

    /// P2: con el FIT sintético, el tramo del rodeo es sobre todo desvío y parada; sin FIT, no
    /// hay reparto.
    #[test]
    fn breakdown_needs_the_track() {
        let (store, result_id) = imported();
        assert_eq!(race_breakdown(&store, result_id).unwrap(), None);

        let mut store = Store::open_in_memory().unwrap();
        let fit = format!(
            "{}/../../fixtures/fit/baltanas-sintetico.fit",
            env!("CARGO_MANIFEST_DIR")
        );
        let p = preview(&store, &spl_path(), Some(&fit), &identity()).unwrap();
        let outcome = import(
            &mut store,
            &ImportRequest {
                spl_path: spl_path(),
                fit_path: Some(fit),
                result: p.candidates[0].result,
                format: p.suggested_format,
                identity: identity(),
            },
        )
        .unwrap();
        let b = race_breakdown(&store, outcome.result_id).unwrap().unwrap();
        assert!(b.usual_ratio.is_some());
        let leg = b.legs.iter().find(|l| l.index == 9).unwrap();
        assert!(leg.is_error);
        assert!(leg.detour_s + leg.stopped_s > 0.5 * leg.loss_s, "{leg:?}");
        // Los errores sumados son los de la tabla de tramos que cuentan.
        let detail = race_detail(&store, outcome.result_id).unwrap();
        let errors = tramos_core::history::pattern_legs(&detail.report.lost_time)
            .filter(|l| l.is_error)
            .count();
        assert_eq!(b.errors.legs + b.errors_without_breakdown, errors);
    }
}
