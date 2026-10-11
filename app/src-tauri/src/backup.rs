//! Copia de seguridad de la base de datos (`docs/app.md`, "Copia de seguridad"): exportarla a un
//! fichero que elige el usuario y restaurarla desde otro. Todo el trabajo lo hace
//! `tramos-store`; aquí solo se adapta a lo que necesita la interfaz.

use std::path::Path;

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use tramos_store::{SCHEMA_VERSION, Store, StoreError, inspect_backup};

/// Qué trae una copia, para enseñarlo antes de restaurarla.
#[derive(Debug, Serialize)]
pub struct BackupSummary {
    /// Cuándo se hizo (RFC 3339, UTC); `None` si el fichero no lo dice.
    pub created_at: Option<String>,
    /// Carreras que contiene.
    pub events: usize,
    /// Etiquetas de tramo que contiene.
    pub tags: usize,
    /// Si es de una versión más vieja de la app y se actualizará al restaurar.
    pub migrates: bool,
}

/// Lee la copia `path` sin tocarla y comprueba que se puede restaurar.
pub fn summary(path: &str) -> Result<BackupSummary, StoreError> {
    let info = inspect_backup(Path::new(path))?;
    Ok(BackupSummary {
        created_at: info
            .created_at
            .map(|t| t.to_rfc3339_opts(SecondsFormat::Secs, true)),
        events: info.events,
        tags: info.tags,
        migrates: info.schema_version < SCHEMA_VERSION,
    })
}

/// Guarda una copia de toda la base en `path`.
pub fn export(store: &Store, path: &str) -> Result<(), StoreError> {
    store.export_backup(Path::new(path), Utc::now())
}

/// Sustituye la base por la copia `path`; devuelve dónde ha quedado la de antes. Si falla, la
/// base sigue como estaba. La interfaz reinicia la app después, para que arranque con la nueva.
pub fn restore(store: &mut Store, path: &str) -> Result<String, StoreError> {
    let outcome = store.prepare_restore(Path::new(path))?.apply(store)?;
    Ok(outcome.saved_previous.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_then_restore_into_another_database_and_summarise() {
        let dir = tempfile::tempdir().unwrap();
        let mut origin = Store::open(dir.path().join("origen.sqlite")).unwrap();
        origin.set_setting("self.full_name", "Ana Ejemplo").unwrap();
        let copy = dir.path().join("copia.tramos");
        let copy = copy.to_str().unwrap();
        export(&origin, copy).unwrap();

        let found = summary(copy).unwrap();
        assert_eq!((found.events, found.tags, found.migrates), (0, 0, false));
        assert!(found.created_at.is_some_and(|t| t.ends_with('Z')));

        let other_path = dir.path().join("otro.sqlite");
        let mut other = Store::open(&other_path).unwrap();
        let saved = restore(&mut other, copy).unwrap();

        assert!(saved.ends_with("otro.sqlite.antes-de-restaurar"));
        assert!(Path::new(&saved).is_file());
        assert_eq!(
            other.setting("self.full_name").unwrap().as_deref(),
            Some("Ana Ejemplo")
        );
    }

    #[test]
    fn a_file_that_is_not_a_copy_is_rejected_with_a_clear_message() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("notas.txt");
        std::fs::write(&bad, "hola").unwrap();
        let mut store = Store::open(dir.path().join("tramos.sqlite")).unwrap();
        store.set_setting("a", "b").unwrap();

        let err = restore(&mut store, bad.to_str().unwrap()).unwrap_err();

        assert!(err.to_string().starts_with("este fichero no vale"), "{err}");
        assert_eq!(store.setting("a").unwrap().as_deref(), Some("b"));
    }
}
