//! Lector de FIT sobre el FIT sintético de Baltanás (contra su `.truth.json`) y, si existen,
//! sobre los FIT reales de `fixtures/private/`.

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeDelta, Utc};
use fitparser::de::{FitObject, FitStreamProcessor};
use serde_json::Value;
use tramos_core::importers::fit;
use tramos_core::model::{Track, TrackPoint};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn load_synthetic() -> (Track, Value) {
    let dir = fixtures().join("fit");
    let data = std::fs::read(dir.join("baltanas-sintetico.fit")).unwrap();
    let truth = std::fs::read_to_string(dir.join("baltanas-sintetico.truth.json")).unwrap();
    (
        fit::read(&data).unwrap(),
        serde_json::from_str(&truth).unwrap(),
    )
}

fn instant(v: &Value) -> DateTime<Utc> {
    v.as_str().unwrap().parse().unwrap()
}

fn semicircles(sc: i32) -> f64 {
    f64::from(sc) * 180.0 / 2f64.powi(31)
}

#[test]
fn synthetic_track_matches_truth() {
    let (track, truth) = load_synthetic();
    let points = &track.points;

    assert_eq!(points.len(), 1661);
    assert_eq!(
        points.len() as u64,
        truth["track"]["records"].as_u64().unwrap()
    );
    assert_eq!(points[0].time, instant(&truth["track"]["start_utc"]));
    assert_eq!(points[1660].time, instant(&truth["track"]["end_utc"]));
    assert_eq!(points[0].time.to_rfc3339(), "2026-10-03T16:12:00+00:00");
    assert_eq!(points[1660].time.to_rfc3339(), "2026-10-03T16:39:40+00:00");

    // Un punto por segundo, sin huecos.
    for pair in points.windows(2) {
        assert_eq!(pair[1].time - pair[0].time, TimeDelta::seconds(1));
    }

    // Todos los records traen pulso, cadencia, altitud y distancia.
    assert!(points.iter().all(|p| p.heart_rate_bpm.is_some()));
    assert!(points.iter().all(|p| p.cadence_spm.is_some()));
    assert!(points.iter().all(|p| p.altitude_m.is_some()));
    assert!(points.iter().all(|p| p.distance_m.is_some()));

    let total = truth["track"]["total_distance_m"].as_f64().unwrap();
    assert!((points[1660].distance_m.unwrap() - total).abs() < 0.01);
}

/// Valores crudos de los records 0 y 499 tal y como los escribe `tools/fit_sintetico.py`:
/// semicírculos, altitud con escala 5 y offset 500, distancia en cm y cadencia en rpm de un pie.
#[test]
fn synthetic_points_convert_units() {
    let (track, _) = load_synthetic();
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;

    let first = &track.points[0];
    assert!(close(first.lat, semicircles(500_339_038)));
    assert!(close(first.lon, semicircles(-50_683_956)));
    assert!(close(first.altitude_m.unwrap(), 6447.0 / 5.0 - 500.0));
    assert_eq!(first.heart_rate_bpm, Some(101));
    assert_eq!(first.cadence_spm, Some(2.0 * 58.0));
    assert_eq!(first.distance_m, Some(0.0));

    let p = &track.points[499];
    assert_eq!(p.time.to_rfc3339(), "2026-10-03T16:20:19+00:00");
    assert!(close(p.lat, semicircles(500_325_229)));
    assert!(close(p.lon, semicircles(-50_666_424)));
    assert!(close(p.altitude_m.unwrap(), 6419.0 / 5.0 - 500.0));
    assert_eq!(p.heart_rate_bpm, Some(181));
    assert_eq!(p.cadence_spm, Some(2.0 * 89.0));
    assert!(close(p.distance_m.unwrap(), 1581.47));
}

#[test]
fn synthetic_cadence_is_steps_per_minute_with_both_feet() {
    let (track, truth) = load_synthetic();

    // Sin `fractional_cadence`, la cadencia es el doble de un entero: siempre par.
    for p in &track.points {
        let spm = p.cadence_spm.unwrap();
        assert_eq!(spm % 2.0, 0.0, "{spm} en {}", p.time);
    }

    // Corriendo, un corredor da unas 160–200 zancadas por minuto (no las 80–100 del FIT): la
    // mediana entre la salida y la parada debe caer ahí (cerca de las balizas frena).
    let start = instant(&truth["controls"][0]["punch_utc"]);
    let (stop_start, stop_end) = (
        instant(&truth["stop"]["start_utc"]),
        instant(&truth["stop"]["end_utc"]),
    );
    let mut running: Vec<f64> = track
        .points
        .iter()
        .filter(|p| p.time >= start && p.time < stop_start)
        .filter_map(|p| p.cadence_spm)
        .collect();
    running.sort_by(f64::total_cmp);
    let median = running[running.len() / 2];
    assert!((160.0..=200.0).contains(&median), "mediana {median}");

    // En la parada, sin moverse, cadencia 0.
    for p in track
        .points
        .iter()
        .filter(|p| p.time > stop_start && p.time <= stop_end)
    {
        assert_eq!(p.cadence_spm, Some(0.0), "{}", p.time);
    }
}

