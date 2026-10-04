//! Ajustes clave-valor.

use rusqlite::{OptionalExtension, params};

use crate::{Store, StoreError};

impl Store {
    /// Valor de un ajuste; `None` si no se ha escrito nunca.
    pub fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    /// Escribe un ajuste, sustituyendo el valor anterior si lo había.
    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}
