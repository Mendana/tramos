//! Copia de seguridad: exportar la base de datos entera a un fichero y restaurarla.
//!
//! Ver `docs/almacenamiento.md`, "Copia de seguridad". La copia es una base SQLite completa
//! (con los ficheros originales dentro) hecha con `VACUUM INTO`, que lee una instantánea
//! consistente aunque la app siga abierta. Restaurar **sustituye** la base actual: antes se
//! valida la copia y se prepara al lado, y la actual solo se toca en el último paso, que
//! guarda la de antes junto a ella.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::migrations::{SCHEMA_VERSION, user_version};
use crate::{Store, StoreError};

/// Ajuste que lleva la copia con el instante en que se hizo (RFC 3339). Solo vive en el fichero
/// de la copia: al restaurar se quita, para que los ajustes queden como estaban.
const CREATED_AT_KEY: &str = "backup.created_at";

/// Sufijo con el que se guarda la base que había antes de restaurar, junto a ella.
pub const SAVED_SUFFIX: &str = ".antes-de-restaurar";

/// Sufijo de la copia ya validada y migrada que espera a sustituir a la base actual.
const STAGING_SUFFIX: &str = ".restaurando";

/// Sufijo del fichero temporal mientras se escribe una copia.
const EXPORTING_SUFFIX: &str = ".exportando";

/// Tablas que tiene cualquier base de Tramos desde la primera versión del esquema.
const CORE_TABLES: [&str; 8] = [
    "classes",
    "events",
    "punches",
    "results",
    "runners",
    "settings",
    "source_files",
    "tracks",
];

/// Qué trae una copia de seguridad, para enseñarlo antes de restaurarla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupInfo {
    /// Versión del esquema de la copia. Si es menor que [`SCHEMA_VERSION`], al restaurar se
    /// migra como al arrancar.
    pub schema_version: i64,
    /// Cuándo se hizo la copia; `None` si el fichero no lo dice.
    pub created_at: Option<DateTime<Utc>>,
    /// Carreras que contiene.
    pub events: usize,
    /// Etiquetas de tramo que contiene.
    pub tags: usize,
}

/// Una copia validada y migrada, lista para sustituir a la base actual con [`apply`](Self::apply).
/// Si se descarta sin aplicar, borra su fichero temporal y la base actual no se ha tocado.
#[derive(Debug)]
pub struct PreparedRestore {
    staged: PathBuf,
    db_path: PathBuf,
    info: BackupInfo,
    applied: bool,
}

/// Resultado de restaurar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreOutcome {
    /// Dónde ha quedado la base que había antes de restaurar.
    pub saved_previous: PathBuf,
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn invalid(detail: impl std::fmt::Display) -> StoreError {
    StoreError::BackupInvalid(detail.to_string())
}

/// Ruta (canónica) de la base de datos de `conn`, si es de fichero.
fn database_path(conn: &Connection) -> Option<PathBuf> {
    conn.path()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .and_then(|p| p.canonicalize().ok())
}

/// Lee una copia sin modificarla y comprueba que es una base de Tramos sana y que esta versión
/// de la app la entiende: es SQLite y no está dañada, tiene las tablas de Tramos y su versión de
/// esquema no es más nueva que la de la app (`BackupTooNew`).
pub fn inspect_backup(path: impl AsRef<Path>) -> Result<BackupInfo, StoreError> {
    let path = path.as_ref();
    if !path.is_file() {
        return Err(invalid("no se encuentra el fichero"));
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| invalid(format!("no se puede abrir: {e}")))?;
    inspect_connection(&conn)
}

