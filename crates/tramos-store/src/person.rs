//! Personas: identidad de un corredor entre carreras.
//!
//! `runners` es por carrera, tal y como aparece en cada .spl. Una persona agrupa los resultados
//! que el usuario sabe que son de la misma persona. El vínculo lo decide el usuario: aquí no se
//! adivina quién es quién. Una persona solo guarda lo que escribe el usuario (nombre visible y
//! notas); nunca la fecha de nacimiento (ver `docs/datos-y-privacidad.md`).

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use tramos_core::model::RaceStatus;

use crate::convert::{ms_to_instant, sql_to_status, text_to_date};
use crate::{EventId, ResultId, Store, StoreError};

/// Identificador de una persona guardada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PersonId(pub i64);

/// Persona tal y como la escribió el usuario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub id: PersonId,
    /// Nombre que se muestra en la app.
    pub display_name: String,
    /// Texto libre del usuario.
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Resultado vinculado a una persona, con lo necesario para listarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonResult {
    pub result: ResultId,
    pub event: EventId,
    /// Día de la carrera (fecha local).
    pub event_date: NaiveDate,
    /// Nombre de la carrera, si la fuente lo trae.
    pub event_name: Option<String>,
    /// Nombre de la categoría.
    pub class_name: String,
    pub status: RaceStatus,
    /// Puesto en la categoría; solo los clasificados lo tienen.
    pub place: Option<u16>,
}

impl Store {
    /// Crea una persona. `display_name` no puede estar vacío ni ser solo espacios; se guarda tal
    /// cual se escribe.
    pub fn create_person(
        &mut self,
        display_name: &str,
        notes: Option<&str>,
    ) -> Result<PersonId, StoreError> {
        if display_name.trim().is_empty() {
            return Err(StoreError::EmptyPersonName);
        }
        self.conn.execute(
            "INSERT INTO people (display_name, notes, created_at_epoch_ms) VALUES (?1, ?2, ?3)",
            params![display_name, notes, Utc::now().timestamp_millis()],
        )?;
        Ok(PersonId(self.conn.last_insert_rowid()))
    }

    /// Todas las personas, en orden de creación.
    pub fn people(&self) -> Result<Vec<Person>, StoreError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, display_name, notes, created_at_epoch_ms FROM people ORDER BY id",
        )?;
        let mut rows = stmt.query([])?;
        let mut people = Vec::new();
        while let Some(row) = rows.next()? {
            people.push(Person {
                id: PersonId(row.get(0)?),
                display_name: row.get(1)?,
                notes: row.get(2)?,
                created_at: ms_to_instant(row.get(3)?)?,
            });
        }
        Ok(people)
    }

    /// Cambia el nombre visible y las notas de una persona. Sustituye los dos: `notes = None`
    /// borra las notas. El nombre se valida como en [`Store::create_person`]; el instante de
    /// creación y los vínculos no cambian.
    pub fn update_person(
        &mut self,
        person: PersonId,
        display_name: &str,
        notes: Option<&str>,
    ) -> Result<(), StoreError> {
        if display_name.trim().is_empty() {
            return Err(StoreError::EmptyPersonName);
        }
        let updated = self.conn.execute(
            "UPDATE people SET display_name = ?1, notes = ?2 WHERE id = ?3",
            params![display_name, notes, person.0],
        )?;
        if updated == 0 {
            return Err(StoreError::PersonNotFound(person.0));
        }
        Ok(())
    }

    /// Borra una persona. Sus resultados quedan sin vincular; no se borra ninguno.
    pub fn delete_person(&mut self, person: PersonId) -> Result<(), StoreError> {
        let deleted = self
            .conn
            .execute("DELETE FROM people WHERE id = ?1", [person.0])?;
        if deleted == 0 {
            return Err(StoreError::PersonNotFound(person.0));
        }
        Ok(())
    }

    /// Vincula un resultado con una persona.
    ///
    /// Un resultado pertenece como mucho a una persona. Si ya está vinculado a esa misma persona
    /// no hace nada; si lo está a otra, falla con [`StoreError::ResultAlreadyLinked`] y no lo
    /// cambia: para reasignarlo hay que llamar antes a [`Store::unlink_result`].
    pub fn link_result(&mut self, result: ResultId, person: PersonId) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        ensure_person(&tx, person)?;
        match linked_person(&tx, result)? {
            Some(current) if current == person => {}
            Some(current) => {
                return Err(StoreError::ResultAlreadyLinked {
                    result: result.0,
                    person: current.0,
                });
            }
            None => {
                tx.execute(
                    "UPDATE results SET person_id = ?1 WHERE id = ?2",
                    params![person.0, result.0],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Desvincula un resultado de su persona. Si no estaba vinculado no hace nada.
    pub fn unlink_result(&mut self, result: ResultId) -> Result<(), StoreError> {
        let updated = self.conn.execute(
            "UPDATE results SET person_id = NULL WHERE id = ?1",
            [result.0],
        )?;
        if updated == 0 {
            return Err(StoreError::ResultNotFound(result.0));
        }
        Ok(())
    }

    /// Persona a la que está vinculado un resultado, si lo está.
    pub fn result_person(&self, result: ResultId) -> Result<Option<PersonId>, StoreError> {
        linked_person(&self.conn, result)
    }

    /// Resultados de una persona por fecha de carrera, de la más antigua a la más reciente.
    ///
    /// Con la misma fecha se ordenan por orden de guardado de la carrera y, dentro de ella, por
    /// el orden de categorías y resultados del modelo.
    pub fn person_results(&self, person: PersonId) -> Result<Vec<PersonResult>, StoreError> {
        ensure_person(&self.conn, person)?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT r.id, e.id, e.date, e.name, c.name, r.status, r.status_code, r.place \
             FROM results r \
             JOIN classes c ON c.id = r.class_id \
             JOIN events e ON e.id = c.event_id \
             WHERE r.person_id = ?1 \
             ORDER BY e.date, e.id, c.position, r.position",
        )?;
        let mut rows = stmt.query([person.0])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            let date: String = row.get(2)?;
            let status: String = row.get(5)?;
            results.push(PersonResult {
                result: ResultId(row.get(0)?),
                event: EventId(row.get(1)?),
                event_date: text_to_date(&date)?,
                event_name: row.get(3)?,
                class_name: row.get(4)?,
                status: sql_to_status(&status, row.get(6)?)?,
                place: row.get(7)?,
            });
        }
        Ok(results)
    }
}

/// Comprueba que existe una persona.
fn ensure_person(conn: &Connection, person: PersonId) -> Result<(), StoreError> {
    conn.query_row("SELECT 1 FROM people WHERE id = ?1", [person.0], |_| Ok(()))
        .optional()?
        .ok_or(StoreError::PersonNotFound(person.0))
}

/// Persona vinculada a un resultado; falla si el resultado no existe.
fn linked_person(conn: &Connection, result: ResultId) -> Result<Option<PersonId>, StoreError> {
    let person: Option<i64> = conn
        .query_row(
            "SELECT person_id FROM results WHERE id = ?1",
            [result.0],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::ResultNotFound(result.0))?;
    Ok(person.map(PersonId))
}
