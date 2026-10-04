//! Guardar y cargar carreras completas.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};
use tramos_core::model::{Class, Course, Event, Punch, RaceResult, Runner};

use crate::convert::{
    date_to_text, instant_to_ms, ms_to_instant, position, sex_to_sql, sql_to_sex, sql_to_status,
    status_to_sql, text_to_date,
};
use crate::{EventId, ResultId, Store, StoreError};

/// Identificadores asignados al guardar una carrera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedEvent {
    pub id: EventId,
    /// `results[c][r]` es el resultado `r` de la categoría `c`, en el orden de `Event`.
    pub results: Vec<Vec<ResultId>>,
}

impl Store {
    /// Guarda una carrera completa en una transacción: o se guarda entera o nada.
    ///
    /// Las categorías con la misma secuencia de balizas comparten una fila de `courses`.
    pub fn save_event(&mut self, event: &Event) -> Result<SavedEvent, StoreError> {
        let tx = self.conn.transaction()?;
        let saved = insert_event(&tx, event)?;
        tx.commit()?;
        Ok(saved)
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

fn insert_event(conn: &Connection, event: &Event) -> Result<SavedEvent, StoreError> {
    conn.execute(
        "INSERT INTO events (name, date) VALUES (?1, ?2)",
        params![event.name, date_to_text(event.date)],
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
         (event_id, source_id, given_name, family_name, club, bib, si_card, sex) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(params![
        event_id,
        runner.id,
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
                ru.source_id, ru.given_name, ru.family_name, ru.club, ru.bib, ru.si_card, ru.sex \
         FROM results r JOIN runners ru ON ru.id = r.runner_id \
         WHERE r.class_id = ?1 ORDER BY r.position",
    )?;
    let mut rows = stmt.query([class_id])?;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        let result_id: i64 = row.get(0)?;
        let status: String = row.get(1)?;
        let sex: Option<String> = row.get(10)?;
        results.push(RaceResult {
            runner: Runner {
                id: row.get(4)?,
                given_name: row.get(5)?,
                family_name: row.get(6)?,
                club: row.get(7)?,
                bib: row.get(8)?,
                si_card: row.get(9)?,
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
