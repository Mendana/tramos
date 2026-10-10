//! Vista histórica (P6): las carreras del usuario agregadas por formato, con filtros de fechas y
//! formato.
//!
//! Como en [`crate::races`], el tiempo perdido de cada carrera se calcula al pedirlo con
//! [`runner_report`] y los umbrales de los ajustes; el agregado es el de
//! [`tramos_core::history`] (`docs/historico.md`).
//!
//! Para P13 (pérdida según desnivel) hacen falta además las métricas del FIT de cada tramo: el
//! track guardado se alinea y se trocea como en el mapa ([`crate::race_map::aligned_legs`]) y
//! las métricas son las de [`tramos_core::metrics::leg_metrics`].
//!
//! Para P9 (errores más comunes) hacen falta las etiquetas de los tramos de cada carrera, y
//! para P14 (cansancio), las métricas del FIT y las etiquetas a la vez.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use chrono::NaiveDate;
use serde::Serialize;
use tramos_core::after_error::{AfterError, after_error};
use tramos_core::common_errors::{CommonErrors, TaggedRace, common_errors};
use tramos_core::consistency::consistency_series;
use tramos_core::days_off::{DaysOff, days_off};
use tramos_core::fatigue::{Fatigue, FatigueRace, fatigue};
use tramos_core::history::{
    History, HistoryFilter, HistoryRace, HistoryStats, TrackedRace, history, race_stats,
};
use tramos_core::insights::{HistoryAnalyses, Insight, history_insights};
use tramos_core::leg_length::{LegLengthStats, leg_length};
use tramos_core::loss_breakdown::{BreakdownHistory, breakdown_history};
use tramos_core::lost_time::LostTimeConfig;
use tramos_core::model::{Event, RaceStatus};
use tramos_core::race_format::RaceFormat;
use tramos_core::runner_report::runner_report;
use tramos_core::slope::{SlopeConfig, SlopeHistory, slope};
use tramos_core::taxonomy::{LegTag, Taxonomy};
use tramos_store::{EventId, Store};

use crate::import::stored_self_person;
use crate::race_map::stored_leg_metrics;
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
    /// Resumen en frases (#126, `docs/frases.md`): como mucho tres, sacadas de los análisis de
    /// abajo, con el mismo filtro.
    pub insights: Vec<Insight>,
    pub history: History,
    /// Pérdida según duración del tramo (P7): los seis cubos de referencia, con el mismo filtro.
    pub by_leg_length: Vec<LegLengthStats>,
    /// Pérdida según desnivel (P13): subida, llano y bajada, con el mismo filtro. Solo aportan
    /// tramos las carreras con track.
    pub by_slope: SlopeHistory,
    /// ¿Lento o desorientado? (P2): la pérdida de los errores repartida en desvío, paradas y
    /// ritmo, con el mismo filtro. Solo aportan las carreras con track.
    pub loss_breakdown: BreakdownHistory,
    /// Después de fallar (P8): encadenamiento, recuperación y rachas limpias, con el mismo
    /// filtro. La recuperación solo con las carreras con track.
    pub after_error: AfterError,
    /// Errores más comunes (P9): reparto por tipo y subtipo de las etiquetas, en total, por
    /// formato y por duración del tramo, con el mismo filtro.
    pub common_errors: CommonErrors,
    /// ¿El cansancio anticipa el error? (P14): deriva del pulso, pulso antes del error y
    /// esfuerzo percibido por tercio de carrera, con el mismo filtro. El pulso solo sale de las
    /// carreras con track (y pulso); el esfuerzo, de las etiquetas.
    pub fatigue: Fatigue,
    /// Las carreras que pasan el filtro, de la más reciente a la más antigua (#98).
    pub races: Vec<HistoryRaceRow>,
    /// Días sin competir (P11): cuatro cubos por días desde la carrera anterior, con el mismo
    /// filtro. La anterior puede ser cualquier carrera del usuario, pase o no el filtro.
    pub days_off: DaysOff,
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
    history_view_where(store, filter, &|_| true)
}

