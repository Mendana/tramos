//! ¿Lento o desorientado? (P2) con el FIT sintético de Baltanás y las picadas del .spl
//! anonimizado. El método está en `docs/tiempo-perdido.md`, "¿Lento o desorientado? (P2)".

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use tramos_core::alignment::{AlignmentOptions, align};
use tramos_core::identify::ResultRef;
use tramos_core::importers::{fit, spl};
use tramos_core::loss_breakdown::race_breakdown;
use tramos_core::lost_time::LostTimeConfig;
use tramos_core::metrics::{MetricsOptions, leg_metrics};
use tramos_core::runner_report::runner_report;
use tramos_core::segmentation::segment;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Criterio de aceptación de #27: el tramo del rodeo (el 9, con 30 s parado) atribuye la mayor
/// parte de su pérdida a desvío y parada.
#[test]
fn the_detour_leg_is_mostly_detour_and_stop() {
    let track =
        fit::read(&std::fs::read(fixtures().join("fit/baltanas-sintetico.fit")).unwrap()).unwrap();
    let event =
        spl::read(&std::fs::read(fixtures().join("spl/baltanas-anon.spl")).unwrap()).unwrap();
    let class_index = event
        .classes
        .iter()
        .position(|c| c.name == "M-SEN")
        .unwrap();
    let at = ResultRef {
        class_index,
        result_index: 15,
    };
    let result = at.get(&event).unwrap();
    assert_eq!(result.runner.si_card, Some(143));

    let alignment = align(&track, result, &AlignmentOptions::default()).unwrap();
    let segmentation = segment(&track, &alignment).unwrap();
    let metrics = leg_metrics(&track, &segmentation, &MetricsOptions::default()).unwrap();
    let report = runner_report(&event, at, &LostTimeConfig::default()).unwrap();
    let b = race_breakdown(&report.lost_time, &metrics);

    // Fuera del rodeo, el corredor sintético va casi en línea recta: r0 algo por encima de 1.
    let r0 = b.usual_ratio.unwrap();
    assert!((1.0..1.5).contains(&r0), "r0 = {r0}");

    let leg = b.legs.iter().find(|l| l.index == 9).unwrap();
    assert!(leg.is_error);
    assert!((leg.stopped_s - 30.0).abs() < 2.0, "{leg:?}");
    assert!(leg.detour_s > 0.0, "{leg:?}");
    assert!(
        leg.detour_s + leg.stopped_s > 0.5 * leg.loss_s,
        "desvío y parada no explican la mayor parte: {leg:?}"
    );
    // Y el desvío pesa más que el ritmo.
    assert!(leg.detour_s > leg.pace_s.abs(), "{leg:?}");
    // Cada tramo reparte exactamente su pérdida.
    for l in &b.legs {
        assert!((l.detour_s + l.stopped_s + l.pace_s - l.loss_s).abs() < 1e-9);
    }
    println!(
        "r0 {r0:.3}; tramo 9: pérdida {:.1} s = desvío {:.1} + paradas {:.1} + ritmo {:.1}",
        leg.loss_s, leg.detour_s, leg.stopped_s, leg.pace_s
    );
}
