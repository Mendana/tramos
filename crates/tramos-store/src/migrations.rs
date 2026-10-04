//! Migraciones versionadas del esquema.
//!
//! Cada script de `migrations/` se aplica una sola vez, en orden. La versión aplicada se guarda
//! en `PRAGMA user_version`: tras la migración `n` (contando desde 1) vale `n`. Las migraciones
//! publicadas no se editan; los cambios van en un script nuevo al final de la lista.

use rusqlite::{Connection, TransactionBehavior};

use crate::StoreError;

/// Scripts SQL en orden. No pueden abrir ni cerrar transacciones: ya van dentro de una.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_initial.sql")];

/// Versión del esquema que deja `migrate`.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

pub(crate) fn user_version(conn: &Connection) -> Result<i64, StoreError> {
    Ok(conn.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

/// Aplica las migraciones pendientes en una sola transacción. Si alguna falla no se aplica
/// ninguna. Llamarla con el esquema al día no hace nada.
pub(crate) fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current = user_version(&tx)?;
    if current > SCHEMA_VERSION {
        return Err(StoreError::SchemaTooNew {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    let applied = usize::try_from(current)
        .map_err(|_| StoreError::InvalidData(format!("user_version negativa: {current}")))?;
    if applied == MIGRATIONS.len() {
        return Ok(());
    }
    for script in &MIGRATIONS[applied..] {
        tx.execute_batch(script)?;
    }
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLES: [&str; 14] = [
        "classes",
        "course_controls",
        "courses",
        "events",
        "legs",
        "punches",
        "results",
        "runners",
        "settings",
        "source_files",
        "tag_causes",
        "tags",
        "track_points",
        "tracks",
    ];

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_schema WHERE type = 'table' \
                 AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn fresh_database_gets_every_table_and_version_1() {
        let mut conn = Connection::open_in_memory().unwrap();
        assert_eq!(user_version(&conn).unwrap(), 0);
        assert!(table_names(&conn).is_empty());

        migrate(&mut conn).unwrap();

        assert_eq!(SCHEMA_VERSION, 1);
        assert_eq!(user_version(&conn).unwrap(), 1);
        assert_eq!(table_names(&conn), TABLES.to_vec());
    }

    #[test]
    fn migrating_twice_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn.execute("INSERT INTO settings (key, value) VALUES ('a', 'b')", [])
            .unwrap();

        migrate(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), 1);
        assert_eq!(table_names(&conn), TABLES.to_vec());
        let value: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'a'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(value, "b");
    }

    #[test]
    fn newer_schema_is_rejected() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        let err = migrate(&mut conn).unwrap_err();
        assert!(
            matches!(
                err,
                StoreError::SchemaTooNew {
                    found: 99,
                    supported: 1
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn failed_migration_leaves_database_untouched() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Una tabla con el mismo nombre hace fallar el script a mitad.
        conn.execute_batch("CREATE TABLE runners (x INTEGER)")
            .unwrap();
        assert!(migrate(&mut conn).is_err());
        assert_eq!(user_version(&conn).unwrap(), 0);
        assert_eq!(table_names(&conn), vec!["runners".to_string()]);
    }
}
