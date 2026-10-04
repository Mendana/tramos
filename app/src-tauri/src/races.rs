//! Carreras del usuario: la lista y la vista de una carrera con su tabla de tramos.
//!
//! El tiempo perdido no se guarda: se calcula al pedirlo con [`runner_report`], la misma función
//! que usa `tramos analizar`, a partir de la carrera guardada (`docs/app.md`).

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use chrono::NaiveDate;
use serde::Serialize;
use thiserror::Error;
use tramos_core::lost_time::LostTimeConfig;
use tramos_core::model::{Event, RaceStatus};
use tramos_core::race_format::RaceFormat;
use tramos_core::runner_report::{RunnerReport, runner_report};
use tramos_store::{EventId, ResultId, Store, StoreError};

use crate::import::stored_self_person;

/// Errores al consultar las carreras. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum RaceError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("el resultado {0} no aparece en el análisis de su recorrido")]
    NotAnalyzed(i64),
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

/// Configuración del tiempo perdido. Hasta que haya ajustes (#17), la de por defecto.
fn config() -> LostTimeConfig {
    LostTimeConfig::default()
}

/// Carreras del usuario (los resultados vinculados a su persona), de la más reciente a la más
/// antigua, con su tiempo perdido. Vacía si aún no ha importado ninguna.
pub fn list_races(store: &Store) -> Result<Vec<RaceRow>, RaceError> {
    let Some(person) = stored_self_person(store)? else {
        return Ok(Vec::new());
    };
    let config = config();
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
    let config = config();
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

    #[test]
    fn missing_result_is_an_error() {
        let store = Store::open_in_memory().unwrap();
        assert!(matches!(
            race_detail(&store, 42),
            Err(RaceError::Store(StoreError::ResultNotFound(42)))
        ));
        assert!(list_races(&store).unwrap().is_empty());
    }
}
