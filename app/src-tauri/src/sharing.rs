//! Carpeta compartida (#36, `docs/paquete.md`, "Carpeta compartida"): la app del corredor
//! exporta sus carreras a una carpeta sincronizada (Drive, OneDrive, Dropbox…) y la de quien
//! entrena importa de ella los paquetes nuevos. La misma app puede hacer las dos cosas con la
//! misma carpeta (#119). No hay servidor: solo ficheros.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::Serialize;
use tramos_core::package::{PackageError, ShareChoice, ShareLevel, package_file_name, race_id};
use tramos_store::{ResultId, SaveOutcome, Store};

use crate::import::stored_self_person;
use crate::package::{PackageAppError, import_package, package_path, race_package, write_package};
use crate::settings;

/// Qué ha pasado al exportar una carrera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Export {
    /// No toca exportar: no comparte lo suyo, sin carpeta o el resultado no es del usuario.
    Skipped,
    /// No se comparte: no hay paquete en la carpeta (si lo había, se ha borrado).
    NotShared,
    /// Está en la carpeta con ese nivel; `written` = se ha escrito ahora (si no, ya estaba igual).
    Shared { level: ShareLevel, written: bool },
}

/// Exporta (o retira) el paquete de un resultado según lo que se comparte de él: lo elegido para
/// esa carrera o, si no, lo de por defecto. Pedir el track de una carrera sin track comparte
/// los tramos. Llamarla sin que haya cambiado nada no toca la carpeta.
pub fn export_result(store: &mut Store, result_id: i64) -> Result<Export, PackageAppError> {
    let sharing = settings::load(store)?.sharing;
    let (true, Some(folder)) = (sharing.share_own, sharing.folder) else {
        return Ok(Export::Skipped);
    };
    let result = ResultId(result_id);
    let me = stored_self_person(store)?;
    if me.is_none() || store.result_person(result)? != me {
        return Ok(Export::Skipped);
    }
    let choice = store
        .result_sharing(result)?
        .unwrap_or(sharing.default_choice);
    let Some(level) = choice.level() else {
        remove_package(store, result, &folder)?;
        return Ok(Export::NotShared);
    };
    let package = match race_package(store, result_id, level) {
        Err(PackageAppError::Package(PackageError::MissingTrack)) => {
            race_package(store, result_id, ShareLevel::Legs)?
        }
        other => other?,
    };
    let written = write_package(&folder, &package)?;
    Ok(Export::Shared {
        level: package.level,
        written,
    })
}

/// Borra de la carpeta el paquete del resultado, si está.
fn remove_package(
    store: &mut Store,
    result: ResultId,
    folder: &str,
) -> Result<(), PackageAppError> {
    let (event_id, _) = store.result_ref(result)?;
    let race = race_id(&store.load_event(event_id)?);
    let path = package_path(
        folder,
        &package_file_name(&race, &store.package_runner_id()?),
    );
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(PackageAppError::Remove { path, source: e })
        }
        _ => Ok(()),
    }
}

/// Cómo queda una carrera del usuario en la carpeta compartida, para la vista de carrera.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RaceSharing {
    /// Se puede compartir: comparte lo suyo, con carpeta, y el resultado es del usuario.
    pub available: bool,
    /// Lo elegido para esta carrera; `None` = lo de por defecto.
    pub choice: Option<ShareChoice>,
    pub default_choice: ShareChoice,
    /// Con qué nivel está en la carpeta; `None` = no está.
    pub shared: Option<ShareLevel>,
    /// Por qué no se ha podido exportar, en español.
    pub problem: Option<String>,
}

/// Exporta la carrera si hace falta y dice cómo queda.
pub fn race_sharing(store: &mut Store, result_id: i64) -> Result<RaceSharing, PackageAppError> {
    let default_choice = settings::load(store)?.sharing.default_choice;
    let choice = store.result_sharing(ResultId(result_id))?;
    let mut view = RaceSharing {
        available: true,
        choice,
        default_choice,
        shared: None,
        problem: None,
    };
    match export_result(store, result_id) {
        Ok(Export::Skipped) => view.available = false,
        Ok(Export::NotShared) => {}
        Ok(Export::Shared { level, .. }) => view.shared = Some(level),
        Err(e) => view.problem = Some(e.to_string()),
    }
    Ok(view)
}

