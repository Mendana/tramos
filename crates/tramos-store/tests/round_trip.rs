//! Ida y vuelta del modelo a través de SQLite, con datos sintéticos.

// `allow-unwrap-in-tests` (clippy.toml) no cubre los helpers de un test de integración.
#![allow(clippy::unwrap_used)]

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use rusqlite::Connection;
use tramos_core::model::{
    Class, Course, Event, FINISH_CODE, Punch, RaceResult, RaceStatus, Runner, START_CODE, Sex,
    Track, TrackPoint,
};
use tramos_core::race_format::RaceFormat;
use tramos_store::{
    EventId, ResultId, SCHEMA_VERSION, SourceFileId, SourceFileKind, Store, StoreError,
};

fn utc(h: u32, m: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 3, h, m, s).unwrap()
}

fn punch(code: u16, time: Option<DateTime<Utc>>) -> Punch {
    Punch { code, time }
}

fn runner(n: u32, given: &str, family: &str) -> Runner {
    Runner {
        given_name: given.into(),
        family_name: family.into(),
        club: Some("Club Sintético".into()),
        bib: Some(100 + n),
        si_card: Some(2_000_000 + n),
        sex: Some(Sex::Female),
    }
}

/// Carrera sintética: F21A y F21B comparten recorrido; M21A tiene otro.
fn sample_event() -> Event {
    let shared = Course {
        controls: vec![31, 45, 100],
    };
    let other = Course {
        controls: vec![31, 52, 47, 100],
    };
    Event {
        name: Some("Sintética de Otoño".into()),
        date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
        classes: vec![
            Class {
                id: 1,
                name: "F21A".into(),
                short_name: Some("F21A".into()),
                course: shared.clone(),
                results: vec![
                    RaceResult {
                        runner: runner(1, "Ana", "Pérez"),
                        status: RaceStatus::Ok,
                        place: Some(1),
                        punches: vec![
                            punch(START_CODE, Some(utc(9, 0, 0))),
                            punch(31, Some(utc(9, 1, 30))),
                            // Picada sin hora.
                            punch(45, None),
                            punch(100, Some(utc(9, 6, 10))),
                            punch(FINISH_CODE, Some(utc(9, 6, 40))),
                        ],
                    },
                    RaceResult {
                        // No clasificada: falta la 45.
                        runner: runner(2, "Berta", "Ruiz"),
                        status: RaceStatus::NotClassified,
                        place: None,
                        punches: vec![
                            punch(START_CODE, Some(utc(9, 2, 0))),
                            punch(31, Some(utc(9, 3, 45))),
                            punch(100, Some(utc(9, 10, 5))),
                            punch(FINISH_CODE, Some(utc(9, 10, 40))),
                        ],
                    },
                ],
            },
            Class {
                id: 2,
                name: "F21B".into(),
                short_name: None,
                course: shared,
                results: vec![
                    RaceResult {
                        // Estado aún sin interpretar; corredora sin datos opcionales.
                        runner: Runner {
                            given_name: "Carla".into(),
                            family_name: "Gómez".into(),
                            club: None,
                            bib: None,
                            si_card: None,
                            sex: None,
                        },
                        status: RaceStatus::Unknown(6),
                        place: None,
                        punches: vec![punch(START_CODE, Some(utc(9, 4, 0)))],
                    },
                    RaceResult {
                        runner: runner(4, "Diana", "López"),
                        status: RaceStatus::DidNotStart,
                        place: None,
                        punches: vec![],
                    },
                ],
            },
            Class {
                id: 3,
                name: "M21A".into(),
                short_name: Some("M21".into()),
                course: other,
                results: vec![RaceResult {
                    runner: Runner {
                        sex: Some(Sex::Male),
                        ..runner(5, "Elio", "Sanz")
                    },
                    status: RaceStatus::Ok,
                    place: Some(1),
                    punches: vec![
                        punch(START_CODE, Some(utc(9, 10, 0))),
                        punch(31, Some(utc(9, 11, 0))),
                        punch(52, Some(utc(9, 13, 0))),
                        punch(47, Some(utc(9, 15, 0))),
                        punch(100, Some(utc(9, 17, 0))),
                        punch(FINISH_CODE, Some(utc(9, 17, 20))),
                    ],
                }],
            },
        ],
    }
}

