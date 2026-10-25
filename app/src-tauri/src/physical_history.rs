//! Pestaña Físico del histórico (#214): el rendimiento físico de las carreras del usuario con FIT
//! que pasan el filtro del histórico, y lo que se saca juntándolas (pendientes y deriva). Todo
//! lo calcula el núcleo (`tramos_core::physical_history`; `docs/rendimiento-fisico.md`,
//! "Histórico"); la interfaz solo dibuja lo que recibe.
//!
//! Como en [`crate::history`], el track guardado se alinea y se trocea igual que en el mapa
//! ([`crate::race_map::aligned_legs`]) y las zonas de pulso salen de los ajustes (la pestaña
//! Físico de la carrera, [`crate::race_physical`]). Sirve también para la vista de un atleta:
//! trabaja sobre la base que se está mirando.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use chrono::NaiveDate;
use serde::Serialize;
use tramos_core::event_kind::EventKind;
use tramos_core::history::HistoryFilter;
use tramos_core::metrics::MetricsOptions;
use tramos_core::model::Event;
use tramos_core::physical::{RacePhysical, race_physical};
use tramos_core::physical_history::{PhysicalHistory, PhysicalRace, RaceFigures, physical_history};
use tramos_core::race_format::RaceFormat;
use tramos_store::{EventId, Store};

use crate::import::stored_self_person;
use crate::race_map::aligned_legs;
use crate::races::RaceError;
use crate::settings;

/// Una carrera con FIT del histórico y sus cifras físicas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalRaceRow {
    /// Para abrir la carrera (`race_detail`).
    pub result_id: i64,
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
    pub kind: EventKind,
    pub class_name: String,
    #[serde(flatten)]
    pub figures: RaceFigures,
}

/// El rendimiento físico del histórico con un filtro.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalHistoryView {
    /// Carreras que pasan el filtro, tengan FIT o no.
    pub races_in_filter: usize,
    /// De ellas, las que no tienen FIT o no se pueden alinear con sus picadas: no aportan nada.
    pub races_without_track: usize,
    /// Si hay zonas de pulso en Ajustes (sin ellas no hay carga).
    pub has_zones: bool,
    /// Las carreras con FIT, de la más antigua a la más reciente.
    pub races: Vec<PhysicalRaceRow>,
    /// Las pendientes y la deriva, con los trozos de todas ellas juntos.
    pub history: PhysicalHistory,
}

