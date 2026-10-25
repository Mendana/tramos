//! Exportar los datos para análisis (#217, `docs/exportar-datos.md`): escribe en una carpeta
//! `races.csv`, `legs.csv`, `schema.json` y `LEEME.md` con las carreras propias y, si se pide
//! (#219), `samples.csv` con la serie de cada carrera con FIT, con o sin coordenadas.
//!
//! Aquí solo se lee la base y se reúne lo que ya calcula el núcleo: el informe de cada carrera
//! con las etiquetas ([`runner_report`], como en el histórico), las métricas de tramo del FIT, el
//! rendimiento físico ([`race_physical`]) y la serie de la carrera ([`race_series`], la misma de
//! la pestaña Físico pero cada segundo). Las filas y los formatos son de [`tramos_core::export`].

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
    Column, ExportError, ExportOptions, ExportSummary, LEG_COLUMNS, RACE_COLUMNS, RaceMeta,
    RaceRecord, SAMPLES_STEP_S, leg_records, readme_md, sample_columns, sample_rows, schema_json,
    write_csv, write_csv_header, write_csv_rows,
};
use tramos_core::metrics::{MetricsOptions, leg_metrics};
use tramos_core::model::{Event, RaceStatus, Track};
use tramos_core::physical::race_physical;
use tramos_core::physical_series::{race_series, series_positions};
use tramos_core::runner_report::runner_report;
use tramos_core::segmentation::Segmentation;
use tramos_store::{EventId, Store, StoreError};

use crate::import::stored_self_person;
use crate::race_map::aligned_legs;
use crate::races::RaceError;
use crate::settings;
use crate::tags::result_tags;

/// El fichero de la serie.
const SAMPLES_FILE: &str = "samples.csv";

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
    /// Carreras con filas en `samples.csv` (0 si no se ha pedido la serie).
    pub sample_races: usize,
    /// Filas de `samples.csv` (0 si no se ha pedido la serie).
    pub sample_rows: usize,
    /// Los ficheros escritos.
    pub files: Vec<String>,
    /// Los ficheros de una exportación anterior que se han borrado.
    pub removed: Vec<String>,
}

/// El `samples.csv` en construcción: ya en bytes, carrera a carrera, porque a un instante por
/// segundo son miles de filas por carrera y no hace falta tenerlas todas como texto a la vez.
struct SamplesCsv {
    columns: &'static [Column],
    coordinates: bool,
    bytes: Vec<u8>,
    races: usize,
    rows: usize,
}

impl SamplesCsv {
    fn new(options: ExportOptions) -> Result<Self, ExportDataError> {
        let columns = sample_columns(options.with_coordinates());
        let mut bytes = Vec::new();
        write_csv_header(&mut bytes, columns)?;
        Ok(Self {
            columns,
            coordinates: options.with_coordinates(),
            bytes,
            races: 0,
            rows: 0,
        })
    }

    /// Añade la serie de una carrera ya alineada. Si el núcleo no puede dar la serie, la carrera
    /// no tiene filas.
    fn add(
        &mut self,
        race_id: i64,
        track: &Track,
        segmentation: &Segmentation,
        options: &MetricsOptions,
    ) -> Result<(), ExportDataError> {
        let Ok(series) = race_series(track, segmentation, options, SAMPLES_STEP_S) else {
            return Ok(());
        };
        if series.samples.is_empty() {
            return Ok(());
        }
        let positions = self
            .coordinates
            .then(|| series_positions(track, segmentation, &series));
        let rows = sample_rows(race_id, &series, positions.as_deref());
        write_csv_rows(&mut self.bytes, self.columns, &rows)?;
        self.races += 1;
        self.rows += rows.len();
        Ok(())
    }
}

/// Las filas de las carreras propias y las de sus tramos, listas para escribir.
struct Tables {
    races: Vec<Vec<String>>,
    legs: Vec<Vec<String>>,
}

