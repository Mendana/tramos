//! Grupos de atletas de quien entrena (#120, `docs/almacenamiento.md`): nombre, descripción,
//! color y miembros por `runner_id` de los paquetes.

use chrono::Utc;
use rusqlite::params;

use crate::{Store, StoreError};

/// Identificador de un grupo de atletas guardado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AthleteGroupId(pub i64);

/// Un grupo de atletas con sus miembros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AthleteGroup {
    pub id: AthleteGroupId,
    pub name: String,
    pub description: String,
    /// `#rrggbb`.
    pub color: String,
    /// `runner_id` de los paquetes de sus miembros, en orden.
    pub members: Vec<String>,
}

impl Store {
    /// Crea un grupo vacío. El nombre y la descripción se guardan sin espacios en los extremos.
    pub fn create_athlete_group(
        &mut self,
        name: &str,
        description: &str,
        color: &str,
    ) -> Result<AthleteGroupId, StoreError> {
        let (name, description, color) = valid_group(name, description, color)?;
        self.conn.execute(
            "INSERT INTO athlete_groups (name, description, color, created_at_epoch_ms) \
             VALUES (?1, ?2, ?3, ?4)",
            params![name, description, color, Utc::now().timestamp_millis()],
        )?;
        Ok(AthleteGroupId(self.conn.last_insert_rowid()))
    }

    /// Cambia el nombre, la descripción y el color de un grupo; los miembros no cambian.
    pub fn update_athlete_group(
        &mut self,
        id: AthleteGroupId,
        name: &str,
        description: &str,
        color: &str,
    ) -> Result<(), StoreError> {
        let (name, description, color) = valid_group(name, description, color)?;
        let changed = self.conn.execute(
            "UPDATE athlete_groups SET name = ?2, description = ?3, color = ?4 WHERE id = ?1",
            params![id.0, name, description, color],
        )?;
        if changed == 0 {
            return Err(StoreError::GroupNotFound(id.0));
        }
        Ok(())
    }

    /// Borra un grupo y sus filas de miembros. Los paquetes recibidos no se tocan.
    pub fn delete_athlete_group(&mut self, id: AthleteGroupId) -> Result<(), StoreError> {
        let changed = self
            .conn
            .execute("DELETE FROM athlete_groups WHERE id = ?1", [id.0])?;
        if changed == 0 {
            return Err(StoreError::GroupNotFound(id.0));
        }
        Ok(())
    }

    /// Mete a un atleta en un grupo (si ya está, no cambia nada).
    pub fn add_athlete_group_member(
        &mut self,
        id: AthleteGroupId,
        runner_id: &str,
    ) -> Result<(), StoreError> {
        self.group_exists(id)?;
        self.conn.execute(
            "INSERT OR IGNORE INTO athlete_group_members (group_id, runner_id) VALUES (?1, ?2)",
            params![id.0, runner_id],
        )?;
        Ok(())
    }

    /// Saca a un atleta de un grupo (si no estaba, no cambia nada).
    pub fn remove_athlete_group_member(
        &mut self,
        id: AthleteGroupId,
        runner_id: &str,
    ) -> Result<(), StoreError> {
        self.group_exists(id)?;
        self.conn.execute(
            "DELETE FROM athlete_group_members WHERE group_id = ?1 AND runner_id = ?2",
            params![id.0, runner_id],
        )?;
        Ok(())
    }

    /// Todos los grupos, por nombre (y, a igual nombre, en orden de creación), con sus miembros.
    pub fn athlete_groups(&self) -> Result<Vec<AthleteGroup>, StoreError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, name, description, color FROM athlete_groups ORDER BY name, id",
        )?;
        let mut rows = stmt.query([])?;
        let mut groups = Vec::new();
        while let Some(row) = rows.next()? {
            groups.push(AthleteGroup {
                id: AthleteGroupId(row.get(0)?),
                name: row.get(1)?,
                description: row.get(2)?,
                color: row.get(3)?,
                members: Vec::new(),
            });
        }
        let mut stmt = self.conn.prepare_cached(
            "SELECT runner_id FROM athlete_group_members WHERE group_id = ?1 ORDER BY runner_id",
        )?;
        for group in &mut groups {
            let mut rows = stmt.query([group.id.0])?;
            while let Some(row) = rows.next()? {
                group.members.push(row.get(0)?);
            }
        }
        Ok(groups)
    }

    fn group_exists(&self, id: AthleteGroupId) -> Result<(), StoreError> {
        let found: bool = self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM athlete_groups WHERE id = ?1)",
            [id.0],
            |row| row.get(0),
        )?;
        if found {
            Ok(())
        } else {
            Err(StoreError::GroupNotFound(id.0))
        }
    }
}

/// Nombre y descripción sin espacios en los extremos y el color en minúsculas, o el error.
fn valid_group<'a>(
    name: &'a str,
    description: &'a str,
    color: &str,
) -> Result<(&'a str, &'a str, String), StoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(StoreError::EmptyGroupName);
    }
    let hex = color.strip_prefix('#').unwrap_or("");
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(StoreError::InvalidColor(color.to_string()));
    }
    Ok((name, description.trim(), color.to_ascii_lowercase()))
}