fn sample_track() -> Track {
    let t0 = utc(9, 0, 0);
    Track {
        points: vec![
            TrackPoint {
                time: t0,
                lat: 40.135_5,
                lon: -3.422_7,
                altitude_m: Some(745.2),
                heart_rate_bpm: Some(152),
                cadence_spm: Some(172.5),
                distance_m: Some(0.0),
            },
            TrackPoint {
                // Instante con milisegundos y todo lo opcional a `None`.
                time: t0 + Duration::milliseconds(1_250),
                lat: 40.135_6,
                lon: -3.422_6,
                altitude_m: None,
                heart_rate_bpm: None,
                cadence_spm: None,
                distance_m: None,
            },
            TrackPoint {
                time: t0 + Duration::seconds(2),
                lat: 40.135_712_345_678_9,
                lon: -3.422_512_345_678_9,
                altitude_m: Some(-1.5),
                heart_rate_bpm: Some(255),
                cadence_spm: None,
                distance_m: Some(7.25),
            },
        ],
        sport: Some("running".into()),
    }
}

#[test]
fn full_event_round_trips_exactly() {
    let mut store = Store::open_in_memory().unwrap();
    let event = sample_event();

    let saved = store.save_event(&event, None).unwrap();

    assert_eq!(store.load_event(saved.id).unwrap(), event);
    // Ids de resultado por categoría, en orden.
    assert_eq!(
        saved.results.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![2, 2, 1]
    );
    assert_eq!(store.result_ids(saved.id).unwrap(), saved.results);
}

#[test]
fn two_events_in_one_database_stay_apart() {
    let mut store = Store::open_in_memory().unwrap();
    let first = sample_event();
    let second = Event {
        name: None,
        date: NaiveDate::from_ymd_opt(2026, 11, 8).unwrap(),
        classes: vec![],
    };

    let a = store.save_event(&first, None).unwrap();
    let b = store.save_event(&second, None).unwrap();

    assert_ne!(a.id, b.id);
    assert_eq!(store.load_event(a.id).unwrap(), first);
    assert_eq!(store.load_event(b.id).unwrap(), second);
    assert!(b.results.is_empty());
}

#[test]
fn missing_event_is_an_error() {
    let store = Store::open_in_memory().unwrap();
    assert!(matches!(
        store.load_event(EventId(42)),
        Err(StoreError::EventNotFound(42))
    ));
    assert!(matches!(
        store.result_ids(EventId(42)),
        Err(StoreError::EventNotFound(42))
    ));
}

#[test]
fn file_database_persists_and_shares_courses() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tramos.sqlite");
    let event = sample_event();

    let saved = {
        let mut store = Store::open(&path).unwrap();
        store.save_event(&event, None).unwrap()
    };

    // Al reabrir no se vuelve a migrar y los datos siguen ahí.
    let store = Store::open(&path).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(store.load_event(saved.id).unwrap(), event);
    drop(store);

    let conn = Connection::open(&path).unwrap();
    let count = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
    // Tres categorías, dos recorridos distintos.
    assert_eq!(count("SELECT count(*) FROM classes"), 3);
    assert_eq!(count("SELECT count(*) FROM courses"), 2);
    assert_eq!(count("SELECT count(DISTINCT course_id) FROM classes"), 2);
    assert_eq!(count("SELECT count(*) FROM runners"), 5);
    assert_eq!(count("SELECT count(*) FROM punches"), 16);
    assert_eq!(
        count("SELECT count(*) FROM punches WHERE time_epoch_ms IS NULL"),
        1
    );
    // Salida de Ana: 2026-10-03T09:00:00Z en milisegundos desde la época.
    assert_eq!(
        count(
            "SELECT p.time_epoch_ms FROM punches p JOIN results r ON r.id = p.result_id \
             JOIN runners ru ON ru.id = r.runner_id \
             WHERE ru.given_name = 'Ana' AND p.position = 0"
        ),
        1_791_018_000_000
    );
    let (status, code): (String, i64) = conn
        .query_row(
            "SELECT status, status_code FROM results WHERE status = 'unknown'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((status.as_str(), code), ("unknown", 6));
}