/// Reúne las filas de todas las carreras propias en las que el usuario tomó la salida, de la más
/// antigua a la más reciente. Con `samples`, añade ahí la serie de las que tienen track.
fn tables(store: &Store, mut samples: Option<&mut SamplesCsv>) -> Result<Tables, ExportDataError> {
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
            // Solo las carreras con `has_track`: las mismas que cuentan con cifras físicas.
            if metrics.is_some()
                && let Some(samples) = samples.as_deref_mut()
            {
                samples.add(r.result.0, &track, &segmentation, &options)?;
            }
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
/// `legs.csv`, `schema.json` y `LEEME.md`, y con `options.samples`, `samples.csv`. Sustituye los
/// que ya hubiera con esos nombres. Sin `options.samples` borra el `samples.csv` de una
/// exportación anterior, para que no quede junto a un esquema que ya no lo cuenta.
pub fn export_analysis(
    store: &Store,
    dir: &str,
    options: ExportOptions,
) -> Result<ExportReport, ExportDataError> {
    let dir = Path::new(dir);
    // Se arma todo antes de escribir nada: si falla, la carpeta no se toca.
    let mut samples = options
        .samples
        .then(|| SamplesCsv::new(options))
        .transpose()?;
    let tables = tables(store, samples.as_mut())?;
    let summary = ExportSummary {
        races: tables.races.len(),
        legs: tables.legs.len(),
        sample_races: samples.as_ref().map_or(0, |s| s.races),
        sample_rows: samples.as_ref().map_or(0, |s| s.rows),
    };
    let mut races = Vec::new();
    write_csv(&mut races, RACE_COLUMNS, &tables.races)?;
    let mut legs = Vec::new();
    write_csv(&mut legs, LEG_COLUMNS, &tables.legs)?;
    let mut schema = serde_json::to_vec_pretty(&schema_json(options))?;
    schema.push(b'\n');
    let readme = readme_md(summary, options, Utc::now());

    fs::create_dir_all(dir).map_err(|source| ExportDataError::Write {
        path: dir.to_string_lossy().into_owned(),
        source,
    })?;
    let mut files = vec!["races.csv".to_string(), "legs.csv".to_string()];
    write_file(dir, "races.csv", &races)?;
    write_file(dir, "legs.csv", &legs)?;
    let mut removed = Vec::new();
    match &samples {
        Some(samples) => {
            write_file(dir, SAMPLES_FILE, &samples.bytes)?;
            files.push(SAMPLES_FILE.to_string());
        }
        None => match fs::remove_file(dir.join(SAMPLES_FILE)) {
            Ok(()) => removed.push(SAMPLES_FILE.to_string()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ExportDataError::Write {
                    path: dir.join(SAMPLES_FILE).to_string_lossy().into_owned(),
                    source,
                });
            }
        },
    }
    write_file(dir, "schema.json", &schema)?;
    write_file(dir, "LEEME.md", readme.as_bytes())?;
    files.extend(["schema.json".to_string(), "LEEME.md".to_string()]);
    Ok(ExportReport {
        folder: dir.to_string_lossy().into_owned(),
        races: summary.races,
        legs: summary.legs,
        sample_races: summary.sample_races,
        sample_rows: summary.sample_rows,
        files,
        removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{ImportRequest, import, preview};
    use crate::race_physical::{PhysicalView, race_physical_view};
    use crate::races::race_detail;
    use crate::tags::save_leg_tag;
    use tramos_core::export::{FORMAT_VERSION, SAMPLE_COLUMNS};
    use tramos_core::identify::RunnerIdentity;
    use tramos_core::importers::spl;
    use tramos_core::race_format::RaceFormat;
    use tramos_core::taxonomy::{Confirmation, LegTag};
    use tramos_store::{PersonId, ResultId};

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
        let report =
            export_analysis(&store, out.to_str().unwrap(), ExportOptions::default()).unwrap();

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
        export_analysis(
            &store,
            dir.path().to_str().unwrap(),
            ExportOptions::default(),
        )
        .unwrap();
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
        export_analysis(
            &store,
            dir.path().to_str().unwrap(),
            ExportOptions::default(),
        )
        .unwrap();

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

    const WITH_SAMPLES: ExportOptions = ExportOptions {
        samples: true,
        coordinates: false,
    };
    const WITH_COORDINATES: ExportOptions = ExportOptions {
        samples: true,
        coordinates: true,
    };

    fn column_names(columns: &[Column]) -> Vec<String> {
        columns.iter().map(|c| c.name.to_string()).collect()
    }

    /// La serie de la pestaña Físico (cada 5 s) del resultado.
    fn tab_series(store: &Store, result_id: i64) -> tramos_core::physical_series::RaceSeries {
        match race_physical_view(store, result_id).unwrap() {
            PhysicalView::Ready(ready) => ready.series,
            other => panic!("se esperaba la pestaña lista: {other:?}"),
        }
    }

    /// Con la casilla de la serie sale `samples.csv`: solo la carrera con track, un instante por
    /// segundo, con los números de la pestaña Físico y sin coordenadas.
    #[test]
    fn exports_the_series_of_the_race_with_track() {
        let (store, result_id) = two_races();
        let dir = tempfile::tempdir().unwrap();
        let report = export_analysis(&store, dir.path().to_str().unwrap(), WITH_SAMPLES).unwrap();

        let (header, rows) = read_csv(&dir.path().join("samples.csv"));
        assert_eq!(header, column_names(sample_columns(false)));
        assert_eq!(header.len(), 11);
        assert!(!header.iter().any(|h| h == "lat" || h == "lon"));
        assert!(rows.iter().all(|r| r.len() == header.len()));
        assert_eq!(
            report.files,
            [
                "races.csv",
                "legs.csv",
                "samples.csv",
                "schema.json",
                "LEEME.md"
            ]
        );
        assert!(report.removed.is_empty());

        // Solo la carrera con track (la copia de junio no tiene FIT).
        let id = result_id.to_string();
        assert!(rows.iter().all(|r| cell(&header, r, "race_id") == id));
        assert_eq!((report.sample_races, report.sample_rows), (1, rows.len()));

        // Un instante cada segundo, de la salida a la meta, más el de la meta.
        let tab = tab_series(&store, result_id);
        let whole = tab.duration_s.floor();
        let expected = whole as usize + 1 + usize::from(tab.duration_s - whole > 1e-9);
        assert_eq!(rows.len(), expected);
        assert!(rows.len() > 300, "{} filas", rows.len());
        let times: Vec<f64> = rows
            .iter()
            .map(|r| cell(&header, r, "t_s").parse().unwrap())
            .collect();
        assert_eq!(times[0], 0.0);
        assert!(
            times
                .windows(2)
                .all(|w| w[1] > w[0] && w[1] - w[0] <= 1.0 + 1e-9)
        );
        assert!((times[times.len() - 1] - tab.duration_s).abs() < 1e-3);

        // Los mismos números que la serie de la pestaña (cada 5 s), con el redondeo del CSV.
        let close = |name: &str, row: &[String], value: Option<f64>| match value {
            Some(v) => {
                let got: f64 = cell(&header, row, name).parse().unwrap();
                assert!((got - v).abs() < 0.0051, "{name}: {got} frente a {v}");
            }
            None => assert_eq!(cell(&header, row, name), "", "{name}"),
        };
        for (k, sample) in tab.samples.iter().enumerate().take(tab.samples.len() - 1) {
            let row = &rows[5 * k];
            close("t_s", row, Some(sample.t_s));
            close("distance_m", row, Some(sample.distance_m));
            close("altitude_m", row, sample.altitude_m);
            close("speed_mps", row, sample.speed_mps);
            close("flat_speed_mps", row, sample.flat_speed_mps);
            close("heart_rate_bpm", row, sample.heart_rate_bpm);
            close("stride_m", row, sample.stride_m);
            assert_eq!(
                cell(&header, row, "leg"),
                sample.leg.map_or_else(String::new, |l| l.to_string())
            );
        }
        // Los estados son los de la serie, y el último instante, la meta.
        let states = ["transit", "attack", "stopped", "gap"];
        assert!(
            rows.iter()
                .all(|r| states.contains(&cell(&header, r, "state")))
        );
        assert!(
            rows.iter().any(|r| cell(&header, r, "state") == "transit"
                && !cell(&header, r, "speed_mps").is_empty())
        );
        let last = rows.last().unwrap();
        assert_eq!(cell(&header, last, "leg"), "21");

        // El esquema y el LEEME cuentan el fichero, su paso y sin coordenadas.
        let schema: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join("schema.json")).unwrap())
                .unwrap();
        let tables = schema["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 3);
        assert_eq!(tables[2]["file"], "samples.csv");
        assert_eq!(tables[2]["step_s"], 1.0);
        assert_eq!(tables[2]["columns"].as_array().unwrap().len(), 11);
        let readme = std::fs::read_to_string(dir.path().join("LEEME.md")).unwrap();
        assert!(readme.contains("`samples.csv`"));
        assert!(readme.contains(&format!("{} filas de 1 carreras", rows.len())));
        assert!(readme.contains("ni coordenadas"));
    }

    /// Con la casilla de coordenadas, `samples.csv` lleva `lat` y `lon` dentro del track del FIT,
    /// y vacíos en los huecos; las otras columnas no cambian.
    #[test]
    fn exports_the_coordinates_only_when_asked() {
        let (store, result_id) = two_races();
        let plain = tempfile::tempdir().unwrap();
        export_analysis(&store, plain.path().to_str().unwrap(), WITH_SAMPLES).unwrap();
        let located = tempfile::tempdir().unwrap();
        let report =
            export_analysis(&store, located.path().to_str().unwrap(), WITH_COORDINATES).unwrap();

        let (plain_header, plain_rows) = read_csv(&plain.path().join("samples.csv"));
        let (header, rows) = read_csv(&located.path().join("samples.csv"));
        assert_eq!(header, column_names(SAMPLE_COLUMNS));
        assert_eq!(&header[11..], ["lat", "lon"]);
        assert_eq!(rows.len(), plain_rows.len());
        assert_eq!(report.sample_rows, rows.len());
        assert!(rows.iter().all(|r| r.len() == 13));
        for (with, without) in rows.iter().zip(&plain_rows) {
            assert_eq!(&with[..11], &without[..]);
        }
        assert_eq!(&header[..11], &plain_header[..]);

        // Dentro de la caja del track guardado (con un margen de redondeo), y vacías solo en los
        // huecos.
        let track = store.load_track(ResultId(result_id)).unwrap().unwrap();
        let (mut lat_min, mut lat_max) = (f64::MAX, f64::MIN);
        let (mut lon_min, mut lon_max) = (f64::MAX, f64::MIN);
        for p in &track.points {
            lat_min = lat_min.min(p.lat);
            lat_max = lat_max.max(p.lat);
            lon_min = lon_min.min(p.lon);
            lon_max = lon_max.max(p.lon);
        }
        let mut positioned = 0;
        for row in &rows {
            let (lat, lon) = (cell(&header, row, "lat"), cell(&header, row, "lon"));
            assert_eq!(lat.is_empty(), lon.is_empty());
            assert_eq!(
                lat.is_empty(),
                cell(&header, row, "state") == "gap",
                "las posiciones faltan solo en los huecos"
            );
            if lat.is_empty() {
                continue;
            }
            let (lat, lon): (f64, f64) = (lat.parse().unwrap(), lon.parse().unwrap());
            assert!((lat_min - 1e-6..=lat_max + 1e-6).contains(&lat), "{lat}");
            assert!((lon_min - 1e-6..=lon_max + 1e-6).contains(&lon), "{lon}");
            positioned += 1;
        }
        assert!(positioned > 300);

        // El LEEME avisa de que hay coordenadas.
        let readme = std::fs::read_to_string(located.path().join("LEEME.md")).unwrap();
        assert!(readme.contains("dicen por dónde has corrido"));
        assert!(!readme.contains("ni coordenadas"));
        let schema = std::fs::read_to_string(located.path().join("schema.json")).unwrap();
        assert!(schema.contains("\"lat\"") && schema.contains("\"lon\""));
    }

    /// Sin la casilla de la serie no sale `samples.csv` ni coordenadas en ningún fichero, aunque
    /// se pidan las coordenadas.
    #[test]
    fn without_the_series_nothing_has_coordinates() {
        let (store, _) = two_races();
        for options in [
            ExportOptions::default(),
            ExportOptions {
                samples: false,
                coordinates: true,
            },
        ] {
            let dir = tempfile::tempdir().unwrap();
            let report = export_analysis(&store, dir.path().to_str().unwrap(), options).unwrap();
            assert_eq!(
                report.files,
                ["races.csv", "legs.csv", "schema.json", "LEEME.md"]
            );
            assert_eq!((report.sample_races, report.sample_rows), (0, 0));
            assert!(report.removed.is_empty());
            assert!(!dir.path().join("samples.csv").exists());
            for name in ["races.csv", "legs.csv", "schema.json", "LEEME.md"] {
                let text = std::fs::read_to_string(dir.path().join(name)).unwrap();
                assert!(!text.contains("samples"), "{name}");
                assert!(
                    !text.contains("\"lat\"") && !text.contains("\"lon\""),
                    "{name}"
                );
                assert!(!text.starts_with("race_id,t_s"), "{name}");
            }
            let (race_header, _) = read_csv(&dir.path().join("races.csv"));
            let (leg_header, _) = read_csv(&dir.path().join("legs.csv"));
            for h in race_header.iter().chain(&leg_header) {
                assert!(h != "lat" && h != "lon", "{h}");
            }
        }
    }

    /// Exportar sin la serie en una carpeta con un `samples.csv` de antes lo borra y lo dice.
    #[test]
    fn a_previous_series_is_removed_when_not_asked() {
        let (store, _) = two_races();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        export_analysis(&store, path, WITH_COORDINATES).unwrap();
        assert!(dir.path().join("samples.csv").exists());

        // Un fichero que no es del formato no se toca.
        std::fs::write(dir.path().join("mis-notas.txt"), "hola").unwrap();
        let report = export_analysis(&store, path, ExportOptions::default()).unwrap();
        assert_eq!(report.removed, ["samples.csv"]);
        assert!(!dir.path().join("samples.csv").exists());
        assert!(dir.path().join("mis-notas.txt").exists());
        assert_eq!(report.files.len(), 4);

        // La siguiente vez ya no hay nada que borrar.
        let again = export_analysis(&store, path, ExportOptions::default()).unwrap();
        assert!(again.removed.is_empty());
    }

    /// Sin carreras con track, `samples.csv` lleva solo la cabecera.
    #[test]
    fn without_a_track_the_series_has_only_the_header() {
        let store = Store::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let report =
            export_analysis(&store, dir.path().to_str().unwrap(), WITH_COORDINATES).unwrap();
        assert_eq!((report.sample_races, report.sample_rows), (0, 0));
        let (header, rows) = read_csv(&dir.path().join("samples.csv"));
        assert_eq!(header.len(), 13);
        assert!(rows.is_empty());
    }

    #[test]
    fn without_a_self_person_it_exports_empty_tables() {
        let store = Store::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let report = export_analysis(
            &store,
            dir.path().to_str().unwrap(),
            ExportOptions::default(),
        )
        .unwrap();
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
        let err =
            export_analysis(&store, file.to_str().unwrap(), ExportOptions::default()).unwrap_err();
        assert!(err.to_string().starts_with("no se pudo escribir"), "{err}");
    }
}