/// Como [`history_view`], solo con los resultados para los que `keep(result_id)` es cierto (al
/// comparar grupos, las carreras que han corrido los dos, #121). Las demás carreras siguen
/// sirviendo de «carrera anterior» (P11) y cuentan en `all_races`.
pub fn history_view_where(
    store: &Store,
    filter: &HistoryFilter,
    keep: &dyn Fn(i64) -> bool,
) -> Result<HistoryView, RaceError> {
    let config = settings::lost_time_config(store)?;
    let results = match stored_self_person(store)? {
        Some(person) => store.person_results(person)?,
        None => Vec::new(),
    };
    let mut events: HashMap<EventId, Event> = HashMap::new();
    let mut races = Vec::new();
    let mut metrics = Vec::new();
    let mut tags: Vec<Vec<(usize, LegTag)>> = Vec::new();
    let mut rows = Vec::new();
    // Solo se cargan y analizan las carreras que pasan el filtro.
    for r in results
        .iter()
        .filter(|r| filter.includes(r.event_date, r.event_format) && keep(r.result.0))
    {
        let (event_id, at) = store.result_ref(r.result)?;
        let event = match events.entry(event_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(store.load_event(event_id)?),
        };
        let report = runner_report(event, at, &config).ok_or(RaceError::NotAnalyzed(r.result.0))?;
        metrics.push(match at.get(event) {
            Some(race_result) if r.has_track => stored_leg_metrics(store, r.result, race_result)?,
            _ => None,
        });
        tags.push(
            store
                .tags(r.result)?
                .into_iter()
                .map(|t| (t.leg_index, t.tag))
                .collect(),
        );
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
    // Carreras en las que el usuario tomó la salida, sin filtrar: sirven de «carrera anterior»
    // (P11) sin tener que analizarlas.
    let competed: Vec<NaiveDate> = results
        .iter()
        .filter(|r| r.status != RaceStatus::DidNotStart)
        .map(|r| r.event_date)
        .collect();
    let tracked: Vec<TrackedRace<'_>> = races
        .iter()
        .zip(&metrics)
        .map(|(race, legs)| TrackedRace {
            race,
            leg_metrics: legs.as_deref(),
        })
        .collect();
    let by_slope = slope(&tracked, filter, &SlopeConfig::default())?;
    let tagged: Vec<TaggedRace<'_>> = races
        .iter()
        .zip(&tags)
        .map(|(race, tags)| TaggedRace { race, tags })
        .collect();
    let fatigue_races: Vec<FatigueRace<'_>> = tracked
        .iter()
        .zip(&tags)
        .map(|(t, tags)| FatigueRace {
            race: t.race,
            leg_metrics: t.leg_metrics,
            tags,
        })
        .collect();
    let history = history(&races, filter);
    let by_leg_length = leg_length(&races, filter);
    let days_off = days_off(&races, &competed, filter);
    let loss_breakdown = breakdown_history(&tracked, filter);
    let after_error = after_error(&tracked, filter);
    let common_errors = common_errors(&tagged, filter);
    // Sin la taxonomía, las frases llevan la clave del tipo de error en vez de su nombre.
    let taxonomy = Taxonomy::builtin().ok();
    let consistency = consistency_series(&races, filter);
    let insights = history_insights(&HistoryAnalyses {
        history: &history,
        by_leg_length: &by_leg_length,
        by_slope: &by_slope,
        common_errors: &common_errors,
        after_error: &after_error,
        days_off: &days_off,
        loss_breakdown: &loss_breakdown,
        consistency: &consistency,
        taxonomy: taxonomy.as_ref(),
    });
    Ok(HistoryView {
        all_races: results.len(),
        // `person_results` va de la más antigua a la más reciente.
        first_date: results.first().map(|r| r.event_date),
        last_date: results.last().map(|r| r.event_date),
        config,
        insights,
        history,
        races: rows,
        by_leg_length,
        days_off,
        by_slope,
        loss_breakdown,
        after_error,
        common_errors,
        fatigue: fatigue(&fatigue_races, filter),
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
    use tramos_core::runner_report::{LegReport, RunnerLostTime};
    use tramos_core::slope::{CLASSES, SlopeClass, classify_leg};
    use tramos_store::{PersonId, ResultId};

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

    fn fit_path() -> String {
        format!(
            "{}/../../fixtures/fit/baltanas-sintetico.fit",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    /// Importa el fixture de Baltanás (3-oct-2026, sprint) como el corredor de la tarjeta 143 y
    /// guarda una copia de la carrera el 1-jun-2026, como media, con el mismo resultado
    /// vinculado a su persona: dos carreras con los mismos números.
    fn two_races() -> (Store, i64) {
        two_races_with(false)
    }

    /// Como [`two_races`]; con `with_fit`, la carrera del 3-oct se importa con el FIT sintético
    /// (la copia del 1-jun nunca tiene track).
    fn two_races_with(with_fit: bool) -> (Store, i64) {
        let mut store = Store::open_in_memory().unwrap();
        let fit = with_fit.then(fit_path);
        let p = preview(&store, &spl_path(), fit.as_deref(), &identity()).unwrap();
        let at = p.candidates[0].result;
        let outcome = import(
            &mut store,
            &ImportRequest {
                spl_path: spl_path(),
                fit_path: fit,
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

    /// Las frases del histórico (#126) salen de sus análisis, con el mismo filtro: con un
    /// filtro que no deja ninguna carrera, ninguna frase.
    #[test]
    fn insights_come_from_the_filtered_analyses() {
        use tramos_core::insights::{HistoryAnalyses, MAX_INSIGHTS, history_insights};

        let (store, _) = two_races_with(true);
        let taxonomy = Taxonomy::builtin().unwrap();
        for filter in [
            HistoryFilter::default(),
            HistoryFilter {
                format: Some(RaceFormat::Sprint),
                ..HistoryFilter::default()
            },
        ] {
            let view = history_view(&store, &filter).unwrap();
            // La consistencia de cada carrera con filtro, de la más antigua a la más reciente.
            let consistency: Vec<f64> = view
                .races
                .iter()
                .rev()
                .filter_map(|r| r.stats?.mean_consistency)
                .collect();
            let expected = history_insights(&HistoryAnalyses {
                history: &view.history,
                by_leg_length: &view.by_leg_length,
                by_slope: &view.by_slope,
                common_errors: &view.common_errors,
                after_error: &view.after_error,
                days_off: &view.days_off,
                loss_breakdown: &view.loss_breakdown,
                consistency: &consistency,
                taxonomy: Some(&taxonomy),
            });
            assert_eq!(view.insights, expected);
            assert!(view.insights.len() <= MAX_INSIGHTS);
            // Dos carreras: todo lo que sale lleva el aviso de pocas carreras.
            assert!(view.insights.iter().all(|i| i.few_data));
        }
        let none = HistoryFilter {
            format: Some(RaceFormat::Long),
            ..HistoryFilter::default()
        };
        assert!(history_view(&store, &none).unwrap().insights.is_empty());
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

    #[test]
    fn days_off_count_against_races_outside_the_filter() {
        let (store, _) = two_races();
        let races = |format| {
            let view = history_view(
                &store,
                &HistoryFilter {
                    format,
                    ..HistoryFilter::default()
                },
            )
            .unwrap();
            let counts: Vec<_> = view.days_off.buckets.iter().map(|b| b.races).collect();
            (counts, view.days_off.without_previous)
        };
        // 1-jun (media) es la primera; 3-oct (sprint) va 124 días después: más de 30.
        assert_eq!(races(None), (vec![0, 0, 0, 1], 1));
        // Solo sprint: la del 3-oct sigue teniendo de anterior la media del 1-jun.
        assert_eq!(races(Some(RaceFormat::Sprint)), (vec![0, 0, 0, 1], 0));
        assert_eq!(races(Some(RaceFormat::Middle)), (vec![0, 0, 0, 0], 1));
    }

    /// P13: la carrera con FIT reparte sus tramos que cuentan entre las tres clases (o sin
    /// clasificar); la copia sin track no aporta ninguno. Los números de cada clase salen de
    /// los tramos de la vista de carrera y de las métricas del track.
    #[test]
    fn slope_classes_split_the_legs_with_track() {
        let (store, result_id) = two_races_with(true);
        let lost = race_detail(&store, result_id).unwrap().report.lost_time;
        let counted: Vec<&LegReport> = pattern_legs(&lost).collect();
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        let s = &view.by_slope;
        let config = SlopeConfig::default();
        assert_eq!(s.config, config);
        assert_eq!((s.races_with_track, s.races_without_track), (1, 1));
        assert_eq!(s.legs_without_track, counted.len());
        let classified: usize = s.by_class.iter().map(|c| c.legs).sum();
        assert_eq!(classified + s.unclassified_legs, counted.len());
        assert_eq!(
            classified + s.unclassified_legs + s.legs_without_track,
            view.history.total.legs
        );
        let classes: Vec<_> = s.by_class.iter().map(|c| c.class).collect();
        assert_eq!(classes, CLASSES);

        // Cada clase, recalculada tramo a tramo.
        let result = ResultId(result_id);
        let (event_id, at) = store.result_ref(result).unwrap();
        let event = store.load_event(event_id).unwrap();
        let metrics = stored_leg_metrics(&store, result, at.get(&event).unwrap())
            .unwrap()
            .unwrap();
        for stats in &s.by_class {
            let legs: Vec<_> = counted
                .iter()
                .filter(|l| classify_leg(l, &metrics, &config) == Some(stats.class))
                .collect();
            let errors = legs.iter().filter(|l| l.is_error).count();
            assert_eq!((stats.legs, stats.errors), (legs.len(), errors));
            let weight: f64 = legs.iter().filter_map(|l| l.reference_s).sum();
            let weighted: f64 = legs
                .iter()
                .filter_map(|l| Some(l.reference_s? * l.performance_index?))
                .sum();
            let ir = (weight > 0.0).then(|| weighted / weight);
            match (stats.mean_performance, ir) {
                (Some(a), Some(b)) => assert!((a - b).abs() < 1e-9, "{a} != {b}"),
                (a, b) => assert_eq!(a, b),
            }
        }
        // El FIT sintético tiene altitud: se clasifican todos y hay de las tres clases.
        assert_eq!(s.unclassified_legs, 0);
        assert!(s.by_class.iter().all(|c| c.legs > 0), "{s:?}");

        // Contra la verdad del FIT sintético (desnivel sin ruido): los tramos claramente por
        // encima o por debajo del umbral (a más de 1,5 m/100 m) caen en su clase.
        let text = std::fs::read_to_string(format!(
            "{}/../../fixtures/fit/baltanas-sintetico.truth.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let truth: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut checked = 0;
        for leg in &counted {
            let Some(t) = truth["legs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["leg"].as_u64() == Some(leg.index as u64))
            else {
                continue;
            };
            let path = t["path_m"].as_f64().unwrap();
            let up = t["ascent_m"].as_f64().unwrap() / path * 100.0;
            let down = t["descent_m"].as_f64().unwrap() / path * 100.0;
            let threshold = config.threshold_m_per_100m;
            if (up - threshold).abs() < 1.5 || (down - threshold).abs() < 1.5 {
                continue;
            }
            let expected = match (up >= threshold, down >= threshold) {
                (true, true) if down > up => SlopeClass::Downhill,
                (true, _) => SlopeClass::Uphill,
                (false, true) => SlopeClass::Downhill,
                (false, false) => SlopeClass::Flat,
            };
            assert_eq!(
                classify_leg(leg, &metrics, &config),
                Some(expected),
                "tramo {}: +{up:.2} −{down:.2} m/100 m",
                leg.index
            );
            checked += 1;
        }
        assert!(checked >= counted.len() / 2, "solo {checked} tramos claros");

        // Con filtro de formato, la media (la copia sin track) no aporta tramos.
        let middle = history_view(
            &store,
            &HistoryFilter {
                format: Some(RaceFormat::Middle),
                ..HistoryFilter::default()
            },
        )
        .unwrap();
        let m = &middle.by_slope;
        assert_eq!((m.races_with_track, m.races_without_track), (0, 1));
        assert!(
            m.by_class
                .iter()
                .all(|c| c.legs == 0 && c.mean_performance.is_none())
        );
    }

    /// Sin FIT no hay tramos de P13; sin carreras, tres clases vacías.
    #[test]
    fn slope_without_tracks_is_empty() {
        let (store, _) = two_races();
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        let s = &view.by_slope;
        assert_eq!((s.races_with_track, s.races_without_track), (0, 2));
        assert_eq!(s.legs_without_track, view.history.total.legs);
        assert_eq!(s.unclassified_legs, 0);
        assert!(
            s.by_class
                .iter()
                .all(|c| c.legs == 0 && c.error_rate.is_none())
        );

        let store = Store::open_in_memory().unwrap();
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        assert_eq!(view.by_slope.by_class.len(), 3);
        assert_eq!(view.by_slope.races_with_track, 0);
    }

    #[test]
    fn slope_json_uses_snake_case() {
        let (store, _) = two_races_with(true);
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["by_slope"]["by_class"][0]["class"], "uphill");
        assert_eq!(json["by_slope"]["config"]["threshold_m_per_100m"], 4.0);
        assert_eq!(json["by_slope"]["races_with_track"], 1);
    }

    /// P2: el agregado son los errores repartidos de la carrera con FIT, que son los de su vista
    /// de carrera; los de la copia sin track quedan sin reparto.
    #[test]
    fn loss_breakdown_adds_the_races_with_track() {
        let (store, result_id) = two_races_with(true);
        let race = crate::races::race_breakdown(&store, result_id)
            .unwrap()
            .unwrap();
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        let b = &view.loss_breakdown;
        assert_eq!((b.races_with_track, b.races_without_track), (1, 1));
        assert_eq!(b.errors, race.errors);
        assert_eq!(
            b.errors.legs + b.errors_without_breakdown,
            view.history.total.errors
        );
    }

    /// P8: cada tramo que cuenta salvo el primero de cada carrera es «tras error» o «tras
    /// limpio», y la recuperación solo sale de la carrera con FIT.
    #[test]
    fn after_error_splits_the_legs_after_the_first() {
        let (store, result_id) = two_races_with(true);
        let lost = race_detail(&store, result_id).unwrap().report.lost_time;
        let counted: Vec<_> = pattern_legs(&lost).collect();
        let errors_before_last = counted[..counted.len() - 1]
            .iter()
            .filter(|l| l.is_error)
            .count();
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        let a = &view.after_error;
        // Dos carreras iguales: dos veces los tramos menos el primero.
        assert_eq!(
            a.after_error.legs + a.after_clean.legs,
            2 * (counted.len() - 1)
        );
        assert_eq!(a.after_error.legs, 2 * errors_before_last);
        // La copia sin track no tiene velocidad.
        assert_eq!(
            a.accelerated.legs + a.not_accelerated.legs,
            errors_before_last
        );
        assert_eq!(a.after_error_without_speed, errors_before_last);
        assert_eq!(a.streaks[0].rate, a.after_error);
        let streak_legs: usize = a.streaks.iter().map(|s| s.rate.legs).sum();
        assert_eq!(streak_legs, 2 * (counted.len() - 1));
    }

    /// P9: las etiquetas de la carrera deciden; la copia sin etiquetar se queda con lo que dice
    /// el cálculo, sin revisar.
    #[test]
    fn common_errors_use_the_tags() {
        use tramos_core::taxonomy::{Confirmation, LegTag};
        let (mut store, result_id) = two_races_with(false);
        let lost = race_detail(&store, result_id).unwrap().report.lost_time;
        let errors: Vec<_> = pattern_legs(&lost)
            .filter(|l| l.is_error)
            .map(|l| l.index)
            .collect();
        assert!(errors.len() >= 2, "el fixture tiene al menos dos errores");
        let typed = LegTag {
            confirmation: Some(Confirmation::Error),
            error_type: Some("navigation".into()),
            error_subtype: Some("parallel".into()),
            ..LegTag::default()
        };
        let physical = LegTag {
            confirmation: Some(Confirmation::Physical),
            ..LegTag::default()
        };
        crate::tags::save_leg_tag(&mut store, result_id, errors[0], typed).unwrap();
        crate::tags::save_leg_tag(&mut store, result_id, errors[1], physical).unwrap();

        let total = history_view(&store, &HistoryFilter::default())
            .unwrap()
            .common_errors
            .total;
        let n = errors.len();
        // Dos copias de la carrera: en la etiquetada, un error tiene tipo y otro es físico.
        assert_eq!(total.errors, 2 * n - 1);
        assert_eq!(total.physical_legs, 1);
        assert_eq!(total.untyped, 2 * n - 2);
        assert_eq!(total.unreviewed, 2 * n - 2);
        assert_eq!(total.by_type.len(), 1);
        assert_eq!(total.by_type[0].error_type, "navigation");
        assert_eq!(total.by_type[0].errors, 1);
        let view = history_view(&store, &HistoryFilter::default()).unwrap();
        let sprint = &view.common_errors.by_format[0];
        assert_eq!(sprint.format, Some(RaceFormat::Sprint));
        assert_eq!((sprint.total.errors, sprint.total.untyped), (n - 1, n - 2));
    }

    /// P14: el pulso sale de la carrera con FIT (el sintético tiene pulso en todos los tramos);
    /// la copia sin track solo cuenta sus errores sin pulso. El esfuerzo sale de las etiquetas.
    #[test]
    fn fatigue_uses_the_track_and_the_tags() {
        use tramos_core::history::race_third;
        use tramos_core::taxonomy::{Confirmation, LegTag};
        let (mut store, result_id) = two_races_with(true);
        let lost = race_detail(&store, result_id).unwrap().report.lost_time;
        let counted: Vec<_> = pattern_legs(&lost).collect();
        let errors: Vec<usize> = counted
            .iter()
            .filter(|l| l.is_error)
            .map(|l| l.index)
            .collect();
        assert!(errors.len() >= 2, "el fixture tiene al menos dos errores");
        let errors_after_first = counted[1..].iter().filter(|l| l.is_error).count();
        let effort = LegTag {
            effort: Some(8),
            ..LegTag::default()
        };
        let physical = LegTag {
            confirmation: Some(Confirmation::Physical),
            ..LegTag::default()
        };
        crate::tags::save_leg_tag(&mut store, result_id, errors[0], effort).unwrap();
        crate::tags::save_leg_tag(&mut store, result_id, errors[1], physical).unwrap();

        let f = history_view(&store, &HistoryFilter::default())
            .unwrap()
            .fatigue;
        assert_eq!(
            (f.races_with_heart_rate, f.races_without_heart_rate),
            (1, 1)
        );
        assert_eq!(f.by_third.len(), 3);
        let sum = |get: fn(&tramos_core::fatigue::ThirdFatigue) -> usize| -> usize {
            f.by_third.iter().map(get).sum()
        };
        // Cada tramo que cuenta salvo el primero, en la carrera con FIT: antes de error o de
        // limpio, salvo el físico.
        assert_eq!(sum(|t| t.before_error.legs), errors_after_first - 1);
        assert_eq!(
            sum(|t| t.before_clean.legs),
            counted.len() - 1 - errors_after_first
        );
        // La copia no tiene pulso: sus errores no se comparan.
        assert_eq!(f.errors_without_heart_rate, errors_after_first);
        // Deriva: todos los tramos limpios de la carrera con FIT.
        assert_eq!(sum(|t| t.drift.legs), counted.len() - errors.len());
        for t in &f.by_third {
            for hr in [
                t.before_error.mean_heart_rate_bpm,
                t.drift.mean_heart_rate_bpm,
            ]
            .into_iter()
            .flatten()
            {
                assert!((90.0..=200.0).contains(&hr), "pulso {hr}");
            }
        }
        // Esfuerzo: el del primer error, en su tercio.
        let third = race_third(errors[0], lost.legs.len());
        assert_eq!(f.by_third[third].effort_error.legs, 1);
        assert_eq!(f.by_third[third].effort_error.mean_effort, Some(8.0));
        assert_eq!(sum(|t| t.effort_error.legs + t.effort_physical.legs), 1);
    }
}
