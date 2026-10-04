//! Importar una carrera: el .spl y, si lo hay, el FIT del corredor.
//!
//! Va en dos pasos para que la interfaz pueda preguntar entre medias:
//!
//! 1. [`preview`] lee los ficheros sin guardar nada, identifica al corredor
//!    (`tramos_core::identify`) y sugiere el formato (`tramos_core::race_format`).
//! 2. [`import`] guarda el .spl original y la carrera (si ese .spl ya estaba importado, reutiliza
//!    la carrera en vez de duplicarla), fija el formato, vincula el resultado elegido con la
//!    persona del usuario y, con FIT, lo alinea y guarda el track.
//!
//! Todo es Rust sin Tauri, para probarlo con los fixtures. El flujo está en `docs/app.md`; la
//! lista de carreras, en [`crate::races`].

use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tramos_core::alignment::{AlignmentOptions, align};
use tramos_core::identify::{Identification, ResultRef, RunnerIdentity, identify_runner};
use tramos_core::importers::fit::{self, FitError};
use tramos_core::importers::spl::{self, SplError};
use tramos_core::model::{Event, RaceStatus, Track};
use tramos_core::race_format::{RaceFormat, median_winner_time_s, suggest_format};
use tramos_store::{EventId, PersonId, ResultId, SourceFileKind, Store, StoreError};

/// Ajuste con el id de la persona del usuario (`people`), a la que se vinculan sus resultados.
pub const SELF_PERSON_KEY: &str = "self.person_id";
/// Ajuste con la tarjeta SI del usuario, para buscarlo en la siguiente carrera.
pub const SELF_SI_CARD_KEY: &str = "self.si_card";
/// Ajuste con el nombre y apellidos del usuario, para lo mismo.
pub const SELF_FULL_NAME_KEY: &str = "self.full_name";
/// Nombre de la persona del usuario si al crearla no ha escrito el suyo.
const DEFAULT_SELF_NAME: &str = "Yo";

/// Errores al importar. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum ImportError {
    #[error("no se pudo leer {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("el .spl no es válido: {0}")]
    Spl(#[from] SplError),
    #[error("el FIT no es válido: {0}")]
    Fit(#[from] FitError),
    #[error("la carrera no tiene ese resultado (categoría {class_index}, posición {result_index})")]
    NoSuchResult {
        class_index: usize,
        result_index: usize,
    },
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Resultado que el usuario puede elegir como suyo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunnerChoice {
    pub result: ResultRef,
    pub class_name: String,
    pub given_name: String,
    pub family_name: String,
    pub club: Option<String>,
    pub si_card: Option<u32>,
    pub status: RaceStatus,
    pub place: Option<u16>,
}

/// Cómo ha ido la identificación del corredor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Match {
    /// Un solo resultado casa: se propone.
    Unique,
    /// Casa por tarjeta pero no por nombre (tarjeta prestada o reutilizada): hay que confirmar.
    UniqueNameMismatch,
    /// Varios resultados casan: hay que elegir entre `candidates`.
    Ambiguous,
    /// Ninguno casa (o no hay identidad): `candidates` trae todos los resultados.
    NotFound,
}

/// Lo que la interfaz enseña antes de importar.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportPreview {
    pub event_name: Option<String>,
    pub event_date: NaiveDate,
    /// Formato sugerido por la mediana de los ganadores de las categorías.
    pub suggested_format: Option<RaceFormat>,
    pub median_winner_s: Option<f64>,
    pub matched: Match,
    /// En el orden de la carrera.
    pub candidates: Vec<RunnerChoice>,
    /// Este .spl ya se importó: se reutilizará su carrera.
    pub already_imported: bool,
    /// Puntos del FIT, si se ha pasado.
    pub fit_points: Option<usize>,
}

/// Lo que confirma el usuario.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ImportRequest {
    pub spl_path: String,
    pub fit_path: Option<String>,
    /// Su resultado en la carrera.
    pub result: ResultRef,
    pub format: Option<RaceFormat>,
    /// Su identidad, que se guarda para la siguiente carrera.
    #[serde(default)]
    pub identity: RunnerIdentity,
}

/// Qué se ha guardado y qué hay que avisar.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImportOutcome {
    pub event_id: i64,
    pub result_id: i64,
    /// La carrera ya estaba guardada y se ha reutilizado.
    pub already_imported: bool,
    /// Solo con FIT.
    pub alignment: Option<AlignmentReport>,
    /// Avisos que no impiden importar, en español.
    pub warnings: Vec<String>,
}

/// Alineación del FIT con las picadas, resumida para la interfaz.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlignmentReport {
    /// El track se ha guardado. Si no se pudo alinear (FIT de otra carrera u hora mal
    /// convertida), no se guarda y `messages` dice por qué.
    pub track_saved: bool,
    pub offset_s: Option<f64>,
    pub confidence: Option<f64>,
    /// Avisos de la alineación o el error, en español.
    pub messages: Vec<String>,
}

