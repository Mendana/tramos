//! Exportar los datos para análisis (#217, `docs/exportar-datos.md`): escribe en una carpeta
//! `races.csv`, `legs.csv`, `schema.json` y `LEEME.md` con las carreras propias.
//!
//! Aquí solo se lee la base y se reúne lo que ya calcula el núcleo: el informe de cada carrera
//! con las etiquetas ([`runner_report`], como en el histórico), las métricas de tramo del FIT y
//! el rendimiento físico ([`race_physical`]). Las filas y los formatos son de
//! [`tramos_core::export`].

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs;
use std::io;
use std::path::Path;

use chrono::{NaiveDate, Utc};
use serde::Serialize;
use thiserror::Error;
use tramos_core::days_off::days_since_previous;
use tramos_core::export::{
    ExportError, ExportSummary, LEG_COLUMNS, RACE_COLUMNS, RaceMeta, RaceRecord, leg_records,
    readme_md, schema_json, write_csv,
};
use tramos_core::metrics::{MetricsOptions, leg_metrics};
use tramos_core::model::{Event, RaceStatus};
use tramos_core::physical::race_physical;
use tramos_core::runner_report::runner_report;
use tramos_store::{EventId, Store, StoreError};

use crate::import::stored_self_person;
use crate::race_map::aligned_legs;
use crate::races::RaceError;
use crate::settings;
use crate::tags::result_tags;

/// Errores al exportar. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum ExportDataError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Race(#[from] RaceError),
    #[error("no se pudo escribir {path}: {source}")]
    Write { path: String, source: io::Error },
    #[error("no se pudo preparar el fichero: {0}")]
    Format(#[from] ExportError),
    #[error("no se pudo escribir el esquema: {0}")]
    Schema(#[from] serde_json::Error),
}

/// Lo que se ha exportado, para enseñarlo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportReport {
    /// La carpeta donde se ha escrito.
    pub folder: String,
    pub races: usize,
    pub legs: usize,
    /// Los ficheros escritos.
    pub files: Vec<String>,
}

/// Las filas de las carreras propias y las de sus tramos, listas para escribir.
struct Tables {
    races: Vec<Vec<String>>,
    legs: Vec<Vec<String>>,
}

/// Reúne las filas de todas las carreras propias en las que el usuario tomó la salida, de la más
/// antigua a la más reciente.
fn tables(store: &Store) -> Result<Tables, ExportDataError> {
    let mut tables = Tables {
        races: Vec::new(),
        legs: Vec::new(),
    };
    let Some(person) = stored_self_person(store)? else {
        return Ok(tables);
    };
    let config = settings::lost_time_config(store)?;
    let heart_rate_limits = settings::load(store)?
        .map
        .heart_rate_zones
        .map(|zones| zones.limits);
    let options = MetricsOptions::default();
    // Las carreras con salida, de la más antigua a la más reciente: no presentarse no es competir
    // ni cuenta como «carrera anterior» (igual que en el histórico, P11).
    let results: Vec<_> = store
        .person_results(person)?
        .into_iter()
        .filter(|r| r.status != RaceStatus::DidNotStart)
        .collect();
    let competed: Vec<NaiveDate> = results.iter().map(|r| r.event_date).collect();

    let mut events: HashMap<EventId, Event> = HashMap::new();
    for r in &results {
        let (event_id, at) = store.result_ref(r.result)?;
        let event = match events.entry(event_id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(store.load_event(event_id)?),
        };
        let tags = result_tags(store, r.result)?;
        let report =
            runner_report(event, at, &config, &tags).ok_or(RaceError::NotAnalyzed(r.result.0))?;

        // El track se alinea y se trocea como en el mapa y el histórico: las métricas de tramo
        // y las físicas salen de los mismos tramos. Sin track, o si ya no se puede alinear, la
        // carrera solo trae lo de las picadas.
        let mut metrics = None;
        let mut physical = None;
        if let (Some(track), Some(race_result)) = (store.load_track(r.result)?, at.get(event))
            && let Ok((_, segmentation)) =
                aligned_legs(&track, race_result, store.manual_offset(r.result)?)
        {
            metrics = leg_metrics(&track, &segmentation, &options).ok();
            physical = race_physical(
                &track,
                &segmentation,
                &options,
                heart_rate_limits.as_deref(),
            )
            .ok();
        }

        let days = days_since_previous(r.event_date, &competed);
        let meta = RaceMeta {
            race_id: r.result.0,
            event_id: event_id.0,
            date: r.event_date,
            kind: r.event_kind,
            format: r.event_format,
            status: r.status,
            place: r.place,
            has_track: metrics.is_some(),
            days_since_previous: days,
        };
        tables
            .races
            .push(RaceRecord::new(meta, &report, physical).cells());
        tables.legs.extend(
            leg_records(r.result.0, &report, &tags, metrics.as_deref(), days)
                .iter()
                .map(|leg| leg.cells()),
        );
    }
    Ok(tables)
}

