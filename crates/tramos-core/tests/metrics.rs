//! Métricas de tramo con el FIT sintético de Baltanás y las picadas del .spl anonimizado (y, si
//! existe, con un par real de `fixtures/private/`). El método está en `docs/metricas.md`.

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use serde_json::Value;
use tramos_core::alignment::{AlignmentOptions, align};
use tramos_core::importers::{fit, spl};
use tramos_core::metrics::{LegMetrics, MetricsOptions, leg_metrics};
use tramos_core::model::{RaceResult, Track};
use tramos_core::segmentation::segment;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn truth() -> Value {
    let text =
        std::fs::read_to_string(fixtures().join("fit/baltanas-sintetico.truth.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// FIT sintético y el resultado de su corredor (M-SEN, tarjeta 143, índice 15 en la categoría).
fn load() -> (Track, RaceResult) {
    let data = std::fs::read(fixtures().join("fit/baltanas-sintetico.fit")).unwrap();
    let track = fit::read(&data).unwrap();
    let event =
        spl::read(&std::fs::read(fixtures().join("spl/baltanas-anon.spl")).unwrap()).unwrap();
    let class = event.classes.iter().find(|c| c.name == "M-SEN").unwrap();
    let result = class.results[15].clone();
    assert_eq!(result.runner.si_card, Some(143));
    (track, result)
}

fn metrics(track: &Track, result: &RaceResult, options: &MetricsOptions) -> Vec<LegMetrics> {
    let alignment = align(track, result, &AlignmentOptions::default()).unwrap();
    let segmentation = segment(track, &alignment).unwrap();
    leg_metrics(track, &segmentation, options).unwrap()
}

#[test]
fn synthetic_legs_match_their_truth() {
    let (track, result) = load();
    let truth = truth();
    let legs = metrics(&track, &result, &MetricsOptions::default());
    let expected = truth["legs"].as_array().unwrap();
    assert_eq!(legs.len(), 21);
    assert_eq!(legs.len(), expected.len());
    for (leg, t) in legs.iter().zip(expected) {
        let ctx = format!("tramo {}", leg.index);
        let field = |k: &str| t[k].as_f64().unwrap();
        assert_eq!(u64::from(leg.to), t["to"].as_u64().unwrap(), "{ctx}");
        let m = leg.track.as_ref().unwrap();
        // El desfase estimado es el mismo en los dos extremos: la duración es el split.
        assert!((m.duration_s - field("split_s")).abs() < 0.01, "{ctx}");
        // Distancia del reloj; la línea recta sale de posiciones interpoladas en cada picada,
        // que el desfase (+0,14 s) mueve unos decímetros.
        assert!((m.distance_m - field("path_m")).abs() < 0.5, "{ctx}");
        assert!((m.straight_m - field("straight_m")).abs() < 1.0, "{ctx}");
        // Subida y bajada del terreno (sin ruido) frente a la altitud suavizada del FIT.
        assert!(
            (m.ascent_m.unwrap() - field("ascent_m")).abs() < 1.5,
            "{ctx}"
        );
        assert!(
            (m.descent_m.unwrap() - field("descent_m")).abs() < 1.5,
            "{ctx}"
        );
        assert_eq!(m.gap_s, 0.0, "{ctx}");
        let hr = m.heart_rate_bpm.unwrap();
        let cadence = m.cadence_spm.unwrap();
        assert!((140.0..=190.0).contains(&hr), "{ctx}: {hr}");
        assert!((150.0..=190.0).contains(&cadence), "{ctx}: {cadence}");
    }
}

/// Criterio de aceptación de #13: el tramo del rodeo tiene una relación distancia / línea recta
/// alta y la parada mide 30 ± 2 s.
#[test]
fn detour_and_stop_are_measured() {
    let (track, result) = load();
    let truth = truth();
    let legs = metrics(&track, &result, &MetricsOptions::default());
    let detour_leg = truth["detour"]["leg"].as_u64().unwrap() as usize;
    let stop_leg = truth["stop"]["leg"].as_u64().unwrap() as usize;
    let stop_s = truth["stop"]["duration_s"].as_f64().unwrap();
    for leg in &legs {
        let m = leg.track.as_ref().unwrap();
        let ratio = m.distance_ratio.unwrap();
        if leg.index == detour_leg {
            assert!(ratio > 4.0, "{ratio}");
        } else {
            assert!(ratio < 2.0, "tramo {}: {ratio}", leg.index);
        }
        if leg.index == stop_leg {
            assert!((m.stopped_s - stop_s).abs() <= 2.0, "{}", m.stopped_s);
            // La parada no cuenta en la velocidad en movimiento.
            assert!(m.moving_s <= m.duration_s - stop_s + 2.0);
            let v = m.distance_m / m.moving_s;
            assert!((m.moving_speed_mps.unwrap() - v).abs() < 0.1);
        } else {
            assert!(m.stopped_s < 2.0, "tramo {}: {}", leg.index, m.stopped_s);
        }
    }
}

#[test]
fn legs_without_track_keep_their_reason() {
    let (track, result) = load();
    // Sin los 5 últimos minutos: la meta y las últimas balizas caen fuera del track.
    let mut short = track.clone();
    short.points.truncate(1300);
    let legs = metrics(&short, &result, &MetricsOptions::default());
    let last = legs.last().unwrap();
    assert!(last.track.is_none());
    assert!(last.missing.is_some());
    assert!(legs[0].track.is_some());
}

/// Par real de `fixtures/private/` (fuera del repositorio), como `private_alignment` en
/// `tests/alignment.rs`: se salta si faltan los ficheros o las variables. Solo imprime números
/// por tramo (nunca nombres ni coordenadas) para revisar a ojo que las métricas son razonables:
///
/// ```bash
/// TRAMOS_PRIVATE_CLASS=<categoría> TRAMOS_PRIVATE_RESULT_INDEX=<n> \
///     cargo test -p tramos-core --test metrics private_metrics -- --nocapture
/// ```
#[test]
fn private_metrics() {
    let dir = fixtures().join("private");
    let (fit_path, spl_path) = (
        dir.join("soria-intermedia.fit"),
        dir.join("soria-intermedia.spl"),
    );
    if !fit_path.exists() || !spl_path.exists() {
        eprintln!(
            "sin soria-intermedia.fit/.spl en {}: test saltado",
            dir.display()
        );
        return;
    }
    let (Ok(class_key), Ok(index)) = (
        std::env::var("TRAMOS_PRIVATE_CLASS"),
        std::env::var("TRAMOS_PRIVATE_RESULT_INDEX"),
    ) else {
        eprintln!("faltan TRAMOS_PRIVATE_CLASS o TRAMOS_PRIVATE_RESULT_INDEX: test saltado");
        return;
    };
    let index: usize = index.trim().parse().unwrap();
    let track = fit::read(&std::fs::read(&fit_path).unwrap()).unwrap();
    let event = spl::read(&std::fs::read(&spl_path).unwrap()).unwrap();
    let class = event
        .classes
        .iter()
        .find(|c| {
            c.name == class_key
                || c.short_name.as_deref() == Some(class_key.as_str())
                || c.id.to_string() == class_key
        })
        .unwrap_or_else(|| panic!("no hay ninguna categoría {class_key:?}"));
    let result = class
        .results
        .get(index)
        .unwrap_or_else(|| panic!("la categoría tiene {} resultados", class.results.len()));

    let fmt =
        |v: Option<f64>, decimals: usize| v.map_or("-".to_string(), |v| format!("{v:.decimals$}"));
    for leg in metrics(&track, result, &MetricsOptions::default()) {
        let Some(m) = &leg.track else {
            eprintln!(
                "  tramo {:>2}: sin sub-track ({:?})",
                leg.index, leg.missing
            );
            continue;
        };
        eprintln!(
            "  tramo {:>2}: {:>5.0} s, {:>6.0} m (recta {:>5.0} m, relación {}), parado {:>3.0} s, \
             hueco {:>3.0} s, {} m/s, +{} / -{} m, pulso {}, cadencia {}",
            leg.index,
            m.duration_s,
            m.distance_m,
            m.straight_m,
            fmt(m.distance_ratio, 2),
            m.stopped_s,
            m.gap_s,
            fmt(m.moving_speed_mps, 2),
            fmt(m.ascent_m, 1),
            fmt(m.descent_m, 1),
            fmt(m.heart_rate_bpm, 0),
            fmt(m.cadence_spm, 0),
        );
    }
}
