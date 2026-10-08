//! Paquete por carrera (`docs/paquete.md`): exportar una carrera del usuario a un fichero e
//! importar el de otro corredor. El formato es el de `tramos_core::package`; aquí se reúne lo que
//! lleva desde la base de datos. La carpeta compartida está en `sharing.rs`.

use std::path::{Path, PathBuf};

use chrono::{SubsecRound, Utc};
use thiserror::Error;
use tramos_core::package::{
    PackageError, PackageInput, PackageRunner, PackageTag, RacePackage, ShareLevel, SharedTrack,
    build,
};
use tramos_store::{ResultId, SaveOutcome, Store, StoreError};

use crate::import::stored_self_person;
use crate::settings;

/// Errores al exportar o importar. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum PackageAppError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Package(#[from] PackageError),
    #[error("solo se pueden compartir tus propias carreras")]
    NotYours,
    #[error("no se pudo escribir el paquete en {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("no se pudo leer el paquete {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("no se pudo borrar el paquete {path}: {source}")]
    Remove {
        path: String,
        source: std::io::Error,
    },
    #[error("no se pudo leer la carpeta compartida {path}: {source}")]
    Folder {
        path: String,
        source: std::io::Error,
    },
}

/// El paquete del resultado `result_id` del usuario con el nivel `level`.
pub fn race_package(
    store: &mut Store,
    result_id: i64,
    level: ShareLevel,
) -> Result<RacePackage, PackageAppError> {
    let result = ResultId(result_id);
    let me = stored_self_person(store)?.ok_or(PackageAppError::NotYours)?;
    if store.result_person(result)? != Some(me) {
        return Err(PackageAppError::NotYours);
    }
    let display_name = store
        .people()?
        .into_iter()
        .find(|p| p.id == me)
        .map(|p| p.display_name)
        .ok_or(PackageAppError::NotYours)?;
    let runner_id = store.package_runner_id()?;
    let (event_id, at) = store.result_ref(result)?;
    let event = store.load_event(event_id)?;
    let tags = store
        .tags(result)?
        .into_iter()
        .map(|t| PackageTag {
            leg_index: t.leg_index,
            taxonomy_version: t.taxonomy_version,
            tag: t.tag,
        })
        .collect();
    // El track solo se lee si va a viajar.
    let track = if level == ShareLevel::Track {
        store
            .load_track(result)?
            .map(|track| -> Result<SharedTrack, StoreError> {
                Ok(SharedTrack {
                    track,
                    manual_offset_s: store.manual_offset(result)?,
                })
            })
            .transpose()?
    } else {
        None
    };
    Ok(build(PackageInput {
        level,
        // Al segundo: no hace falta más y el nombre del instante queda limpio.
        exported_at: Utc::now().trunc_subsecs(0),
        runner: PackageRunner {
            runner_id,
            display_name,
        },
        event: &event,
        at,
        format: store.event_format(event_id)?,
        config: settings::lost_time_config(store)?,
        tags,
        track,
    })?)
}

/// Exporta el paquete a la carpeta `folder`, con su nombre de fichero (`RacePackage::file_name`):
/// exportar otra vez la misma carrera sobrescribe el mismo fichero. Devuelve la ruta.
pub fn export_package(
    store: &mut Store,
    result_id: i64,
    level: ShareLevel,
    folder: &str,
) -> Result<String, PackageAppError> {
    let package = race_package(store, result_id, level)?;
    write_package(folder, &package)?;
    Ok(package_path(folder, &package.file_name()))
}

/// Ruta del paquete `file_name` en la carpeta, como texto.
pub fn package_path(folder: &str, file_name: &str) -> String {
    Path::new(folder)
        .join(file_name)
        .to_string_lossy()
        .into_owned()
}

