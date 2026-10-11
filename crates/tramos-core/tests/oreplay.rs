//! Importador de OReplay con respuestas sintéticas de su API (`docs/formato-oreplay.md`).

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use chrono::{DateTime, NaiveDate, Utc};
use serde_json::{Value, json};
use tramos_core::identify::{Identification, RunnerIdentity, identify_runner};
use tramos_core::importers::oreplay::{
    self, ClassResults, Download, Link, OReplayError, parse_classes, parse_event, parse_results,
    parse_url,
};
use tramos_core::model::{Event, FINISH_CODE, Punch, RaceStatus, START_CODE, Sex};

const EVENT: &str = "0fb98d3c-d506-4406-95c2-f28cb1863889";
const STAGE: &str = "557abbec-9bb7-4cea-9c45-c0634aedf22e";
const RELAY: &str = "99b4b528-3d13-4194-b884-1dfad8a03a79";

fn at(text: &str) -> DateTime<Utc> {
    text.parse().unwrap()
}

fn bytes(value: Value) -> Vec<u8> {
    serde_json::to_vec(&value).unwrap()
}

/// Evento con una etapa clásica que sale a las 23:30 UTC (ya es el día siguiente en Madrid) y un
/// relevo.
fn event_response() -> Vec<u8> {
    bytes(json!({"data": {
        "id": EVENT,
        "description": "Trofeo de prueba",
        "timezone": "Europe/Madrid",
        "initial_date": "2026-03-07",
        "stages": [
            {"id": STAGE, "description": "Media", "start": "2026-03-07T23:30:00.000+00:00",
             "stage_type": {"id": "x", "description": "Classic"}},
            {"id": RELAY, "description": "Relevos", "start": "2026-03-08T10:00:00.000+00:00",
             "stage_type": {"id": "y", "description": "Relay"}}
        ]
    }}))
}

fn classes_response() -> Vec<u8> {
    bytes(json!({"data": [
        {"id": "c1", "short_name": "M21", "long_name": "Hombres 21", "splits": []},
        {"id": "c2", "short_name": "F21", "long_name": "", "splits": []}
    ]}))
}

fn split(order: i64, station: Value, time: Option<&str>) -> Value {
    json!({
        "id": format!("s{order}"), "is_intermediate": false, "points": 0,
        "order_number": order, "reading_time": time,
        "control": {"id": "k", "station": station, "control_type": {"description": "Normal Control"}}
    })
}

fn runner(name: &str, status: &str, position: i64, splits: Vec<Value>) -> Value {
    json!({
        "id": "r", "full_name": name, "sicard": "2000001", "sex": "M", "bib_number": null,
        "is_nc": false, "club": {"id": "k", "short_name": "Club A"},
        "stage": {
            "start_time": "2026-03-07T23:30:00.000+00:00",
            "finish_time": "2026-03-07T23:40:00.000+00:00",
            "upload_type": "res_splits", "time_seconds": 600,
            "position": position, "status_code": status, "is_nc": false,
            "splits": splits
        }
    })
}

/// Recorrido 31 → 45 → 100; la API da las picadas desordenadas.
fn course_splits(t31: Option<&str>, t45: Option<&str>, t100: Option<&str>) -> Vec<Value> {
    vec![
        split(3, json!("100"), t100),
        split(1, json!("31"), t31),
        split(2, json!("45"), t45),
    ]
}