#[test]
fn synthetic_track_passes_through_controls_at_punch_times() {
    let (track, truth) = load_synthetic();
    for control in truth["controls"].as_array().unwrap() {
        let k = control["sample_index"].as_u64().unwrap() as usize;
        let p = &track.points[k];
        assert_eq!(p.time, instant(&control["punch_utc"]));
        // La verdad redondea a 7 decimales.
        assert!((p.lat - control["lat"].as_f64().unwrap()).abs() < 1e-7);
        assert!((p.lon - control["lon"].as_f64().unwrap()).abs() < 1e-7);
    }
}

/// FIT reales del usuario en `fixtures/private/` (fuera del repositorio). Si no hay ninguno, el
/// test no comprueba nada. Para revisar el resumen:
///
/// ```bash
/// cargo test -p tramos-core --test fit_import private_fit_files -- --nocapture
/// ```
#[test]
fn private_fit_files_read_cleanly() {
    let dir = fixtures().join("private");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| is_fit(p))
                .collect()
        })
        .unwrap_or_default();
    if files.is_empty() {
        eprintln!("sin FIT en {}: test saltado", dir.display());
        return;
    }
    files.sort();

    for path in files {
        let data = std::fs::read(&path).unwrap();
        let track = fit::read(&data)
            .unwrap_or_else(|e| panic!("{}: no se puede leer: {e}", path.display()));
        let points = &track.points;
        assert!(!points.is_empty(), "{}: sin puntos", path.display());
        for pair in points.windows(2) {
            assert!(
                pair[0].time <= pair[1].time,
                "{}: instantes desordenados en {}",
                path.display(),
                pair[1].time
            );
        }
        for p in points {
            assert!(
                (-90.0..=90.0).contains(&p.lat),
                "{}: lat {}",
                path.display(),
                p.lat
            );
            assert!(
                (-180.0..=180.0).contains(&p.lon),
                "{}: lon {}",
                path.display(),
                p.lon
            );
        }
        eprintln!("{}", summary(&path, &track, record_messages(&data)));
    }
}

fn is_fit(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("fit"))
}

/// Número de mensajes `record` del fichero, con o sin posición. Solo separa los mensajes, sin
/// decodificar sus campos.
fn record_messages(data: &[u8]) -> usize {
    let record = fitparser::profile::MesgNum::Record.as_u16();
    let mut processor = FitStreamProcessor::new();
    let mut input = data;
    let mut count = 0;
    while !input.is_empty() {
        let (rest, object) = processor.deserialize_next(input).unwrap();
        match object {
            FitObject::Crc(_) => processor.reset(),
            FitObject::DataMessage(m) if m.global_message_number() == record => count += 1,
            _ => {}
        }
        input = rest;
    }
    count
}

/// Resumen sin datos personales: recuentos, rangos e instantes, nunca coordenadas.
fn summary(path: &Path, track: &Track, records: usize) -> String {
    let points = &track.points;
    let count = |f: fn(&TrackPoint) -> bool| points.iter().filter(|p| f(p)).count();
    let range = |f: fn(&TrackPoint) -> Option<f64>| {
        let values: Vec<f64> = points.iter().filter_map(f).collect();
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if values.is_empty() {
            "-".to_string()
        } else {
            format!("{min:.1}–{max:.1}")
        }
    };
    let repeated = points.windows(2).filter(|w| w[0].time == w[1].time).count();
    let gaps = points
        .windows(2)
        .filter(|w| w[1].time - w[0].time > TimeDelta::seconds(1))
        .count();
    let (first, last) = (points[0].time, points[points.len() - 1].time);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    format!(
        "{name}: {records} records, {} puntos, {first} – {last} ({} s); instantes repetidos {repeated}, saltos > 1 s \
         {gaps}\n  pulso {} (ppm {}), cadencia {} (pasos/min {}), altitud {} (m {}), \
         distancia {} (m {})",
        points.len(),
        (last - first).num_seconds(),
        count(|p| p.heart_rate_bpm.is_some()),
        range(|p| p.heart_rate_bpm.map(f64::from)),
        count(|p| p.cadence_spm.is_some()),
        range(|p| p.cadence_spm),
        count(|p| p.altitude_m.is_some()),
        range(|p| p.altitude_m),
        count(|p| p.distance_m.is_some()),
        range(|p| p.distance_m),
    )
}
