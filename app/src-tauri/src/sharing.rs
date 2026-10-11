//! Carpeta compartida (#36, `docs/paquete.md`, "Carpeta compartida"): la app del corredor
//! exporta sus carreras a una carpeta sincronizada (Drive, OneDrive, Dropbox…) y la de quien
//! entrena importa de ella los paquetes nuevos. La misma app puede hacer las dos cosas con la
//! misma carpeta (#119). No hay servidor: solo ficheros.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;
use tramos_core::package::{PackageError, ShareChoice, ShareLevel, package_file_name, race_id};
use tramos_store::{ResultId, SaveOutcome, Store};

use crate::import::stored_self_person;
use crate::package::{PackageAppError, import_package, package_path, race_package, write_package};
use crate::settings::{self, SharingSettings};

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
    let (true, Some(folder)) = (sharing.share_own, sharing.export_dir()) else {
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
        remove_package(store, result, folder)?;
        return Ok(Export::NotShared);
    };
    let package = match race_package(store, result_id, level) {
        Err(PackageAppError::Package(PackageError::MissingTrack)) => {
            race_package(store, result_id, ShareLevel::Legs)?
        }
        other => other?,
    };
    let written = write_package(folder, &package)?;
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
    if !sharing.share_own || sharing.export_dir().is_none() {
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

/// Resultado de mover los paquetes propios a otra carpeta.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MoveReport {
    /// Paquetes movidos a la carpeta nueva.
    pub moved: usize,
    /// Los que ya estaban en la carpeta nueva: no se duplican, se quita el de la vieja.
    pub already_there: usize,
    /// Los que no se han podido mover, con el motivo, en español.
    pub problems: Vec<String>,
}

/// Cuando quien entrena y comparte lo suyo pasa a tener subcarpeta propia (o la cambia), mueve a
/// ella los paquetes propios que había en la carpeta de exportación anterior (#167): así no quedan
/// duplicados ni los atletas con copias viejas. Solo mira los ficheros de esa carpeta, no sus
/// subcarpetas. Si no hay cambio de carpeta, o el usuario no entrena y comparte con subcarpeta
/// propia, no hace nada. Se llama justo después de guardar los ajustes.
pub fn move_own_packages(
    store: &mut Store,
    old: &SharingSettings,
    new: &SharingSettings,
) -> Result<MoveReport, PackageAppError> {
    let mut report = MoveReport::default();
    let uses_own_subfolder = new.share_own && new.coach && new.export_folder.is_some();
    let (true, Some(from), Some(to)) = (uses_own_subfolder, old.export_dir(), new.export_dir())
    else {
        return Ok(report);
    };
    let (from, to) = (Path::new(from), Path::new(to));
    if same_folder(from, to) {
        return Ok(report);
    }
    let own = format!("-{}.json", store.package_runner_id()?);
    let entries = match std::fs::read_dir(from) {
        Ok(entries) => entries,
        // La carpeta de antes ya no está: no hay nada que mover.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(report),
        Err(source) => {
            return Err(PackageAppError::Folder {
                path: from.to_string_lossy().into_owned(),
                source,
            });
        }
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("tramos-") && name.ends_with(&own))
        .collect();
    names.sort();
    for name in names {
        let (source, target) = (from.join(&name), to.join(&name));
        if target.exists() {
            // Ya hay uno igual en la carpeta nueva: se queda ese (la siguiente exportación lo
            // pone al día si hace falta) y se quita el viejo.
            match std::fs::remove_file(&source) {
                Ok(()) => report.already_there += 1,
                Err(e) => report.problems.push(format!("{}: {e}", source.display())),
            }
        } else {
            match move_file(&source, &target) {
                Ok(()) => report.moved += 1,
                Err(e) => report.problems.push(format!("{}: {e}", source.display())),
            }
        }
    }
    Ok(report)
}

