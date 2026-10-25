//! Rendimiento físico (#212) con el FIT sintético de Baltanás y las picadas del .spl anonimizado
//! (y, si existe, con un par real de `fixtures/private/`). El método está en
//! `docs/rendimiento-fisico.md`.

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use serde_json::Value;
use tramos_core::alignment::{AlignmentOptions, align};
use tramos_core::importers::{fit, spl};
use tramos_core::metrics::{MetricsOptions, leg_metrics};
use tramos_core::model::{RaceResult, Track};
use tramos_core::physical::{BLOCKS, RacePhysical, SpeedKind, race_physical};
use tramos_core::physical_series::{SeriesState, race_series};
use tramos_core::segmentation::{Segmentation, segment};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
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

fn segmentation(track: &Track, result: &RaceResult) -> Segmentation {
    let alignment = align(track, result, &AlignmentOptions::default()).unwrap();
    segment(track, &alignment).unwrap()
}

fn physical(track: &Track, result: &RaceResult, limits: Option<&[f64]>) -> RacePhysical {
    race_physical(
        track,
        &segmentation(track, result),
        &MetricsOptions::default(),
        limits,
    )
    .unwrap()
}

#[test]
fn synthetic_race_adds_up() {
    let (track, result) = load();
    let truth: Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("fit/baltanas-sintetico.truth.json")).unwrap(),
    )
    .unwrap();
    let p = physical(&track, &result, Some(&[150.0, 165.0, 180.0]));
    let legs = leg_metrics(
        &track,
        &segmentation(&track, &result),
        &MetricsOptions::default(),
    )
    .unwrap();
    let sum = |f: fn(&tramos_core::metrics::TrackMetrics) -> f64| -> f64 {
        legs.iter().map(|l| f(l.track.as_ref().unwrap())).sum()
    };

    // Los totales son la suma de las métricas de tramo.
    assert!((p.duration_s - sum(|m| m.duration_s)).abs() < 1e-6);
    assert!((p.distance_m - sum(|m| m.distance_m)).abs() < 1e-6);
    assert!((p.moving_s - sum(|m| m.moving_s)).abs() < 1e-6);
    assert!((p.stopped_s - sum(|m| m.stopped_s)).abs() < 1e-6);
    assert!((p.ascent_m.unwrap() - sum(|m| m.ascent_m.unwrap())).abs() < 1e-6);
    let stop_s = truth["stop"]["duration_s"].as_f64().unwrap();
    assert!((p.stopped_s - stop_s).abs() <= 2.0, "{}", p.stopped_s);

    // El traslado es el movimiento sin el de los ataques.
    assert!(p.transit.transit_s < p.moving_s);
    assert!(p.transit.transit_s >= p.moving_s - p.attack_s - 1e-6);
    assert!(p.transit.distance_m < p.distance_m);

    // Con altitud, pulso y cadencia en todo el track hay todas las cifras.
    assert_eq!(p.speed, SpeedKind::Flat);
    let hr = p.transit.heart_rate_bpm.unwrap();
    assert!((140.0..=190.0).contains(&hr), "{hr}");
    let efficiency = p.transit.efficiency_m_per_beat.unwrap();
    assert!((0.5..=3.0).contains(&efficiency), "{efficiency}");
    let stride = p.transit.stride_m.unwrap();
    assert!((0.5..=2.5).contains(&stride), "{stride}");
    assert!(p.decoupling_pct.is_some());
    assert!(p.heart_rate_max_bpm.unwrap() >= p.heart_rate_bpm.unwrap());

    // Las décimas y las clases de pendiente reparten todo el traslado.
    assert_eq!(p.blocks.len(), BLOCKS);
    let blocks_s: f64 = p.blocks.iter().map(|b| b.stats.transit_s).sum();
    assert!((blocks_s - p.transit.transit_s).abs() < 1e-6);
    assert_eq!(p.by_grade.len(), 5);
    let grades_s: f64 = p.by_grade.iter().map(|g| g.stats.transit_s).sum();
    assert!((grades_s - p.transit.transit_s).abs() < 1e-6);

    // Las zonas reparten el tiempo sin huecos con pulso.
    let zones = p.heart_rate_zones.unwrap();
    assert_eq!(zones.seconds.len(), 4);
    let zones_s: f64 = zones.seconds.iter().sum();
    assert!((zones_s - p.duration_s).abs() < 1.0, "{zones_s}");
}

#[test]
fn without_altitude_it_compares_the_real_speed() {
    let (mut track, result) = load();
    for point in &mut track.points {
        point.altitude_m = None;
    }
    let p = physical(&track, &result, None);
    assert_eq!(p.speed, SpeedKind::Moving);
    assert_eq!(p.transit.flat_speed_mps, None);
    assert!(p.by_grade.is_empty());
    assert_eq!(p.vam_m_per_h, None);
    // La eficiencia sigue, con la velocidad real.
    assert!(p.transit.efficiency_m_per_beat.is_some());
    assert!(p.decoupling_pct.is_some());
}