/// Cambia lo que se comparte de una carrera (`None` = lo de por defecto), la exporta o la
/// retira y dice cómo queda.
pub fn set_race_sharing(
    store: &mut Store,
    result_id: i64,
    choice: Option<ShareChoice>,
) -> Result<RaceSharing, PackageAppError> {
    let result = ResultId(result_id);
    let me = stored_self_person(store)?.ok_or(PackageAppError::NotYours)?;
    if store.result_person(result)? != Some(me) {
        return Err(PackageAppError::NotYours);
    }
    store.set_result_sharing(result, choice)?;
    race_sharing(store, result_id)
}

/// Resultado de exportar todas las carreras del usuario.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ShareReport {
    /// Paquetes escritos ahora.
    pub written: usize,
    /// Paquetes que ya estaban igual.
    pub unchanged: usize,
    /// Carreras que no se comparten (o que no se pueden analizar).
    pub not_shared: usize,
    /// Las que no se han podido exportar, con el motivo, en español.
    pub problems: Vec<String>,
}

/// Exporta todas las carreras del usuario (al elegir la carpeta, cambiar los umbrales o
/// importar varias). Si no comparte lo suyo o no hay carpeta, no hace nada.
pub fn share_all(store: &mut Store) -> Result<ShareReport, PackageAppError> {
    let mut report = ShareReport::default();
    let sharing = settings::load(store)?.sharing;
    if !sharing.share_own || sharing.folder.is_none() {
        return Ok(report);
    }
    let Some(me) = stored_self_person(store)? else {
        return Ok(report);
    };
    for r in store.person_results(me)? {
        match export_result(store, r.result.0) {
            Ok(Export::Shared { written: true, .. }) => report.written += 1,
            Ok(Export::Shared { written: false, .. }) => report.unchanged += 1,
            Ok(Export::NotShared | Export::Skipped)
            | Err(PackageAppError::Package(PackageError::NotAnalyzed)) => report.not_shared += 1,
            Err(e) => report.problems.push(format!(
                "{} {}: {e}",
                r.event_date,
                r.event_name.as_deref().unwrap_or("carrera sin nombre")
            )),
        }
    }
    Ok(report)
}

/// Ficheros de la carpeta ya importados, con su fecha de modificación y tamaño: si no cambian,
/// no se vuelven a leer (un paquete con track puede pesar varios MB).
pub type SeenFiles = HashMap<PathBuf, (SystemTime, u64)>;

/// Resultado de buscar paquetes nuevos en la carpeta.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ReceiveReport {
    /// Paquetes nuevos.
    pub created: usize,
    /// Paquetes que sustituyen a otro del mismo corredor y carrera.
    pub replaced: usize,
    /// Ficheros leídos que no cambian nada (iguales o más antiguos que los guardados).
    pub unchanged: usize,
    /// Ficheros que no se han podido importar, con el motivo, en español.
    pub problems: Vec<String>,
    /// Paquetes guardados en total y de cuántos corredores.
    pub packages: usize,
    pub runners: usize,
}