fn same_folder(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Renombra; si las carpetas están en sistemas de ficheros distintos, copia y borra el original.
fn move_file(source: &Path, target: &Path) -> std::io::Result<()> {
    if std::fs::rename(source, target).is_ok() {
        return Ok(());
    }
    std::fs::copy(source, target)?;
    std::fs::remove_file(source)
}

/// Nombre de carpeta válido en cualquier sistema de ficheros a partir del nombre del usuario:
/// sin separadores ni caracteres reservados, sin espacios repetidos ni puntos al principio o al
/// final. Sin nombre, «Mis carreras».
pub fn folder_name(full_name: &str) -> String {
    const RESERVED: &str = "/\\:*?\"<>|";
    let cleaned: String = full_name
        .chars()
        .map(|c| {
            if c.is_control() || RESERVED.contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let joined = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let name: String = joined
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .chars()
        .take(80)
        .collect();
    let name = name.trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    if name.is_empty() {
        "Mis carreras".to_string()
    } else {
        name.to_string()
    }
}

/// La subcarpeta propia que se propone dentro de la carpeta madre: `<madre>/<nombre>`.
pub fn propose_own_folder(mother: &str, full_name: &str) -> String {
    Path::new(mother.trim())
        .join(folder_name(full_name))
        .to_string_lossy()
        .into_owned()
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

/// Cuántos niveles de subcarpetas se miran por debajo de la carpeta elegida: carpeta madre ->
/// una por atleta -> (por ejemplo) una por temporada. Más hondo no se lee.
pub const MAX_FOLDER_DEPTH: usize = 3;

type Stamp = (SystemTime, u64);

/// Los paquetes de `folder` y de sus subcarpetas (hasta `MAX_FOLDER_DEPTH` niveles), nuevos o
/// cambiados respecto a `seen`, en orden de ruta. Se salta los ficheros que no son `tramos-*.json`,
/// los que terminan en `own`, las carpetas ocultas y todo enlace simbólico (a fichero o a
/// carpeta). Una subcarpeta que no se puede leer se anota en `problems` y no impide leer el resto.
fn find_packages(
    folder: &Path,
    own: &str,
    seen: &SeenFiles,
    problems: &mut Vec<String>,
) -> std::io::Result<Vec<(PathBuf, Stamp)>> {
    fn walk(
        dir: &Path,
        depth: usize,
        own: &str,
        seen: &SeenFiles,
        files: &mut Vec<(PathBuf, Stamp)>,
        problems: &mut Vec<String>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            // `file_type` no sigue los enlaces: uno a fichero o a carpeta no es ni lo uno ni lo otro.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if depth < MAX_FOLDER_DEPTH
                    && !name.starts_with('.')
                    && let Err(source) = walk(&entry.path(), depth + 1, own, seen, files, problems)
                {
                    let path = entry.path().to_string_lossy().into_owned();
                    problems.push(PackageAppError::Folder { path, source }.to_string());
                }
                continue;
            }
            let is_package =
                kind.is_file() && name.starts_with("tramos-") && name.ends_with(".json");
            if !is_package || name.ends_with(own) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let stamp = (
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                meta.len(),
            );
            if seen.get(&entry.path()) != Some(&stamp) {
                files.push((entry.path(), stamp));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(folder, 0, own, seen, &mut files, problems)?;
    // En orden de ruta, para que el resultado no dependa del sistema de ficheros.
    files.sort();
    Ok(files)
}

/// Si entrena a otros, importa los paquetes nuevos o cambiados de la carpeta y de sus subcarpetas
/// (`tramos-*.json`, hasta `MAX_FOLDER_DEPTH` niveles; los demás ficheros se ignoran, igual que
/// los enlaces simbólicos y las carpetas ocultas). Así sirve una carpeta madre con una subcarpeta
/// por atleta. Los suyos (los que exporta esta misma app, con su identificador de corredor en el
/// nombre) no se importan, estén donde estén. Si el mismo paquete (mismo corredor y carrera) está
/// en varios sitios, se queda el exportado más recientemente, sea cual sea su carpeta; los demás
/// cuentan como «sin cambios». Uno que no se puede leer no impide importar los demás y se vuelve
/// a intentar la próxima vez. Si no entrena o no hay carpeta, no importa nada.
pub fn receive(store: &mut Store, seen: &mut SeenFiles) -> Result<ReceiveReport, PackageAppError> {
    let mut report = ReceiveReport::default();
    let sharing = settings::load(store)?.sharing;
    if let (true, Some(folder)) = (sharing.coach, sharing.folder) {
        let own = format!("-{}.json", store.package_runner_id()?);
        let files = find_packages(Path::new(&folder), &own, seen, &mut report.problems).map_err(
            |source| PackageAppError::Folder {
                path: folder.clone(),
                source,
            },
        )?;
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
            export_folder: None,
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

    /// Un atleta nuevo (otra base, otro identificador) que exporta su carrera a `folder`.
    fn athlete_exports(folder: &Path) -> Store {
        let (mut athlete, _) = imported(false);
        configure(&mut athlete, Role::Runner, folder);
        assert_eq!(share_all(&mut athlete).unwrap().written, 1);
        athlete
    }

    fn coach_of(folder: &Path) -> Store {
        let mut coach = Store::open_in_memory().unwrap();
        configure(&mut coach, Role::Coach, folder);
        coach
    }

    #[test]
    fn the_coach_reads_one_subfolder_per_athlete() {
        let mother = tempfile::tempdir().unwrap();
        for name in ["ana", "beto"] {
            let sub = mother.path().join(name);
            std::fs::create_dir(&sub).unwrap();
            athlete_exports(&sub);
        }
        let mut coach = coach_of(mother.path());
        let mut seen = SeenFiles::new();
        let report = receive(&mut coach, &mut seen).unwrap();
        assert_eq!((report.created, report.packages, report.runners), (2, 2, 2));
        assert!(report.problems.is_empty());
        // Lo ya leído, en cualquier subcarpeta, no se vuelve a leer.
        let again = receive(&mut coach, &mut seen).unwrap();
        assert_eq!((again.created, again.replaced, again.unchanged), (0, 0, 0));
    }

    #[test]
    fn subfolders_are_read_down_to_the_depth_limit_and_no_further() {
        let mother = tempfile::tempdir().unwrap();
        let mut deepest = mother.path().to_path_buf();
        for level in 1..=MAX_FOLDER_DEPTH {
            deepest = deepest.join(format!("n{level}"));
            std::fs::create_dir(&deepest).unwrap();
        }
        athlete_exports(&deepest);
        let too_deep = deepest.join("mas-hondo");
        std::fs::create_dir(&too_deep).unwrap();
        athlete_exports(&too_deep);
        let report = receive(&mut coach_of(mother.path()), &mut SeenFiles::new()).unwrap();
        assert_eq!((report.created, report.runners), (1, 1));
    }

    #[test]
    fn hidden_folders_and_other_files_are_skipped() {
        let mother = tempfile::tempdir().unwrap();
        let hidden = mother.path().join(".oculta");
        std::fs::create_dir(&hidden).unwrap();
        athlete_exports(&hidden);
        let sub = mother.path().join("ana");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("notas.json"), "{}").unwrap();
        std::fs::write(sub.join("tramos-no-es-un-paquete.txt"), "x").unwrap();
        let report = receive(&mut coach_of(mother.path()), &mut SeenFiles::new()).unwrap();
        assert_eq!((report.created, report.problems.len()), (0, 0));
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_not_followed() {
        let outside = tempfile::tempdir().unwrap();
        athlete_exports(outside.path());
        let package = std::fs::read_dir(outside.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let mother = tempfile::tempdir().unwrap();
        // Un enlace a una carpeta con paquetes y otro a un paquete suelto.
        std::os::unix::fs::symlink(outside.path(), mother.path().join("enlace-carpeta")).unwrap();
        std::os::unix::fs::symlink(
            &package,
            mother
                .path()
                .join(package.file_name().unwrap().to_str().unwrap()),
        )
        .unwrap();
        let report = receive(&mut coach_of(mother.path()), &mut SeenFiles::new()).unwrap();
        assert_eq!((report.created, report.problems.len()), (0, 0));
    }

    #[test]
    fn own_packages_are_skipped_in_subfolders_too() {
        let mother = tempfile::tempdir().unwrap();
        let sub = mother.path().join("ana");
        std::fs::create_dir(&sub).unwrap();
        athlete_exports(&sub);
        // Quien entrena y corre exporta lo suyo a su subcarpeta de la carpeta madre.
        let (mut both, result_id) = imported(false);
        configure(&mut both, Role::Both, mother.path());
        let mine = mother.path().join("entrenador");
        let mut settings = settings::load(&both).unwrap();
        settings.sharing.export_folder = Some(mine.to_string_lossy().into_owned());
        settings::save(&mut both, &settings).unwrap();
        export_result(&mut both, result_id).unwrap();

        let report = receive(&mut both, &mut SeenFiles::new()).unwrap();
        assert_eq!((report.created, report.packages, report.runners), (1, 1, 1));
    }

    fn set_sharing(
        store: &mut Store,
        change: impl FnOnce(&mut SharingSettings),
    ) -> SharingSettings {
        let before = settings::load(store).unwrap().sharing;
        let mut s = settings::load(store).unwrap();
        change(&mut s.sharing);
        settings::save(store, &s).unwrap();
        before
    }

    fn path_text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn with_a_mother_folder_and_own_subfolder_it_exports_in_the_subfolder_and_reads_the_rest() {
        let mother = tempfile::tempdir().unwrap();
        let ana = mother.path().join("ana");
        std::fs::create_dir(&ana).unwrap();
        athlete_exports(&ana);
        let (mut both, result_id) = imported(false);
        configure(&mut both, Role::Both, mother.path());
        let mine = mother.path().join("entrenador");
        set_sharing(&mut both, |s| s.export_folder = Some(path_text(&mine)));
        // La subcarpeta la crea el guardado de los ajustes.
        assert!(mine.is_dir());

        assert!(matches!(
            export_result(&mut both, result_id).unwrap(),
            Export::Shared { written: true, .. }
        ));
        assert_eq!(packages_in(&mine).len(), 1);
        // En la carpeta madre solo está la subcarpeta de cada uno.
        assert_eq!(packages_in(mother.path()), ["ana", "entrenador"]);
        assert_eq!(share_all(&mut both).unwrap().unchanged, 1);

        let mut seen = SeenFiles::new();
        let report = receive(&mut both, &mut seen).unwrap();
        assert_eq!((report.created, report.packages, report.runners), (1, 1, 1));
        let again = receive(&mut both, &mut seen).unwrap();
        assert_eq!((again.created, again.replaced, again.unchanged), (0, 0, 0));
        // Dejar de compartir la carrera borra el fichero de la subcarpeta.
        set_race_sharing(&mut both, result_id, Some(ShareChoice::Nothing)).unwrap();
        assert!(packages_in(&mine).is_empty());
    }

    #[test]
    fn own_packages_move_from_the_mother_folder_to_the_new_subfolder() {
        let mother = tempfile::tempdir().unwrap();
        // Una atleta deja su paquete en la raíz de la carpeta madre; no es de quien entrena.
        athlete_exports(mother.path());
        let (mut both, _) = imported(false);
        configure(&mut both, Role::Both, mother.path());
        assert_eq!(share_all(&mut both).unwrap().written, 1);
        assert_eq!(packages_in(mother.path()).len(), 2);
        let own = format!("-{}.json", both.package_runner_id().unwrap());

        let mine = mother.path().join("entrenador");
        let before = set_sharing(&mut both, |s| s.export_folder = Some(path_text(&mine)));
        let after = settings::load(&both).unwrap().sharing;
        let report = move_own_packages(&mut both, &before, &after).unwrap();
        assert_eq!(
            (report.moved, report.already_there, report.problems.len()),
            (1, 0, 0)
        );
        let in_mine = packages_in(&mine);
        assert!(in_mine.len() == 1 && in_mine[0].ends_with(&own));
        // El de la atleta sigue donde estaba y el propio ya no.
        let in_mother = packages_in(mother.path());
        assert_eq!(in_mother.len(), 2);
        assert!(in_mother.iter().filter(|n| n.ends_with(&own)).count() == 0);
        // Exportar de nuevo no escribe nada: ya está igual.
        let report = share_all(&mut both).unwrap();
        assert_eq!((report.written, report.unchanged), (0, 1));
        // Y al leer solo entra lo de la atleta.
        let received = receive(&mut both, &mut SeenFiles::new()).unwrap();
        assert_eq!((received.created, received.runners), (1, 1));
    }

    #[test]
    fn moving_does_not_duplicate_a_package_already_in_the_destination() {
        let mother = tempfile::tempdir().unwrap();
        let (mut both, _) = imported(false);
        configure(&mut both, Role::Both, mother.path());
        share_all(&mut both).unwrap();
        let mine = mother.path().join("entrenador");
        let before = set_sharing(&mut both, |s| s.export_folder = Some(path_text(&mine)));
        // El destino ya tiene el suyo (de una vez anterior).
        assert_eq!(share_all(&mut both).unwrap().written, 1);
        assert_eq!(packages_in(mother.path()).len(), 2);

        let after = settings::load(&both).unwrap().sharing;
        let report = move_own_packages(&mut both, &before, &after).unwrap();
        assert_eq!((report.moved, report.already_there), (0, 1));
        assert_eq!(packages_in(mother.path()), ["entrenador"]);
        assert_eq!(packages_in(&mine).len(), 1);
    }

    #[test]
    fn nothing_moves_without_an_own_subfolder_or_a_change() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        // Quien solo corre y cambia de carpeta: no se mueve nada.
        let (mut runner, _) = imported(false);
        configure(&mut runner, Role::Runner, first.path());
        share_all(&mut runner).unwrap();
        let before = set_sharing(&mut runner, |s| s.folder = Some(path_text(second.path())));
        let after = settings::load(&runner).unwrap().sharing;
        assert_eq!(
            move_own_packages(&mut runner, &before, &after).unwrap(),
            MoveReport::default()
        );
        assert_eq!(packages_in(first.path()).len(), 1);
        assert!(packages_in(second.path()).is_empty());

        // Con subcarpeta propia, volver a guardar la misma tampoco mueve nada.
        let (mut both, _) = imported(false);
        configure(&mut both, Role::Both, first.path());
        let mine = first.path().join("yo");
        set_sharing(&mut both, |s| s.export_folder = Some(path_text(&mine)));
        let same = settings::load(&both).unwrap().sharing;
        assert_eq!(
            move_own_packages(&mut both, &same, &same).unwrap(),
            MoveReport::default()
        );
    }

    #[test]
    fn the_own_subfolder_is_only_kept_while_coaching() {
        let mother = tempfile::tempdir().unwrap();
        let (mut both, _) = imported(false);
        configure(&mut both, Role::Both, mother.path());
        let mine = mother.path().join("yo");
        set_sharing(&mut both, |s| s.export_folder = Some(path_text(&mine)));
        assert_eq!(
            settings::load(&both).unwrap().sharing.export_dir(),
            Some(path_text(&mine).as_str())
        );
        set_sharing(&mut both, |s| s.coach = false);
        let sharing = settings::load(&both).unwrap().sharing;
        assert_eq!(sharing.export_folder, None);
        assert_eq!(
            sharing.export_dir(),
            Some(path_text(mother.path()).as_str())
        );
    }

    #[test]
    fn the_proposed_folder_is_the_name_inside_the_mother_folder() {
        assert_eq!(
            folder_name("Ana  García/López: «Test»?"),
            "Ana García López «Test»"
        );
        assert_eq!(folder_name("  ..Ana. "), "Ana");
        assert_eq!(folder_name(""), "Mis carreras");
        assert_eq!(folder_name("///"), "Mis carreras");
        let proposed = propose_own_folder("/datos/madre", "N1 Apellido1");
        assert_eq!(Path::new(&proposed), Path::new("/datos/madre/N1 Apellido1"));
    }

    #[test]
    fn the_same_package_in_two_folders_keeps_the_most_recent_in_any_order() {
        // Dos copias del mismo paquete (mismo corredor y carrera): una vieja y una nueva.
        // Se prueba con la vieja antes y después de la nueva en el orden de rutas.
        for (old_folder, new_folder) in [("a-vieja", "z-nueva"), ("z-vieja", "a-nueva")] {
            let mother = tempfile::tempdir().unwrap();
            let old = mother.path().join(old_folder);
            let new = mother.path().join(new_folder);
            std::fs::create_dir(&old).unwrap();
            std::fs::create_dir(&new).unwrap();
            let mut athlete = athlete_exports(&old);
            // `exported_at` va en segundos: hay que dejar pasar uno entero.
            std::thread::sleep(std::time::Duration::from_millis(1100));
            // El atleta cambia lo que comparte y lo exporta a otra carpeta: más reciente.
            let mut s = settings::load(&athlete).unwrap();
            s.sharing.default_choice = ShareChoice::Aggregates;
            s.sharing.folder = Some(new.to_string_lossy().into_owned());
            settings::save(&mut athlete, &s).unwrap();
            assert_eq!(share_all(&mut athlete).unwrap().written, 1);

            let mut coach = coach_of(mother.path());
            let report = receive(&mut coach, &mut SeenFiles::new()).unwrap();
            assert_eq!((report.packages, report.runners), (1, 1), "{old_folder}");
            assert_eq!(report.created + report.replaced + report.unchanged, 2);
            assert_eq!(
                coach.received_packages().unwrap()[0].level,
                ShareLevel::Aggregates,
                "{old_folder}"
            );
        }
    }

    #[test]
    fn an_unreadable_root_folder_is_an_error() {
        let mother = tempfile::tempdir().unwrap();
        let missing = mother.path().join("no-existe");
        std::fs::create_dir(&missing).unwrap();
        let mut coach = coach_of(&missing);
        std::fs::remove_dir(&missing).unwrap();
        assert!(matches!(
            receive(&mut coach, &mut SeenFiles::new()),
            Err(PackageAppError::Folder { .. })
        ));
    }
}