#[test]
fn series_adds_up_to_the_physical_figures() {
    let (track, result) = load();
    let segmentation = segmentation(&track, &result);
    let options = MetricsOptions::default();
    let p = race_physical(&track, &segmentation, &options, None).unwrap();
    // Un instante por segundo: cada uno cae en un intervalo del FIT (a 1 Hz), así que los segundos
    // de cada estado son los de `race_physical`, salvo por los intervalos partidos en las
    // picadas, que no empiezan en un segundo entero: como mucho 1 s por tramo y estado.
    let s = race_series(&track, &segmentation, &options, 1.0).unwrap();
    assert!(
        (s.duration_s - p.duration_s).abs() < 1.0,
        "{}",
        s.duration_s
    );
    let (last, rest) = s.samples.split_last().unwrap();
    let legs = segmentation
        .legs
        .iter()
        .filter(|l| l.track.is_some())
        .count();
    let seconds = |wanted: SeriesState| rest.iter().filter(|x| x.state == wanted).count() as f64;
    for (state, expected) in [
        (SeriesState::Transit, p.transit.transit_s),
        (SeriesState::Attack, p.attack_s),
        (SeriesState::Gap, p.gap_s),
    ] {
        let diff = (seconds(state) - expected).abs();
        assert!(diff <= legs as f64, "{state:?}: {diff}");
    }
    assert!((last.distance_m - p.distance_m).abs() < 1e-6);
    // Sube y baja de verdad, y la distancia solo crece.
    assert!(
        s.samples
            .windows(2)
            .all(|w| w[1].distance_m >= w[0].distance_m)
    );
    let altitudes: Vec<f64> = s.samples.iter().filter_map(|x| x.altitude_m).collect();
    assert_eq!(altitudes.len(), s.samples.len());
    assert!(altitudes.iter().cloned().fold(f64::MIN, f64::max) > altitudes[0] + 5.0);
    // Pulso máximo de la serie: el de `race_physical` (suavizado igual, pero centrado en los puntos
    // del track y no en los instantes de la serie), con menos de 1 ppm de diferencia.
    let max_hr = s
        .samples
        .iter()
        .filter_map(|x| x.heart_rate_bpm)
        .fold(f64::MIN, f64::max);
    assert!(
        (max_hr - p.heart_rate_max_bpm.unwrap()).abs() < 1.0,
        "{max_hr}"
    );
    assert!(s.samples.iter().all(|x| x.cadence_spm.is_some()));
    // Un tramo por cada tramo del resultado y una picada por baliza con hora.
    let legs = segmentation
        .legs
        .iter()
        .filter(|l| l.track.is_some())
        .count();
    assert_eq!(s.legs.len(), legs);
    assert!(s.controls.len() >= legs);
    assert!(s.controls.windows(2).all(|w| w[1].t_s >= w[0].t_s));
    // Con 5 s se cubre lo mismo con menos puntos.
    let coarse = race_series(&track, &segmentation, &options, 5.0).unwrap();
    assert!(coarse.samples.len() < s.samples.len() / 4);
    assert!((coarse.duration_s - s.duration_s).abs() < 1e-9);
}

/// Par real de `fixtures/private/` (fuera del repositorio), como `private_metrics` en
/// `tests/metrics.rs`: se salta si faltan los ficheros o las variables. Solo imprime cifras de
/// la carrera (nunca nombres ni coordenadas) para revisar a ojo que son razonables:
///
/// ```bash
/// TRAMOS_PRIVATE_CLASS=<categoría> TRAMOS_PRIVATE_RESULT_INDEX=<n> \
///     cargo test -p tramos-core --test physical private_physical -- --nocapture
/// ```
#[test]
fn private_physical() {
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
    let p = physical(&track, result, Some(&[120.0, 140.0, 160.0, 175.0]));
    let mut json = serde_json::to_value(&p).unwrap();
    // Sin instantes ni posiciones: solo cifras.
    json["blocks"] = Value::Null;
    eprintln!("{}", serde_json::to_string_pretty(&json).unwrap());
    for block in &p.blocks {
        let s = &block.stats;
        eprintln!(
            "  décima {}: {:.0} s, {:?} m/s llano, {:?} ppm, {:?} m/latido, {:?} m/paso",
            block.index,
            s.transit_s,
            s.flat_speed_mps.map(|v| (v * 100.0).round() / 100.0),
            s.heart_rate_bpm.map(f64::round),
            s.efficiency_m_per_beat
                .map(|v| (v * 1000.0).round() / 1000.0),
            s.stride_m.map(|v| (v * 100.0).round() / 100.0),
        );
    }
}