fn inspect_connection(conn: &Connection) -> Result<BackupInfo, StoreError> {
    // Un fichero que no es SQLite falla ya en la primera lectura.
    let schema_version = user_version(conn)
        .map_err(|_| invalid("no es una base de datos de SQLite o está dañado"))?;
    if schema_version > SCHEMA_VERSION {
        return Err(StoreError::BackupTooNew {
            found: schema_version,
            supported: SCHEMA_VERSION,
        });
    }
    let damaged = |e: rusqlite::Error| invalid(format!("está dañado ({e})"));
    let check: String = conn
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(damaged)?;
    if check != "ok" {
        return Err(invalid(format!("está dañado ({check})")));
    }
    let present: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type = 'table'")
        .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
        .map_err(damaged)?;
    if schema_version < 1 || CORE_TABLES.iter().any(|t| !present.iter().any(|p| p == t)) {
        return Err(invalid("no es una copia de seguridad de Tramos"));
    }
    let orphan: Option<String> = conn
        .query_row("PRAGMA foreign_key_check", [], |r| r.get(0))
        .optional()
        .map_err(damaged)?;
    if let Some(table) = orphan {
        return Err(invalid(format!(
            "está dañado (datos huérfanos en la tabla {table})"
        )));
    }
    let count = |table: &str| -> Result<usize, StoreError> {
        let n: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .map_err(damaged)?;
        usize::try_from(n).map_err(|_| invalid("recuento de filas negativo"))
    };
    let created_at: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [CREATED_AT_KEY],
            |r| r.get(0),
        )
        .optional()
        .map_err(damaged)?;
    Ok(BackupInfo {
        schema_version,
        created_at: created_at
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|t| t.with_timezone(&Utc)),
        events: count("events")?,
        // `tags` existe desde la migración 1.
        tags: count("tags")?,
    })
}

impl Store {
    /// Guarda una copia de seguridad de toda la base en `dest`, sustituyendo el fichero si ya
    /// existe. La copia es consistente aunque haya otras conexiones abiertas, y lleva dentro
    /// `created_at`. Se escribe en un fichero temporal y solo al final ocupa el sitio de `dest`,
    /// así que un fallo no deja una copia a medias ni estropea una anterior.
    ///
    /// La copia incluye los ficheros originales importados (nombres y fechas de nacimiento del
    /// .spl, GPS y pulso del FIT): `docs/datos-y-privacidad.md`.
    pub fn export_backup(
        &self,
        dest: impl AsRef<Path>,
        created_at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let dest = dest.as_ref();
        if let (Some(live), Ok(target)) = (database_path(&self.conn), dest.canonicalize())
            && live == target
        {
            return Err(StoreError::BackupOverwritesDatabase);
        }
        let tmp = with_suffix(dest, EXPORTING_SUFFIX);
        let result = self.write_backup(&tmp, created_at).and_then(|()| {
            fs::rename(&tmp, dest)?;
            Ok(())
        });
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }

    fn write_backup(&self, tmp: &Path, created_at: DateTime<Utc>) -> Result<(), StoreError> {
        match fs::remove_file(tmp) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        let tmp_text = tmp
            .to_str()
            .ok_or_else(|| StoreError::InvalidData("la ruta de la copia no es UTF-8".into()))?;
        self.conn.execute("VACUUM INTO ?1", [tmp_text])?;
        let copy = Connection::open(tmp)?;
        copy.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [
                CREATED_AT_KEY,
                &created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            ],
        )?;
        copy.close().map_err(|(_, e)| e)?;
        Ok(())
    }

    /// Valida la copia `backup` y la deja preparada al lado de esta base para sustituirla, sin
    /// tocar la base actual: se copia a un fichero temporal, se migra a la versión de esta app
    /// y se le quita el ajuste de la fecha. Si algo falla, no queda nada a medias. Una copia de
    /// un esquema más nuevo se rechaza (`BackupTooNew`).
    pub fn prepare_restore(&self, backup: impl AsRef<Path>) -> Result<PreparedRestore, StoreError> {
        let backup = backup.as_ref();
        let info = inspect_backup(backup)?;
        let db_path = database_path(&self.conn).ok_or_else(|| {
            StoreError::InvalidData("la base de datos no está en un fichero".into())
        })?;
        let prepared = PreparedRestore {
            staged: with_suffix(&db_path, STAGING_SUFFIX),
            db_path,
            info,
            applied: false,
        };
        // Con `prepared` ya creado, cualquier fallo de aquí en adelante borra el temporal.
        fs::copy(backup, &prepared.staged)?;
        let migrated = Store::open(&prepared.staged)?;
        migrated
            .conn
            .execute("DELETE FROM settings WHERE key = ?1", [CREATED_AT_KEY])?;
        migrated.conn.close().map_err(|(_, e)| e)?;
        Ok(prepared)
    }
}