/// Si entrena a otros, importa los paquetes nuevos o cambiados de la carpeta (`tramos-*.json`;
/// los demás ficheros se ignoran). Los suyos (los que exporta esta misma app, con su
/// identificador de corredor en el nombre) no se importan. Uno que no se puede leer no impide
/// importar los demás y se vuelve a intentar la próxima vez. Si no entrena o no hay carpeta, no
/// importa nada.
pub fn receive(store: &mut Store, seen: &mut SeenFiles) -> Result<ReceiveReport, PackageAppError> {
    let mut report = ReceiveReport::default();
    let sharing = settings::load(store)?.sharing;
    if let (true, Some(folder)) = (sharing.coach, sharing.folder) {
        let own = format!("-{}.json", store.package_runner_id()?);
        let folder_error = |source| PackageAppError::Folder {
            path: folder.clone(),
            source,
        };
        let mut files: Vec<(PathBuf, (SystemTime, u64))> = Vec::new();
        for entry in std::fs::read_dir(&folder).map_err(folder_error)? {
            let entry = entry.map_err(folder_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !(name.starts_with("tramos-") && name.ends_with(".json")) || name.ends_with(&own) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let stamp = (
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                meta.len(),
            );
            if meta.is_file() && seen.get(&entry.path()) != Some(&stamp) {
                files.push((entry.path(), stamp));
            }
        }
        // En orden de nombre, para que el resultado no dependa del sistema de ficheros.
        files.sort();
        for (path, stamp) in files {
            let text = path.to_string_lossy().into_owned();
            match import_package(store, &text) {
                Ok(outcome) => {
                    match outcome {
                        SaveOutcome::Created => report.created += 1,
                        SaveOutcome::Replaced => report.replaced += 1,
                        SaveOutcome::Unchanged | SaveOutcome::IgnoredOlder => report.unchanged += 1,
                    }
                    seen.insert(path, stamp);
                }
                Err(e) => report.problems.push(e.to_string()),
            }
        }
    }
    (report.packages, report.runners) = store.received_package_counts()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race_map::tests::imported;
    use crate::settings::{Role, SharingSettings};
    use tramos_core::package::RacePackage;
    use tramos_core::taxonomy::{Confirmation, LegTag};

    fn configure(store: &mut Store, role: Role, folder: &std::path::Path) {
        let mut s = settings::load(store).unwrap();
        s.sharing = SharingSettings {
            share_own: role != Role::Coach,
            coach: role != Role::Runner,
            folder: Some(folder.to_string_lossy().into_owned()),
            default_choice: ShareChoice::Legs,
        };
        settings::save(store, &s).unwrap();
    }

    fn packages_in(folder: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn what_one_app_exports_appears_in_the_other() {
        let dir = tempfile::tempdir().unwrap();
        let (mut runner, result_id) = imported(true);
        configure(&mut runner, Role::Runner, dir.path());
        let mut coach = Store::open_in_memory().unwrap();
        configure(&mut coach, Role::Coach, dir.path());
        let mut seen = SeenFiles::new();

        // Por defecto se comparten los tramos.
        assert_eq!(
            export_result(&mut runner, result_id).unwrap(),
            Export::Shared {
                level: ShareLevel::Legs,
                written: true
            }
        );
        // Sin cambios, no se vuelve a escribir.
        assert_eq!(
            export_result(&mut runner, result_id).unwrap(),
            Export::Shared {
                level: ShareLevel::Legs,
                written: false
            }
        );
        let report = receive(&mut coach, &mut seen).unwrap();
        assert_eq!((report.created, report.packages, report.runners), (1, 1, 1));
        // Lo ya leído no se vuelve a leer.
        let again = receive(&mut coach, &mut seen).unwrap();
        assert_eq!((again.created, again.replaced, again.unchanged), (0, 0, 0));

        // Una etiqueta nueva cambia el paquete: la entrenadora recibe el nuevo.
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
        export_result(&mut runner, result_id).unwrap();
        let report = receive(&mut coach, &mut seen).unwrap();
        assert_eq!((report.created, report.replaced), (0, 1));
        let received = coach.received_packages().unwrap();
        let package = RacePackage::parse(&received[0].content).unwrap();
        assert_eq!(package.tags.len(), 1);

        // Compartir el track para esta carrera.
        let view = set_race_sharing(&mut runner, result_id, Some(ShareChoice::Track)).unwrap();
        assert_eq!(
            (view.available, view.shared, view.problem),
            (true, Some(ShareLevel::Track), None)
        );
        receive(&mut coach, &mut seen).unwrap();
        assert_eq!(
            coach.received_packages().unwrap()[0].level,
            ShareLevel::Track
        );

        // Dejar de compartirla la quita de la carpeta (la entrenadora conserva lo recibido).
        let view = set_race_sharing(&mut runner, result_id, Some(ShareChoice::Nothing)).unwrap();
        assert_eq!((view.available, view.shared), (true, None));
        assert!(packages_in(dir.path()).is_empty());
        assert_eq!(receive(&mut coach, &mut seen).unwrap().packages, 1);
    }

    #[test]
    fn the_track_level_without_track_shares_the_legs() {
        let dir = tempfile::tempdir().unwrap();
        let (mut runner, result_id) = imported(false);
        configure(&mut runner, Role::Runner, dir.path());
        let view = set_race_sharing(&mut runner, result_id, Some(ShareChoice::Track)).unwrap();
        assert_eq!(view.shared, Some(ShareLevel::Legs));
        // Volver a lo de por defecto.
        let view = set_race_sharing(&mut runner, result_id, None).unwrap();
        assert_eq!(
            (view.choice, view.default_choice, view.shared),
            (None, ShareChoice::Legs, Some(ShareLevel::Legs))
        );
    }

    #[test]
    fn nothing_is_exported_without_folder_in_coach_mode_or_for_others() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, result_id) = imported(false);
        // Sin carpeta.
        assert_eq!(
            export_result(&mut store, result_id).unwrap(),
            Export::Skipped
        );
        assert!(!race_sharing(&mut store, result_id).unwrap().available);
        assert_eq!(share_all(&mut store).unwrap(), ShareReport::default());
        // Sin compartir lo suyo.
        configure(&mut store, Role::Coach, dir.path());
        assert_eq!(
            export_result(&mut store, result_id).unwrap(),
            Export::Skipped
        );
        // El resultado de otro corredor.
        configure(&mut store, Role::Runner, dir.path());
        let (event_id, _) = store.result_ref(ResultId(result_id)).unwrap();
        let other = store.result_ids(event_id).unwrap()[0][0];
        assert_eq!(export_result(&mut store, other.0).unwrap(), Export::Skipped);
        assert!(matches!(
            set_race_sharing(&mut store, other.0, Some(ShareChoice::Track)),
            Err(PackageAppError::NotYours)
        ));
        assert!(packages_in(dir.path()).is_empty());
    }

    #[test]
    fn share_all_exports_every_race_of_the_runner() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _) = imported(false);
        configure(&mut store, Role::Runner, dir.path());
        let report = share_all(&mut store).unwrap();
        assert_eq!((report.written, report.unchanged), (1, 0));
        assert!(report.problems.is_empty());
        assert_eq!(share_all(&mut store).unwrap().unchanged, 1);
        let names = packages_in(dir.path());
        assert_eq!(names.len(), 1);
        assert!(names[0].starts_with("tramos-") && names[0].ends_with(".json"));
    }

    #[test]
    fn a_broken_file_does_not_stop_the_others() {
        let dir = tempfile::tempdir().unwrap();
        let (mut runner, _) = imported(false);
        configure(&mut runner, Role::Runner, dir.path());
        share_all(&mut runner).unwrap();
        std::fs::write(dir.path().join("tramos-roto.json"), "{").unwrap();
        std::fs::write(dir.path().join("notas.txt"), "otra cosa").unwrap();

        let mut coach = Store::open_in_memory().unwrap();
        configure(&mut coach, Role::Coach, dir.path());
        let mut seen = SeenFiles::new();
        let report = receive(&mut coach, &mut seen).unwrap();
        assert_eq!((report.created, report.problems.len()), (1, 1));
        // El roto se vuelve a intentar; el bueno no.
        let again = receive(&mut coach, &mut seen).unwrap();
        assert_eq!(
            (again.created, again.unchanged, again.problems.len()),
            (0, 0, 1)
        );

        // Si no entrena, no se importa nada.
        let mut other = Store::open_in_memory().unwrap();
        configure(&mut other, Role::Runner, dir.path());
        let report = receive(&mut other, &mut SeenFiles::new()).unwrap();
        assert_eq!((report.created, report.packages), (0, 0));
    }

    #[test]
    fn whoever_runs_and_coaches_does_not_receive_their_own_races() {
        let dir = tempfile::tempdir().unwrap();
        // Una atleta comparte su carrera.
        let (mut athlete, _) = imported(false);
        configure(&mut athlete, Role::Runner, dir.path());
        share_all(&mut athlete).unwrap();
        // Quien entrena también corre, comparte la suya en la misma carpeta y recibe.
        let (mut both, result_id) = imported(false);
        configure(&mut both, Role::Both, dir.path());
        assert!(matches!(
            export_result(&mut both, result_id).unwrap(),
            Export::Shared { .. }
        ));
        assert_eq!(packages_in(dir.path()).len(), 2);

        let report = receive(&mut both, &mut SeenFiles::new()).unwrap();
        assert_eq!((report.created, report.packages, report.runners), (1, 1, 1));
        let received = both.received_packages().unwrap();
        assert_ne!(received[0].runner_id, both.package_runner_id().unwrap());
        // Sus carreras siguen siendo editables y suyas.
        assert!(race_sharing(&mut both, result_id).unwrap().available);
    }
}
