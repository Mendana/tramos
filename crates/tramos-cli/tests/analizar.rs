//! `tramos analizar` sobre los fixtures públicos de Baltanás: el .spl anonimizado y el FIT
//! sintético del corredor de M-SEN con tarjeta 143 (`fixtures/README.md`).
//!
//! La salida se compara con `fixtures/cli/baltanas-msen-143.expected.json` (generado con la
//! propia CLI). Para que la comparación no sea circular, los números del tiempo perdido se
//! comprueban además contra el oráculo de Python
//! (`fixtures/spl/baltanas-anon.tiempo-perdido.expected.json`) y el desfase contra la respuesta
//! conocida del FIT sintético, construido con las horas reales de picada (desfase 0).

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::{Command, Output};

use chrono::{DateTime, Utc};
use serde_json::Value;

/// Redondeo del oráculo (6 decimales) más margen para el error de coma flotante.
const TOLERANCE: f64 = 1e-6;

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(path)
}

fn tramos(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tramos"))
        .args(args)
        .output()
        .unwrap()
}

/// `tramos analizar --spl <fixture> [args…]`; tiene que acabar bien.
fn analyze(args: &[&str]) -> Output {
    let spl = fixture("spl/baltanas-anon.spl");
    let mut all = vec!["analizar", "--spl", spl.to_str().unwrap()];
    all.extend_from_slice(args);
    let output = tramos(&all);
    assert!(
        output.status.success(),
        "falló: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn analyze_json(args: &[&str]) -> Value {
    serde_json::from_slice(&analyze(args).stdout).unwrap()
}

fn fit_path() -> String {
    fixture("fit/baltanas-sintetico.fit")
        .to_str()
        .unwrap()
        .to_string()
}

fn read_json(path: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(fixture(path)).unwrap()).unwrap()
}

fn f64_at(value: &Value, key: &str) -> Option<f64> {
    value[key].as_f64()
}

