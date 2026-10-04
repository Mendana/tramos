//! Personas: identidad del corredor entre carreras, con datos sintéticos.

// `allow-unwrap-in-tests` (clippy.toml) no cubre los helpers de un test de integración.
#![allow(clippy::unwrap_used)]

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use rusqlite::Connection;
use tramos_core::model::{
    Class, Course, Event, FINISH_CODE, Punch, RaceResult, RaceStatus, Runner, START_CODE,
};
use tramos_store::{
    EventId, PersonId, PersonResult, ResultId, SCHEMA_VERSION, SavedEvent, Store, StoreError,
};

fn result(given: &str, status: RaceStatus, place: Option<u16>) -> RaceResult {
    RaceResult {
        runner: Runner {
            given_name: given.into(),
            family_name: "Sintética".into(),
            club: None,
            bib: None,
            si_card: None,
            sex: None,
        },
        status,
        place,
        punches: vec![],
    }
}

/// Carrera sintética con dos categorías: F21A (Ana, Berta) y F21B (Carla).
fn event(
    name: Option<&str>,
    date: (i32, u32, u32),
    ana: RaceStatus,
    ana_place: Option<u16>,
) -> Event {
    let course = Course {
        controls: vec![31, 45],
    };
    Event {
        name: name.map(Into::into),
        date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
        classes: vec![
            Class {
                id: 1,
                name: "F21A".into(),
                short_name: None,
                course: course.clone(),
                results: vec![
                    result("Ana", ana, ana_place),
                    result("Berta", RaceStatus::Ok, Some(1)),
                ],
            },
            Class {
                id: 2,
                name: "F21B".into(),
                short_name: None,
                course,
                results: vec![result("Carla", RaceStatus::Ok, Some(1))],
            },
        ],
    }
}

/// Guarda dos carreras en orden inverso de fecha: primero la de noviembre, luego la de octubre.
fn two_events(store: &mut Store) -> (SavedEvent, SavedEvent) {
    let november = store
        .save_event(
            &event(None, (2026, 11, 8), RaceStatus::NotClassified, None),
            None,
        )
        .unwrap();
    let october = store
        .save_event(
            &event(
                Some("Sintética de Otoño"),
                (2026, 10, 3),
                RaceStatus::Ok,
                Some(2),
            ),
            None,
        )
        .unwrap();
    (november, october)
}

#[test]
fn person_results_come_back_in_race_date_order() {
    let mut store = Store::open_in_memory().unwrap();
    let (november, october) = two_events(&mut store);
    let ana = store.create_person("Ana", None).unwrap();
    let berta = store.create_person("Berta", None).unwrap();

    // Se vincula primero la carrera más reciente.
    store.link_result(november.results[0][0], ana).unwrap();
    store.link_result(october.results[0][0], ana).unwrap();
    store.link_result(october.results[0][1], berta).unwrap();

    assert_eq!(
        store.person_results(ana).unwrap(),
        vec![
            PersonResult {
                result: october.results[0][0],
                event: october.id,
                event_date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
                event_name: Some("Sintética de Otoño".into()),
                event_start: None,
                class_name: "F21A".into(),
                status: RaceStatus::Ok,
                place: Some(2),
            },
            PersonResult {
                result: november.results[0][0],
                event: november.id,
                event_date: NaiveDate::from_ymd_opt(2026, 11, 8).unwrap(),
                event_name: None,
                event_start: None,
                class_name: "F21A".into(),
                status: RaceStatus::NotClassified,
                place: None,
            },
        ]
    );
    let berta_results = store.person_results(berta).unwrap();
    assert_eq!(berta_results.len(), 1);
    assert_eq!(berta_results[0].result, october.results[0][1]);

    assert_eq!(
        store.result_person(october.results[0][0]).unwrap(),
        Some(ana)
    );
    assert_eq!(store.result_person(october.results[1][0]).unwrap(), None);
    // Vincular no cambia la carrera.
    assert_eq!(
        store.load_event(october.id).unwrap(),
        event(
            Some("Sintética de Otoño"),
            (2026, 10, 3),
            RaceStatus::Ok,
            Some(2)
        )
    );
}

