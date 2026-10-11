//! Copia de seguridad: exportar y restaurar la base (docs/almacenamiento.md), con datos
//! sintéticos.

// `allow-unwrap-in-tests` (clippy.toml) no cubre los helpers de un test de integración.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rusqlite::Connection;
use tramos_core::model::{Class, Course, Event, RaceResult, RaceStatus, Runner};
use tramos_core::race_format::RaceFormat;
use tramos_core::taxonomy::{Confirmation, LegTag};
use tramos_store::{EventId, SCHEMA_VERSION, SourceFileKind, Store, StoreError, inspect_backup};

fn made_at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 11, 9, 30, 0).unwrap()
}

fn event(name: &str, date: (i32, u32, u32), given: &str) -> Event {
    Event {
        name: Some(name.into()),
        date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
        classes: vec![Class {
            id: 1,
            name: "F21A".into(),
            short_name: None,
            course: Course {
                controls: vec![31, 45, 52],
            },
            results: vec![RaceResult {
                runner: Runner {
                    given_name: given.into(),
                    family_name: "Sintética".into(),
                    club: Some("Club Ejemplo".into()),
                    bib: Some(7),
                    si_card: None,
                    sex: None,
                },
                status: RaceStatus::Ok,
                place: Some(2),
                punches: vec![],
            }],
        }],
    }
}

/// Una base con una carrera completa: .spl guardado, formato, persona vinculada, etiqueta y
/// ajustes. Devuelve la base y el id de la carrera.
fn populated(path: &Path) -> (Store, EventId) {
    let mut store = Store::open(path).unwrap();
    let spl = store
        .save_source_file(SourceFileKind::Spl, "carrera.spl", b"contenido sintetico")
        .unwrap();
    let saved = store
        .save_event(&event("Media de prueba", (2026, 5, 17), "Ana"), Some(spl))
        .unwrap();
    store
        .set_event_format(saved.id, Some(RaceFormat::Middle))
        .unwrap();
    let result = saved.results[0][0];
    let person = store.create_person("Ana Ejemplo", Some("notas")).unwrap();
    store.link_result(result, person).unwrap();
    let tag = LegTag {
        confirmation: Some(Confirmation::Error),
        causes: vec!["rush".into(), "fatigue".into()],
        effort: Some(7),
        note: Some("Subí demasiado".into()),
        ..LegTag::default()
    };
    store.save_tag(result, 2, "0", &tag).unwrap();
    store
        .set_setting("lost_time.error_threshold_s", "20")
        .unwrap();
    store.set_setting("self.full_name", "Ana Ejemplo").unwrap();
    (store, saved.id)
}

fn dir_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