/// Lee los ficheros, identifica al corredor y sugiere el formato, sin guardar nada.
pub fn preview(
    store: &Store,
    spl_path: &str,
    fit_path: Option<&str>,
    identity: &RunnerIdentity,
) -> Result<ImportPreview, ImportError> {
    let spl_bytes = read_file(spl_path)?;
    let event = spl::read(&spl_bytes)?;
    let fit_points = fit_path
        .map(|path| read_track(path).map(|track| track.points.len()))
        .transpose()?;

    let (matched, refs) = match identify_runner(&event, identity) {
        Identification::Unique {
            candidate,
            name_mismatch,
        } => {
            let matched = if name_mismatch {
                Match::UniqueNameMismatch
            } else {
                Match::Unique
            };
            (matched, vec![candidate.result])
        }
        Identification::Ambiguous { candidates } => (
            Match::Ambiguous,
            candidates.iter().map(|c| c.result).collect(),
        ),
        Identification::NotFound => (Match::NotFound, all_results(&event)),
    };

    Ok(ImportPreview {
        event_name: event.name.clone(),
        event_date: event.date,
        suggested_format: suggest_format(&event),
        median_winner_s: median_winner_time_s(&event),
        matched,
        candidates: refs.iter().filter_map(|r| choice(&event, *r)).collect(),
        already_imported: store
            .find_source_file(&spl_bytes)?
            .map(|source| store.event_by_source_file(source))
            .transpose()?
            .flatten()
            .is_some(),
        fit_points,
    })
}

/// Guarda la carrera, el resultado elegido y, con FIT, el track (ver el módulo).
///
/// Lee y valida los dos ficheros antes de guardar nada: si alguno no vale, no se guarda nada.
pub fn import(store: &mut Store, request: &ImportRequest) -> Result<ImportOutcome, ImportError> {
    let spl_bytes = read_file(&request.spl_path)?;
    let event = spl::read(&spl_bytes)?;
    let race_result = request
        .result
        .get(&event)
        .ok_or(ImportError::NoSuchResult {
            class_index: request.result.class_index,
            result_index: request.result.result_index,
        })?
        .clone();
    let fit = request
        .fit_path
        .as_deref()
        .map(|path| Ok::<_, ImportError>((path, read_file(path)?)))
        .transpose()?;
    let track = fit
        .as_ref()
        .map(|(_, bytes)| fit::read(bytes))
        .transpose()?;

    let spl_source = store.save_source_file(SourceFileKind::Spl, &request.spl_path, &spl_bytes)?;
    let (event_id, already_imported) = match store.event_by_source_file(spl_source)? {
        Some(id) => (id, true),
        None => (store.save_event(&event, Some(spl_source))?.id, false),
    };
    let result_id = result_id(store, event_id, request.result)?;
    if let Some(format) = request.format {
        store.set_event_format(event_id, Some(format))?;
    }

    let mut warnings = Vec::new();
    let person = self_person(store, &request.identity)?;
    match store.result_person(result_id)? {
        Some(current) if current != person => warnings.push(
            "Ese resultado ya estaba vinculado a otra persona: no se ha cambiado.".to_string(),
        ),
        Some(_) => {}
        None => store.link_result(result_id, person)?,
    }
    save_identity(store, &request.identity)?;

    let alignment = match (fit, track) {
        (Some((path, bytes)), Some(track)) => Some(
            match align(&track, &race_result, &AlignmentOptions::default()) {
                Ok(a) => {
                    let source = store.save_source_file(SourceFileKind::Fit, path, &bytes)?;
                    store.save_track(result_id, &track, Some(source))?;
                    AlignmentReport {
                        track_saved: true,
                        offset_s: Some(a.offset_s),
                        confidence: Some(a.confidence),
                        messages: a.warnings.into_iter().map(|w| w.message).collect(),
                    }
                }
                Err(err) => AlignmentReport {
                    track_saved: false,
                    offset_s: None,
                    confidence: None,
                    messages: vec![format!("No se ha guardado el FIT: {err}")],
                },
            },
        ),
        _ => None,
    };

    Ok(ImportOutcome {
        event_id: event_id.0,
        result_id: result_id.0,
        already_imported,
        alignment,
        warnings,
    })
}

/// Identidad guardada en los ajustes, para rellenar el formulario.
pub fn stored_identity(store: &Store) -> Result<RunnerIdentity, ImportError> {
    Ok(RunnerIdentity {
        si_card: store
            .setting(SELF_SI_CARD_KEY)?
            .and_then(|v| v.parse().ok()),
        full_name: store.setting(SELF_FULL_NAME_KEY)?,
    })
}

