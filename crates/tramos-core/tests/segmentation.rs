//! Corte en tramos y posición de las balizas con el FIT sintético de Baltanás y las picadas del
//! .spl anonimizado. El método está en `docs/segmentacion.md`.

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use serde_json::Value;
use tramos_core::alignment::{AlignmentOptions, align};
use tramos_core::importers::{fit, spl};
use tramos_core::model::{FINISH_CODE, RaceResult, START_CODE, Track, TrackPoint};
use tramos_core::segmentation::{
    ControlRole, MissingLegTrack, MissingPosition, Segmentation, median_control_positions, segment,
};

/// Radio medio de la Tierra (el mismo que usa `tools/fit_sintetico.py`).
const EARTH_RADIUS_M: f64 = 6_371_008.8;

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

fn run(track: &Track, result: &RaceResult) -> Segmentation {
    let alignment = align(track, result, &AlignmentOptions::default()).unwrap();
    segment(track, &alignment).unwrap()
}

fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * a.sqrt().asin()
}

fn path_m(points: &[TrackPoint]) -> f64 {
    points
        .windows(2)
        .map(|w| haversine_m(w[0].lat, w[0].lon, w[1].lat, w[1].lon))
        .sum()
}

#[test]
fn synthetic_controls_within_ten_metres_of_truth() {
    let (track, result) = load();
    let s = run(&track, &result);
    let truth = truth();
    let expected = truth["controls"].as_array().unwrap();

    // 20 balizas + salida + meta; 21 tramos.
    assert_eq!(s.controls.len(), expected.len());
    assert_eq!(s.controls.len(), 22);
    assert_eq!(s.legs.len(), 21);
    assert_eq!(s.legs.len(), truth["legs"].as_array().unwrap().len());
    assert_eq!(s.controls[0].role, ControlRole::Start);
    assert_eq!(s.controls[21].role, ControlRole::Finish);

    let mut worst: f64 = 0.0;
    for (control, want) in s.controls.iter().zip(expected) {
        assert_eq!(u64::from(control.code), want["code"].as_u64().unwrap());
        let p = control.point.unwrap();
        assert!(
            p.altitude_m.is_some(),
            "baliza {} sin altitud",
            control.code
        );
        let d = haversine_m(
            p.lat,
            p.lon,
            want["lat"].as_f64().unwrap(),
            want["lon"].as_f64().unwrap(),
        );
        assert!(d < 10.0, "baliza {}: {d:.2} m de la verdad", control.code);
        worst = worst.max(d);
    }
    println!("distancia máxima baliza-verdad: {worst:.3} m");
}

#[test]
fn synthetic_legs_are_continuous() {
    let (track, result) = load();
    let s = run(&track, &result);
    let truth = truth();

    for (leg, want) in s.legs.iter().zip(truth["legs"].as_array().unwrap()) {
        assert_eq!(u64::from(leg.from), want["from"].as_u64().unwrap());
        assert_eq!(u64::from(leg.to), want["to"].as_u64().unwrap());
        let points = &leg.track.as_ref().unwrap().points;
        // Duración del sub-track = split del tramo (el desfase es el mismo en los dos extremos).
        let duration = (points.last().unwrap().time - points[0].time).num_milliseconds() as f64;
        let split = want["split_s"].as_f64().unwrap();
        assert!(
            (duration / 1000.0 - split).abs() < 1e-3,
            "tramo {}",
            leg.index
        );
        // Recorrido del tramo: a menos de 1 m del de la verdad.
        let d = path_m(points);
        let expected = want["path_m"].as_f64().unwrap();
        assert!((d - expected).abs() < 1.0, "tramo {}: {d:.2} m", leg.index);
    }
    // Límites compartidos: el final de cada tramo es el principio del siguiente.
    for pair in s.legs.windows(2) {
        let (a, b) = (
            pair[0].track.as_ref().unwrap(),
            pair[1].track.as_ref().unwrap(),
        );
        assert_eq!(a.points.last(), b.points.first());
    }
    // La suma de los tramos es el recorrido de la salida a la meta.
    let total: f64 = s
        .legs
        .iter()
        .map(|l| path_m(&l.track.as_ref().unwrap().points))
        .sum();
    let (start, finish) = (s.controls[0].time.unwrap(), s.controls[21].time.unwrap());
    let mut whole = vec![s.legs[0].track.as_ref().unwrap().points[0].clone()];
    whole.extend(
        track
            .points
            .iter()
            .filter(|p| p.time > start && p.time < finish)
            .cloned(),
    );
    whole.push(
        s.legs[20]
            .track
            .as_ref()
            .unwrap()
            .points
            .last()
            .unwrap()
            .clone(),
    );
    assert!((total - path_m(&whole)).abs() < 1e-6);
}

#[test]
fn punch_without_time_and_trimmed_track() {
    let (mut track, mut result) = load();
    // La baliza 32 (posición 3) sin hora y el track recortado 10 s antes de la meta (la última
    // baliza, la 100, queda 17 s antes y sigue dentro).
    assert_eq!(result.punches[3].code, 32);
    result.punches[3].time = None;
    let finish = result.punches.last().unwrap().time.unwrap();
    track
        .points
        .retain(|p| p.time < finish - chrono::TimeDelta::seconds(10));

    let s = run(&track, &result);
    assert_eq!(s.legs.len(), 21);
    assert_eq!(s.controls[3].missing, Some(MissingPosition::NoPunchTime));
    for leg in &s.legs[2..4] {
        assert_eq!(leg.missing, Some(MissingLegTrack::NoPunchTime { code: 32 }));
    }
    let last = s.controls.last().unwrap();
    assert_eq!(last.code, FINISH_CODE);
    assert_eq!(last.missing, Some(MissingPosition::OutsideTrack));
    assert_eq!(
        s.legs[20].missing,
        Some(MissingLegTrack::OutsideTrack { code: FINISH_CODE })
    );
    let with_track = s.legs.iter().filter(|l| l.track.is_some()).count();
    assert_eq!(with_track, 21 - 3);
}

#[test]
fn median_of_one_runner_is_its_own_positions() {
    let (track, result) = load();
    let s = run(&track, &result);
    let m = median_control_positions([s.controls.as_slice(), s.controls.as_slice()]);
    assert_eq!(m.len(), 22);
    assert_eq!((m[0].position, m[0].code), (0, START_CODE));
    for (median, control) in m.iter().zip(&s.controls) {
        let p = control.point.unwrap();
        assert_eq!(median.runners, 2);
        assert_eq!(
            (median.lat, median.lon, median.altitude_m),
            (p.lat, p.lon, p.altitude_m)
        );
    }
}