/// El rendimiento físico de las carreras del usuario (los resultados vinculados a su persona)
/// que pasan `filter`.
pub fn physical_history_view(
    store: &Store,
    filter: &HistoryFilter,
) -> Result<PhysicalHistoryView, RaceError> {
    let zones = settings::load(store)?.map.heart_rate_zones;
    let limits = zones.as_ref().map(|z| z.limits.as_slice());
    let options = MetricsOptions::default();
    let results = match stored_self_person(store)? {
        Some(person) => store.person_results(person)?,
        None => Vec::new(),
    };
    let mut events: HashMap<EventId, Event> = HashMap::new();
    let mut in_filter = 0;
    let mut found: Vec<(PhysicalRaceRow, RacePhysical)> = Vec::new();
    // `person_results` va de la más antigua a la más reciente.
    for r in results
        .iter()
        .filter(|r| filter.includes(r.event_date, r.event_format, r.event_kind))
    {
        in_filter += 1;
        if !r.has_track {
            continue;
        }
        let (event_id, at) = store.result_ref(r.result)?;
        let event = match events.entry(event_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(store.load_event(event_id)?),
        };
        let (Some(track), Some(race_result)) = (store.load_track(r.result)?, at.get(event)) else {
            continue;
        };
        let Ok((_, segmentation)) =
            aligned_legs(&track, race_result, store.manual_offset(r.result)?)
        else {
            continue;
        };
        let Ok(physical) = race_physical(&track, &segmentation, &options, limits) else {
            continue;
        };
        found.push((
            PhysicalRaceRow {
                result_id: r.result.0,
                date: r.event_date,
                name: r.event_name.clone(),
                format: r.event_format,
                kind: r.event_kind,
                class_name: r.class_name.clone(),
                figures: RaceFigures::of(&physical),
            },
            physical,
        ));
    }

    let races: Vec<PhysicalRace<'_>> = found
        .iter()
        .map(|(row, physical)| PhysicalRace {
            date: row.date,
            format: row.format,
            kind: row.kind,
            physical,
        })
        .collect();
    let history = physical_history(&races, filter);
    Ok(PhysicalHistoryView {
        races_in_filter: in_filter,
        races_without_track: in_filter - found.len(),
        has_zones: zones.is_some(),
        races: found.into_iter().map(|(row, _)| row).collect(),
        history,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{ImportRequest, import, preview};
    use crate::race_physical::{PhysicalView, race_physical_view};
    use crate::zones::Zones;
    use tramos_core::identify::RunnerIdentity;
    use tramos_core::importers::spl;
    use tramos_core::physical::{BLOCKS, GradeClass, SpeedKind};
    use tramos_store::PersonId;

    fn fixture(name: &str) -> String {
        format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    /// El fixture de Baltanás (3-oct-2026, sprint) importado con el FIT sintético como el
    /// corredor de la tarjeta 143, y una copia sin FIT del 1-jun-2026 como media. Devuelve la
    /// base y el resultado con FIT.
    fn two_races() -> (Store, i64) {
        let mut store = Store::open_in_memory().unwrap();
        let identity = RunnerIdentity {
            si_card: Some(143),
            full_name: Some("N143 Apellido143".into()),
        };
        let spl_path = fixture("spl/baltanas-anon.spl");
        let fit = Some(fixture("fit/baltanas-sintetico.fit"));
        let p = preview(&store, &spl_path, fit.as_deref(), &identity).unwrap();
        let at = p.candidates[0].result;
        let outcome = import(
            &mut store,
            &ImportRequest {
                spl_path: spl_path.clone(),
                fit_path: fit,
                result: at,
                format: p.suggested_format,
                training: false,
                identity,
            },
        )
        .unwrap();
        assert_eq!(p.suggested_format, Some(RaceFormat::Sprint));

        let mut copy = spl::read(&std::fs::read(spl_path).unwrap()).unwrap();
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

    fn ready(store: &Store, result_id: i64) -> RacePhysical {
        match race_physical_view(store, result_id).unwrap() {
            PhysicalView::Ready(ready) => ready.physical,
            other => panic!("se esperaba la pestaña lista: {other:?}"),
        }
    }

    /// Solo aporta la carrera con FIT, con las cifras de su pestaña Físico; la copia sin FIT se
    /// cuenta aparte.
    #[test]
    fn only_races_with_a_fit_count_and_match_the_race_tab() {
        let (store, result_id) = two_races();
        let view = physical_history_view(&store, &HistoryFilter::default()).unwrap();
        assert_eq!(view.races_in_filter, 2);
        assert_eq!(view.races_without_track, 1);
        assert!(!view.has_zones);
        assert_eq!(view.races.len(), 1);

        let row = &view.races[0];
        let physical = ready(&store, result_id);
        assert_eq!(row.result_id, result_id);
        assert_eq!(row.date, date("2026-10-03"));
        assert_eq!(row.format, Some(RaceFormat::Sprint));
        assert_eq!(row.kind, EventKind::Race);
        assert_eq!(row.figures, RaceFigures::of(&physical));
        assert_eq!(row.figures.speed, SpeedKind::Flat);
        assert!(row.figures.efficiency_m_per_beat.is_some());
        assert!(row.figures.transit_speed_mps.is_some());
        assert_eq!(row.figures.load, None);

        // Con una sola carrera, sus clases y sus décimas son las suyas.
        let h = &view.history;
        assert_eq!((h.races, h.graded_races), (1, 1));
        assert_eq!(h.by_grade.len(), 5);
        for (grade, own) in h.by_grade.iter().zip(&physical.by_grade) {
            assert_eq!(grade.class, own.class);
            assert_eq!(grade.races, usize::from(own.stats.transit_s > 0.0));
            // Juntar un solo grupo da el mismo grupo, salvo por el último decimal.
            let (a, b) = (&grade.stats, &own.stats);
            let same = |x: Option<f64>, y: Option<f64>| match (x, y) {
                (Some(x), Some(y)) => (x - y).abs() < 1e-9 * y.abs().max(1.0),
                (x, y) => x == y,
            };
            assert!(
                (a.transit_s - b.transit_s).abs() < 1e-9,
                "{:?}",
                grade.class
            );
            assert!(same(a.speed_mps, b.speed_mps), "{:?}", grade.class);
            assert!(
                same(a.flat_speed_mps, b.flat_speed_mps),
                "{:?}",
                grade.class
            );
            assert!(
                same(a.heart_rate_bpm, b.heart_rate_bpm),
                "{:?}",
                grade.class
            );
            assert!(same(a.efficiency_m_per_beat, b.efficiency_m_per_beat));
            assert!(same(a.cadence_spm, b.cadence_spm), "{:?}", grade.class);
            assert!(same(a.stride_m, b.stride_m), "{:?}", grade.class);
        }
        assert!(
            h.by_grade
                .iter()
                .any(|g| g.class == GradeClass::Flat && g.races == 1)
        );
        assert_eq!(h.drift.len(), BLOCKS);
        let base = physical.transit.efficiency_m_per_beat.unwrap();
        for block in &physical.blocks {
            let tenth = &h.drift[block.index];
            assert_eq!(tenth.races, 1);
            let relative = block.stats.efficiency_m_per_beat.unwrap() / base;
            assert!((tenth.relative_efficiency.unwrap() - relative).abs() < 1e-12);
        }
    }

    /// El filtro es el del histórico: fechas, formato y tipo.
    #[test]
    fn the_history_filter_applies() {
        let (store, _) = two_races();
        let count = |filter: HistoryFilter| {
            let view = physical_history_view(&store, &filter).unwrap();
            (view.races_in_filter, view.races.len(), view.history.races)
        };
        assert_eq!(count(HistoryFilter::default()), (2, 1, 1));
        let middle = HistoryFilter {
            format: Some(RaceFormat::Middle),
            ..HistoryFilter::default()
        };
        assert_eq!(count(middle), (1, 0, 0));
        let june = HistoryFilter {
            to: Some(date("2026-09-30")),
            ..HistoryFilter::default()
        };
        assert_eq!(count(june), (1, 0, 0));
        let training = HistoryFilter {
            kind: Some(EventKind::Training),
            ..HistoryFilter::default()
        };
        assert_eq!(count(training), (0, 0, 0));
    }

    /// Las zonas de Ajustes llegan al núcleo: con ellas hay carga.
    #[test]
    fn the_load_comes_from_the_heart_rate_zones_of_the_settings() {
        let (mut store, result_id) = two_races();
        let zones = Zones {
            limits: vec![150.0, 165.0, 180.0],
            colors: vec![
                "#6b7280".into(),
                "#2563eb".into(),
                "#15803d".into(),
                "#b91c1c".into(),
            ],
        };
        store
            .set_setting(
                settings::HEART_RATE_ZONES_KEY,
                &Zones::to_setting(Some(&zones)),
            )
            .unwrap();
        let view = physical_history_view(&store, &HistoryFilter::default()).unwrap();
        assert!(view.has_zones);
        let load = ready(&store, result_id).heart_rate_zones.unwrap().load;
        assert_eq!(view.races[0].figures.load, Some(load));
    }

    /// Sin carreras, o sin ninguna con FIT, hay cifras vacías y no un error.
    #[test]
    fn no_races_or_no_fit_give_empty_figures() {
        let store = Store::open_in_memory().unwrap();
        let view = physical_history_view(&store, &HistoryFilter::default()).unwrap();
        assert_eq!((view.races_in_filter, view.races_without_track), (0, 0));
        assert!(view.races.is_empty());
        assert_eq!(view.history.by_grade.len(), 5);
        assert_eq!(view.history.drift.len(), BLOCKS);

        // Una carrera de las dos sin FIT bajo el filtro de la media: nada que dibujar.
        let (store, _) = two_races();
        let filter = HistoryFilter {
            format: Some(RaceFormat::Middle),
            ..HistoryFilter::default()
        };
        let view = physical_history_view(&store, &filter).unwrap();
        assert_eq!((view.races_in_filter, view.races_without_track), (1, 1));
        assert!(view.races.is_empty());
        assert!(view.history.drift.iter().all(|d| d.races == 0));
    }
}