fn m21_response() -> Vec<u8> {
    let mut winner = runner(
        "Ana Pérez",
        "0",
        1,
        course_splits(
            Some("2026-03-07T23:32:00.000+00:00"),
            Some("2026-03-07T23:35:30.000+00:00"),
            Some("2026-03-07T23:39:00.000+00:00"),
        ),
    );
    // La tarjeta como número, el dorsal como texto, y una radio que no es del recorrido.
    winner["sicard"] = json!(2000002);
    winner["bib_number"] = json!("17");
    winner["sex"] = json!("F");
    winner["stage"]["splits"].as_array_mut().unwrap().push(
        json!({"is_intermediate": true, "order_number": 2,
                     "reading_time": "2026-03-07T23:34:00.000+00:00",
                     "control": {"station": "90"}}),
    );

    let second = runner(
        "Luis Gómez",
        "0",
        2,
        course_splits(
            Some("2026-03-07T23:33:00.000+00:00"),
            Some("2026-03-07T23:36:00.000+00:00"),
            Some("2026-03-07T23:39:30.000+00:00"),
        ),
    );
    // Falta la 45: viene con su código y sin hora.
    let mp = runner(
        "Eva Ruiz",
        "3",
        0,
        course_splits(
            Some("2026-03-07T23:34:00.000+00:00"),
            None,
            Some("2026-03-07T23:41:00.000+00:00"),
        ),
    );
    let mut dns = runner("Iker Sanz", "1", 0, course_splits(None, None, None));
    dns["stage"]["start_time"] = Value::Null;
    dns["stage"]["finish_time"] = Value::Null;
    // Fuera de concurso: OK en OReplay, no clasificado para el tiempo perdido.
    let mut nc = runner(
        "Pau Vidal",
        "0",
        0,
        course_splits(
            Some("2026-03-07T23:32:30.000+00:00"),
            Some("2026-03-07T23:36:30.000+00:00"),
            Some("2026-03-07T23:39:45.000+00:00"),
        ),
    );
    nc["is_nc"] = json!(true);
    // Solo tiempo de meta (el organizador no subió sus parciales).
    let finish_only = runner("Sara Gil", "0", 3, vec![]);
    // Sin resultado en la etapa (lista de salida).
    let entry = json!({"full_name": "Jon Mas", "sicard": null, "sex": null, "stage": null});

    bytes(json!({"data": [winner, second, mp, dns, nc, finish_only, entry], "last_logs": []}))
}

fn f21_response() -> Vec<u8> {
    let alone = runner(
        "Marta León",
        "0",
        1,
        vec![
            split(2, json!(45), Some("2026-03-07T23:36:00.000+00:00")),
            split(1, json!(31), Some("2026-03-07T23:33:00.000+00:00")),
        ],
    );
    bytes(json!({"data": [alone], "last_logs": []}))
}

fn download() -> Download {
    let event = parse_event(&event_response()).unwrap();
    let stage = event
        .classic_stages()
        .find(|s| s.id == STAGE)
        .cloned()
        .unwrap();
    let classes = parse_classes(&classes_response()).unwrap();
    let runners = [
        parse_results(&m21_response()).unwrap(),
        parse_results(&f21_response()).unwrap(),
    ];
    let classes = classes
        .into_iter()
        .zip(runners)
        .map(|(class, runners)| ClassResults { class, runners })
        .collect();
    Download::new(event, stage, classes)
}

fn event() -> Event {
    oreplay::to_event(&download()).unwrap()
}

fn times(punches: &[Punch]) -> Vec<Option<DateTime<Utc>>> {
    punches.iter().map(|p| p.time).collect()
}

#[test]
fn links_from_the_web() {
    let both = Link {
        event_id: EVENT.to_string(),
        stage_id: Some(STAGE.to_string()),
    };
    for url in [
        format!("https://www.oreplay.es/competitions/{EVENT}/{STAGE}"),
        format!("  oreplay.es/competitions/{EVENT}/{STAGE}/splits?class=x#top "),
        format!(
            "http://www.oreplay.es/competitions/{}/{}",
            EVENT.to_uppercase(),
            STAGE
        ),
    ] {
        assert_eq!(parse_url(&url).unwrap(), both, "{url}");
    }
    assert_eq!(
        parse_url(&format!("https://www.oreplay.es/competitions/{EVENT}")).unwrap(),
        Link {
            event_id: EVENT.to_string(),
            stage_id: None
        }
    );
    for url in [
        "https://www.oreplay.es/".to_string(),
        format!("https://www.example.com/competitions/{EVENT}"),
        "https://www.oreplay.es/competitions/no-es-un-uuid".to_string(),
        format!("https://www.oreplay.es/admin/{EVENT}"),
    ] {
        assert!(
            matches!(parse_url(&url), Err(OReplayError::InvalidUrl(_))),
            "{url}"
        );
    }
}