#[test]
fn track_round_trips_with_optional_fields() {
    let mut store = Store::open_in_memory().unwrap();
    let saved = store.save_event(&sample_event(), None).unwrap();
    let ana = saved.results[0][0];
    let berta = saved.results[0][1];
    let track = sample_track();

    store.save_track(ana, &track, None).unwrap();

    assert_eq!(store.load_track(ana).unwrap(), Some(track.clone()));
    assert_eq!(store.load_track(berta).unwrap(), None);
    // La carrera no cambia por tener un track.
    assert_eq!(store.load_event(saved.id).unwrap(), sample_event());
}

#[test]
fn saving_a_track_again_replaces_it() {
    let mut store = Store::open_in_memory().unwrap();
    let saved = store.save_event(&sample_event(), None).unwrap();
    let ana = saved.results[0][0];
    store.save_track(ana, &sample_track(), None).unwrap();

    let shorter = Track {
        points: sample_track().points[1..2].to_vec(),
        sport: None,
    };
    store.save_track(ana, &shorter, None).unwrap();
    assert_eq!(store.load_track(ana).unwrap(), Some(shorter));

    store.save_track(ana, &Track::default(), None).unwrap();
    assert_eq!(store.load_track(ana).unwrap(), Some(Track::default()));
}

#[test]
fn track_errors_leave_nothing_behind() {
    let mut store = Store::open_in_memory().unwrap();
    let saved = store.save_event(&sample_event(), None).unwrap();
    let ana = saved.results[0][0];

    assert!(matches!(
        store.save_track(ResultId(9_999), &sample_track(), None),
        Err(StoreError::ResultNotFound(9_999))
    ));

    // Un instante con microsegundos no volvería igual: se rechaza y no se guarda nada.
    let mut bad = sample_track();
    bad.points[2].time += Duration::microseconds(1);
    assert!(matches!(
        store.save_track(ana, &bad, None),
        Err(StoreError::SubMillisecondInstant(_))
    ));
    assert_eq!(store.load_track(ana).unwrap(), None);

    let mut nan = sample_track();
    nan.points[0].altitude_m = Some(f64::NAN);
    assert!(matches!(
        store.save_track(ana, &nan, None),
        Err(StoreError::NotANumber("altitude_m"))
    ));
    assert_eq!(store.load_track(ana).unwrap(), None);
}

#[test]
fn settings_write_overwrite_and_read() {
    let mut store = Store::open_in_memory().unwrap();
    assert_eq!(store.setting("error_threshold_s").unwrap(), None);

    store.set_setting("error_threshold_s", "15").unwrap();
    store.set_setting("error_threshold_ratio", "0.10").unwrap();
    assert_eq!(
        store.setting("error_threshold_s").unwrap().as_deref(),
        Some("15")
    );

    store.set_setting("error_threshold_s", "20").unwrap();
    assert_eq!(
        store.setting("error_threshold_s").unwrap().as_deref(),
        Some("20")
    );
    assert_eq!(
        store.setting("error_threshold_ratio").unwrap().as_deref(),
        Some("0.10")
    );
}

