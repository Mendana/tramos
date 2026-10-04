//! Guardar y cargar carreras completas.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use tramos_core::identify::ResultRef;
use tramos_core::model::{Class, Course, Event, Punch, RaceResult, Runner};
use tramos_core::race_format::RaceFormat;

use crate::convert::{
    date_to_text, format_to_sql, instant_to_ms, ms_to_instant, position, sex_to_sql, sql_to_format,
    sql_to_sex, sql_to_status, status_to_sql, text_to_date,
};
use crate::source::ensure_source_file;
use crate::{EventId, ResultId, SourceFileId, Store, StoreError};

/// Expresión SQL con el inicio de la carrera `e` (ver [`Store::event_start`]): la primera picada
/// con hora de cualquiera de sus resultados, en milisegundos desde la época Unix, o `NULL`.
pub(crate) const EVENT_START_MS_SQL: &str = "(SELECT MIN(p.time_epoch_ms) \
     FROM classes ec \
     JOIN results er ON er.class_id = ec.id \
     JOIN punches p ON p.result_id = er.id \
     WHERE ec.event_id = e.id)";

/// Identificadores asignados al guardar una carrera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedEvent {
    pub id: EventId,
    /// `results[c][r]` es el resultado `r` de la categoría `c`, en el orden de `Event`.
    pub results: Vec<Vec<ResultId>>,
}

impl Store {
    /// Guarda una carrera completa en una transacción: o se guarda entera o nada.
    /// `source` enlaza el .spl del que sale, si se guardó con [`Store::save_source_file`].
    ///
    /// Las categorías con la misma secuencia de balizas comparten una fila de `courses`.
    pub fn save_event(
        &mut self,
        event: &Event,
        source: Option<SourceFileId>,
    ) -> Result<SavedEvent, StoreError> {
        let tx = self.conn.transaction()?;
        let source = ensure_source_file(&tx, source)?;
        let saved = insert_event(&tx, event, source)?;
        tx.commit()?;
        Ok(saved)
    }

    /// Fichero original enlazado a una carrera, si lo tiene.
    pub fn event_source_file(&self, id: EventId) -> Result<Option<SourceFileId>, StoreError> {
        let source: Option<i64> = self
            .conn
            .query_row(
                "SELECT source_file_id FROM events WHERE id = ?1",
                [id.0],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::EventNotFound(id.0))?;
        Ok(source.map(SourceFileId))
    }