/// La carrera de [`event`] con picadas: cada resultado sale a `start` + 1 min por resultado,
/// pica la 31 sin hora y la 45 y la meta con hora.
fn event_with_punches(date: (i32, u32, u32), start: DateTime<Utc>) -> Event {
    let mut event = event(None, date, RaceStatus::Ok, Some(1));
    let mut offset = 0;
    for class in &mut event.classes {
        for result in &mut class.results {
            let t = start + Duration::minutes(offset);
            offset += 1;
            result.punches = vec![
                Punch {
                    code: START_CODE,
                    time: Some(t),
                },
                Punch {
                    code: 31,
                    time: None,
                },
                Punch {
                    code: 45,
                    time: Some(t + Duration::seconds(300)),
                },
                Punch {
                    code: FINISH_CODE,
                    time: Some(t + Duration::seconds(400)),
                },
            ];
        }
    }
    event
}

#[test]
fn same_date_is_ordered_by_first_start() {
    let mut store = Store::open_in_memory().unwrap();
    let day = (2026, 10, 3);
    let morning_start = Utc.with_ymd_and_hms(2026, 10, 3, 8, 30, 0).unwrap();
    let afternoon_start = Utc.with_ymd_and_hms(2026, 10, 3, 14, 0, 0).unwrap();
    // Se guarda primero la de la tarde, luego una sin horas y al final la de la mañana.
    let afternoon = store
        .save_event(&event_with_punches(day, afternoon_start), None)
        .unwrap();
    let untimed = store
        .save_event(&event(None, day, RaceStatus::Ok, Some(1)), None)
        .unwrap();
    let morning = store
        .save_event(&event_with_punches(day, morning_start), None)
        .unwrap();
    // Y una del día anterior sin horas: la fecha manda sobre el inicio.
    let day_before = store
        .save_event(&event(None, (2026, 10, 2), RaceStatus::Ok, Some(1)), None)
        .unwrap();

    assert_eq!(store.event_start(morning.id).unwrap(), Some(morning_start));
    assert_eq!(
        store.event_start(afternoon.id).unwrap(),
        Some(afternoon_start)
    );
    assert_eq!(store.event_start(untimed.id).unwrap(), None);
    assert!(matches!(
        store.event_start(EventId(9_999)),
        Err(StoreError::EventNotFound(9_999))
    ));

    let person = store.create_person("Comodín", None).unwrap();
    for saved in [&afternoon, &untimed, &morning, &day_before] {
        // El último resultado (Carla) sale el último de su carrera: el inicio es el de Ana.
        store.link_result(saved.results[1][0], person).unwrap();
    }
    let results = store.person_results(person).unwrap();
    let order: Vec<EventId> = results.iter().map(|r| r.event).collect();
    assert_eq!(
        order,
        vec![day_before.id, morning.id, afternoon.id, untimed.id]
    );
    assert_eq!(results[1].event_start, Some(morning_start));
    assert_eq!(results[3].event_start, None);
}

#[test]
fn same_date_is_ordered_by_save_order_then_class_and_result() {
    let mut store = Store::open_in_memory().unwrap();
    let morning = store
        .save_event(&event(None, (2026, 10, 3), RaceStatus::Ok, Some(1)), None)
        .unwrap();
    let afternoon = store
        .save_event(&event(None, (2026, 10, 3), RaceStatus::Ok, Some(1)), None)
        .unwrap();
    let person = store.create_person("Comodín", None).unwrap();
    for id in [
        afternoon.results[0][0],
        morning.results[1][0],
        morning.results[0][1],
    ] {
        store.link_result(id, person).unwrap();
    }

    let ids: Vec<ResultId> = store
        .person_results(person)
        .unwrap()
        .into_iter()
        .map(|r| r.result)
        .collect();
    assert_eq!(
        ids,
        vec![
            morning.results[0][1],
            morning.results[1][0],
            afternoon.results[0][0]
        ]
    );
}

