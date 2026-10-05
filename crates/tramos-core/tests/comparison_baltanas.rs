//! Frente al grupo (P4) sobre el fixture público de Baltanás: criterio de aceptación de #23.
//!
//! La diferencia acumulada del ganador de cada recorrido en meta es su tiempo menos el ideal. Para
//! que no sea circular, el tiempo del ganador y el ideal salen del oráculo de Python
//! (`fixtures/spl/baltanas-anon.tiempo-perdido.expected.json`), no del núcleo.

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use serde_json::Value;
use tramos_core::comparison::course_comparison;
use tramos_core::identify::ResultRef;
use tramos_core::importers::spl;
use tramos_core::lost_time::LostTimeConfig;

const TOLERANCE: f64 = 1e-6;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/spl")
        .join(name)
}

#[test]
fn winner_finishes_behind_the_ideal_by_total_minus_ideal() {
    let event = spl::read(&std::fs::read(fixture("baltanas-anon.spl")).unwrap()).unwrap();
    let oracle: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture("baltanas-anon.tiempo-perdido.expected.json")).unwrap(),
    )
    .unwrap();
    let config = LostTimeConfig::default();

    let mut checked = 0;
    for course in oracle["courses"].as_array().unwrap() {
        let ideal = course["legs"].as_array().unwrap().last().unwrap()["ideal_elapsed_s"].as_f64();
        let winner_total = course["runners"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["status"] == "ok")
            .filter_map(|r| r["total_s"].as_f64())
            .min_by(f64::total_cmp);
        let (Some(ideal), Some(winner_total)) = (ideal, winner_total) else {
            continue;
        };
        // Cualquier corredor del recorrido sirve para pedir la comparación.
        let first = &course["runners"][0];
        let at = ResultRef {
            class_index: first["class_index"].as_u64().unwrap() as usize,
            result_index: first["result_index"].as_u64().unwrap() as usize,
        };
        let comparison = course_comparison(&event, at, &config).unwrap();
        let winner = &comparison.runners[0];
        assert_eq!(winner.course_place, Some(1));
        assert!((winner.total_s.unwrap() - winner_total).abs() <= TOLERANCE);
        assert!((comparison.ideal_time_s.unwrap() - ideal).abs() <= TOLERANCE);
        let at_finish = winner.behind_ideal_s.last().copied().flatten().unwrap();
        assert!(
            (at_finish - (winner_total - ideal)).abs() <= TOLERANCE,
            "{at_finish} != {winner_total} - {ideal}"
        );
        checked += 1;
    }
    assert!(checked > 5, "solo {checked} recorridos comprobados");
}