fn save_identity(store: &mut Store, identity: &RunnerIdentity) -> Result<(), ImportError> {
    if let Some(card) = identity.si_card {
        store.set_setting(SELF_SI_CARD_KEY, &card.to_string())?;
    }
    if let Some(name) = identity.full_name.as_deref().map(str::trim)
        && !name.is_empty()
    {
        store.set_setting(SELF_FULL_NAME_KEY, name)?;
    }
    Ok(())
}

/// Persona del usuario guardada en los ajustes, si existe todavía.
pub(crate) fn stored_self_person(store: &Store) -> Result<Option<PersonId>, StoreError> {
    let Some(id) = store
        .setting(SELF_PERSON_KEY)?
        .and_then(|v| v.parse::<i64>().ok())
    else {
        return Ok(None);
    };
    Ok(store
        .people()?
        .into_iter()
        .find(|p| p.id.0 == id)
        .map(|p| p.id))
}

/// Persona del usuario; la crea la primera vez con el nombre que ha escrito (o «Yo»), nunca con
/// datos del .spl (`docs/almacenamiento.md`, Personas).
fn self_person(store: &mut Store, identity: &RunnerIdentity) -> Result<PersonId, ImportError> {
    if let Some(person) = stored_self_person(store)? {
        return Ok(person);
    }
    let name = identity
        .full_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(DEFAULT_SELF_NAME);
    let person = store.create_person(name, None)?;
    store.set_setting(SELF_PERSON_KEY, &person.0.to_string())?;
    Ok(person)
}

fn result_id(store: &Store, event: EventId, r: ResultRef) -> Result<ResultId, ImportError> {
    store
        .result_ids(event)?
        .get(r.class_index)
        .and_then(|class| class.get(r.result_index))
        .copied()
        .ok_or(ImportError::NoSuchResult {
            class_index: r.class_index,
            result_index: r.result_index,
        })
}

fn all_results(event: &Event) -> Vec<ResultRef> {
    event
        .classes
        .iter()
        .enumerate()
        .flat_map(|(class_index, class)| {
            (0..class.results.len()).map(move |result_index| ResultRef {
                class_index,
                result_index,
            })
        })
        .collect()
}

fn choice(event: &Event, r: ResultRef) -> Option<RunnerChoice> {
    let class = event.classes.get(r.class_index)?;
    let result = class.results.get(r.result_index)?;
    Some(RunnerChoice {
        result: r,
        class_name: class.name.clone(),
        given_name: result.runner.given_name.clone(),
        family_name: result.runner.family_name.clone(),
        club: result.runner.club.clone(),
        si_card: result.runner.si_card,
        status: result.status,
        place: result.place,
    })
}

fn read_file(path: &str) -> Result<Vec<u8>, ImportError> {
    std::fs::read(Path::new(path)).map_err(|source| ImportError::Read {
        path: path.to_string(),
        source,
    })
}

