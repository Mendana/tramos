//! Almacenamiento local de Tramos en SQLite.
//!
//! Guarda y recupera el modelo de `tramos_core::model` en un único fichero por usuario.
//! El esquema, la convención de tiempos y la política de ficheros originales están en
//! `docs/almacenamiento.md`.

mod convert;
mod error;
mod event;
mod migrations;
mod package;
mod person;
mod settings;
mod source;
mod tag;
mod track;

use std::path::Path;

use rusqlite::Connection;

pub use error::StoreError;
pub use event::SavedEvent;
pub use migrations::SCHEMA_VERSION;
pub use package::{ReceivedPackage, SaveOutcome};
pub use person::{Person, PersonId, PersonResult};
pub use source::{SourceFile, SourceFileId, SourceFileKind};
pub use tag::StoredTag;

/// Identificador de una carrera guardada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EventId(pub i64);

/// Identificador de un resultado guardado (un corredor en una categoría de una carrera).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResultId(pub i64);

/// Base de datos local de Tramos.
#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Abre (o crea) la base de datos del fichero `path` y aplica las migraciones pendientes.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Crea una base de datos en memoria con el esquema al día. Útil en tests.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut conn: Connection) -> Result<Self, StoreError> {
        // Fuera de una transacción: SQLite ignora este PRAGMA dentro de una.
        conn.pragma_update(None, "foreign_keys", true)?;
        migrations::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Versión del esquema de la base de datos (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<i64, StoreError> {
        migrations::user_version(&self.conn)
    }
}