/// Compara dos JSON: flotantes con [`TOLERANCE`], instantes con 1 ms y lo demás exacto.
fn compare(got: &Value, want: &Value, path: &str, diffs: &mut Vec<String>) {
    match (got, want) {
        (Value::Number(g), Value::Number(w)) if g.is_f64() || w.is_f64() => {
            let (g, w) = (g.as_f64().unwrap(), w.as_f64().unwrap());
            if (g - w).abs() > TOLERANCE {
                diffs.push(format!("{path}: {g} != {w}"));
            }
        }
        (Value::String(g), Value::String(w)) if g != w => {
            match (g.parse::<DateTime<Utc>>(), w.parse::<DateTime<Utc>>()) {
                (Ok(g), Ok(w)) if (g - w).num_microseconds().unwrap().abs() <= 1000 => {}
                _ => diffs.push(format!("{path}: {g:?} != {w:?}")),
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

fn assert_close(got: Option<f64>, want: Option<f64>, what: &str) {
    match (got, want) {
        (Some(g), Some(w)) => assert!((g - w).abs() <= TOLERANCE, "{what}: {g} != {w}"),
        _ => assert_eq!(got, want, "{what}"),
    }
}

#[test]
fn output_matches_expected_json() {
    let got = analyze_json(&["--fit", &fit_path(), "--corredor", "143"]);
    let want = read_json("cli/baltanas-msen-143.expected.json");
    let mut diffs = Vec::new();
    compare(&got, &want, "$", &mut diffs);
    assert!(diffs.is_empty(), "diferencias:\n{}", diffs.join("\n"));
}

/// Los números del corredor coinciden con los del oráculo de Python (no con la propia CLI).
#[test]
fn lost_time_matches_reference_oracle() {
    let got = analyze_json(&["--corredor", "143"]);
    let oracle = read_json("spl/baltanas-anon.tiempo-perdido.expected.json");
    let truth = read_json("fit/baltanas-sintetico.truth.json");

    let runner = &got["runner"];
    assert_eq!(runner["class_name"], truth["runner"]["class_name"]);
    assert_eq!(runner["class_id"], truth["runner"]["class_id"]);
    assert_eq!(runner["result_index"], truth["runner"]["index_in_class"]);
    assert_eq!(runner["si_card"], 143);
    assert_eq!(runner["place"], truth["runner"]["place"]);
    assert_eq!(runner["status"], "ok");

    let class_index = runner["class_index"].as_u64().unwrap();
    let result_index = runner["result_index"].as_u64().unwrap();
    let course = oracle["courses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| {
            c["classes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|class| class["index"].as_u64() == Some(class_index))
        })
        .unwrap();
    let expected = course["runners"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| {
            r["class_index"].as_u64() == Some(class_index)
                && r["result_index"].as_u64() == Some(result_index)
        })
        .unwrap();

    assert_eq!(got["course"]["controls"], course["course"]["controls"]);
    assert_eq!(got["course"]["classes"], course["classes"]);
    assert_eq!(got["course"]["valid_runners"], course["valid_runners"]);
    assert_eq!(got["course"]["weak_reference"], course["weak_reference"]);

    let lost = &got["lost_time"];
    for key in [
        "total_s",
        "usual_performance",
        "lost_time_s",
        "time_without_errors_s",
    ] {
        assert_close(f64_at(lost, key), f64_at(expected, key), key);
    }
    assert_eq!(lost["error_count"], expected["error_count"]);
    assert_eq!(lost["error_count"], 3);

    let legs = lost["legs"].as_array().unwrap();
    let ref_legs = course["legs"].as_array().unwrap();
    // El JSON del oráculo va en forma compacta: cada tramo es una fila con las columnas de
    // `runner_leg_columns` (`docs/tiempo-perdido.md`).
    let columns = oracle["runner_leg_columns"].as_array().unwrap();
    let runner_legs: Vec<Value> = expected["legs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let row = row.as_array().unwrap();
            assert_eq!(row.len(), columns.len());
            Value::Object(
                columns
                    .iter()
                    .map(|c| c.as_str().unwrap().to_string())
                    .zip(row.iter().cloned())
                    .collect(),
            )
        })
        .collect();
    assert_eq!(legs.len(), 21);
    assert_eq!(legs.len(), ref_legs.len());
    assert_eq!(legs.len(), runner_legs.len());
    for ((leg, reference), runner_leg) in legs.iter().zip(ref_legs).zip(&runner_legs) {
        let n = &leg["index"];
        for key in [
            "index",
            "from",
            "to",
            "valid_splits",
            "reference_count",
            "is_last",
            "short_reference",
            "excluded_from_patterns",
        ] {
            assert_eq!(leg[key], reference[key], "tramo {n}: {key}");
        }
        for key in ["reference_s", "ideal_elapsed_s"] {
            assert_close(
                f64_at(leg, key),
                f64_at(reference, key),
                &format!("tramo {n}: {key}"),
            );
        }
        for key in ["place", "is_error"] {
            assert_eq!(leg[key], runner_leg[key], "tramo {n}: {key}");
        }
        for key in [
            "split_s",
            "elapsed_s",
            "performance_index",
            "expected_s",
            "loss_s",
            "loss_pct",
            "behind_ideal_s",
        ] {
            assert_close(
                f64_at(leg, key),
                f64_at(runner_leg, key),
                &format!("tramo {n}: {key}"),
            );
        }
    }

    // Totales del tiempo ideal: el acumulado del último tramo.
    let last = ref_legs.last().unwrap();
    assert_close(
        f64_at(lost, "ideal_time_s"),
        f64_at(last, "ideal_elapsed_s"),
        "ideal_time_s",
    );
    assert_close(
        f64_at(lost, "behind_ideal_s"),
        f64_at(runner_legs.last().unwrap(), "behind_ideal_s"),
        "behind_ideal_s",
    );
    assert!(got["alignment"].is_null());
}

/// El FIT sintético pasa por cada baliza en su hora de picada: el desfase real es 0.
#[test]
fn alignment_offset_is_close_to_zero() {
    let got = analyze_json(&["--fit", &fit_path(), "--corredor", "143"]);
    let alignment = &got["alignment"];
    let offset = alignment["offset_s"].as_f64().unwrap();
    assert!(offset.abs() <= 2.0, "desfase {offset}");
    assert_eq!(alignment["offset_estimated"], true);
    assert!(alignment["confidence"].as_f64().unwrap() >= 0.8);
    assert!(alignment["warnings"].as_array().unwrap().is_empty());
    // No se vuelcan las picadas situadas en el track.
    assert!(alignment.get("punches").is_none());

    let truth = read_json("fit/baltanas-sintetico.truth.json");
    assert_eq!(
        alignment["coverage"]["track_start"],
        truth["track"]["start_utc"]
    );
    assert_eq!(
        alignment["coverage"]["track_end"],
        truth["track"]["end_utc"]
    );
}

#[test]
fn runner_by_name_is_the_same_as_by_si_card() {
    let by_card = analyze_json(&["--corredor", "143"]);
    // El nombre anonimizado lleva `_`, que la normalización trata como separador.
    for name in ["N143_ Apellido143_______", "apellido143 n143"] {
        let by_name = analyze_json(&["--corredor", name]);
        assert_eq!(by_name, by_card, "--corredor {name:?}");
    }
}

#[test]
fn unknown_runner_fails_with_message() {
    let spl = fixture("spl/baltanas-anon.spl");
    let spl = spl.to_str().unwrap();
    for (args, message) in [
        (
            vec!["analizar", "--spl", spl, "--corredor", "Nadie Nadie"],
            "ningún resultado",
        ),
        (
            vec!["analizar", "--spl", spl, "--corredor", "99999"],
            "tarjeta SI 99999",
        ),
        (vec!["analizar", "--spl", spl], "falta --corredor"),
    ] {
        let output = tramos(&args);
        assert!(!output.status.success(), "{args:?}");
        assert_ne!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}

#[test]
fn best_splits_ideal_changes_only_the_ideal_time() {
    let references = analyze_json(&["--corredor", "143"]);
    let best = analyze_json(&["--corredor", "143", "--ideal", "suma-mejores"]);
    assert_eq!(best["config"]["ideal_time"], "sum_of_best_splits");

    let ideal_ref = references["lost_time"]["ideal_time_s"].as_f64().unwrap();
    let ideal_best = best["lost_time"]["ideal_time_s"].as_f64().unwrap();
    assert!(ideal_best < ideal_ref, "{ideal_best} >= {ideal_ref}");
    let behind_best = best["lost_time"]["behind_ideal_s"].as_f64().unwrap();
    let total = best["lost_time"]["total_s"].as_f64().unwrap();
    assert!((total - ideal_best - behind_best).abs() <= TOLERANCE);

    // Pérdidas y errores no dependen del tiempo ideal.
    for key in ["usual_performance", "lost_time_s", "error_count"] {
        assert_eq!(
            best["lost_time"][key], references["lost_time"][key],
            "{key}"
        );
    }
}

#[test]
fn thresholds_change_errors() {
    let strict = analyze_json(&["--corredor", "143", "--umbral-s", "100"]);
    assert_eq!(strict["config"]["error_threshold_s"], 100.0);
    // Solo el tramo 9 (283,6 s de pérdida) supera 100 s.
    assert_eq!(strict["lost_time"]["error_count"], 1);
}

#[test]
fn table_format_is_text() {
    let output = analyze(&[
        "--fit",
        &fit_path(),
        "--corredor",
        "143",
        "--formato",
        "tabla",
    ]);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(serde_json::from_str::<Value>(&text).is_err());
    assert!(text.contains("M-SEN"));
    assert!(text.contains("Tramo"));
    assert!(text.contains("Alineación del FIT"));
    // Una fila por tramo: empieza por su número.
    let rows: Vec<&str> = text
        .lines()
        .filter(|l| {
            l.trim_start()
                .split(' ')
                .next()
                .is_some_and(|w| w.parse::<u32>().is_ok())
        })
        .collect();
    assert_eq!(rows.len(), 21, "{text}");
    assert_eq!(rows.iter().filter(|r| r.contains("ERROR")).count(), 3);
    assert!(rows[8].contains("42→46") && rows[8].contains("ERROR"));
    assert!(rows[20].contains("100→M") && rows[20].contains("último"));
}
