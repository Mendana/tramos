//! Paridad del lector de .spl con el lector de referencia (`tools/reference/winsplits_spl.py`)
//! sobre el fixture público de Baltanás.

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::collections::HashSet;
use std::path::PathBuf;

use chrono::{DateTime, NaiveDate, Timelike, Utc};
use serde_json::Value;
use tramos_core::importers::spl::{self, RACE_TIME_ZONE};
use tramos_core::model::{Event, FINISH_CODE, RaceStatus, Sex};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/spl")
        .join(name)
}

fn load() -> (Event, Value) {
    let data = std::fs::read(fixture("baltanas-anon.spl")).unwrap();
    let expected = std::fs::read_to_string(fixture("baltanas-anon.expected.json")).unwrap();
    (
        spl::read(&data).unwrap(),
        serde_json::from_str(&expected).unwrap(),
    )
}

/// Instante UTC → segundos desde la medianoche local del día de la carrera.
fn local_time_of_day_s(t: DateTime<Utc>, date: NaiveDate) -> f64 {
    let local = t.with_timezone(&RACE_TIME_ZONE);
    let days = (local.date_naive() - date).num_days() as f64;
    days * 86_400.0
        + f64::from(local.num_seconds_from_midnight())
        + f64::from(local.nanosecond()) / 1e9
}

fn opt_u64(v: &Value, key: &str) -> Option<u64> {
    v.get(key).map(|x| x.as_u64().unwrap())
}

fn opt_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).map(|x| x.as_str().unwrap())
}

#[test]
fn baltanas_matches_reference_reader() {
    let (event, expected) = load();

    assert_eq!(event.name, None);
    assert_eq!(
        event.date.to_string(),
        expected["event"]["date"].as_str().unwrap()
    );

    let expected_classes = expected["classes"].as_array().unwrap();
    assert_eq!(event.classes.len(), 18);
    assert_eq!(event.classes.len(), expected_classes.len());

    let runners: usize = event.classes.iter().map(|c| c.results.len()).sum();
    assert_eq!(runners, 275);
    let courses: HashSet<_> = event.classes.iter().map(|c| &c.course).collect();
    assert_eq!(courses.len(), 9);

    let mut punches_checked = 0;
    for (class, exp) in event.classes.iter().zip(expected_classes) {
        let ctx = format!("categoría {}", class.id);
        assert_eq!(u64::from(class.id), exp["id"].as_u64().unwrap(), "{ctx}");
        assert_eq!(class.name, exp["name"].as_str().unwrap(), "{ctx}");
        assert_eq!(
            class.short_name.as_deref(),
            opt_str(exp, "short_name"),
            "{ctx}"
        );

        // La referencia da los destinos de los tramos, que acaban en la meta; el modelo no la lleva.
        let mut course: Vec<u16> = exp["course"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| u16::try_from(c.as_u64().unwrap()).unwrap())
            .collect();
        assert_eq!(course.pop(), Some(FINISH_CODE), "{ctx}");
        assert_eq!(class.course.controls, course, "{ctx}");

        let exp_runners = exp["runners"].as_array().unwrap();
        assert_eq!(class.results.len(), exp_runners.len(), "{ctx}");
        for (result, er) in class.results.iter().zip(exp_runners) {
            let r = &result.runner;
            let ctx = format!("{ctx}, corredor {}", r.id);
            assert_eq!(u64::from(r.id), er["id"].as_u64().unwrap(), "{ctx}");
            assert_eq!(r.given_name, opt_str(er, "given").unwrap_or(""), "{ctx}");
            assert_eq!(r.family_name, opt_str(er, "family").unwrap_or(""), "{ctx}");
            assert_eq!(r.club.as_deref(), opt_str(er, "club"), "{ctx}");
            // Dorsal, tarjeta y puesto: 0 = sin valor.
            let non_zero = |v: Option<u64>| v.filter(|&n| n != 0);
            assert_eq!(r.bib.map(u64::from), non_zero(opt_u64(er, "bib")), "{ctx}");
            assert_eq!(
                r.si_card.map(u64::from),
                non_zero(opt_u64(er, "si_card")),
                "{ctx}"
            );
            assert_eq!(
                result.place.map(u64::from),
                non_zero(opt_u64(er, "place")),
                "{ctx}"
            );

            let status = match er["status"].as_u64().unwrap() {
                0 => RaceStatus::Ok,
                6 => RaceStatus::NotClassified,
                10 => RaceStatus::DidNotStart,
                n => RaceStatus::Unknown(u8::try_from(n).unwrap()),
            };
            assert_eq!(result.status, status, "{ctx}");

            let sex = match opt_u64(er, "sex") {
                Some(1) => Some(Sex::Male),
                Some(2) => Some(Sex::Female),
                _ => None,
            };
            assert_eq!(r.sex, sex, "{ctx}");

            let exp_punches: &[Value] = er
                .get("punches")
                .map(|p| p.as_array().unwrap().as_slice())
                .unwrap_or_default();
            assert_eq!(result.punches.len(), exp_punches.len(), "{ctx}");
            for (punch, ep) in result.punches.iter().zip(exp_punches) {
                assert_eq!(u64::from(punch.code), ep["code"].as_u64().unwrap(), "{ctx}");
                let got = punch.time.map(|t| local_time_of_day_s(t, event.date));
                match (got, ep["time_of_day_s"].as_f64()) {
                    (None, None) => {}
                    (Some(got), Some(want)) => {
                        assert!((got - want).abs() < 0.005, "{ctx}: {got} != {want}")
                    }
                    (got, want) => panic!("{ctx}: {got:?} != {want:?}"),
                }
                punches_checked += 1;
            }
        }
    }
    assert_eq!(punches_checked, 4031);
}

#[test]
fn baltanas_times_are_utc() {
    let (event, _) = load();
    // Primer corredor del fixture: salida a las 17:31:00 locales del 3 de octubre de 2026
    // (horario de verano, UTC+2) → 15:31:00Z.
    let start = event.classes[0].results[0].punches[0].time.unwrap();
    assert_eq!(start.to_rfc3339(), "2026-10-03T15:31:00+00:00");
}