    /// Carrera de un resultado y su posición en ella (categoría y resultado en el orden del
    /// modelo), para buscarlo en lo que devuelve [`Store::load_event`].
    pub fn result_ref(&self, result: ResultId) -> Result<(EventId, ResultRef), StoreError> {
        let (event, class_position, result_position): (i64, i64, i64) = self
            .conn
            .query_row(
                "SELECT c.event_id, c.position, r.position FROM results r \
                 JOIN classes c ON c.id = r.class_id WHERE r.id = ?1",
                [result.0],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(StoreError::ResultNotFound(result.0))?;
        let index = |position: i64| {
            usize::try_from(position)
                .map_err(|_| StoreError::InvalidData(format!("posición {position}")))
        };
        Ok((
            EventId(event),
            ResultRef {
                class_index: index(class_position)?,
                result_index: index(result_position)?,
            },
        ))
    }

    /// Primera carrera guardada (la de menor id) enlazada a un .spl, si hay alguna. Sirve para no
    /// duplicar una carrera al reimportar el mismo fichero.
    pub fn event_by_source_file(
        &self,
        source: SourceFileId,
    ) -> Result<Option<EventId>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT MIN(id) FROM events WHERE source_file_id = ?1",
                [source.0],
                |row| row.get::<_, Option<i64>>(0),
            )?
            .map(EventId))
    }

    /// Formato de una carrera; `None` si no se ha fijado.
    pub fn event_format(&self, id: EventId) -> Result<Option<RaceFormat>, StoreError> {
        let format: Option<String> = self
            .conn
            .query_row("SELECT format FROM events WHERE id = ?1", [id.0], |row| {
                row.get(0)
            })
            .optional()?
            .ok_or(StoreError::EventNotFound(id.0))?;
        format.as_deref().map(sql_to_format).transpose()
    }

    /// Fija el formato de una carrera; `None` lo borra.
    pub fn set_event_format(
        &mut self,
        id: EventId,
        format: Option<RaceFormat>,
    ) -> Result<(), StoreError> {
        let updated = self.conn.execute(
            "UPDATE events SET format = ?1 WHERE id = ?2",
            params![format.map(format_to_sql), id.0],
        )?;
        if updated == 0 {
            return Err(StoreError::EventNotFound(id.0));
        }
        Ok(())
    }

    /// Inicio de una carrera para ordenar las del mismo día: la primera picada con hora de
    /// cualquiera de sus resultados (en la práctica, la primera salida). `None` si ningún
    /// resultado tiene picadas con hora.
    ///
    /// No usa el FIT, que solo tienen algunos resultados, ni las fechas de la cabecera del .spl,
    /// que parecen de creación del fichero (`docs/almacenamiento.md`).
    pub fn event_start(&self, id: EventId) -> Result<Option<DateTime<Utc>>, StoreError> {
        let ms: Option<i64> = self
            .conn
            .query_row(
                &format!("SELECT {EVENT_START_MS_SQL} FROM events e WHERE e.id = ?1"),
                [id.0],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::EventNotFound(id.0))?;
        ms.map(ms_to_instant).transpose()
    }

    /// Carga una carrera tal y como se guardó.
    pub fn load_event(&self, id: EventId) -> Result<Event, StoreError> {
        let (name, date): (Option<String>, String) = self
            .conn
            .query_row(
                "SELECT name, date FROM events WHERE id = ?1",
                [id.0],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(StoreError::EventNotFound(id.0))?;

        let mut classes = Vec::new();
        let mut courses: HashMap<i64, Course> = HashMap::new();
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, source_id, name, short_name, course_id FROM classes \
             WHERE event_id = ?1 ORDER BY position",
        )?;
        let mut rows = stmt.query([id.0])?;
        while let Some(row) = rows.next()? {
            let class_row_id: i64 = row.get(0)?;
            let course_id: i64 = row.get(4)?;
            let course = match courses.get(&course_id) {
                Some(course) => course.clone(),
                None => {
                    let course = load_course(&self.conn, course_id)?;
                    courses.insert(course_id, course.clone());
                    course
                }
            };
            classes.push(Class {
                id: row.get(1)?,
                name: row.get(2)?,
                short_name: row.get(3)?,
                course,
                results: load_results(&self.conn, class_row_id)?,
            });
        }

        Ok(Event {
            name,
            date: text_to_date(&date)?,
            classes,
        })
    }

    /// Identificadores de los resultados de una carrera, como en [`SavedEvent::results`].
    pub fn result_ids(&self, id: EventId) -> Result<Vec<Vec<ResultId>>, StoreError> {
        let exists = self
            .conn
            .query_row("SELECT 1 FROM events WHERE id = ?1", [id.0], |_| Ok(()))
            .optional()?;
        if exists.is_none() {
            return Err(StoreError::EventNotFound(id.0));
        }
        let mut classes = self
            .conn
            .prepare_cached("SELECT id FROM classes WHERE event_id = ?1 ORDER BY position")?;
        let mut results = self
            .conn
            .prepare_cached("SELECT id FROM results WHERE class_id = ?1 ORDER BY position")?;
        let class_ids = classes
            .query_map([id.0], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let mut out = Vec::with_capacity(class_ids.len());
        for class_id in class_ids {
            let ids = results
                .query_map([class_id], |row| row.get(0).map(ResultId))?
                .collect::<Result<Vec<_>, _>>()?;
            out.push(ids);
        }
        Ok(out)
    }
}