/// Escribe `bytes` en `dir/name`.
fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), ExportDataError> {
    let path = dir.join(name);
    fs::write(&path, bytes).map_err(|source| ExportDataError::Write {
        path: path.to_string_lossy().into_owned(),
        source,
    })
}

/// Exporta las carreras propias a la carpeta `dir` (la crea si no existe): `races.csv`,
/// `legs.csv`, `schema.json` y `LEEME.md`. Sustituye los que ya hubiera con esos nombres.
pub fn export_analysis(store: &Store, dir: &str) -> Result<ExportReport, ExportDataError> {
    let dir = Path::new(dir);
    // Se arma todo antes de escribir nada: si falla, la carpeta no se toca.
    let tables = tables(store)?;
    let summary = ExportSummary {
        races: tables.races.len(),
        legs: tables.legs.len(),
    };
    let mut races = Vec::new();
    write_csv(&mut races, RACE_COLUMNS, &tables.races)?;
    let mut legs = Vec::new();
    write_csv(&mut legs, LEG_COLUMNS, &tables.legs)?;
    let mut schema = serde_json::to_vec_pretty(&schema_json())?;
    schema.push(b'\n');
    let readme = readme_md(summary, Utc::now());

    fs::create_dir_all(dir).map_err(|source| ExportDataError::Write {
        path: dir.to_string_lossy().into_owned(),
        source,
    })?;
    write_file(dir, "races.csv", &races)?;
    write_file(dir, "legs.csv", &legs)?;
    write_file(dir, "schema.json", &schema)?;
    write_file(dir, "LEEME.md", readme.as_bytes())?;
    Ok(ExportReport {
        folder: dir.to_string_lossy().into_owned(),
        races: summary.races,
        legs: summary.legs,
        files: ["races.csv", "legs.csv", "schema.json", "LEEME.md"]
            .map(String::from)
            .to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{ImportRequest, import, preview};
    use crate::races::race_detail;
    use crate::tags::save_leg_tag;
    use tramos_core::export::FORMAT_VERSION;
    use tramos_core::identify::RunnerIdentity;
    use tramos_core::importers::spl;
    use tramos_core::race_format::RaceFormat;
    use tramos_core::taxonomy::{Confirmation, LegTag};
    use tramos_store::PersonId;

    fn spl_path() -> String {
        format!(
            "{}/../../fixtures/spl/baltanas-anon.spl",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn fit_path() -> String {
        format!(
            "{}/../../fixtures/fit/baltanas-sintetico.fit",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn identity() -> RunnerIdentity {
        RunnerIdentity {
            si_card: Some(143),
            full_name: Some("N143 Apellido143".into()),
        }
    }

    /// Importa el fixture de Baltanás (3-oct-2026) con su FIT sintético como el corredor de la
    /// tarjeta 143, y guarda una copia sin track el 1-jun-2026, como media, con el mismo
    /// resultado vinculado a su persona.
    fn two_races() -> (Store, i64) {
        let mut store = Store::open_in_memory().unwrap();
        let fit = Some(fit_path());
        let p = preview(&store, &spl_path(), fit.as_deref(), &identity()).unwrap();
        let at = p.candidates[0].result;
        let outcome = import(
            &mut store,
            &ImportRequest {
                spl_path: spl_path(),
                fit_path: fit,
                result: at,
                format: p.suggested_format,
                training: false,
                identity: identity(),
            },
        )
        .unwrap();

        let mut copy = spl::read(&std::fs::read(spl_path()).unwrap()).unwrap();
        copy.date = "2026-06-01".parse().unwrap();
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

    /// Un CSV sin campos entre comillas (no hay texto libre en estas tablas) como filas de
    /// celdas.
    fn read_csv(path: &Path) -> (Vec<String>, Vec<Vec<String>>) {
        let text = std::fs::read_to_string(path).unwrap();
        assert!(!text.contains('"'), "no debería hacer falta entrecomillar");
        let mut lines = text
            .lines()
            .map(|l| l.split(',').map(str::to_string).collect::<Vec<String>>());
        let header = lines.next().unwrap();
        (header, lines.collect())
    }

    fn cell<'a>(header: &[String], row: &'a [String], name: &str) -> &'a str {
        let i = header.iter().position(|h| h == name).unwrap();
        &row[i]
    }

    #[test]
    fn exports_the_races_and_legs_with_the_documented_columns() {
        let (store, result_id) = two_races();
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("analisis");
        let report = export_analysis(&store, out.to_str().unwrap()).unwrap();

        let detail = race_detail(&store, result_id).unwrap();
        let lost = &detail.report.lost_time;
        assert_eq!((report.races, report.legs), (2, 2 * lost.legs.len()));
        assert_eq!(
            report.files,
            ["races.csv", "legs.csv", "schema.json", "LEEME.md"]
        );

        // Columnas: las del núcleo, en su orden.
        let (race_header, races) = read_csv(&out.join("races.csv"));
        let (leg_header, legs) = read_csv(&out.join("legs.csv"));
        let names = |cols: &[tramos_core::export::Column]| -> Vec<String> {
            cols.iter().map(|c| c.name.to_string()).collect()
        };
        assert_eq!(race_header, names(RACE_COLUMNS));
        assert_eq!(leg_header, names(LEG_COLUMNS));
        assert!(races.iter().all(|r| r.len() == race_header.len()));
        assert!(legs.iter().all(|r| r.len() == leg_header.len()));

        // De la más antigua a la más reciente: la copia del 1-jun y la del 3-oct.
        assert_eq!(races.len(), 2);
        let (june, october) = (&races[0], &races[1]);
        assert_eq!(cell(&race_header, june, "date"), "2026-06-01");
        assert_eq!(cell(&race_header, june, "format"), "middle");
        assert_eq!(cell(&race_header, june, "has_track"), "false");
        assert_eq!(cell(&race_header, june, "days_since_previous_race"), "");
        assert_eq!(cell(&race_header, june, "efficiency_m_per_beat"), "");
        assert_eq!(cell(&race_header, june, "transit_s"), "");

        // La carrera del 3-oct, con los números de la vista de carrera.
        assert_eq!(
            cell(&race_header, october, "race_id"),
            result_id.to_string()
        );
        assert_eq!(cell(&race_header, october, "date"), "2026-10-03");
        assert_eq!(cell(&race_header, october, "kind"), "race");
        assert_eq!(cell(&race_header, october, "format"), "sprint");
        assert_eq!(cell(&race_header, october, "status"), "ok");
        assert_eq!(cell(&race_header, october, "legs"), "21");
        assert_eq!(
            cell(&race_header, october, "total_s"),
            lost.total_s.unwrap().to_string()
        );
        assert_eq!(
            cell(&race_header, october, "place"),
            detail.place.unwrap().to_string()
        );
        assert_eq!(
            cell(&race_header, october, "error_count"),
            lost.error_count.to_string()
        );
        assert_eq!(
            cell(&race_header, october, "usual_performance"),
            lost.usual_performance.unwrap().to_string()
        );
        // De junio a octubre: 124 días.
        assert_eq!(
            cell(&race_header, october, "days_since_previous_race"),
            "124"
        );
        // Con FIT: las columnas físicas de #212 vienen rellenas.
        assert_eq!(cell(&race_header, october, "has_track"), "true");
        for name in ["track_duration_s", "transit_s", "transit_speed_mps"] {
            assert!(
                !cell(&race_header, october, name).is_empty(),
                "{name} vacía"
            );
        }

        // Tramos: 21 por carrera; los de la carrera con track traen las métricas del FIT.
        let id = result_id.to_string();
        let tracked: Vec<_> = legs
            .iter()
            .filter(|l| cell(&leg_header, l, "race_id") == id)
            .collect();
        assert_eq!(tracked.len(), 21);
        let numbers: Vec<_> = tracked
            .iter()
            .map(|l| cell(&leg_header, l, "leg").to_string())
            .collect();
        assert_eq!(numbers, (1..=21).map(|i| i.to_string()).collect::<Vec<_>>());
        assert_eq!(cell(&leg_header, tracked[20], "is_last"), "true");
        assert_eq!(cell(&leg_header, tracked[20], "position"), "1");
        assert_eq!(cell(&leg_header, tracked[0], "third"), "1");
        assert_eq!(cell(&leg_header, tracked[20], "third"), "3");
        assert_eq!(cell(&leg_header, tracked[0], "previous_leg_error"), "");
        assert!(
            tracked
                .iter()
                .any(|l| !cell(&leg_header, l, "distance_m").is_empty())
        );
        let errors = tracked
            .iter()
            .filter(|l| cell(&leg_header, l, "is_error") == "true")
            .count();
        assert_eq!(errors, lost.error_count);
        // Los tramos de la copia sin track no traen métricas del FIT.
        assert!(
            legs.iter()
                .filter(|l| cell(&leg_header, l, "race_id") != id)
                .all(|l| cell(&leg_header, l, "distance_m").is_empty())
        );
        let first = &detail.report.lost_time.legs[0];
        assert_eq!(
            cell(&leg_header, tracked[0], "split_s"),
            first.split_s.unwrap().to_string()
        );
        assert_eq!(
            cell(&leg_header, tracked[0], "reference_s"),
            first.reference_s.unwrap().to_string()
        );

        // El esquema y el LEEME.
        let schema: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(out.join("schema.json")).unwrap())
                .unwrap();
        assert_eq!(schema["format_version"], FORMAT_VERSION);
        assert_eq!(
            schema["tables"][0]["columns"].as_array().unwrap().len(),
            RACE_COLUMNS.len()
        );
        let readme = std::fs::read_to_string(out.join("LEEME.md")).unwrap();
        assert!(readme.contains("2 carreras y 42 tramos"));
    }

    /// El error revisado y el propuesto se exportan por separado, con la etiqueta; las notas, que
    /// pueden llevar nombres, no salen.
    #[test]
    fn exports_the_tag_and_both_error_flags_but_not_the_note() {
        let (mut store, result_id) = two_races();
        let lost = race_detail(&store, result_id).unwrap().report.lost_time;
        let error = lost.legs.iter().find(|l| l.calculated_error).unwrap().index;
        let tag = LegTag {
            confirmation: Some(Confirmation::NoError),
            effort: Some(7),
            note: Some("Me perdí con Lucía Ejemplo".into()),
            ..LegTag::default()
        };
        save_leg_tag(&mut store, result_id, error, tag).unwrap();

        let dir = tempfile::tempdir().unwrap();
        export_analysis(&store, dir.path().to_str().unwrap()).unwrap();
        let (header, legs) = read_csv(&dir.path().join("legs.csv"));
        let row = legs
            .iter()
            .find(|l| {
                cell(&header, l, "race_id") == result_id.to_string()
                    && cell(&header, l, "leg") == error.to_string()
            })
            .unwrap();
        assert_eq!(cell(&header, row, "calculated_error"), "true");
        assert_eq!(cell(&header, row, "is_error"), "false");
        assert_eq!(cell(&header, row, "tag_confirmation"), "no_error");
        assert_eq!(cell(&header, row, "effort"), "7");
        for name in ["races.csv", "legs.csv", "schema.json", "LEEME.md"] {
            let text = std::fs::read_to_string(dir.path().join(name)).unwrap();
            assert!(!text.contains("Lucía"), "{name} lleva la nota");
        }
        let (race_header, races) = read_csv(&dir.path().join("races.csv"));
        let october = races
            .iter()
            .find(|r| cell(&race_header, r, "race_id") == result_id.to_string())
            .unwrap();
        // La etiqueta manda: un error menos que los propuestos.
        assert_eq!(
            cell(&race_header, october, "error_count"),
            (lost.error_count - 1).to_string()
        );
    }

    /// Ni nombres de nadie ni nada de los demás corredores: ni el corredor, ni los demás de la
    /// prueba, ni su club, ni el nombre de la prueba ni la categoría.
    #[test]
    fn exports_no_names() {
        let (store, result_id) = two_races();
        let detail = race_detail(&store, result_id).unwrap();
        let event = spl::read(&std::fs::read(spl_path()).unwrap()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        export_analysis(&store, dir.path().to_str().unwrap()).unwrap();

        let mut private: Vec<String> = vec![detail.class_name.clone()];
        private.extend(detail.name.clone());
        for class in &event.classes {
            for result in &class.results {
                let r = &result.runner;
                private.extend([r.given_name.clone(), r.family_name.clone()]);
                private.extend(r.club.clone());
            }
        }
        private.retain(|p| p.chars().count() >= 4);
        assert!(private.len() > 100);
        for name in ["races.csv", "legs.csv", "schema.json", "LEEME.md"] {
            let text = std::fs::read_to_string(dir.path().join(name)).unwrap();
            for p in &private {
                assert!(!text.contains(p.as_str()), "{name} lleva {p:?}");
            }
        }
        // Y ninguna columna de posiciones.
        for column in RACE_COLUMNS.iter().chain(LEG_COLUMNS) {
            let n = column.name;
            let positions = [
                "lat",
                "lon",
                "latitude",
                "longitude",
                "position_lat",
                "position_long",
            ];
            assert!(!positions.contains(&n) && !n.contains("name"), "{n}");
        }
    }

    #[test]
    fn without_a_self_person_it_exports_empty_tables() {
        let store = Store::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let report = export_analysis(&store, dir.path().to_str().unwrap()).unwrap();
        assert_eq!((report.races, report.legs), (0, 0));
        let (header, rows) = read_csv(&dir.path().join("races.csv"));
        assert_eq!(header.len(), RACE_COLUMNS.len());
        assert!(rows.is_empty());
    }

    #[test]
    fn a_folder_that_is_a_file_gives_a_clear_error() {
        let store = Store::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("no-es-carpeta");
        std::fs::write(&file, "x").unwrap();
        let err = export_analysis(&store, file.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().starts_with("no se pudo escribir"), "{err}");
    }
}