fn read_track(path: &str) -> Result<Track, ImportError> {
    Ok(fit::read(&read_file(path)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::races::list_races;

    fn fixture(path: &str) -> String {
        format!("{}/../../fixtures/{path}", env!("CARGO_MANIFEST_DIR"))
    }

    fn spl() -> String {
        fixture("spl/baltanas-anon.spl")
    }

    fn fit() -> String {
        fixture("fit/baltanas-sintetico.fit")
    }

    /// El corredor del FIT sintético: tarjeta 143, con su seudónimo de nombre.
    fn identity() -> RunnerIdentity {
        RunnerIdentity {
            si_card: Some(143),
            full_name: Some("N143 Apellido143".into()),
        }
    }

    fn request(preview: &ImportPreview, fit_path: Option<String>) -> ImportRequest {
        ImportRequest {
            spl_path: spl(),
            fit_path,
            result: preview.candidates[0].result,
            format: preview.suggested_format,
            identity: identity(),
        }
    }

    #[test]
    fn preview_identifies_the_runner_and_suggests_sprint() {
        let store = Store::open_in_memory().unwrap();
        let p = preview(&store, &spl(), Some(&fit()), &identity()).unwrap();
        assert_eq!(p.event_date, NaiveDate::from_ymd_opt(2026, 10, 3).unwrap());
        assert_eq!(p.matched, Match::Unique);
        assert_eq!(p.candidates.len(), 1);
        let c = &p.candidates[0];
        assert_eq!(
            (c.class_name.as_str(), c.si_card, c.place),
            ("M-SEN", Some(143), Some(16))
        );
        assert_eq!(p.suggested_format, Some(RaceFormat::Sprint));
        assert!(!p.already_imported);
        assert_eq!(p.fit_points, Some(1661));
    }

    #[test]
    fn preview_without_identity_lists_every_result() {
        let store = Store::open_in_memory().unwrap();
        let p = preview(&store, &spl(), None, &RunnerIdentity::default()).unwrap();
        assert_eq!(p.matched, Match::NotFound);
        assert_eq!(p.candidates.len(), 275);
        assert_eq!(p.fit_points, None);
    }

    #[test]
    fn importing_the_fixtures_leaves_a_race_in_the_list() {
        let mut store = Store::open_in_memory().unwrap();
        assert!(list_races(&store).unwrap().is_empty());
        let p = preview(&store, &spl(), Some(&fit()), &identity()).unwrap();
        let outcome = import(&mut store, &request(&p, Some(fit()))).unwrap();
        assert!(!outcome.already_imported);
        assert!(outcome.warnings.is_empty());
        let alignment = outcome.alignment.unwrap();
        assert!(alignment.track_saved);
        assert!(alignment.offset_s.unwrap().abs() < 2.0);
        assert!(alignment.confidence.unwrap() > 0.8);

        let races = list_races(&store).unwrap();
        assert_eq!(races.len(), 1);
        let race = &races[0];
        assert_eq!(race.class_name, "M-SEN");
        assert_eq!(race.place, Some(16));
        assert_eq!(race.format, Some(RaceFormat::Sprint));
        assert!(race.has_track);
        assert_eq!(
            race.name.as_deref(),
            Some("Cto. SPRINT Liga Norte/Liga FOCYL Baltanas")
        );

        // La identidad queda guardada para la siguiente carrera, y la persona se llama como
        // escribió el usuario.
        assert_eq!(stored_identity(&store).unwrap(), identity());
        let people = store.people().unwrap();
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].display_name, "N143 Apellido143");
    }

    #[test]
    fn reimporting_the_same_race_does_not_duplicate_it() {
        let mut store = Store::open_in_memory().unwrap();
        let p = preview(&store, &spl(), None, &identity()).unwrap();
        let first = import(&mut store, &request(&p, None)).unwrap();

        let again = preview(&store, &spl(), Some(&fit()), &identity()).unwrap();
        assert!(again.already_imported);
        let second = import(&mut store, &request(&again, Some(fit()))).unwrap();
        assert!(second.already_imported);
        assert_eq!(
            (second.event_id, second.result_id),
            (first.event_id, first.result_id)
        );

        let races = list_races(&store).unwrap();
        assert_eq!(races.len(), 1);
        // Esta vez con FIT: el track se añade a la misma carrera.
        assert!(races[0].has_track);
        assert_eq!(store.people().unwrap().len(), 1);
    }

    #[test]
    fn a_fit_from_another_time_is_not_saved() {
        let mut store = Store::open_in_memory().unwrap();
        let p = preview(&store, &spl(), None, &identity()).unwrap();
        // Otro corredor de la misma carrera, que salió a otra hora: el FIT no es suyo.
        let other = all_results(&spl::read(&std::fs::read(spl()).unwrap()).unwrap())[0];
        let mut req = request(&p, Some(fit()));
        req.result = other;
        let outcome = import(&mut store, &req).unwrap();
        let alignment = outcome.alignment.unwrap();
        // Salió más de media hora antes que el corredor del FIT: el track no solapa su carrera.
        assert!(!alignment.track_saved);
        assert!(
            alignment.messages[0].starts_with("No se ha guardado el FIT"),
            "{:?}",
            alignment.messages
        );
        assert!(!list_races(&store).unwrap()[0].has_track);
        assert_eq!(list_races(&store).unwrap().len(), 1);
    }

    #[test]
    fn invalid_files_save_nothing() {
        let mut store = Store::open_in_memory().unwrap();
        let p = preview(&store, &spl(), None, &identity()).unwrap();
        // El .spl como FIT: no es un FIT válido.
        let err = import(&mut store, &request(&p, Some(spl()))).unwrap_err();
        assert!(matches!(err, ImportError::Fit(_)), "{err}");
        let missing = preview(&store, "/no/existe.spl", None, &identity()).unwrap_err();
        assert!(
            missing
                .to_string()
                .starts_with("no se pudo leer /no/existe.spl")
        );
        assert!(list_races(&store).unwrap().is_empty());
        assert!(store.people().unwrap().is_empty());
        assert_eq!(
            store
                .find_source_file(&std::fs::read(spl()).unwrap())
                .unwrap(),
            None
        );
    }

    #[test]
    fn a_result_outside_the_race_is_an_error() {
        let mut store = Store::open_in_memory().unwrap();
        let p = preview(&store, &spl(), None, &identity()).unwrap();
        let mut req = request(&p, None);
        req.result = ResultRef {
            class_index: 99,
            result_index: 0,
        };
        assert!(matches!(
            import(&mut store, &req),
            Err(ImportError::NoSuchResult {
                class_index: 99,
                ..
            })
        ));
    }
}