#[test]
fn people_are_listed_in_creation_order_with_their_data() {
    let mut store = Store::open_in_memory().unwrap();
    assert!(store.people().unwrap().is_empty());
    let before = Utc::now();

    let yo = store
        .create_person("Yo", Some("tarjeta SI nueva desde 2026"))
        .unwrap();
    let ana = store.create_person("Ana P.", None).unwrap();

    let people = store.people().unwrap();
    assert_eq!(people.len(), 2);
    assert_eq!(people[0].id, yo);
    assert_eq!(people[0].display_name, "Yo");
    assert_eq!(
        people[0].notes.as_deref(),
        Some("tarjeta SI nueva desde 2026")
    );
    assert_eq!(people[1].id, ana);
    assert_eq!(people[1].display_name, "Ana P.");
    assert_eq!(people[1].notes, None);
    for person in &people {
        assert!(person.created_at >= before - Duration::milliseconds(1));
        assert!(person.created_at <= Utc::now());
    }
    // Sin resultados vinculados, la lista está vacía.
    assert!(store.person_results(yo).unwrap().is_empty());
}

#[test]
fn empty_names_are_rejected() {
    let mut store = Store::open_in_memory().unwrap();
    for name in ["", "   ", "\t\n"] {
        assert!(matches!(
            store.create_person(name, None),
            Err(StoreError::EmptyPersonName)
        ));
    }
    assert!(store.people().unwrap().is_empty());
}

#[test]
fn a_linked_result_is_not_reassigned_silently() {
    let mut store = Store::open_in_memory().unwrap();
    let (_, october) = two_events(&mut store);
    let ana = store.create_person("Ana", None).unwrap();
    let berta = store.create_person("Berta", None).unwrap();
    let result = october.results[0][0];
    store.link_result(result, ana).unwrap();

    // Volver a vincular con la misma persona no hace nada.
    store.link_result(result, ana).unwrap();
    assert_eq!(store.person_results(ana).unwrap().len(), 1);

    // Con otra persona es un error y el vínculo no cambia.
    let err = store.link_result(result, berta).unwrap_err();
    assert!(
        matches!(err, StoreError::ResultAlreadyLinked { result: r, person: p }
            if r == result.0 && p == ana.0),
        "{err:?}"
    );
    assert_eq!(store.result_person(result).unwrap(), Some(ana));
    assert!(store.person_results(berta).unwrap().is_empty());

    // Para reasignar: desvincular y vincular.
    store.unlink_result(result).unwrap();
    assert_eq!(store.result_person(result).unwrap(), None);
    assert!(store.person_results(ana).unwrap().is_empty());
    // Desvincular un resultado sin persona no hace nada.
    store.unlink_result(result).unwrap();
    store.link_result(result, berta).unwrap();
    assert_eq!(store.result_person(result).unwrap(), Some(berta));
}

#[test]
fn deleting_a_person_unlinks_but_keeps_results() {
    let mut store = Store::open_in_memory().unwrap();
    let (november, october) = two_events(&mut store);
    let ana = store.create_person("Ana", None).unwrap();
    let berta = store.create_person("Berta", None).unwrap();
    store.link_result(november.results[0][0], ana).unwrap();
    store.link_result(october.results[0][0], ana).unwrap();
    store.link_result(october.results[0][1], berta).unwrap();

    store.delete_person(ana).unwrap();

    assert_eq!(store.people().unwrap().len(), 1);
    assert_eq!(store.result_person(november.results[0][0]).unwrap(), None);
    assert_eq!(store.result_person(october.results[0][0]).unwrap(), None);
    assert_eq!(
        store.result_person(october.results[0][1]).unwrap(),
        Some(berta)
    );
    assert_eq!(store.result_ids(october.id).unwrap(), october.results);
    assert_eq!(
        store.load_event(november.id).unwrap(),
        event(None, (2026, 11, 8), RaceStatus::NotClassified, None)
    );
    assert!(matches!(
        store.person_results(ana),
        Err(StoreError::PersonNotFound(_))
    ));
    assert!(matches!(
        store.delete_person(ana),
        Err(StoreError::PersonNotFound(_))
    ));
}

