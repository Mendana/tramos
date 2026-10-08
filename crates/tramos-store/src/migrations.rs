//! Migraciones versionadas del esquema.
//!
//! Cada script de `migrations/` se aplica una sola vez, en orden. La versión aplicada se guarda
//! en `PRAGMA user_version`: tras la migración `n` (contando desde 1) vale `n`. Las migraciones
//! publicadas no se editan; los cambios van en un script nuevo al final de la lista.

use rusqlite::{Connection, TransactionBehavior};

use crate::StoreError;

/// Scripts SQL en orden. No pueden abrir ni cerrar transacciones: ya van dentro de una.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_initial.sql"),
    include_str!("../migrations/0002_people.sql"),
    include_str!("../migrations/0003_track_sport.sql"),
    include_str!("../migrations/0004_drop_runner_source_id.sql"),
    include_str!("../migrations/0005_event_format.sql"),
    include_str!("../migrations/0006_track_manual_offset.sql"),
    include_str!("../migrations/0007_received_packages.sql"),
    include_str!("../migrations/0008_result_sharing.sql"),
];

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

    const TABLES: [&str; 17] = [
        "classes",
        "course_controls",
        "courses",
        "events",
        "legs",
        "people",
        "punches",
        "received_packages",
        "result_sharing",
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
    fn fresh_database_gets_every_table_and_latest_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        assert_eq!(user_version(&conn).unwrap(), 0);
        assert!(table_names(&conn).is_empty());

        migrate(&mut conn).unwrap();

        assert_eq!(SCHEMA_VERSION, 8);
        assert_eq!(user_version(&conn).unwrap(), 8);
        assert_eq!(table_names(&conn), TABLES.to_vec());
    }

    #[test]
    fn migrating_twice_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn.execute("INSERT INTO settings (key, value) VALUES ('a', 'b')", [])
            .unwrap();

        migrate(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
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
                    supported: SCHEMA_VERSION
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

    /// Deja la base como la dejaba una versión de Tramos con solo las `n` primeras migraciones.
    fn migrate_to(conn: &Connection, n: usize) {
        for script in &MIGRATIONS[..n] {
            conn.execute_batch(script).unwrap();
        }
        conn.pragma_update(None, "user_version", n as i64).unwrap();
    }

    /// Columnas de una tabla, en orden.
    fn columns(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT name FROM pragma_table_info('{table}') ORDER BY cid"
            ))
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn version_5_tracks_stay_automatic() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrate_to(&conn, 5);
        conn.execute_batch(
            "INSERT INTO events (id, name, date) VALUES (1, 'Vieja', '2025-05-04');
             INSERT INTO courses (id, event_id) VALUES (1, 1);
             INSERT INTO classes (id, event_id, position, source_id, name, course_id)
                 VALUES (1, 1, 0, 1, 'F21A', 1);
             INSERT INTO runners (id, event_id, given_name, family_name)
                 VALUES (1, 1, 'Ana', 'Pérez');
             INSERT INTO results (id, class_id, position, runner_id, status, place)
                 VALUES (1, 1, 0, 1, 'ok', 3);
             INSERT INTO tracks (id, result_id, sport) VALUES (1, 1, 'running');",
        )
        .unwrap();

        migrate(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
        let (sport, offset): (Option<String>, Option<f64>) = conn
            .query_row(
                "SELECT sport, manual_offset_s FROM tracks WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((sport.as_deref(), offset), (Some("running"), None));
    }

    #[test]
    fn version_4_events_gain_an_empty_format() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_to(&conn, 4);
        conn.execute(
            "INSERT INTO events (id, name, date) VALUES (1, 'Vieja', '2025-05-04')",
            [],
        )
        .unwrap();

        migrate(&mut conn).unwrap();

        let format: Option<String> = conn
            .query_row("SELECT format FROM events WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(format, None);
        // Solo los tres formatos.
        assert!(
            conn.execute("UPDATE events SET format = 'ultra' WHERE id = 1", [])
                .is_err()
        );
    }

    #[test]
    fn version_3_runners_lose_source_id_and_keep_the_rest() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrate_to(&conn, 3);
        conn.execute_batch(
            "INSERT INTO events (id, name, date) VALUES (1, 'Vieja', '2025-05-04');
             INSERT INTO courses (id, event_id) VALUES (1, 1);
             INSERT INTO classes (id, event_id, position, source_id, name, course_id)
                 VALUES (1, 1, 0, 1, 'F21A', 1);
             INSERT INTO runners (id, event_id, source_id, given_name, family_name, bib)
                 VALUES (1, 1, 154, 'Ana', 'Pérez', 7);
             INSERT INTO results (id, class_id, position, runner_id, status, place)
                 VALUES (1, 1, 0, 1, 'ok', 3);",
        )
        .unwrap();

        migrate(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
        assert!(!columns(&conn, "runners").contains(&"source_id".to_string()));
        // La de categorías es el id de categoría del fichero y se queda.
        assert!(columns(&conn, "classes").contains(&"source_id".to_string()));
        let (given, bib, place): (String, i64, i64) = conn
            .query_row(
                "SELECT ru.given_name, ru.bib, r.place FROM results r \
                 JOIN runners ru ON ru.id = r.runner_id WHERE r.id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((given.as_str(), bib, place), ("Ana", 7, 3));
    }

    #[test]
    fn version_2_tracks_gain_an_empty_sport() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrate_to(&conn, 2);
        conn.execute_batch(
            "INSERT INTO events (id, name, date) VALUES (1, 'Vieja', '2025-05-04');
             INSERT INTO courses (id, event_id) VALUES (1, 1);
             INSERT INTO classes (id, event_id, position, source_id, name, course_id)
                 VALUES (1, 1, 0, 1, 'F21A', 1);
             INSERT INTO runners (id, event_id, source_id, given_name, family_name)
                 VALUES (1, 1, 1, 'Ana', 'Pérez');
             INSERT INTO results (id, class_id, position, runner_id, status, place)
                 VALUES (1, 1, 0, 1, 'ok', 3);
             INSERT INTO tracks (id, result_id) VALUES (1, 1);",
        )
        .unwrap();

        migrate(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
        let sport: Option<String> = conn
            .query_row("SELECT sport FROM tracks WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sport, None);
    }

    #[test]
    fn version_1_database_gains_people_and_keeps_its_results() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrate_to(&conn, 1);
        conn.execute_batch(
            "INSERT INTO events (id, name, date) VALUES (1, 'Vieja', '2025-05-04');
             INSERT INTO courses (id, event_id) VALUES (1, 1);
             INSERT INTO classes (id, event_id, position, source_id, name, course_id)
                 VALUES (1, 1, 0, 1, 'F21A', 1);
             INSERT INTO runners (id, event_id, source_id, given_name, family_name)
                 VALUES (1, 1, 1, 'Ana', 'Pérez');
             INSERT INTO results (id, class_id, position, runner_id, status, place)
                 VALUES (1, 1, 0, 1, 'ok', 3);",
        )
        .unwrap();

        migrate(&mut conn).unwrap();

        assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
        assert_eq!(table_names(&conn), TABLES.to_vec());
        let (place, person): (i64, Option<i64>) = conn
            .query_row(
                "SELECT place, person_id FROM results WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((place, person), (3, None));
    }
}
