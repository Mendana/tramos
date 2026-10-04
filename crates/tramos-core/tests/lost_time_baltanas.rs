//! Paridad del tiempo perdido con la implementación de referencia
//! (`tools/reference/tiempo_perdido.py`) sobre el fixture público de Baltanás.
//!
//! El JSON esperado (`fixtures/spl/baltanas-anon.tiempo-perdido.expected.json`) redondea los
//! flotantes a 6 decimales, así que se comparan con una tolerancia absoluta de [`TOLERANCE`].
//! Todo lo demás (claves, enteros, booleanos, textos y `null`) tiene que coincidir exactamente.
//!
//! El JSON usa el tiempo ideal por defecto (suma de referencias). La otra definición (suma de
//! los mejores splits) se comprueba contra los splits del oráculo, sin otro JSON esperado.

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use serde_json::Value;
use tramos_core::importers::spl;
use tramos_core::lost_time::{IdealTime, LostTimeConfig, analyze_event};
use tramos_core::model::{Event, RaceStatus};

/// Redondeo del JSON (5e-7) más margen para el error de coma flotante.
const TOLERANCE: f64 = 1e-6;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/spl")
        .join(name)
}

fn load() -> (Event, Value) {
    let data = std::fs::read(fixture("baltanas-anon.spl")).unwrap();
    let text =
        std::fs::read_to_string(fixture("baltanas-anon.tiempo-perdido.expected.json")).unwrap();
    let mut expected: Value = serde_json::from_str(&text).unwrap();
    // El resumen legible es para revisión humana; no forma parte de la salida del núcleo.
    expected.as_object_mut().unwrap().remove("resumen").unwrap();
    (spl::read(&data).unwrap(), expected)
}

/// Compara dos JSON y acumula las diferencias con su ruta.
fn compare(got: &Value, want: &Value, path: &str, diffs: &mut Vec<String>) {
    match (got, want) {
        (Value::Number(g), Value::Number(w)) if g.is_f64() || w.is_f64() => {
            let (g, w) = (g.as_f64().unwrap(), w.as_f64().unwrap());
            if (g - w).abs() > TOLERANCE {
                diffs.push(format!("{path}: {g} != {w}"));
            }
        }
        (Value::Object(g), Value::Object(w)) => {
            let mut gk: Vec<&String> = g.keys().collect();
            let mut wk: Vec<&String> = w.keys().collect();
            gk.sort();
            wk.sort();
            if gk != wk {
                diffs.push(format!("{path}: claves {gk:?} != {wk:?}"));
                return;
            }
            for (k, gv) in g {
                compare(gv, &w[k], &format!("{path}.{k}"), diffs);
            }
        }
        (Value::Array(g), Value::Array(w)) => {
            if g.len() != w.len() {
                diffs.push(format!("{path}: longitud {} != {}", g.len(), w.len()));
                return;
            }
            for (i, (gv, wv)) in g.iter().zip(w).enumerate() {
                compare(gv, wv, &format!("{path}[{i}]"), diffs);
            }
        }
        _ => {
            if got != want {
                diffs.push(format!("{path}: {got} != {want}"));
            }
        }
    }
}

#[test]
fn baltanas_matches_reference_implementation() {
    let (event, expected) = load();
    let report = analyze_event(&event, &LostTimeConfig::default());
    let got = serde_json::to_value(&report).unwrap();

    let mut diffs = Vec::new();
    compare(&got, &expected, "$", &mut diffs);
    assert!(
        diffs.is_empty(),
        "{} diferencias; primeras:\n{}",
        diffs.len(),
        diffs[..diffs.len().min(20)].join("\n")
    );
}