#[test]
fn api_urls() {
    assert_eq!(
        oreplay::results_url(EVENT, STAGE, "c1"),
        format!(
            "https://www.oreplay.es/api/v1/events/{EVENT}/stages/{STAGE}/results?class_id=c1&forceSameDay=true"
        )
    );
    assert_eq!(
        oreplay::classes_url(EVENT, STAGE),
        format!("https://www.oreplay.es/api/v1/events/{EVENT}/stages/{STAGE}/classes")
    );
}

#[test]
fn only_classic_stages_are_offered() {
    let event = parse_event(&event_response()).unwrap();
    let ids: Vec<_> = event.classic_stages().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, [STAGE]);
}

#[test]
fn event_name_date_and_classes() {
    let event = event();
    assert_eq!(event.name.as_deref(), Some("Trofeo de prueba · Media"));
    // 23:30 UTC del 7 son las 00:30 del 8 en Madrid.
    assert_eq!(event.date, NaiveDate::from_ymd_opt(2026, 3, 8).unwrap());
    let names: Vec<_> = event
        .classes
        .iter()
        .map(|c| (c.id, c.name.as_str(), c.short_name.as_deref()))
        .collect();
    assert_eq!(
        names,
        [(1, "Hombres 21", Some("M21")), (2, "F21", Some("F21"))]
    );
}

#[test]
fn the_course_comes_from_the_finishers_in_order() {
    let event = event();
    assert_eq!(event.classes[0].course.controls, [31, 45, 100]);
    assert_eq!(event.classes[1].course.controls, [31, 45]);
}

#[test]
fn punches_go_from_start_to_finish_in_course_order() {
    let winner = &event().classes[0].results[0];
    let codes: Vec<_> = winner.punches.iter().map(|p| p.code).collect();
    assert_eq!(codes, [START_CODE, 31, 45, 100, FINISH_CODE]);
    assert_eq!(
        times(&winner.punches),
        [
            Some(at("2026-03-07T23:30:00Z")),
            Some(at("2026-03-07T23:32:00Z")),
            Some(at("2026-03-07T23:35:30Z")),
            Some(at("2026-03-07T23:39:00Z")),
            Some(at("2026-03-07T23:40:00Z")),
        ]
    );
}

#[test]
fn runner_fields() {
    let results = &event().classes[0].results;
    let winner = &results[0];
    assert_eq!(winner.runner.given_name, "Ana Pérez");
    assert_eq!(winner.runner.family_name, "");
    assert_eq!(winner.runner.club.as_deref(), Some("Club A"));
    assert_eq!(winner.runner.si_card, Some(2000002));
    assert_eq!(winner.runner.bib, Some(17));
    assert_eq!(winner.runner.sex, Some(Sex::Female));
    assert_eq!(winner.place, Some(1));
    assert_eq!(results[1].runner.si_card, Some(2000001));
    assert_eq!(results[1].runner.bib, None);
    assert_eq!(results[1].runner.sex, Some(Sex::Male));
}

#[test]
fn statuses() {
    let results = &event().classes[0].results;
    let statuses: Vec<_> = results.iter().map(|r| (r.status, r.place)).collect();
    assert_eq!(
        statuses,
        [
            (RaceStatus::Ok, Some(1)),
            (RaceStatus::Ok, Some(2)),
            (RaceStatus::NotClassified, None),
            (RaceStatus::DidNotStart, None),
            // Fuera de concurso.
            (RaceStatus::NotClassified, None),
            (RaceStatus::Ok, Some(3)),
            // Sin resultado en la etapa.
            (RaceStatus::DidNotStart, None),
        ]
    );
}

