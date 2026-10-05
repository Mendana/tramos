//! Etiquetas del corredor sobre los tramos de un resultado (`docs/taxonomia.md`).
//!
//! Apuntan a `(result_id, leg_index)` y no a `legs.id`, para sobrevivir a un recálculo de los
//! tramos. Hay como mucho una por tramo. Tipos, subtipos y causas son claves del fichero de
//! taxonomía: aquí se guardan tal cual, sin comprobarlas (eso lo hace
//! `tramos_core::taxonomy::Taxonomy::validate`), junto con la versión de la taxonomía.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use tramos_core::taxonomy::{Confirmation, LegPart, LegTag};

use crate::convert::{ms_to_instant, opt_real};
use crate::{ResultId, Store, StoreError};

/// Etiqueta guardada de un tramo.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredTag {
    /// Tramo, desde 1 (como `LegReport::index`).
    pub leg_index: usize,
    /// Versión de la taxonomía con la que se escribió la etiqueta por última vez.
    pub taxonomy_version: String,
    /// Contenido. Las causas vuelven ordenadas por clave.
    pub tag: LegTag,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Store {
    /// Etiquetas de un resultado, por tramo.
    pub fn tags(&self, result: ResultId) -> Result<Vec<StoredTag>, StoreError> {
        ensure_result(&self.conn, result)?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, leg_index, taxonomy_version, confirmation, error_type, error_subtype, \
             leg_part, perceived_loss_s, effort, note, created_at_epoch_ms, updated_at_epoch_ms \
             FROM tags WHERE result_id = ?1 ORDER BY leg_index",
        )?;
        let mut rows = stmt.query([result.0])?;
        let mut tags = Vec::new();
        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let leg_index: i64 = row.get(1)?;
            tags.push(StoredTag {
                leg_index: usize::try_from(leg_index).map_err(|_| {
                    StoreError::InvalidData(format!("tramo de etiqueta inválido: {leg_index}"))
                })?,
                taxonomy_version: row.get(2)?,
                tag: LegTag {
                    confirmation: row
                        .get::<_, Option<String>>(3)?
                        .as_deref()
                        .map(sql_to_confirmation)
                        .transpose()?,
                    error_type: row.get(4)?,
                    error_subtype: row.get(5)?,
                    causes: causes(&self.conn, id)?,
                    leg_part: row
                        .get::<_, Option<String>>(6)?
                        .as_deref()
                        .map(sql_to_leg_part)
                        .transpose()?,
                    perceived_loss_s: row.get(7)?,
                    effort: row.get(8)?,
                    note: row.get(9)?,
                },
                created_at: ms_to_instant(row.get(10)?)?,
                updated_at: ms_to_instant(row.get(11)?)?,
            });
        }
        Ok(tags)
    }

    /// Guarda la etiqueta de un tramo, sustituyendo la que hubiera (con sus causas) pero
    /// conservando su instante de creación. Una etiqueta vacía ([`LegTag::is_empty`]) borra la
    /// del tramo y devuelve `None`.
    ///
    /// No comprueba las claves contra la taxonomía ni que el tramo exista en el recorrido: eso
    /// es cosa de quien llama.
    pub fn save_tag(
        &mut self,
        result: ResultId,
        leg_index: usize,
        taxonomy_version: &str,
        tag: &LegTag,
    ) -> Result<Option<StoredTag>, StoreError> {
        let index = leg_index_to_sql(leg_index)?;
        if tag.is_empty() {
            self.delete_tag(result, leg_index)?;
            return Ok(None);
        }
        let perceived_loss_s = opt_real(tag.perceived_loss_s, "perceived_loss_s")?;
        let now = Utc::now().timestamp_millis();
        let tx = self.conn.transaction()?;
        ensure_result(&tx, result)?;
        let (id, created_ms): (i64, i64) = tx.query_row(
            "INSERT INTO tags (result_id, leg_index, taxonomy_version, confirmation, error_type, \
             error_subtype, leg_part, perceived_loss_s, effort, note, created_at_epoch_ms, \
             updated_at_epoch_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11) \
             ON CONFLICT (result_id, leg_index) DO UPDATE SET \
             taxonomy_version = excluded.taxonomy_version, \
             confirmation = excluded.confirmation, error_type = excluded.error_type, \
             error_subtype = excluded.error_subtype, leg_part = excluded.leg_part, \
             perceived_loss_s = excluded.perceived_loss_s, effort = excluded.effort, \
             note = excluded.note, updated_at_epoch_ms = excluded.updated_at_epoch_ms \
             RETURNING id, created_at_epoch_ms",
            params![
                result.0,
                index,
                taxonomy_version,
                tag.confirmation.map(confirmation_to_sql),
                tag.error_type,
                tag.error_subtype,
                tag.leg_part.map(leg_part_to_sql),
                perceived_loss_s,
                tag.effort,
                tag.note,
                now,
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        tx.execute("DELETE FROM tag_causes WHERE tag_id = ?1", [id])?;
        for cause in &tag.causes {
            tx.execute(
                "INSERT OR IGNORE INTO tag_causes (tag_id, cause) VALUES (?1, ?2)",
                params![id, cause],
            )?;
        }
        let causes = causes(&tx, id)?;
        tx.commit()?;
        Ok(Some(StoredTag {
            leg_index,
            taxonomy_version: taxonomy_version.to_string(),
            tag: LegTag {
                causes,
                ..tag.clone()
            },
            created_at: ms_to_instant(created_ms)?,
            updated_at: ms_to_instant(now)?,
        }))
    }

    /// Borra la etiqueta de un tramo (con sus causas). Si no la tenía no hace nada.
    pub fn delete_tag(&mut self, result: ResultId, leg_index: usize) -> Result<(), StoreError> {
        let index = leg_index_to_sql(leg_index)?;
        ensure_result(&self.conn, result)?;
        self.conn.execute(
            "DELETE FROM tags WHERE result_id = ?1 AND leg_index = ?2",
            params![result.0, index],
        )?;
        Ok(())
    }
}

fn ensure_result(conn: &Connection, result: ResultId) -> Result<(), StoreError> {
    conn.query_row(
        "SELECT 1 FROM results WHERE id = ?1",
        [result.0],
        |_| Ok(()),
    )
    .optional()?
    .ok_or(StoreError::ResultNotFound(result.0))
}

/// Causas de una etiqueta, ordenadas por clave.
fn causes(conn: &Connection, tag_id: i64) -> Result<Vec<String>, StoreError> {
    let mut stmt =
        conn.prepare_cached("SELECT cause FROM tag_causes WHERE tag_id = ?1 ORDER BY cause")?;
    let causes = stmt
        .query_map([tag_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(causes)
}

fn leg_index_to_sql(leg_index: usize) -> Result<i64, StoreError> {
    match i64::try_from(leg_index) {
        Ok(index) if index >= 1 => Ok(index),
        _ => Err(StoreError::InvalidLegIndex(leg_index)),
    }
}

fn confirmation_to_sql(c: Confirmation) -> &'static str {
    match c {
        Confirmation::Error => "error",
        Confirmation::NoError => "no_error",
        Confirmation::Physical => "physical",
    }
}

fn sql_to_confirmation(text: &str) -> Result<Confirmation, StoreError> {
    match text {
        "error" => Ok(Confirmation::Error),
        "no_error" => Ok(Confirmation::NoError),
        "physical" => Ok(Confirmation::Physical),
        other => Err(StoreError::InvalidData(format!(
            "confirmación desconocida: {other}"
        ))),
    }
}

fn leg_part_to_sql(p: LegPart) -> &'static str {
    match p {
        LegPart::Start => "start",
        LegPart::Middle => "middle",
        LegPart::Attack => "attack",
    }
}

fn sql_to_leg_part(text: &str) -> Result<LegPart, StoreError> {
    match text {
        "start" => Ok(LegPart::Start),
        "middle" => Ok(LegPart::Middle),
        "attack" => Ok(LegPart::Attack),
        other => Err(StoreError::InvalidData(format!(
            "parte del tramo desconocida: {other}"
        ))),
    }
}