impl PreparedRestore {
    /// Qué trae la copia.
    pub fn info(&self) -> &BackupInfo {
        &self.info
    }

    /// Sustituye la base actual por la copia. `store` tiene que ser la base que se usó en
    /// [`Store::prepare_restore`]: se cierra para poder mover su fichero (en Windows no se puede
    /// mover abierto) y se vuelve a abrir al terminar, con la copia si todo fue bien o con la
    /// base de antes si no. La base de antes queda junto a ella con el sufijo
    /// [`SAVED_SUFFIX`] (sustituyendo una anterior).
    pub fn apply(mut self, store: &mut Store) -> Result<RestoreOutcome, StoreError> {
        let saved_previous = with_suffix(&self.db_path, SAVED_SUFFIX);
        let moved = self.swap(store, &saved_previous);
        // Con la copia puesta o no, la app sigue con una conexión abierta en la ruta de la base.
        let reopened = Store::open(&self.db_path);
        match (moved, reopened) {
            (Ok(()), Ok(reopened)) => {
                *store = reopened;
                self.applied = true;
                Ok(RestoreOutcome { saved_previous })
            }
            (Err(e), Ok(reopened)) => {
                *store = reopened;
                Err(e)
            }
            (_, Err(e)) => Err(e),
        }
    }

    fn swap(&self, store: &mut Store, saved_previous: &Path) -> Result<(), StoreError> {
        let placeholder = Connection::open_in_memory()?;
        let live = std::mem::replace(&mut store.conn, placeholder);
        live.close().map_err(|(_, e)| e)?;
        fs::rename(&self.db_path, saved_previous)?;
        if let Err(e) = fs::rename(&self.staged, &self.db_path) {
            // Dejarlo como estaba.
            let _ = fs::rename(saved_previous, &self.db_path);
            return Err(e.into());
        }
        Ok(())
    }
}

impl Drop for PreparedRestore {
    fn drop(&mut self) {
        // Si se aplicó, el temporal ya es la base; si no, sobra.
        if !self.applied {
            let _ = fs::remove_file(&self.staged);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::MIGRATIONS;

    #[test]
    fn a_copy_from_an_older_version_is_migrated_when_restored() {
        let dir = tempfile::tempdir().unwrap();
        // Una copia como la hacía una versión de Tramos con solo las 5 primeras migraciones.
        let old = dir.path().join("vieja.tramos");
        let conn = Connection::open(&old).unwrap();
        for script in &MIGRATIONS[..5] {
            conn.execute_batch(script).unwrap();
        }
        conn.pragma_update(None, "user_version", 5).unwrap();
        conn.execute_batch(
            "INSERT INTO events (id, name, date) VALUES (1, 'Vieja', '2025-05-04');
             INSERT INTO settings (key, value) VALUES ('self.full_name', 'Ana');",
        )
        .unwrap();
        conn.close().unwrap();

        let mut current = Store::open(dir.path().join("tramos.sqlite")).unwrap();
        assert_eq!(inspect_backup(&old).unwrap().schema_version, 5);
        let prepared = current.prepare_restore(&old).unwrap();
        assert_eq!(prepared.info().events, 1);
        // Sin fecha: esta copia no la lleva.
        assert_eq!(prepared.info().created_at, None);
        prepared.apply(&mut current).unwrap();

        assert_eq!(current.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(
            current.setting("self.full_name").unwrap().as_deref(),
            Some("Ana")
        );
        // Ya tiene las tablas de las migraciones nuevas.
        assert_eq!(current.received_package_counts().unwrap(), (0, 0));
        // La vieja no se ha tocado.
        assert_eq!(inspect_backup(&old).unwrap().schema_version, 5);
    }
}
