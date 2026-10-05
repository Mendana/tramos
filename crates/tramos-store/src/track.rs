//! Guardar y cargar el track del reloj asociado a un resultado.

use rusqlite::{OptionalExtension, params};
use tramos_core::model::{Track, TrackPoint};

use crate::convert::{instant_to_ms, ms_to_instant, opt_real, position, real};
use crate::source::ensure_source_file;
use crate::{ResultId, SourceFileId, Store, StoreError};

impl Store {
    /// Guarda el track de un resultado. Si ya tenía uno, lo sustituye.
    /// `source` enlaza el FIT del que sale, si se guardó con [`Store::save_source_file`].
    ///
    /// El desfase manual del track anterior ([`Store::set_manual_offset`]) solo se conserva si
    /// los dos salen del mismo fichero original: es el del reloj de ese FIT. Con otro FIT (o sin
    /// fichero) el track vuelve al desfase automático.
    pub fn save_track(
        &mut self,
        result: ResultId,
        track: &Track,
        source: Option<SourceFileId>,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        let source = ensure_source_file(&tx, source)?;
        let exists = tx
            .query_row(
                "SELECT 1 FROM results WHERE id = ?1",
                [result.0],
                |_| Ok(()),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StoreError::ResultNotFound(result.0));
        }
        let previous: Option<(Option<i64>, Option<f64>)> = tx
            .query_row(
                "SELECT source_file_id, manual_offset_s FROM tracks WHERE result_id = ?1",
                [result.0],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let manual_offset_s = match previous {
            Some((Some(old), offset)) if source == Some(old) => offset,
            _ => None,
        };
        // Los puntos del track anterior se borran en cascada.
        tx.execute("DELETE FROM tracks WHERE result_id = ?1", [result.0])?;
        tx.execute(
            "INSERT INTO tracks (result_id, source_file_id, sport, manual_offset_s) \
             VALUES (?1, ?2, ?3, ?4)",
            params![result.0, source, track.sport, manual_offset_s],
        )?;
        let track_id = tx.last_insert_rowid();
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO track_points (track_id, position, time_epoch_ms, lat, lon, \
                 altitude_m, heart_rate_bpm, cadence_spm, distance_m) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for (pos, p) in track.points.iter().enumerate() {
                stmt.execute(params![
                    track_id,
                    position(pos)?,
                    instant_to_ms(p.time)?,
                    real(p.lat, "lat")?,
                    real(p.lon, "lon")?,
                    opt_real(p.altitude_m, "altitude_m")?,
                    p.heart_rate_bpm,
                    opt_real(p.cadence_spm, "cadence_spm")?,
                    opt_real(p.distance_m, "distance_m")?,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Fichero original enlazado al track de un resultado; `None` si no tiene track o el
    /// track no tiene fichero enlazado.
    pub fn track_source_file(&self, result: ResultId) -> Result<Option<SourceFileId>, StoreError> {
        let source: Option<Option<i64>> = self
            .conn
            .query_row(
                "SELECT source_file_id FROM tracks WHERE result_id = ?1",
                [result.0],
                |row| row.get(0),
            )
            .optional()?;
        Ok(source.flatten().map(SourceFileId))
    }

    /// Desfase entre el reloj y el cronometraje fijado a mano para el track de un resultado, en
    /// segundos y con el convenio de `docs/alineacion.md` (instante en el track = picada +
    /// desfase). `None` si es automático o el resultado no tiene track.
    pub fn manual_offset(&self, result: ResultId) -> Result<Option<f64>, StoreError> {
        let offset: Option<Option<f64>> = self
            .conn
            .query_row(
                "SELECT manual_offset_s FROM tracks WHERE result_id = ?1",
                [result.0],
                |row| row.get(0),
            )
            .optional()?;
        Ok(offset.flatten())
    }

    /// Fija el desfase manual del track de un resultado; `None` vuelve al automático. No comprueba
    /// que encaje con el track: eso lo hace quien llama. Resultado inexistente: `ResultNotFound`;
    /// sin track: `TrackNotFound`; NaN: `NotANumber`.
    pub fn set_manual_offset(
        &mut self,
        result: ResultId,
        offset_s: Option<f64>,
    ) -> Result<(), StoreError> {
        let offset_s = opt_real(offset_s, "manual_offset_s")?;
        let updated = self.conn.execute(
            "UPDATE tracks SET manual_offset_s = ?1 WHERE result_id = ?2",
            params![offset_s, result.0],
        )?;
        if updated > 0 {
            return Ok(());
        }
        let exists = self
            .conn
            .query_row(
                "SELECT 1 FROM results WHERE id = ?1",
                [result.0],
                |_| Ok(()),
            )
            .optional()?;
        Err(match exists {
            Some(()) => StoreError::TrackNotFound(result.0),
            None => StoreError::ResultNotFound(result.0),
        })
    }

    /// Carga el track de un resultado; `None` si no tiene.
    pub fn load_track(&self, result: ResultId) -> Result<Option<Track>, StoreError> {
        let track: Option<(i64, Option<String>)> = self
            .conn
            .query_row(
                "SELECT id, sport FROM tracks WHERE result_id = ?1",
                [result.0],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((track_id, sport)) = track else {
            return Ok(None);
        };
        let mut stmt = self.conn.prepare_cached(
            "SELECT time_epoch_ms, lat, lon, altitude_m, heart_rate_bpm, cadence_spm, distance_m \
             FROM track_points WHERE track_id = ?1 ORDER BY position",
        )?;
        let mut rows = stmt.query([track_id])?;
        let mut points = Vec::new();
        while let Some(row) = rows.next()? {
            points.push(TrackPoint {
                time: ms_to_instant(row.get(0)?)?,
                lat: row.get(1)?,
                lon: row.get(2)?,
                altitude_m: row.get(3)?,
                heart_rate_bpm: row.get(4)?,
                cadence_spm: row.get(5)?,
                distance_m: row.get(6)?,
            });
        }
        Ok(Some(Track { points, sport }))
    }
}