#[test]
fn baltanas_sanity_checks() {
    let (event, _) = load();
    let report = analyze_event(&event, &LostTimeConfig::default());
    assert_eq!(report.courses.len(), 9);

    // Todos los resultados del fichero aparecen una vez.
    let runners: usize = report.courses.iter().map(|c| c.runners.len()).sum();
    assert_eq!(runners, 275);

    // Recorrido de M-SEN: 20 balizas, 21 tramos, 18 clasificados, 5 splits por referencia.
    let msen = report
        .courses
        .iter()
        .find(|c| c.classes.iter().any(|class| class.name == "M-SEN"))
        .unwrap();
    assert_eq!(msen.legs.len(), 21);
    assert_eq!(msen.valid_runners, 18);
    assert!(!msen.weak_reference);
    assert!(msen.legs.iter().all(|l| l.reference_count == 5));
    // Referencias cortas (< 20 s): tramos 2, 5 y 21 (este además es el último).
    let short: Vec<usize> = msen
        .legs
        .iter()
        .filter(|l| l.short_reference)
        .map(|l| l.index)
        .collect();
    assert_eq!(short, [2, 5, 21]);

    // El ganador no comete errores: su tiempo sin errores es su tiempo total (977 s).
    let winner = msen.runners.iter().find(|r| r.place == Some(1)).unwrap();
    assert_eq!(winner.total_s, Some(977.0));
    assert_eq!(winner.error_count, 0);
    assert_eq!(winner.time_without_errors_s, Some(977.0));

    // Los no presentados no tienen números.
    for course in &report.courses {
        for r in course
            .runners
            .iter()
            .filter(|r| r.status == RaceStatus::DidNotStart)
        {
            assert_eq!(r.usual_performance, None);
            assert!(r.legs.iter().all(|l| l.split_s.is_none()));
        }
    }
}

#[test]
fn baltanas_ideal_time_as_sum_of_best_splits() {
    let (event, expected) = load();
    let config = LostTimeConfig {
        ideal_time: IdealTime::SumOfBestSplits,
        ..LostTimeConfig::default()
    };
    let report = analyze_event(&event, &config);
    let by_refs = analyze_event(&event, &LostTimeConfig::default());
    let expected_courses = expected["courses"].as_array().unwrap();
    assert_eq!(report.courses.len(), expected_courses.len());

    for ((course, refs_course), exp) in report
        .courses
        .iter()
        .zip(&by_refs.courses)
        .zip(expected_courses)
    {
        let exp_runners = exp["runners"].as_array().unwrap();
        // Ideal acumulado a partir de los splits del oráculo: mejor split de los clasificados.
        let mut ideal = Some(0.0);
        for (leg, refs_leg) in course.legs.iter().zip(&refs_course.legs) {
            let best = exp_runners
                .iter()
                .filter(|r| r["status"] == "ok")
                .filter_map(|r| r["legs"][leg.index - 1]["split_s"].as_f64())
                .min_by(f64::total_cmp);
            ideal = ideal.zip(best).map(|(a, b)| a + b);
            match (leg.ideal_elapsed_s, ideal) {
                (Some(got), Some(want)) => assert!((got - want).abs() < TOLERANCE),
                (got, want) => assert_eq!(got, want),
            }
            // El mejor split nunca es mayor que la referencia (media de los más rápidos).
            if let (Some(best), Some(by_ref)) = (leg.ideal_elapsed_s, refs_leg.ideal_elapsed_s) {
                assert!(best <= by_ref + TOLERANCE);
            }
        }
        for (runner, refs_runner) in course.runners.iter().zip(&refs_course.runners) {
            // Solo cambia la diferencia respecto al ideal.
            assert_eq!(runner.usual_performance, refs_runner.usual_performance);
            assert_eq!(runner.lost_time_s, refs_runner.lost_time_s);
            for (leg, course_leg) in runner.legs.iter().zip(&course.legs) {
                let want = leg
                    .elapsed_s
                    .zip(course_leg.ideal_elapsed_s)
                    .map(|(e, i)| e - i);
                assert_eq!(leg.behind_ideal_s, want);
            }
        }
    }

    // Ganador de M-SEN: con el "superman" va por detrás del ideal al final (no tiene todos
    // los mejores splits), mientras que respecto a la suma de referencias iba 62,8 s por delante.
    let msen = report
        .courses
        .iter()
        .position(|c| c.classes.iter().any(|class| class.name == "M-SEN"))
        .unwrap();
    let winner = |r: &tramos_core::lost_time::CourseAnalysis| {
        r.runners
            .iter()
            .find(|x| x.place == Some(1))
            .and_then(|x| x.legs.last())
            .and_then(|l| l.behind_ideal_s)
            .unwrap()
    };
    assert!(winner(&report.courses[msen]) > 0.0);
    assert!((winner(&by_refs.courses[msen]) + 62.8).abs() < TOLERANCE);
}