/// Escribe el paquete en la carpeta con su nombre, salvo que ya esté igual sin contar el
/// instante de exportación: así no cambia el fichero (ni lo vuelve a subir la sincronización)
/// si no ha cambiado nada. Escribe antes un temporal oculto y lo renombra, para que quien lea
/// la carpeta nunca encuentre el paquete a medias. Devuelve si lo ha escrito.
pub fn write_package(folder: &str, package: &RacePackage) -> Result<bool, PackageAppError> {
    let path: PathBuf = Path::new(folder).join(package.file_name());
    let same = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| RacePackage::parse(&text).ok())
        .is_some_and(|mut old| {
            old.exported_at = package.exported_at;
            old == *package
        });
    if same {
        return Ok(false);
    }
    let temporary = Path::new(folder).join(format!(".{}.tmp", package.file_name()));
    let write_error = |source| PackageAppError::Write {
        path: path.to_string_lossy().into_owned(),
        source,
    };
    std::fs::write(&temporary, package.to_json()?).map_err(write_error)?;
    std::fs::rename(&temporary, &path).map_err(|source| {
        let _ = std::fs::remove_file(&temporary);
        write_error(source)
    })?;
    Ok(true)
}

/// Importa un paquete de otro corredor. Reimportar no duplica: sustituye o se ignora si es más
/// antiguo (`docs/paquete.md`).
pub fn import_package(store: &mut Store, path: &str) -> Result<SaveOutcome, PackageAppError> {
    let text = std::fs::read_to_string(path).map_err(|source| PackageAppError::Read {
        path: path.to_string(),
        source,
    })?;
    let package = RacePackage::parse(&text)?;
    Ok(store.save_received_package(&package)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race_map::tests::imported;
    use tramos_core::taxonomy::{Confirmation, LegTag};

    #[test]
    fn every_level_goes_from_one_app_to_another() {
        let (mut store, result_id) = imported(true);
        crate::tags::save_leg_tag(
            &mut store,
            result_id,
            3,
            LegTag {
                confirmation: Some(Confirmation::Error),
                ..LegTag::default()
            },
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().to_str().unwrap();
        for level in [ShareLevel::Aggregates, ShareLevel::Legs, ShareLevel::Track] {
            let sent = race_package(&mut store, result_id, level).unwrap();
            let path = export_package(&mut store, result_id, level, folder).unwrap();
            assert!(path.ends_with(&sent.file_name()));

            let mut coach = Store::open_in_memory().unwrap();
            assert_eq!(
                import_package(&mut coach, &path).unwrap(),
                SaveOutcome::Created
            );
            let received = coach.received_packages().unwrap();
            let package = RacePackage::parse(&received[0].content).unwrap();
            assert_eq!(package.level, level);
            assert_eq!(package.runner, sent.runner);
            assert_eq!(package.summary, sent.summary);
            assert_eq!(package.tags.len(), usize::from(level >= ShareLevel::Legs));
            assert_eq!(
                package.track.map(|t| t.track),
                (level == ShareLevel::Track)
                    .then(|| store.load_track(ResultId(result_id)).unwrap().unwrap())
            );
        }
    }

    #[test]
    fn reimporting_a_new_export_does_not_duplicate() {
        let (mut store, result_id) = imported(false);
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().to_str().unwrap();
        let path = export_package(&mut store, result_id, ShareLevel::Legs, folder).unwrap();
        let mut coach = Store::open_in_memory().unwrap();
        import_package(&mut coach, &path).unwrap();
        // Reexportar escribe el mismo fichero; importarlo sustituye al anterior.
        let again = export_package(&mut store, result_id, ShareLevel::Aggregates, folder).unwrap();
        assert_eq!(again, path);
        assert_eq!(
            import_package(&mut coach, &again).unwrap(),
            SaveOutcome::Replaced
        );
        let received = coach.received_packages().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].level, ShareLevel::Aggregates);
        // El mismo corredor siempre con el mismo identificador.
        assert_eq!(received[0].runner_id, store.package_runner_id().unwrap());
    }

    #[test]
    fn only_your_own_results_can_be_shared() {
        let (mut store, result_id) = imported(false);
        let (event_id, _) = store.result_ref(ResultId(result_id)).unwrap();
        let other = store.result_ids(event_id).unwrap()[0][0];
        assert_ne!(other.0, result_id);
        assert!(matches!(
            race_package(&mut store, other.0, ShareLevel::Aggregates),
            Err(PackageAppError::NotYours)
        ));
    }

    #[test]
    fn a_broken_file_is_not_imported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roto.json");
        std::fs::write(&path, "{}").unwrap();
        let mut coach = Store::open_in_memory().unwrap();
        assert!(matches!(
            import_package(&mut coach, path.to_str().unwrap()),
            Err(PackageAppError::Package(PackageError::NotAPackage))
        ));
        assert!(coach.received_packages().unwrap().is_empty());
    }
}
