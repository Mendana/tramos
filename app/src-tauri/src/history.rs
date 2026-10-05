//! Vista histórica (P6): las carreras del usuario agregadas por formato, con filtros de fechas y
//! formato.
//!
//! Como en [`crate::races`], el tiempo perdido de cada carrera se calcula al pedirlo con
//! [`runner_report`] y los umbrales de los ajustes; el agregado es el de
//! [`tramos_core::history`] (`docs/historico.md`).

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use chrono::NaiveDate;
use serde::Serialize;
use tramos_core::history::{
    History, HistoryFilter, HistoryRace, HistoryStats, history, race_stats,
};
use tramos_core::leg_length::{LegLengthStats, leg_length};
use tramos_core::lost_time::LostTimeConfig;
use tramos_core::model::{Event, RaceStatus};
use tramos_core::race_format::RaceFormat;
use tramos_core::runner_report::runner_report;
use tramos_store::{EventId, Store};

use crate::import::stored_self_person;
use crate::races::RaceError;
use crate::settings;

/// El histórico del usuario con un filtro.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryView {
    /// Carreras del usuario, sin filtrar: con 0, la pantalla invita a importar.
    pub all_races: usize,
    /// Fecha de la primera y la última carrera del usuario, sin filtrar.
    pub first_date: Option<NaiveDate>,
    pub last_date: Option<NaiveDate>,
    /// Umbrales y tiempo ideal con los que se ha calculado.
    pub config: LostTimeConfig,
    pub history: History,
    /// Pérdida según duración del tramo (P7): los seis cubos de referencia, con el mismo filtro.
    pub by_leg_length: Vec<LegLengthStats>,
    /// Las carreras que pasan el filtro, de la más reciente a la más antigua (#98).
    pub races: Vec<HistoryRaceRow>,
}

/// Una carrera del histórico con sus números.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HistoryRaceRow {
    /// Para abrir la carrera (`race_detail`).
    pub result_id: i64,
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
    pub class_name: String,
    pub status: RaceStatus,
    pub place: Option<u16>,
    /// Con las definiciones del histórico; `None` sin rendimiento habitual (no cuenta).
    pub stats: Option<HistoryStats>,
}