#[test]
fn missing_punches_keep_their_control() {
    let results = &event().classes[0].results;
    let mp = &results[2];
    assert_eq!(mp.punches[2].code, 45);
    assert_eq!(mp.punches[2].time, None);
    assert!(mp.punches[1].time.is_some() && mp.punches[3].time.is_some());

    // Solo meta: salida y meta con hora, balizas sin.
    let finish_only = &results[5];
    assert_eq!(
        times(&finish_only.punches),
        [
            Some(at("2026-03-07T23:30:00Z")),
            None,
            None,
            None,
            Some(at("2026-03-07T23:40:00Z")),
        ]
    );
    // Sin resultado: todo sin hora.
    assert!(results[6].punches.iter().all(|p| p.time.is_none()));
    assert_eq!(results[6].punches.len(), 5);
}

#[test]
fn a_runner_with_another_sequence_gets_the_controls_that_match() {
    let mut d = download();
    // Pica 45 antes que 31: la 31 se busca después de la 45 y queda sin hora.
    d.classes[0].runners[1].stage.as_mut().unwrap().splits[1].order_number = Some(5);
    let event = oreplay::to_event(&d).unwrap();
    assert_eq!(event.classes[0].course.controls, [31, 45, 100]);
    let r = &event.classes[0].results[1];
    let codes: Vec<_> = r
        .punches
        .iter()
        .map(|p| (p.code, p.time.is_some()))
        .collect();
    assert_eq!(
        codes,
        [
            (START_CODE, true),
            (31, true),
            (45, false),
            (100, false),
            (FINISH_CODE, true)
        ]
    );
}

#[test]
fn the_most_common_sequence_wins_and_ties_keep_the_first() {
    let mut d = download();
    let other = d.classes[0].runners[0].clone();
    let mut swapped = other.clone();
    let splits = &mut swapped.stage.as_mut().unwrap().splits;
    // Orden 45 → 31 → 100.
    splits[1].order_number = Some(2);
    splits[2].order_number = Some(1);
    d.classes[0].runners = vec![swapped.clone(), other.clone()];
    assert_eq!(
        oreplay::to_event(&d).unwrap().classes[0].course.controls,
        [45, 31, 100]
    );
    d.classes[0].runners = vec![swapped, other.clone(), other];
    assert_eq!(
        oreplay::to_event(&d).unwrap().classes[0].course.controls,
        [31, 45, 100]
    );
}

#[test]
fn without_finishers_the_course_comes_from_anyone_with_punches() {
    let mut d = download();
    d.classes[1].runners[0].stage.as_mut().unwrap().status_code = Some("3".to_string());
    assert_eq!(
        oreplay::to_event(&d).unwrap().classes[1].course.controls,
        [31, 45]
    );
}

#[test]
fn the_download_round_trips_and_reads() {
    let d = download();
    let json = d.to_json();
    assert!(oreplay::is_download(&json));
    assert_eq!(Download::from_json(&json).unwrap(), d);
    assert_eq!(oreplay::read(&json).unwrap(), event());
    // Las etapas del evento no se guardan.
    assert!(d.event.stages.is_empty());
    // Dos descargas iguales dan el mismo fichero.
    assert_eq!(download().to_json(), json);
}

#[test]
fn identification_by_card_and_full_name() {
    let event = event();
    let by_name = RunnerIdentity {
        si_card: None,
        full_name: Some("luis gomez".to_string()),
    };
    assert!(matches!(
        identify_runner(&event, &by_name),
        Identification::Unique { candidate, name_mismatch: false } if candidate.result.result_index == 1
    ));
    let by_card = RunnerIdentity {
        si_card: Some(2000002),
        full_name: Some("Ana Pérez".to_string()),
    };
    assert!(matches!(
        identify_runner(&event, &by_card),
        Identification::Unique { candidate, name_mismatch: false } if candidate.result.result_index == 0
    ));
}