#[test]
fn failed_event_save_is_rolled_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tramos.sqlite");
    let mut event = sample_event();
    // La última picada de la última categoría tiene microsegundos: falla al final del guardado.
    let last = event.classes[2].results[0].punches.last_mut().unwrap();
    last.time = last.time.map(|t| t + Duration::microseconds(1));

    let mut store = Store::open(&path).unwrap();
    assert!(matches!(
        store.save_event(&event, None),
        Err(StoreError::SubMillisecondInstant(_))
    ));
    drop(store);

    let conn = Connection::open(&path).unwrap();
    for table in [
        "events", "courses", "classes", "runners", "results", "punches",
    ] {
        let n: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "{table}");
    }
}

const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[test]
fn source_file_content_round_trips() {
    let mut store = Store::open_in_memory().unwrap();
    let before = Utc::now();

    let id = store
        .save_source_file(SourceFileKind::Spl, "C:/carreras/otono.spl", b"abc")
        .unwrap();
    let file = store.load_source_file(id).unwrap();

    assert_eq!(file.kind, SourceFileKind::Spl);
    assert_eq!(file.path, "C:/carreras/otono.spl");
    assert_eq!(file.sha256, ABC_SHA256);
    assert_eq!(file.content, b"abc".to_vec());
    // Se guarda al milisegundo.
    assert!(file.imported_at >= before - Duration::milliseconds(1));
    assert!(file.imported_at <= Utc::now());

    // Contenido binario arbitrario, con ceros y bytes no UTF-8.
    let binary: Vec<u8> = (0..=255u8).chain([0, 0, 0xff]).collect();
    let fit = store
        .save_source_file(SourceFileKind::Fit, "reloj.fit", &binary)
        .unwrap();
    assert_ne!(fit, id);
    let file = store.load_source_file(fit).unwrap();
    assert_eq!(file.kind, SourceFileKind::Fit);
    assert_eq!(file.content, binary);
}

#[test]
fn same_content_is_stored_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tramos.sqlite");
    let mut store = Store::open(&path).unwrap();

    let first = store
        .save_source_file(SourceFileKind::Spl, "a/otono.spl", b"abc")
        .unwrap();
    let again = store
        .save_source_file(SourceFileKind::Spl, "b/copia.spl", b"abc")
        .unwrap();
    let other = store
        .save_source_file(SourceFileKind::Spl, "a/otono.spl", b"abd")
        .unwrap();

    assert_eq!(again, first);
    assert_ne!(other, first);
    // Se conserva la ruta de la primera importación.
    assert_eq!(store.load_source_file(again).unwrap().path, "a/otono.spl");
    drop(store);

    let conn = Connection::open(&path).unwrap();
    let (rows, size): (i64, i64) = conn
        .query_row(
            "SELECT count(*), sum(size_bytes) FROM source_files",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((rows, size), (2, 6));
    // El esquema no admite un tamaño que no cuadre con el contenido.
    let bad = conn.execute(
        "INSERT INTO source_files (kind, path, sha256, size_bytes, content, imported_at_epoch_ms) \
         VALUES ('fit', 'x', ?1, 99, x'00', 0)",
        ["0".repeat(64)],
    );
    assert!(bad.is_err());
}