fn all_settings(path: &Path) -> Vec<(String, String)> {
    let conn = Connection::open(path).unwrap();
    let mut stmt = conn
        .prepare("SELECT key, value FROM settings ORDER BY key")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn export_and_restore_into_another_database_keeps_races_tags_and_settings() {
    let dir = tempfile::tempdir().unwrap();
    let origin_path = dir.path().join("origen.sqlite");
    let (origin, origin_event) = populated(&origin_path);
    let copy = dir.path().join("tramos-copia-2026-10-11.tramos");
    origin.export_backup(&copy, made_at()).unwrap();

    // Otro equipo, con otra carrera y otros ajustes.
    let other_path = dir.path().join("otro.sqlite");
    let mut other = Store::open(&other_path).unwrap();
    other
        .save_event(&event("Sprint de otro", (2025, 1, 2), "Berta"), None)
        .unwrap();
    other.set_setting("self.full_name", "Berta").unwrap();
    other.set_setting("solo.en.el.otro", "1").unwrap();

    let info = inspect_backup(&copy).unwrap();
    assert_eq!(info.created_at, Some(made_at()));
    assert_eq!((info.events, info.tags), (1, 1));
    assert_eq!(info.schema_version, SCHEMA_VERSION);

    let prepared = other.prepare_restore(&copy).unwrap();
    assert_eq!(prepared.info(), &info);
    let outcome = prepared.apply(&mut other).unwrap();

    // La base de antes se guarda al lado y los temporales desaparecen. Rutas canónicas: en
    // Windows la carpeta temporal puede salir con su nombre corto (`RUNNER~1`).
    assert_eq!(
        outcome.saved_previous.canonicalize().unwrap(),
        dir.path()
            .join("otro.sqlite.antes-de-restaurar")
            .canonicalize()
            .unwrap()
    );
    assert_eq!(
        dir_files(dir.path()),
        [
            "origen.sqlite",
            "otro.sqlite",
            "otro.sqlite.antes-de-restaurar",
            "tramos-copia-2026-10-11.tramos",
        ]
    );
    let before = Store::open(&outcome.saved_previous).unwrap();
    assert_eq!(
        before.load_event(EventId(1)).unwrap().name.as_deref(),
        Some("Sprint de otro")
    );
    assert_eq!(
        before.setting("solo.en.el.otro").unwrap().as_deref(),
        Some("1")
    );

    // La base abierta (y la del fichero) es ahora la restaurada.
    for restored in [&other, &Store::open(&other_path).unwrap()] {
        assert_eq!(
            restored.load_event(origin_event).unwrap(),
            origin.load_event(origin_event).unwrap()
        );
        assert_eq!(restored.setting("solo.en.el.otro").unwrap(), None);
        assert_eq!(
            restored.event_format(origin_event).unwrap(),
            Some(RaceFormat::Middle)
        );
        let result = restored.result_ids(origin_event).unwrap()[0][0];
        let origin_result = origin.result_ids(origin_event).unwrap()[0][0];
        assert_eq!(
            restored.tags(result).unwrap(),
            origin.tags(origin_result).unwrap()
        );
        assert_eq!(restored.people().unwrap(), origin.people().unwrap());
        assert_eq!(
            restored.result_person(result).unwrap(),
            Some(restored.people().unwrap()[0].id)
        );
        // Incluso el .spl original.
        let source = restored.event_source_file(origin_event).unwrap().unwrap();
        assert_eq!(
            restored.load_source_file(source).unwrap().content,
            b"contenido sintetico"
        );
    }
    // Los ajustes quedan como en el origen: la fecha de la copia no se cuela.
    assert_eq!(all_settings(&other_path), all_settings(&origin_path));
    assert_eq!(other.schema_version().unwrap(), SCHEMA_VERSION);
}

#[test]
fn export_replaces_an_existing_file_and_leaves_no_temporary() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = populated(&dir.path().join("tramos.sqlite"));
    let copy = dir.path().join("copia.tramos");
    fs::write(&copy, b"una copia vieja").unwrap();

    store.export_backup(&copy, made_at()).unwrap();

    assert_eq!(inspect_backup(&copy).unwrap().events, 1);
    assert_eq!(dir_files(dir.path()), ["copia.tramos", "tramos.sqlite"]);
}

#[test]
fn export_is_consistent_while_another_connection_is_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tramos.sqlite");
    let (store, _) = populated(&path);
    // Otra conexión con una lectura abierta, como una vista de la app.
    let reader = Connection::open(&path).unwrap();
    let mut stmt = reader.prepare("SELECT id FROM events").unwrap();
    let mut rows = stmt.query([]).unwrap();
    assert!(rows.next().unwrap().is_some());

    let copy = dir.path().join("copia.tramos");
    store.export_backup(&copy, made_at()).unwrap();

    assert_eq!(inspect_backup(&copy).unwrap().events, 1);
}

#[test]
fn export_refuses_to_overwrite_the_database_itself() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tramos.sqlite");
    let (store, event) = populated(&path);

    let err = store.export_backup(&path, made_at()).unwrap_err();

    assert!(
        matches!(err, StoreError::BackupOverwritesDatabase),
        "{err:?}"
    );
    assert_eq!(store.load_event(event).unwrap().classes.len(), 1);
    assert_eq!(dir_files(dir.path()), ["tramos.sqlite"]);
}

/// Intenta restaurar `backup` en una base con una carrera y comprueba que se rechaza sin
/// tocarla ni dejar ficheros de más. Devuelve el error.
fn rejected(dir: &Path, backup: &Path) -> StoreError {
    let path = dir.join("actual.sqlite");
    let (mut store, event) = populated(&path);
    let files = dir_files(dir);

    let err = store.prepare_restore(backup).unwrap_err();

    assert_eq!(dir_files(dir), files, "no debe quedar nada de más");
    assert_eq!(store.load_event(event).unwrap().classes.len(), 1);
    assert_eq!(
        store.setting("self.full_name").unwrap().as_deref(),
        Some("Ana Ejemplo")
    );
    // La base del fichero sigue igual de sana.
    store.set_setting("despues", "1").unwrap();
    assert_eq!(
        Store::open(&path)
            .unwrap()
            .setting("despues")
            .unwrap()
            .as_deref(),
        Some("1")
    );
    err
}

fn backups_dir() -> (tempfile::TempDir, tempfile::TempDir) {
    (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap())
}