/// El histórico de las carreras del usuario (los resultados vinculados a su persona) que pasan
/// `filter`.
pub fn history_view(store: &Store, filter: &HistoryFilter) -> Result<HistoryView, RaceError> {
    let config = settings::lost_time_config(store)?;
    let results = match stored_self_person(store)? {
        Some(person) => store.person_results(person)?,
        None => Vec::new(),
    };
    let mut events: HashMap<EventId, Event> = HashMap::new();
    let mut races = Vec::new();
    let mut rows = Vec::new();
    // Solo se cargan y analizan las carreras que pasan el filtro.
    for r in results
        .iter()
        .filter(|r| filter.includes(r.event_date, r.event_format))
    {
        let (event_id, at) = store.result_ref(r.result)?;
        let event = match events.entry(event_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(store.load_event(event_id)?),
        };
        let report = runner_report(event, at, &config).ok_or(RaceError::NotAnalyzed(r.result.0))?;
        rows.push(HistoryRaceRow {
            result_id: r.result.0,
            date: r.event_date,
            name: r.event_name.clone(),
            format: r.event_format,
            class_name: r.class_name.clone(),
            status: r.status,
            place: r.place,
            stats: race_stats(&report.lost_time),
        });
        races.push(HistoryRace {
            date: r.event_date,
            format: r.event_format,
            lost_time: report.lost_time,
        });
    }
    rows.reverse();
    Ok(HistoryView {
        all_races: results.len(),
        // `person_results` va de la más antigua a la más reciente.
        first_date: results.first().map(|r| r.event_date),
        last_date: results.last().map(|r| r.event_date),
        config,
        history: history(&races, filter),
        races: rows,
        by_leg_length: leg_length(&races, filter),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{ImportRequest, import, preview};
    use crate::races::race_detail;
    use tramos_core::history::{HistoryStats, pattern_legs};
    use tramos_core::identify::RunnerIdentity;
    use tramos_core::importers::spl;
    use tramos_core::race_format::RaceFormat;
    use tramos_core::runner_report::RunnerLostTime;
    use tramos_store::PersonId;

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

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    /// Importa el fixture de Baltanás (3-oct-2026, sprint) como el corredor de la tarjeta 143 y
    /// guarda una copia de la carrera el 1-jun-2026, como media, con el mismo resultado
    /// vinculado a su persona: dos carreras con los mismos números.
    fn two_races() -> (Store, i64) {
        let mut store = Store::open_in_memory().unwrap();
        let p = preview(&store, &spl_path(), None, &identity()).unwrap();
        let at = p.candidates[0].result;
        let outcome = import(
            &mut store,
            &ImportRequest {
                spl_path: spl_path(),
                fit_path: None,
                result: at,
                format: p.suggested_format,
                identity: identity(),
            },
        )
        .unwrap();
        assert_eq!(p.suggested_format, Some(RaceFormat::Sprint));

        let mut copy = spl::read(&std::fs::read(spl_path()).unwrap()).unwrap();
        copy.date = date("2026-06-01");
        let saved = store.save_event(&copy, None).unwrap();
        store
            .set_event_format(saved.id, Some(RaceFormat::Middle))
            .unwrap();
        let person: PersonId = stored_self_person(&store).unwrap().unwrap();
        store
            .link_result(saved.results[at.class_index][at.result_index], person)
            .unwrap();
        (store, outcome.result_id)
    }

    /// Lo que aporta una carrera: tramos que cuentan, errores y pérdida de los errores (s y %).
    fn counted(lost: &RunnerLostTime) -> (usize, usize, f64, f64) {
        let legs: Vec<_> = pattern_legs(lost).collect();
        let errors: Vec<_> = legs.iter().filter(|l| l.is_error).collect();
        let loss = errors.iter().filter_map(|l| l.loss_s).sum();
        let pct = errors.iter().filter_map(|l| l.loss_pct).sum();
        (legs.len(), errors.len(), loss, pct)
    }

    fn stats(view: &HistoryView, format: Option<RaceFormat>) -> HistoryStats {
        view.history
            .by_format
            .iter()
            .find(|g| g.format == format)
            .unwrap()
            .stats
    }

    /// Los números del histórico salen de los de la vista de carrera.
    #[test]
    fn history_matches_the_race_detail() {
        let (store, result_id) = two_races();
        let lost = race_detail(&store, result_id).unwrap().report.lost_time;
        let (legs, errors, loss, pct) = counted(&lost);
        let usual = lost.usual_performance.unwrap();
        // El último tramo no cuenta: menos tramos que en la carrera.
        assert!(legs > 0 && legs < lost.legs.len());
        assert!(errors > 0);

        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        assert_eq!(view.all_races, 2);
        assert_eq!(view.first_date, Some(date("2026-06-01")));
        assert_eq!(view.last_date, Some(date("2026-10-03")));
        assert_eq!(view.config, LostTimeConfig::default());

        let expected = HistoryStats {
            races: 1,
            legs,
            errors,
            mean_performance: Some(usual),
            error_rate: Some(errors as f64 / legs as f64),
            mean_loss_s: Some(loss / legs as f64),
            mean_loss_pct: Some(pct / legs as f64),
            // La misma carrera dos veces: la consistencia media es la suya.
            mean_consistency: lost.consistency,
        };
        assert_eq!(stats(&view, Some(RaceFormat::Sprint)), expected);
        assert_eq!(stats(&view, Some(RaceFormat::Middle)), expected);
        assert_eq!(
            stats(&view, Some(RaceFormat::Long)),
            HistoryStats::default()
        );
        let total = view.history.total;
        assert_eq!(
            (total.races, total.legs, total.errors),
            (2, 2 * legs, 2 * errors)
        );
        assert_eq!(total.error_rate, expected.error_rate);
        assert!(lost.consistency.is_some());
        assert_eq!(total.mean_consistency, lost.consistency);
        assert_eq!(view.history.races_without_data, 0);
    }

    #[test]
    fn filters_by_date_and_format() {
        let (store, _) = two_races();
        let view = history_view(
            &store,
            &HistoryFilter {
                from: Some(date("2026-06-01")),
                to: Some(date("2026-06-01")),
                format: None,
            },
        )
        .unwrap();
        assert_eq!(view.all_races, 2);
        assert_eq!(view.history.total.races, 1);
        assert_eq!(stats(&view, Some(RaceFormat::Middle)).races, 1);
        assert_eq!(stats(&view, Some(RaceFormat::Sprint)).races, 0);

        let view = history_view(
            &store,
            &HistoryFilter {
                format: Some(RaceFormat::Sprint),
                ..HistoryFilter::default()
            },
        )
        .unwrap();
        assert_eq!(view.history.by_format.len(), 1);
        assert_eq!(view.history.total.races, 1);
        assert_eq!(view.history.total, view.history.by_format[0].stats);
    }

    #[test]
    fn races_without_format_go_apart() {
        let (mut store, result_id) = two_races();
        let (event_id, _) = store.result_ref(tramos_store::ResultId(result_id)).unwrap();
        store.set_event_format(event_id, None).unwrap();
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        assert_eq!(stats(&view, None).races, 1);
        assert_eq!(stats(&view, Some(RaceFormat::Sprint)).races, 0);
        assert_eq!(view.history.total.races, 2);
        // Con filtro de formato no entran.
        let view = history_view(
            &store,
            &HistoryFilter {
                format: Some(RaceFormat::Middle),
                ..HistoryFilter::default()
            },
        )
        .unwrap();
        assert_eq!(view.history.total.races, 1);
        assert!(view.history.by_format.iter().all(|g| g.format.is_some()));
    }

    /// Los umbrales de los ajustes cambian los errores del histórico.
    #[test]
    fn thresholds_from_settings_change_the_error_rate() {
        let (mut store, _) = two_races();
        let before = history_view(&store, &HistoryFilter::default()).unwrap();
        assert!(before.history.total.errors > 0);

        let mut lenient = settings::load(&store).unwrap();
        lenient.error_threshold_s = 600.0;
        settings::save(&mut store, &lenient).unwrap();
        let after = history_view(&store, &HistoryFilter::default()).unwrap();
        assert_eq!(after.history.total.errors, 0);
        assert_eq!(after.history.total.error_rate, Some(0.0));
        assert_eq!(after.history.total.mean_loss_s, Some(0.0));
        // El IR no depende de los umbrales.
        assert_eq!(
            after.history.total.mean_performance,
            before.history.total.mean_performance
        );
        assert_eq!(after.config.error_threshold_s, 600.0);
    }

    /// Criterio de aceptación de #98: una fila por carrera que pasa el filtro, de la más reciente
    /// a la más antigua, y sus números suman los del total.
    #[test]
    fn rows_are_the_filtered_races_and_add_up_to_the_total() {
        let (store, result_id) = two_races();
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        let dates: Vec<_> = view.races.iter().map(|r| r.date).collect();
        assert_eq!(dates, [date("2026-10-03"), date("2026-06-01")]);
        assert_eq!(view.races[0].result_id, result_id);
        assert_eq!(view.races[0].format, Some(RaceFormat::Sprint));
        assert_eq!(view.races[1].format, Some(RaceFormat::Middle));
        assert_eq!(view.races[0].class_name, "M-SEN");

        let rows: Vec<HistoryStats> = view.races.iter().filter_map(|r| r.stats).collect();
        let total = view.history.total;
        assert_eq!(rows.iter().map(|r| r.races).sum::<usize>(), total.races);
        assert_eq!(rows.iter().map(|r| r.legs).sum::<usize>(), total.legs);
        assert_eq!(rows.iter().map(|r| r.errors).sum::<usize>(), total.errors);
        // Cada fila es su carrera sola: la de la vista de carrera.
        let lost = race_detail(&store, result_id).unwrap().report.lost_time;
        assert_eq!(rows[0].mean_performance, lost.usual_performance);

        // Con filtro, solo las que lo pasan.
        let view = history_view(
            &store,
            &HistoryFilter {
                format: Some(RaceFormat::Middle),
                ..HistoryFilter::default()
            },
        )
        .unwrap();
        let dates: Vec<_> = view.races.iter().map(|r| r.date).collect();
        assert_eq!(dates, [date("2026-06-01")]);
    }

    #[test]
    fn empty_store_has_no_races() {
        let store = Store::open_in_memory().unwrap();
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        assert_eq!(view.all_races, 0);
        assert_eq!((view.first_date, view.last_date), (None, None));
        assert_eq!(view.history.total, HistoryStats::default());
        assert_eq!(view.history.by_format.len(), 3);
        assert!(view.races.is_empty());
    }

    /// Los cubos de P7 reparten los mismos tramos que cuentan en el histórico, también con
    /// filtro de formato.
    #[test]
    fn leg_length_buckets_split_the_counted_legs() {
        let (store, _) = two_races();
        for format in [None, Some(RaceFormat::Sprint), Some(RaceFormat::Long)] {
            let view = history_view(
                &store,
                &HistoryFilter {
                    format,
                    ..HistoryFilter::default()
                },
            )
            .unwrap();
            let buckets = &view.by_leg_length;
            assert_eq!(buckets.len(), 6);
            let legs: usize = buckets.iter().map(|b| b.legs).sum();
            let errors: usize = buckets.iter().map(|b| b.errors).sum();
            assert_eq!(legs, view.history.total.legs);
            assert_eq!(errors, view.history.total.errors);
        }
        // Sin carreras, los seis cubos vacíos.
        let store = Store::open_in_memory().unwrap();
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        assert_eq!(view.by_leg_length.len(), 6);
        assert!(view.by_leg_length.iter().all(|b| b.legs == 0));
    }
}