#[test]
fn events_and_tracks_link_to_their_source_files() {
    let mut store = Store::open_in_memory().unwrap();
    let spl = store
        .save_source_file(SourceFileKind::Spl, "otono.spl", b"spl sintetico")
        .unwrap();
    let fit = store
        .save_source_file(SourceFileKind::Fit, "ana.fit", b"fit sintetico")
        .unwrap();

    let linked = store.save_event(&sample_event(), Some(spl)).unwrap();
    let unlinked = store.save_event(&sample_event(), None).unwrap();
    let ana = linked.results[0][0];
    let berta = linked.results[0][1];
    store.save_track(ana, &sample_track(), Some(fit)).unwrap();
    store.save_track(berta, &sample_track(), None).unwrap();

    assert_eq!(store.event_source_file(linked.id).unwrap(), Some(spl));
    assert_eq!(store.event_source_file(unlinked.id).unwrap(), None);
    assert_eq!(store.track_source_file(ana).unwrap(), Some(fit));
    assert_eq!(store.track_source_file(berta).unwrap(), None);
    assert_eq!(store.track_source_file(linked.results[2][0]).unwrap(), None);
    assert_eq!(store.load_event(linked.id).unwrap(), sample_event());
    assert_eq!(store.load_track(ana).unwrap(), Some(sample_track()));

    assert!(matches!(
        store.load_source_file(SourceFileId(9_999)),
        Err(StoreError::SourceFileNotFound(9_999))
    ));
    assert!(matches!(
        store.save_event(&sample_event(), Some(SourceFileId(9_999))),
        Err(StoreError::SourceFileNotFound(9_999))
    ));
    assert!(matches!(
        store.save_track(berta, &sample_track(), Some(SourceFileId(9_999))),
        Err(StoreError::SourceFileNotFound(9_999))
    ));
    // El fallo no toca el track que ya había.
    assert_eq!(store.load_track(berta).unwrap(), Some(sample_track()));
    assert_eq!(store.result_ids(unlinked.id).unwrap().len(), 3);
}

#[test]
fn event_format_is_set_read_and_cleared() {
    let mut store = Store::open_in_memory().unwrap();
    let saved = store.save_event(&sample_event(), None).unwrap();
    assert_eq!(store.event_format(saved.id).unwrap(), None);

    store
        .set_event_format(saved.id, Some(RaceFormat::Sprint))
        .unwrap();
    assert_eq!(
        store.event_format(saved.id).unwrap(),
        Some(RaceFormat::Sprint)
    );
    store.set_event_format(saved.id, None).unwrap();
    assert_eq!(store.event_format(saved.id).unwrap(), None);

    let missing = EventId(saved.id.0 + 1);
    assert!(matches!(
        store.set_event_format(missing, Some(RaceFormat::Long)),
        Err(StoreError::EventNotFound(_))
    ));
    assert!(matches!(
        store.event_format(missing),
        Err(StoreError::EventNotFound(_))
    ));
}

#[test]
fn event_is_found_by_its_source_file() {
    let mut store = Store::open_in_memory().unwrap();
    let spl = store
        .save_source_file(SourceFileKind::Spl, "a.spl", b"spl4 contenido")
        .unwrap();
    let other = store
        .save_source_file(SourceFileKind::Spl, "b.spl", b"spl4 otro")
        .unwrap();
    assert_eq!(store.event_by_source_file(spl).unwrap(), None);
    assert_eq!(
        store.find_source_file(b"spl4 contenido").unwrap(),
        Some(spl)
    );
    assert_eq!(store.find_source_file(b"spl4 nuevo").unwrap(), None);

    let first = store.save_event(&sample_event(), Some(spl)).unwrap();
    let second = store.save_event(&sample_event(), Some(spl)).unwrap();
    store.save_event(&sample_event(), None).unwrap();
    // La primera guardada, aunque haya otra con el mismo fichero.
    assert_eq!(store.event_by_source_file(spl).unwrap(), Some(first.id));
    assert_ne!(first.id, second.id);
    assert_eq!(store.event_by_source_file(other).unwrap(), None);
}

#[test]
fn result_ref_points_back_into_the_loaded_event() {
    let mut store = Store::open_in_memory().unwrap();
    store.save_event(&sample_event(), None).unwrap();
    let event = sample_event();
    let saved = store.save_event(&event, None).unwrap();
    let loaded = store.load_event(saved.id).unwrap();
    for (c, class) in saved.results.iter().enumerate() {
        for (r, &id) in class.iter().enumerate() {
            let (event_id, at) = store.result_ref(id).unwrap();
            assert_eq!(event_id, saved.id);
            assert_eq!((at.class_index, at.result_index), (c, r));
            assert_eq!(at.get(&loaded), Some(&event.classes[c].results[r]));
        }
    }
    assert!(matches!(
        store.result_ref(ResultId(9999)),
        Err(StoreError::ResultNotFound(9999))
    ));
}