#[test]
fn errors() {
    let mut relay = download();
    relay.stage.stage_type.as_mut().unwrap().description = Some("Relay".to_string());
    assert_eq!(
        oreplay::to_event(&relay),
        Err(OReplayError::UnsupportedStage("Relay".to_string()))
    );

    let mut no_splits = download();
    for class in &mut no_splits.classes {
        for runner in &mut class.runners {
            if let Some(stage) = runner.stage.as_mut() {
                stage.splits.clear();
            }
        }
    }
    assert_eq!(oreplay::to_event(&no_splits), Err(OReplayError::NoSplits));

    let mut bad_station = download();
    bad_station.classes[1].runners[0]
        .stage
        .as_mut()
        .unwrap()
        .splits[0]
        .control
        .as_mut()
        .unwrap()
        .station = Some("S1".to_string());
    assert_eq!(
        oreplay::to_event(&bad_station),
        Err(OReplayError::InvalidStation {
            class: "F21".to_string(),
            station: "S1".to_string()
        })
    );

    let mut no_date = download();
    no_date.stage.start = None;
    for class in &mut no_date.classes {
        for runner in &mut class.runners {
            if let Some(stage) = runner.stage.as_mut() {
                stage.start_time = None;
            }
        }
    }
    assert_eq!(oreplay::to_event(&no_date), Err(OReplayError::MissingDate));

    assert_eq!(oreplay::read(b"{}"), Err(OReplayError::NotADownload));
    assert_eq!(oreplay::read(b"spl4...."), Err(OReplayError::NotADownload));
    assert!(!oreplay::is_download(b"spl4...."));
    let mut newer: Value = serde_json::from_slice(&download().to_json()).unwrap();
    newer["version"] = json!(2);
    assert_eq!(
        oreplay::read(&bytes(newer)),
        Err(OReplayError::UnsupportedVersion(2))
    );
    assert!(matches!(
        parse_results(b"{\"error\": \"Internal Server Error\", \"code\": 500}"),
        Err(OReplayError::InvalidResponse(_))
    ));
}

#[test]
fn without_stage_start_the_date_is_the_first_start() {
    let mut d = download();
    d.stage.start = None;
    // La primera salida, 23:30 UTC del 7: día 8 en Madrid.
    assert_eq!(
        oreplay::to_event(&d).unwrap().date,
        NaiveDate::from_ymd_opt(2026, 3, 8).unwrap()
    );
    d.event.timezone = Some("UTC".to_string());
    assert_eq!(
        oreplay::to_event(&d).unwrap().date,
        NaiveDate::from_ymd_opt(2026, 3, 7).unwrap()
    );
}

// --- El .spl anonimizado de Baltanás como si viniera de OReplay ---

const BALTANAS_SPL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/spl/baltanas-anon.spl"
);
const BALTANAS_OREPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/oreplay/baltanas-anon.json"
);

fn baltanas() -> Event {
    tramos_core::importers::spl::read(&std::fs::read(BALTANAS_SPL).unwrap()).unwrap()
}

