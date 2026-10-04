//! Guardar y cargar el track del reloj asociado a un resultado.

use rusqlite::{OptionalExtension, params};
use tramos_core::model::{Track, TrackPoint};

use crate::convert::{instant_to_ms, ms_to_instant, opt_real, position, real};
use crate::{ResultId, Store, StoreError};

impl Store {
    /// Guarda el track de un resultado. Si ya tenía uno, lo sustituye.
    pub fn save_track(&mut self, result: ResultId, track: &Track) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
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
        // Los puntos del track anterior se borran en cascada.
        tx.execute("DELETE FROM tracks WHERE result_id = ?1", [result.0])?;
        tx.execute("INSERT INTO tracks (result_id) VALUES (?1)", [result.0])?;
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

    /// Carga el track de un resultado; `None` si no tiene.
    pub fn load_track(&self, result: ResultId) -> Result<Option<Track>, StoreError> {
        let track_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM tracks WHERE result_id = ?1",
                [result.0],
                |row| row.get(0),
            )
            .optional()?;
        let Some(track_id) = track_id else {
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
        Ok(Some(Track { points }))
    }
}
