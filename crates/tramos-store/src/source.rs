//! Ficheros originales importados (.spl, FIT), guardados con su contenido.
//!
//! El .spl incluye nombres y fechas de nacimiento: estos datos nunca salen de la base local
//! (ver `docs/datos-y-privacidad.md`).

use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::convert::{ms_to_instant, position};
use crate::{Store, StoreError};

/// Identificador de un fichero original guardado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceFileId(pub i64);

/// Tipo de fichero original.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceFileKind {
    /// Splits de WinSplits.
    Spl,
    /// Registro del reloj.
    Fit,
}

impl SourceFileKind {
    fn as_sql(self) -> &'static str {
        match self {
            Self::Spl => "spl",
            Self::Fit => "fit",
        }
    }

    fn from_sql(text: &str) -> Result<Self, StoreError> {
        match text {
            "spl" => Ok(Self::Spl),
            "fit" => Ok(Self::Fit),
            _ => Err(StoreError::InvalidData(format!(
                "tipo de fichero inválido: {text:?}"
            ))),
        }
    }
}

/// Fichero original tal y como se guardó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub kind: SourceFileKind,
    /// Ruta de origen al importarlo; solo informativa.
    pub path: String,
    /// SHA-256 del contenido, en hexadecimal en minúsculas.
    pub sha256: String,
    pub content: Vec<u8>,
    pub imported_at: DateTime<Utc>,
}

/// SHA-256 en hexadecimal en minúsculas.
pub(crate) fn sha256_hex(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    let mut hex = String::with_capacity(64);
    for byte in digest.iter() {
        // Escribir en un `String` no falla.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

impl Store {
    /// Guarda un fichero original con su contenido. Si ya hay uno con el mismo SHA-256,
    /// no lo duplica y devuelve el id existente (con su tipo y ruta originales).
    pub fn save_source_file(
        &mut self,
        kind: SourceFileKind,
        path: &str,
        content: &[u8],
    ) -> Result<SourceFileId, StoreError> {
        let sha256 = sha256_hex(content);
        let tx = self.conn.transaction()?;
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM source_files WHERE sha256 = ?1",
                [&sha256],
                |row| row.get(0),
            )
            .optional()?;
        let id = match existing {
            Some(id) => id,
            None => {
                tx.execute(
                    "INSERT INTO source_files \
                     (kind, path, sha256, size_bytes, content, imported_at_epoch_ms) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        kind.as_sql(),
                        path,
                        sha256,
                        position(content.len())?,
                        content,
                        Utc::now().timestamp_millis(),
                    ],
                )?;
                tx.last_insert_rowid()
            }
        };
        tx.commit()?;
        Ok(SourceFileId(id))
    }

    /// Fichero original ya guardado con el mismo contenido (mismo SHA-256), sin guardar nada.
    pub fn find_source_file(&self, content: &[u8]) -> Result<Option<SourceFileId>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM source_files WHERE sha256 = ?1",
                [sha256_hex(content)],
                |row| row.get(0),
            )
            .optional()?
            .map(SourceFileId))
    }

    /// Carga un fichero original con su contenido.
    pub fn load_source_file(&self, id: SourceFileId) -> Result<SourceFile, StoreError> {
        let (kind, path, sha256, content, imported_at): (String, String, String, Vec<u8>, i64) =
            self.conn
                .query_row(
                    "SELECT kind, path, sha256, content, imported_at_epoch_ms \
                     FROM source_files WHERE id = ?1",
                    [id.0],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()?
                .ok_or(StoreError::SourceFileNotFound(id.0))?;
        Ok(SourceFile {
            kind: SourceFileKind::from_sql(&kind)?,
            path,
            sha256,
            content,
            imported_at: ms_to_instant(imported_at)?,
        })
    }
}

/// Comprueba que existe un fichero original antes de enlazarlo.
pub(crate) fn ensure_source_file(
    conn: &Connection,
    id: Option<SourceFileId>,
) -> Result<Option<i64>, StoreError> {
    let Some(id) = id else {
        return Ok(None);
    };
    let exists = conn
        .query_row("SELECT 1 FROM source_files WHERE id = ?1", [id.0], |_| {
            Ok(())
        })
        .optional()?;
    match exists {
        Some(()) => Ok(Some(id.0)),
        None => Err(StoreError::SourceFileNotFound(id.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vectors() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