/// La descarga que daría OReplay con los mismos resultados que el .spl: mismas picadas, en el
/// formato de su API.
fn download_from_spl(event: &Event) -> Download {
    let api_event: oreplay::ApiEvent = serde_json::from_value(json!({
        "id": EVENT, "description": event.name, "timezone": "Europe/Madrid"
    }))
    .unwrap();
    let first_start = event
        .classes
        .iter()
        .flat_map(|c| &c.results)
        .filter_map(|r| r.punches.first()?.time)
        .min();
    let stage: oreplay::ApiStage = serde_json::from_value(json!({
        "id": STAGE, "description": null, "start": first_start,
        "stage_type": {"description": "Classic"}
    }))
    .unwrap();
    let classes = event
        .classes
        .iter()
        .enumerate()
        .map(|(i, class)| {
            let runners = class
                .results
                .iter()
                .map(|r| {
                    let time = |code| r.punches.iter().find(|p| p.code == code)?.time;
                    let splits: Vec<Value> = r
                        .punches
                        .iter()
                        .filter(|p| p.code != START_CODE && p.code != FINISH_CODE)
                        .enumerate()
                        .map(|(k, p)| {
                            json!({"is_intermediate": false, "order_number": k + 1,
                                   "reading_time": p.time, "control": {"station": p.code.to_string()}})
                        })
                        .collect();
                    let status = match r.status {
                        RaceStatus::Ok => "0",
                        RaceStatus::DidNotStart => "1",
                        _ => "3",
                    };
                    serde_json::from_value(json!({
                        "full_name": format!("{} {}", r.runner.given_name, r.runner.family_name),
                        "club": {"short_name": r.runner.club},
                        "sicard": r.runner.si_card,
                        "bib_number": r.runner.bib,
                        "sex": match r.runner.sex { Some(Sex::Male) => "M", Some(Sex::Female) => "F", None => "" },
                        "stage": {
                            "start_time": time(START_CODE), "finish_time": time(FINISH_CODE),
                            "status_code": status, "position": r.place, "splits": splits
                        }
                    }))
                    .unwrap()
                })
                .collect();
            let class_json: oreplay::ApiClass = serde_json::from_value(json!({
                "id": format!("c{i}"), "short_name": class.short_name, "long_name": class.name
            }))
            .unwrap();
            ClassResults {
                class: class_json,
                runners,
            }
        })
        .collect();
    Download::new(api_event, stage, classes)
}

#[test]
fn the_baltanas_spl_and_its_oreplay_twin_are_the_same_race() {
    let spl = baltanas();
    let from_oreplay = oreplay::to_event(&download_from_spl(&spl)).unwrap();
    assert_eq!(from_oreplay.date, spl.date);
    assert_eq!(from_oreplay.name, spl.name);
    assert_eq!(from_oreplay.classes.len(), spl.classes.len());
    for (a, b) in spl.classes.iter().zip(&from_oreplay.classes) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.course, b.course, "{}", a.name);
        assert_eq!(a.results.len(), b.results.len());
        for (x, y) in a.results.iter().zip(&b.results) {
            assert_eq!(x.punches, y.punches, "{} {}", a.name, x.runner.given_name);
            assert_eq!(
                format!("{} {}", x.runner.given_name, x.runner.family_name).trim(),
                y.runner.given_name
            );
            assert_eq!(
                (x.runner.si_card, x.runner.bib, x.runner.sex, &x.runner.club),
                (y.runner.si_card, y.runner.bib, y.runner.sex, &y.runner.club)
            );
            let expected = match x.status {
                RaceStatus::Ok | RaceStatus::DidNotStart => x.status,
                _ => RaceStatus::NotClassified,
            };
            assert_eq!(y.status, expected);
            if x.status == RaceStatus::Ok {
                assert_eq!(x.place, y.place);
            }
        }
    }
    assert_eq!(
        tramos_core::courses::group_by_course(&from_oreplay).len(),
        tramos_core::courses::group_by_course(&spl).len()
    );
}

/// `fixtures/oreplay/baltanas-anon.json` es la descarga de arriba. Para regenerarlo:
/// `cargo test -p tramos-core --test oreplay -- --ignored write_baltanas_fixture`.
#[test]
#[ignore]
fn write_baltanas_fixture() {
    let path = std::path::Path::new(BALTANAS_OREPLAY);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, download_from_spl(&baltanas()).to_json()).unwrap();
}

#[test]
fn the_baltanas_fixture_is_up_to_date() {
    assert_eq!(
        std::fs::read(BALTANAS_OREPLAY).unwrap(),
        download_from_spl(&baltanas()).to_json()
    );
}
