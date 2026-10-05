//! Paquetes por carrera (`docs/paquete.md`): el identificador del corredor que exporta y los
//! paquetes recibidos de otros corredores.

use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use tramos_core::package::{RacePackage, ShareLevel, hex};

use crate::convert::ms_to_instant;
use crate::{Store, StoreError};

/// Ajuste con el `runner_id` de esta base de datos.
const RUNNER_ID_KEY: &str = "package_runner_id";

/// Un paquete recibido, con el JSON tal cual llegó.
#[derive(Debug, Clone, PartialEq)]
pub struct ReceivedPackage {
    pub runner_id: String,
    pub race_id: String,
    pub level: ShareLevel,
    pub format_version: u32,
    pub exported_at: DateTime<Utc>,
    pub imported_at: DateTime<Utc>,
    /// El paquete en JSON (`RacePackage::parse` lo lee).
    pub content: String,
}

/// Qué ha pasado al guardar un paquete recibido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutcome {
    /// No había ninguno de ese corredor y esa carrera.
    Created,
    /// Sustituye al que había, que era igual de reciente o más antiguo.
    Replaced,
    /// Ya había uno más reciente: no se ha guardado.
    IgnoredOlder,
}

impl Store {
    /// Identificador de corredor de esta base de datos para los paquetes. La primera vez se
    /// genera (32 cifras hexadecimales sin relación con el nombre ni la tarjeta) y se guarda en
    /// los ajustes; después siempre es el mismo.
    pub fn package_runner_id(&mut self) -> Result<String, StoreError> {
        if let Some(id) = self.setting(RUNNER_ID_KEY)? {
            return Ok(id);
        }
        let id = random_id();
        self.set_setting(RUNNER_ID_KEY, &id)?;
        Ok(id)
    }

    /// Guarda un paquete recibido. Como mucho hay uno por (`runner_id`, `race_id`): uno igual de
    /// reciente o más (`exported_at`) sustituye al que había, también si baja de nivel; uno más
    /// antiguo se ignora.
    pub fn save_received_package(
        &mut self,
        package: &RacePackage,
    ) -> Result<SaveOutcome, StoreError> {
        let content = package
            .to_json()
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let exported_ms = package.exported_at.timestamp_millis();
        let tx = self.conn.transaction()?;
        let existing: Option<i64> = tx
            .query_row(
                "SELECT exported_at_epoch_ms FROM received_packages \
                 WHERE runner_id = ?1 AND race_id = ?2",
                params![package.runner.runner_id, package.race.race_id],
                |row| row.get(0),
            )
            .optional()?;
        let outcome = match existing {
            Some(previous) if previous > exported_ms => return Ok(SaveOutcome::IgnoredOlder),
            Some(_) => SaveOutcome::Replaced,
            None => SaveOutcome::Created,
        };
        tx.execute(
            "INSERT INTO received_packages (runner_id, race_id, level, format_version, \
             exported_at_epoch_ms, imported_at_epoch_ms, content) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT (runner_id, race_id) DO UPDATE SET level = excluded.level, \
             format_version = excluded.format_version, \
             exported_at_epoch_ms = excluded.exported_at_epoch_ms, \
             imported_at_epoch_ms = excluded.imported_at_epoch_ms, content = excluded.content",
            params![
                package.runner.runner_id,
                package.race.race_id,
                level_to_sql(package.level),
                package.version,
                exported_ms,
                Utc::now().timestamp_millis(),
                content,
            ],
        )?;
        tx.commit()?;
        Ok(outcome)
    }

    /// Paquetes recibidos, por corredor y carrera.
    pub fn received_packages(&self) -> Result<Vec<ReceivedPackage>, StoreError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT runner_id, race_id, level, format_version, exported_at_epoch_ms, \
             imported_at_epoch_ms, content FROM received_packages ORDER BY runner_id, race_id",
        )?;
        let mut rows = stmt.query([])?;
        let mut packages = Vec::new();
        while let Some(row) = rows.next()? {
            let level: String = row.get(2)?;
            packages.push(ReceivedPackage {
                runner_id: row.get(0)?,
                race_id: row.get(1)?,
                level: sql_to_level(&level)?,
                format_version: row.get(3)?,
                exported_at: ms_to_instant(row.get(4)?)?,
                imported_at: ms_to_instant(row.get(5)?)?,
                content: row.get(6)?,
            });
        }
        Ok(packages)
    }
}

/// 16 bytes sin relación con nada del corredor: SHA-256 del instante, el proceso y una
/// dirección de memoria. No es criptográfico; basta para no coincidir en un grupo de corredores.
fn random_id() -> String {
    let marker = 0u8;
    let mut hasher = Sha256::new();
    hasher.update(
        Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or_default()
            .to_le_bytes(),
    );
    hasher.update(std::process::id().to_le_bytes());
    hasher.update((&marker as *const u8 as usize).to_le_bytes());
    hex(&hasher.finalize()[..16])
}

fn level_to_sql(level: ShareLevel) -> &'static str {
    match level {
        ShareLevel::Aggregates => "aggregates",
        ShareLevel::Legs => "legs",
        ShareLevel::Track => "track",
    }
}

fn sql_to_level(text: &str) -> Result<ShareLevel, StoreError> {
    match text {
        "aggregates" => Ok(ShareLevel::Aggregates),
        "legs" => Ok(ShareLevel::Legs),
        "track" => Ok(ShareLevel::Track),
        other => Err(StoreError::InvalidData(format!(
            "nivel de paquete desconocido: {other}"
        ))),
    }
}