fn insert_event(
    conn: &Connection,
    event: &Event,
    source: Option<i64>,
) -> Result<SavedEvent, StoreError> {
    conn.execute(
        "INSERT INTO events (name, date, source_file_id) VALUES (?1, ?2, ?3)",
        params![event.name, date_to_text(event.date), source],
    )?;
    let event_id = conn.last_insert_rowid();

    let mut course_ids: HashMap<&Course, i64> = HashMap::new();
    let mut results = Vec::with_capacity(event.classes.len());
    for (class_pos, class) in event.classes.iter().enumerate() {
        let course_id = match course_ids.get(&class.course) {
            Some(&id) => id,
            None => {
                let id = insert_course(conn, event_id, &class.course)?;
                course_ids.insert(&class.course, id);
                id
            }
        };
        conn.prepare_cached(
            "INSERT INTO classes (event_id, position, source_id, name, short_name, course_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?
        .execute(params![
            event_id,
            position(class_pos)?,
            class.id,
            class.name,
            class.short_name,
            course_id
        ])?;
        let class_id = conn.last_insert_rowid();

        let mut ids = Vec::with_capacity(class.results.len());
        for (result_pos, result) in class.results.iter().enumerate() {
            ids.push(insert_result(conn, event_id, class_id, result_pos, result)?);
        }
        results.push(ids);
    }

    Ok(SavedEvent {
        id: EventId(event_id),
        results,
    })
}

fn insert_course(conn: &Connection, event_id: i64, course: &Course) -> Result<i64, StoreError> {
    conn.prepare_cached("INSERT INTO courses (event_id) VALUES (?1)")?
        .execute([event_id])?;
    let course_id = conn.last_insert_rowid();
    let mut stmt = conn.prepare_cached(
        "INSERT INTO course_controls (course_id, position, code) VALUES (?1, ?2, ?3)",
    )?;
    for (pos, code) in course.controls.iter().enumerate() {
        stmt.execute(params![course_id, position(pos)?, code])?;
    }
    Ok(course_id)
}

fn insert_result(
    conn: &Connection,
    event_id: i64,
    class_id: i64,
    result_pos: usize,
    result: &RaceResult,
) -> Result<ResultId, StoreError> {
    let runner = &result.runner;
    conn.prepare_cached(
        "INSERT INTO runners \
         (event_id, given_name, family_name, club, bib, si_card, sex) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(params![
        event_id,
        runner.given_name,
        runner.family_name,
        runner.club,
        runner.bib,
        runner.si_card,
        runner.sex.map(sex_to_sql),
    ])?;
    let runner_id = conn.last_insert_rowid();

    let (status, status_code) = status_to_sql(result.status);
    conn.prepare_cached(
        "INSERT INTO results (class_id, position, runner_id, status, status_code, place) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(params![
        class_id,
        position(result_pos)?,
        runner_id,
        status,
        status_code,
        result.place
    ])?;
    let result_id = conn.last_insert_rowid();

    let mut stmt = conn.prepare_cached(
        "INSERT INTO punches (result_id, position, code, time_epoch_ms) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (pos, punch) in result.punches.iter().enumerate() {
        let time = punch.time.map(instant_to_ms).transpose()?;
        stmt.execute(params![result_id, position(pos)?, punch.code, time])?;
    }
    Ok(ResultId(result_id))
}

fn load_course(conn: &Connection, course_id: i64) -> Result<Course, StoreError> {
    let controls = conn
        .prepare_cached("SELECT code FROM course_controls WHERE course_id = ?1 ORDER BY position")?
        .query_map([course_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Course { controls })
}

fn load_results(conn: &Connection, class_id: i64) -> Result<Vec<RaceResult>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT r.id, r.status, r.status_code, r.place, \
                ru.given_name, ru.family_name, ru.club, ru.bib, ru.si_card, ru.sex \
         FROM results r JOIN runners ru ON ru.id = r.runner_id \
         WHERE r.class_id = ?1 ORDER BY r.position",
    )?;
    let mut rows = stmt.query([class_id])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        let result_id: i64 = row.get(0)?;
        let status: String = row.get(1)?;
        let sex: Option<String> = row.get(9)?;
        results.push(RaceResult {
            runner: Runner {
                given_name: row.get(4)?,
                family_name: row.get(5)?,
                club: row.get(6)?,
                bib: row.get(7)?,
                si_card: row.get(8)?,
                sex: sex.as_deref().map(sql_to_sex).transpose()?,
            },
            status: sql_to_status(&status, row.get(2)?)?,
            place: row.get(3)?,
            punches: load_punches(conn, result_id)?,
        });
    }
    Ok(results)
}

fn load_punches(conn: &Connection, result_id: i64) -> Result<Vec<Punch>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT code, time_epoch_ms FROM punches WHERE result_id = ?1 ORDER BY position",
    )?;
    let mut rows = stmt.query([result_id])?;
    let mut punches = Vec::new();
    while let Some(row) = rows.next()? {
        let time: Option<i64> = row.get(1)?;
        punches.push(Punch {
            code: row.get(0)?,
            time: time.map(ms_to_instant).transpose()?,
        });
    }
    Ok(punches)
}