#[test]
fn a_corrupt_or_foreign_file_is_rejected_without_touching_the_current_database() {
    let (db_dir, files_dir) = backups_dir();

    // Una copia buena, a la que se le estropea el medio.
    let (good, _) = populated(&files_dir.path().join("buena.sqlite"));
    let good_copy = files_dir.path().join("buena.tramos");
    good.export_backup(&good_copy, made_at()).unwrap();
    let mut bytes = fs::read(&good_copy).unwrap();
    for b in &mut bytes[4096..8192] {
        *b = 0xA5;
    }
    let damaged = files_dir.path().join("dañada.tramos");
    fs::write(&damaged, &bytes).unwrap();
    let truncated = files_dir.path().join("cortada.tramos");
    fs::write(&truncated, &bytes[..bytes.len() / 2]).unwrap();

    // Un fichero que no es SQLite, uno vacío y una base SQLite que no es de Tramos.
    let text = files_dir.path().join("texto.tramos");
    fs::write(&text, "esto no es una base de datos".repeat(200)).unwrap();
    let empty = files_dir.path().join("vacio.tramos");
    fs::write(&empty, b"").unwrap();
    let foreign = files_dir.path().join("otra.sqlite");
    Connection::open(&foreign)
        .unwrap()
        .execute_batch("CREATE TABLE notas (x TEXT); PRAGMA user_version = 3;")
        .unwrap();
    let missing = files_dir.path().join("no-existe.tramos");

    for bad in [&damaged, &truncated, &text, &empty, &foreign, &missing] {
        let err = rejected(db_dir.path(), bad);
        assert!(
            matches!(err, StoreError::BackupInvalid(_)),
            "{}: {err:?}",
            bad.display()
        );
        assert!(
            err.to_string()
                .starts_with("este fichero no vale como copia")
        );
        // Para el siguiente.
        fs::remove_file(db_dir.path().join("actual.sqlite")).unwrap();
    }
}

#[test]
fn a_copy_from_a_newer_version_is_rejected_without_touching_the_current_database() {
    let (db_dir, files_dir) = backups_dir();
    let (origin, _) = populated(&files_dir.path().join("origen.sqlite"));
    let copy = files_dir.path().join("nueva.tramos");
    origin.export_backup(&copy, made_at()).unwrap();
    Connection::open(&copy)
        .unwrap()
        .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .unwrap();

    let err = rejected(db_dir.path(), &copy);

    assert!(
        matches!(
            err,
            StoreError::BackupTooNew { found, supported }
                if found == SCHEMA_VERSION + 1 && supported == SCHEMA_VERSION
        ),
        "{err:?}"
    );
    assert!(err.to_string().contains("más nueva"));
}

#[test]
fn the_copy_file_itself_is_never_modified_by_restoring_it() {
    let dir = tempfile::tempdir().unwrap();
    let (origin, _) = populated(&dir.path().join("origen.sqlite"));
    let copy: PathBuf = dir.path().join("copia.tramos");
    origin.export_backup(&copy, made_at()).unwrap();
    let before = fs::read(&copy).unwrap();

    let mut other = Store::open(dir.path().join("otro.sqlite")).unwrap();
    other
        .prepare_restore(&copy)
        .unwrap()
        .apply(&mut other)
        .unwrap();

    assert_eq!(fs::read(&copy).unwrap(), before);
}

#[test]
fn dropping_a_prepared_restore_without_applying_it_leaves_everything_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let (origin, _) = populated(&dir.path().join("origen.sqlite"));
    let copy = dir.path().join("copia.tramos");
    origin.export_backup(&copy, made_at()).unwrap();
    let other = Store::open(dir.path().join("otro.sqlite")).unwrap();
    let files = dir_files(dir.path());

    let prepared = other.prepare_restore(&copy).unwrap();
    assert!(dir_files(dir.path()).contains(&"otro.sqlite.restaurando".to_string()));
    drop(prepared);

    assert_eq!(dir_files(dir.path()), files);
    assert!(other.setting("self.full_name").unwrap().is_none());
}

#[test]
fn an_in_memory_database_cannot_be_restored_into() {
    let dir = tempfile::tempdir().unwrap();
    let (origin, _) = populated(&dir.path().join("origen.sqlite"));
    let copy = dir.path().join("copia.tramos");
    origin.export_backup(&copy, made_at()).unwrap();

    let memory = Store::open_in_memory().unwrap();

    assert!(matches!(
        memory.prepare_restore(&copy),
        Err(StoreError::InvalidData(_))
    ));
}
