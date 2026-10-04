//! Paridad del tiempo perdido con la implementación de referencia
//! (`tools/reference/tiempo_perdido.py`) sobre el fixture público de Baltanás.
//!
//! El JSON esperado (`fixtures/spl/baltanas-anon.tiempo-perdido.expected.json`) redondea los
//! flotantes a 6 decimales, así que se comparan con una tolerancia absoluta de [`TOLERANCE`].
//! Todo lo demás (claves, enteros, booleanos, textos y `null`) tiene que coincidir exactamente.

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use serde_json::Value;
use tramos_core::importers::spl;
use tramos_core::lost_time::{LostTimeConfig, analyze_event};
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