#[test]
fn updating_a_person_replaces_name_and_notes_only() {
    let mut store = Store::open_in_memory().unwrap();
    let (_, october) = two_events(&mut store);
    let ana = store.create_person("Ana", Some("zurda")).unwrap();
    let berta = store.create_person("Berta", None).unwrap();
    store.link_result(october.results[0][0], ana).unwrap();
    let before = store.people().unwrap();

    store
        .update_person(ana, "Ana Sintética", Some("lee mal las curvas"))
        .unwrap();
    let after = store.people().unwrap();
    assert_eq!(after[0].id, ana);
    assert_eq!(after[0].display_name, "Ana Sintética");
    assert_eq!(after[0].notes.as_deref(), Some("lee mal las curvas"));
    assert_eq!(after[0].created_at, before[0].created_at);
    assert_eq!(after[1], before[1]); // Berta no cambia
    assert_eq!(
        store.result_person(october.results[0][0]).unwrap(),
        Some(ana)
    );

    // `None` borra las notas.
    store.update_person(ana, "Ana Sintética", None).unwrap();
    assert_eq!(store.people().unwrap()[0].notes, None);

    // Nombre vacío: error y nada cambia.
    for name in ["", "  \t"] {
        assert!(matches!(
            store.update_person(berta, name, Some("x")),
            Err(StoreError::EmptyPersonName)
        ));
    }
    assert_eq!(store.people().unwrap()[1], before[1]);
}

#[test]
fn missing_people_and_results_are_errors() {
    let mut store = Store::open_in_memory().unwrap();
    let (_, october) = two_events(&mut store);
    let ana = store.create_person("Ana", None).unwrap();

    assert!(matches!(
        store.link_result(october.results[0][0], PersonId(9_999)),
        Err(StoreError::PersonNotFound(9_999))
    ));
    assert!(matches!(
        store.link_result(ResultId(9_999), ana),
        Err(StoreError::ResultNotFound(9_999))
    ));
    assert!(matches!(
        store.unlink_result(ResultId(9_999)),
        Err(StoreError::ResultNotFound(9_999))
    ));
    assert!(matches!(
        store.result_person(ResultId(9_999)),
        Err(StoreError::ResultNotFound(9_999))
    ));
    assert!(matches!(
        store.person_results(PersonId(9_999)),
        Err(StoreError::PersonNotFound(9_999))
    ));
    assert!(matches!(
        store.update_person(PersonId(9_999), "Nadie", None),
        Err(StoreError::PersonNotFound(9_999))
    ));
    assert_eq!(store.result_person(october.results[0][0]).unwrap(), None);
}

#[test]
fn version_1_database_is_migrated_and_can_link_people() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tramos.sqlite");
    {
        // Base tal y como la dejaba Tramos con el esquema v1.
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(include_str!("../migrations/0001_initial.sql"))
            .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute_batch(
            "INSERT INTO events (id, name, date) VALUES (7, 'Antigua', '2025-05-04');
             INSERT INTO courses (id, event_id) VALUES (1, 7);
             INSERT INTO course_controls (course_id, position, code) VALUES (1, 0, 31);
             INSERT INTO classes (id, event_id, position, source_id, name, course_id)
                 VALUES (1, 7, 0, 1, 'F21A', 1);
             INSERT INTO runners (id, event_id, source_id, given_name, family_name)
                 VALUES (1, 7, 1, 'Ana', 'Sintética');
             INSERT INTO results (id, class_id, position, runner_id, status, place)
                 VALUES (5, 1, 0, 1, 'ok', 3);",
        )
        .unwrap();
    }

    let mut store = Store::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert!(store.people().unwrap().is_empty());
    let old = store.load_event(EventId(7)).unwrap();
    assert_eq!(old.name.as_deref(), Some("Antigua"));
    assert_eq!(old.classes[0].results[0].place, Some(3));
    assert_eq!(store.result_person(ResultId(5)).unwrap(), None);

    // El resultado antiguo se puede vincular junto a uno nuevo, y salen en orden de fecha.
    let new = store
        .save_event(&event(None, (2026, 10, 3), RaceStatus::Ok, Some(1)), None)
        .unwrap();
    let ana = store.create_person("Ana", None).unwrap();
    store.link_result(new.results[0][0], ana).unwrap();
    store.link_result(ResultId(5), ana).unwrap();
    let dates: Vec<NaiveDate> = store
        .person_results(ana)
        .unwrap()
        .into_iter()
        .map(|r| r.event_date)
        .collect();
    assert_eq!(
        dates,
        vec![
            NaiveDate::from_ymd_opt(2025, 5, 4).unwrap(),
            NaiveDate::from_ymd_opt(2026, 10, 3).unwrap()
        ]
    );
    drop(store);

    // Reabrir no vuelve a migrar y conserva los vínculos.
    let store = Store::open(&path).unwrap();
    assert_eq!(store.result_person(ResultId(5)).unwrap(), Some(ana));
}
