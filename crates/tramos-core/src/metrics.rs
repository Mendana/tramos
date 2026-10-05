//! Métricas de cada tramo a partir del track del reloj.
//!
//! [`leg_metrics`] recorre los sub-tracks de una [`Segmentation`] y da, por tramo, distancia
//! recorrida, línea recta entre balizas, velocidad en movimiento, tiempo parado, subida y bajada
//! con la altitud suavizada, pulso medio y cadencia media. El método está en
//! `docs/metricas.md`. Resumen:
//!
//! - El sub-track se mide por intervalos entre puntos consecutivos. La distancia de cada
//!   intervalo es la del reloj (`distance_m`) si la tienen los dos puntos; si no, la del GPS.
//! - Un intervalo más lento que `stop_speed_mps` es tiempo parado, salvo en los primeros
//!   `punch_grace_s` segundos del tramo (el corredor acaba de picar). Uno más largo que
//!   `max_gap_s` es un hueco: no cuenta como parado ni en movimiento.
//! - La altitud se suaviza con una media móvil en el tiempo sobre el track entero, y la subida
//!   y la bajada son la suma de los cambios de la altitud suavizada dentro del tramo.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::{ControlCode, Track, TrackPoint};
use crate::segmentation::{MissingLegTrack, Segmentation};

/// Radio medio de la Tierra (m), el de `docs/alineacion.md` y el generador del FIT sintético.
const EARTH_RADIUS_M: f64 = 6_371_008.8;
/// Por debajo de esta línea recta (m) no se da la relación distancia / línea recta.
const MIN_STRAIGHT_M: f64 = 1.0;
/// Mayor semiancho del suavizado de la altitud aceptado (s).
const MAX_ALTITUDE_SMOOTHING_S: f64 = 120.0;

/// Parámetros de las métricas. Los valores por defecto son los de `docs/metricas.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MetricsOptions {
    /// Velocidad (m/s) por debajo de la cual el corredor está parado.
    pub stop_speed_mps: f64,
    /// Segundos tras la picada de salida del tramo que no cuentan como parado.
    pub punch_grace_s: f64,
    /// Dos puntos seguidos separados más que esto (s) forman un hueco.
    pub max_gap_s: f64,
    /// Semiancho (s) de la media móvil con la que se suaviza la altitud.
    pub altitude_smoothing_s: f64,
}

impl Default for MetricsOptions {
    fn default() -> Self {
        Self {
            stop_speed_mps: 0.5,
            punch_grace_s: 5.0,
            max_gap_s: 10.0,
            altitude_smoothing_s: 5.0,
        }
    }
}

/// Errores de las métricas.
#[derive(Debug, Error, PartialEq)]
pub enum MetricsError {
    #[error("opciones de las métricas inválidas: {0}")]
    InvalidOptions(&'static str),
}

/// Métricas de un tramo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegMetrics {
    /// Número del tramo, empezando en 1, como en [`crate::segmentation::LegTrack`].
    pub index: usize,
    pub from: ControlCode,
    pub to: ControlCode,
    /// Métricas del sub-track; `None` si el tramo no tiene sub-track (ver `missing`).
    pub track: Option<TrackMetrics>,
    /// Por qué no hay sub-track.
    pub missing: Option<MissingLegTrack>,
}

/// Métricas calculadas sobre el sub-track de un tramo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackMetrics {
    /// Del primer al último punto del sub-track (s): el split, con el desfase del reloj.
    pub duration_s: f64,
    /// Distancia recorrida (m).
    pub distance_m: f64,
    /// Línea recta entre las posiciones de las dos balizas (m).
    pub straight_m: f64,
    /// `distance_m / straight_m`; `None` si la línea recta mide menos de 1 m.
    pub distance_ratio: Option<f64>,
    /// Tiempo en movimiento (s): intervalos a `stop_speed_mps` o más.
    pub moving_s: f64,
    /// Tiempo parado (s): intervalos más lentos que `stop_speed_mps`, sin los primeros
    /// `punch_grace_s` segundos del tramo.
    pub stopped_s: f64,
    /// Tiempo en huecos del track (s): intervalos de más de `max_gap_s`.
    pub gap_s: f64,
    /// Distancia en movimiento / tiempo en movimiento (m/s); `None` si no hay movimiento.
    pub moving_speed_mps: Option<f64>,
    /// Subida acumulada con la altitud suavizada (m); `None` sin altitud.
    pub ascent_m: Option<f64>,
    /// Bajada acumulada con la altitud suavizada (m, positiva); `None` sin altitud.
    pub descent_m: Option<f64>,
    /// Pulso medio ponderado por tiempo (ppm); `None` sin pulso.
    pub heart_rate_bpm: Option<f64>,
    /// Cadencia media ponderada por tiempo (pasos/min); `None` sin cadencia.
    pub cadence_spm: Option<f64>,
}

/// Calcula las métricas de cada tramo de `segmentation`, que debe salir de `track`.
///
/// `track` (el track entero) solo se usa para suavizar la altitud sin efectos de borde en cada
/// tramo. Un tramo sin sub-track sale con `track: None` y el motivo de la segmentación.
pub fn leg_metrics(
    track: &Track,
    segmentation: &Segmentation,
    options: &MetricsOptions,
) -> Result<Vec<LegMetrics>, MetricsError> {
    validate(options)?;
    let altitude = SmoothedAltitude::build(track, options);
    Ok(segmentation
        .legs
        .iter()
        .map(|leg| LegMetrics {
            index: leg.index,
            from: leg.from,
            to: leg.to,
            track: leg
                .track
                .as_ref()
                .and_then(|t| track_metrics(&t.points, &altitude, options)),
            missing: leg.missing,
        })
        .collect())
}

fn validate(options: &MetricsOptions) -> Result<(), MetricsError> {
    let non_negative = |v: f64| v.is_finite() && v >= 0.0;
    if !(options.stop_speed_mps.is_finite() && options.stop_speed_mps > 0.0) {
        return Err(MetricsError::InvalidOptions(
            "stop_speed_mps debe ser positiva",
        ));
    }
    if !non_negative(options.punch_grace_s) {
        return Err(MetricsError::InvalidOptions(
            "punch_grace_s no puede ser negativo",
        ));
    }
    if !(options.max_gap_s.is_finite() && options.max_gap_s >= 1.0) {
        return Err(MetricsError::InvalidOptions(
            "max_gap_s debe ser al menos 1 s",
        ));
    }
    if !(non_negative(options.altitude_smoothing_s)
        && options.altitude_smoothing_s <= MAX_ALTITUDE_SMOOTHING_S)
    {
        return Err(MetricsError::InvalidOptions(
            "altitude_smoothing_s debe estar entre 0 y 120 s",
        ));
    }
    Ok(())
}

/// Métricas de un sub-track; `None` si tiene menos de dos puntos.
fn track_metrics(
    points: &[TrackPoint],
    altitude: &SmoothedAltitude,
    options: &MetricsOptions,
) -> Option<TrackMetrics> {
    if points.len() < 2 {
        return None;
    }
    let (first, last) = (points.first()?, points.last()?);
    let grace_end = options.punch_grace_s;

    let mut distance_m = 0.0;
    let mut moving_s = 0.0;
    let mut moving_m = 0.0;
    let mut stopped_s = 0.0;
    let mut gap_s = 0.0;
    let mut heart_rate = WeightedMean::default();
    let mut cadence = WeightedMean::default();

    for pair in points.windows(2) {
        let [a, b] = pair else { continue };
        let dt = seconds_between(a.time, b.time);
        let d = interval_distance_m(a, b);
        distance_m += d;
        if dt <= 0.0 {
            continue;
        }
        if dt > options.max_gap_s {
            gap_s += dt;
            continue;
        }
        if d / dt >= options.stop_speed_mps {
            moving_s += dt;
            moving_m += d;
        } else {
            // Solo la parte del intervalo posterior a los primeros `punch_grace_s` del tramo.
            let start = seconds_between(first.time, a.time).max(grace_end);
            stopped_s += (seconds_between(first.time, b.time) - start).max(0.0);
        }
        if let (Some(u), Some(v)) = (a.heart_rate_bpm, b.heart_rate_bpm) {
            heart_rate.add((f64::from(u) + f64::from(v)) / 2.0, dt);
        }
        if let (Some(u), Some(v)) = (a.cadence_spm, b.cadence_spm) {
            cadence.add((u + v) / 2.0, dt);
        }
    }

    let straight_m = haversine_m(first, last);
    let (ascent_m, descent_m) = altitude.climb(points).unzip();
    Some(TrackMetrics {
        duration_s: seconds_between(first.time, last.time),
        distance_m,
        straight_m,
        distance_ratio: (straight_m >= MIN_STRAIGHT_M).then(|| distance_m / straight_m),
        moving_s,
        stopped_s,
        gap_s,
        moving_speed_mps: (moving_s > 0.0).then(|| moving_m / moving_s),
        ascent_m,
        descent_m,
        heart_rate_bpm: heart_rate.value(),
        cadence_spm: cadence.value(),
    })
}

/// Distancia de un intervalo: la del reloj si la tienen los dos puntos y no retrocede; si no,
/// la del GPS (ver "Intervalos" en `docs/metricas.md`).
pub fn interval_distance_m(a: &TrackPoint, b: &TrackPoint) -> f64 {
    match (a.distance_m, b.distance_m) {
        (Some(u), Some(v)) if v >= u => v - u,
        _ => haversine_m(a, b),
    }
}

/// Distancia de círculo máximo entre dos puntos (m).
pub(crate) fn haversine_m(a: &TrackPoint, b: &TrackPoint) -> f64 {
    let (lat1, lat2) = (a.lat.to_radians(), b.lat.to_radians());
    let dlat = lat2 - lat1;
    let dlon = (b.lon - a.lon).to_radians();
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_M * h.sqrt().min(1.0).asin()
}

fn seconds_between(a: DateTime<Utc>, b: DateTime<Utc>) -> f64 {
    (b - a)
        .num_microseconds()
        .map_or(f64::NAN, |us| us as f64 / 1e6)
}

/// Media ponderada por tiempo.
#[derive(Default)]
struct WeightedMean {
    total: f64,
    weight: f64,
}

impl WeightedMean {
    fn add(&mut self, value: f64, weight: f64) {
        self.total += value * weight;
        self.weight += weight;
    }

    fn value(&self) -> Option<f64> {
        (self.weight > 0.0).then(|| self.total / self.weight)
    }
}

/// Altitud del track entero suavizada con una media móvil en el tiempo.
struct SmoothedAltitude {
    /// Instantes y altitudes suavizadas de los puntos con altitud, en orden.
    times: Vec<DateTime<Utc>>,
    values: Vec<f64>,
}

impl SmoothedAltitude {
    /// Media de las altitudes a `altitude_smoothing_s` o menos de cada punto, sin cruzar huecos
    /// de más de `max_gap_s` (a un lado y otro de un hueco el terreno puede ser otro).
    fn build(track: &Track, options: &MetricsOptions) -> Self {
        let raw: Vec<(DateTime<Utc>, f64)> = track
            .points
            .iter()
            .filter_map(|p| Some((p.time, p.altitude_m?)))
            .filter(|(_, alt)| alt.is_finite())
            .collect();
        // Tramo continuo (sin huecos) de cada punto, para no promediar a través de un hueco.
        let mut run = Vec::with_capacity(raw.len());
        let mut current = 0usize;
        let mut previous: Option<DateTime<Utc>> = None;
        for &(t, _) in &raw {
            if previous.is_some_and(|p| seconds_between(p, t) > options.max_gap_s) {
                current += 1;
            }
            run.push(current);
            previous = Some(t);
        }

        let w = options.altitude_smoothing_s;
        let (mut lo, mut hi, mut sum) = (0usize, 0usize, 0.0);
        let mut values = Vec::with_capacity(raw.len());
        for (i, &(t, _)) in raw.iter().enumerate() {
            // Ventana [lo, hi): puntos del mismo tramo continuo a `w` s o menos de `t`.
            while hi < raw.len() && run[hi] == run[i] && seconds_between(t, raw[hi].0) <= w {
                sum += raw[hi].1;
                hi += 1;
            }
            while lo < i && (run[lo] != run[i] || seconds_between(raw[lo].0, t) > w) {
                sum -= raw[lo].1;
                lo += 1;
            }
            values.push(sum / (hi - lo) as f64);
        }
        Self {
            times: raw.iter().map(|(t, _)| *t).collect(),
            values,
        }
    }

    /// Altitud suavizada en `t`, interpolada entre los dos puntos que lo rodean; `None` fuera
    /// del tramo con altitud o sin altitud.
    fn at(&self, t: DateTime<Utc>) -> Option<f64> {
        let after = self.times.partition_point(|&p| p < t);
        if self.times.get(after) == Some(&t) {
            return self.values.get(after).copied();
        }
        let before = after.checked_sub(1)?;
        let (t0, t1) = (self.times[before], *self.times.get(after)?);
        let (v0, v1) = (self.values[before], self.values[after]);
        let f = seconds_between(t0, t) / seconds_between(t0, t1);
        Some(v0 + f * (v1 - v0))
    }

    /// Subida y bajada acumuladas de la altitud suavizada en los instantes de `points`.
    fn climb(&self, points: &[TrackPoint]) -> Option<(f64, f64)> {
        let series: Vec<f64> = points.iter().filter_map(|p| self.at(p.time)).collect();
        if series.len() < 2 {
            return None;
        }
        Some(series.windows(2).fold((0.0, 0.0), |(up, down), pair| {
            let change = pair[1] - pair[0];
            (up + change.max(0.0), down + (-change).max(0.0))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segmentation::LegTrack;
    use chrono::TimeDelta;

    /// Metros por grado de latitud con `EARTH_RADIUS_M`.
    const M_PER_DEG: f64 = EARTH_RADIUS_M * std::f64::consts::PI / 180.0;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-03T16:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    /// Punto a `s` segundos de `t0` y `north_m` metros al norte de (40°, −4°).
    fn point(s: f64, north_m: f64) -> TrackPoint {
        TrackPoint {
            time: t0() + TimeDelta::milliseconds((s * 1000.0) as i64),
            lat: 40.0 + north_m / M_PER_DEG,
            lon: -4.0,
            altitude_m: None,
            heart_rate_bpm: None,
            cadence_spm: None,
            distance_m: None,
        }
    }

    fn one_leg(points: Vec<TrackPoint>) -> (Track, Segmentation) {
        let track = Track {
            points,
            sport: None,
        };
        let segmentation = Segmentation {
            controls: Vec::new(),
            legs: vec![LegTrack {
                index: 1,
                from: 31,
                to: 32,
                track: Some(track.clone()),
                missing: None,
            }],
        };
        (track, segmentation)
    }

    fn run(points: Vec<TrackPoint>, options: &MetricsOptions) -> TrackMetrics {
        let (track, segmentation) = one_leg(points);
        let legs = leg_metrics(&track, &segmentation, options).unwrap();
        legs[0].track.clone().unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn distance_speed_and_stops() {
        // 0–10 s: 3 m/s. 10–30 s: parado. 30–40 s: 3 m/s. Total 60 m en 40 s.
        let mut points: Vec<TrackPoint> =
            (0..=10).map(|s| point(s as f64, 3.0 * s as f64)).collect();
        points.extend((11..=30).map(|s| point(s as f64, 30.0)));
        points.extend((31..=40).map(|s| point(s as f64, 30.0 + 3.0 * (s - 30) as f64)));
        let m = run(points, &MetricsOptions::default());
        assert!(close(m.duration_s, 40.0));
        assert!((m.distance_m - 60.0).abs() < 1e-3);
        assert!((m.straight_m - 60.0).abs() < 1e-3);
        assert!((m.distance_ratio.unwrap() - 1.0).abs() < 1e-6);
        assert!(close(m.moving_s, 20.0));
        assert!(close(m.stopped_s, 20.0));
        assert!((m.moving_speed_mps.unwrap() - 3.0).abs() < 1e-4);
        assert_eq!(m.gap_s, 0.0);
        assert_eq!(
            (m.ascent_m, m.heart_rate_bpm, m.cadence_spm),
            (None, None, None)
        );
    }

    #[test]
    fn stops_right_after_the_punch_do_not_count() {
        // Parado los 8 primeros segundos y luego a 3 m/s: solo cuentan 8 − 5 = 3 s.
        let mut points: Vec<TrackPoint> = (0..=8).map(|s| point(s as f64, 0.0)).collect();
        points.extend((9..=20).map(|s| point(s as f64, 3.0 * (s - 8) as f64)));
        let m = run(points.clone(), &MetricsOptions::default());
        assert!(close(m.stopped_s, 3.0));
        assert!(close(m.moving_s, 12.0));
        // El margen es configurable.
        let none = MetricsOptions {
            punch_grace_s: 0.0,
            ..MetricsOptions::default()
        };
        assert!(close(run(points, &none).stopped_s, 8.0));
    }

    #[test]
    fn gaps_are_neither_moving_nor_stopped() {
        // 0–5 s a 2 m/s, hueco de 20 s (40 m en recta), 25–30 s a 2 m/s.
        let mut points: Vec<TrackPoint> =
            (0..=5).map(|s| point(s as f64, 2.0 * s as f64)).collect();
        points.extend((25..=30).map(|s| point(s as f64, 50.0 + 2.0 * (s - 25) as f64)));
        let m = run(points, &MetricsOptions::default());
        assert!(close(m.gap_s, 20.0));
        assert!(close(m.moving_s, 10.0));
        assert_eq!(m.stopped_s, 0.0);
        assert!((m.distance_m - 60.0).abs() < 1e-3);
        assert!((m.moving_speed_mps.unwrap() - 2.0).abs() < 1e-4);
    }

    #[test]
    fn watch_distance_wins_over_gps() {
        // El GPS dice 10 m por segundo; el reloj, 3 m (filtra el ruido). Se usa el reloj, salvo
        // donde falta en algún punto o retrocede.
        let mut points: Vec<TrackPoint> = (0..=4)
            .map(|s| TrackPoint {
                distance_m: Some(3.0 * s as f64),
                ..point(s as f64, 10.0 * s as f64)
            })
            .collect();
        assert!((run(points.clone(), &MetricsOptions::default()).distance_m - 12.0).abs() < 1e-9);
        points[4].distance_m = None;
        assert!((run(points.clone(), &MetricsOptions::default()).distance_m - 19.0).abs() < 1e-3);
        points[4].distance_m = Some(1.0);
        assert!((run(points, &MetricsOptions::default()).distance_m - 19.0).abs() < 1e-3);
    }

    #[test]
    fn no_ratio_when_controls_coincide() {
        // Ida y vuelta: 20 m recorridos y 0 m en línea recta.
        let points = vec![point(0.0, 0.0), point(5.0, 10.0), point(10.0, 0.0)];
        let m = run(points, &MetricsOptions::default());
        assert!((m.distance_m - 20.0).abs() < 1e-3);
        assert!(m.straight_m < 1e-6);
        assert_eq!(m.distance_ratio, None);
    }

    #[test]
    fn heart_rate_and_cadence_are_time_weighted() {
        // Puntos irregulares (intervalos de 1 s y 3 s); cada intervalo vale la media de sus extremos.
        let mut points = vec![point(0.0, 0.0), point(1.0, 3.0), point(4.0, 12.0)];
        for (p, (hr, cadence)) in points
            .iter_mut()
            .zip([(100, 100.0), (160, 100.0), (160, 160.0)])
        {
            p.heart_rate_bpm = Some(hr);
            p.cadence_spm = Some(cadence);
        }
        let m = run(points, &MetricsOptions::default());
        // Pulso: 130 durante 1 s y 160 durante 3 s. Cadencia: 100 durante 1 s y 130 durante 3 s.
        assert!(close(
            m.heart_rate_bpm.unwrap(),
            (130.0 + 3.0 * 160.0) / 4.0
        ));
        assert!(close(m.cadence_spm.unwrap(), (100.0 + 3.0 * 130.0) / 4.0));
    }

    #[test]
    fn altitude_is_smoothed_before_climbing() {
        // Cuesta de 0,5 m/s durante 20 s (10 m) con ruido de ±1 m alterno: sin suavizar, el
        // ruido suma metros de subida y de bajada; con la media móvil, casi solo la cuesta.
        let points: Vec<TrackPoint> = (0..=20)
            .map(|s| TrackPoint {
                altitude_m: Some(0.5 * s as f64 + if s % 2 == 0 { 1.0 } else { -1.0 }),
                ..point(s as f64, 3.0 * s as f64)
            })
            .collect();
        let raw = MetricsOptions {
            altitude_smoothing_s: 0.0,
            ..MetricsOptions::default()
        };
        // Sin suavizar: 10 intervalos de +2,5 m y 10 de −1,5 m.
        let m = run(points.clone(), &raw);
        assert!(close(m.ascent_m.unwrap(), 25.0));
        assert!(close(m.descent_m.unwrap(), 15.0));
        // Con ±5 s, la altitud suavizada va de 1,25 a 8,75 m (ventanas recortadas en los bordes)
        // y sin bajadas.
        let m = run(points, &MetricsOptions::default());
        assert!(close(m.ascent_m.unwrap(), 7.5), "{:?}", m.ascent_m);
        assert_eq!(m.descent_m, Some(0.0));
    }

    #[test]
    fn smoothing_does_not_cross_gaps() {
        // 0–10 s a 100 m de altitud, hueco de 30 s, 40–50 s a 200 m: cada lado conserva su
        // altitud; solo el salto del hueco suma subida.
        let mut points: Vec<TrackPoint> =
            (0..=10).map(|s| point(s as f64, 3.0 * s as f64)).collect();
        points.extend((40..=50).map(|s| point(s as f64, 3.0 * s as f64)));
        for p in &mut points {
            p.altitude_m = Some(if p.time < t0() + TimeDelta::seconds(20) {
                100.0
            } else {
                200.0
            });
        }
        let (track, _) = one_leg(points.clone());
        let altitude = SmoothedAltitude::build(&track, &MetricsOptions::default());
        assert_eq!(altitude.at(t0() + TimeDelta::seconds(10)), Some(100.0));
        assert_eq!(altitude.at(t0() + TimeDelta::seconds(40)), Some(200.0));
        assert_eq!(altitude.at(t0() + TimeDelta::seconds(25)), Some(150.0));
        assert_eq!(altitude.at(t0() + TimeDelta::seconds(51)), None);
        let m = run(points, &MetricsOptions::default());
        assert!(close(m.ascent_m.unwrap(), 100.0));
        assert_eq!(m.descent_m, Some(0.0));
    }

    #[test]
    fn legs_without_track_or_points_have_no_metrics() {
        let (track, mut segmentation) = one_leg(vec![point(0.0, 0.0)]);
        segmentation.legs.push(LegTrack {
            index: 2,
            from: 32,
            to: 33,
            track: None,
            missing: Some(MissingLegTrack::OutOfOrder),
        });
        let legs = leg_metrics(&track, &segmentation, &MetricsOptions::default()).unwrap();
        assert_eq!(legs[0].track, None);
        assert_eq!(legs[1].track, None);
        assert_eq!(legs[1].missing, Some(MissingLegTrack::OutOfOrder));
        assert_eq!((legs[1].index, legs[1].from, legs[1].to), (2, 32, 33));
    }

    #[test]
    fn invalid_options_are_rejected() {
        let (track, segmentation) = one_leg(vec![point(0.0, 0.0), point(1.0, 1.0)]);
        let bad = [
            MetricsOptions {
                stop_speed_mps: 0.0,
                ..MetricsOptions::default()
            },
            MetricsOptions {
                punch_grace_s: -1.0,
                ..MetricsOptions::default()
            },
            MetricsOptions {
                max_gap_s: 0.5,
                ..MetricsOptions::default()
            },
            MetricsOptions {
                altitude_smoothing_s: f64::NAN,
                ..MetricsOptions::default()
            },
            MetricsOptions {
                altitude_smoothing_s: 121.0,
                ..MetricsOptions::default()
            },
        ];
        for options in bad {
            assert!(matches!(
                leg_metrics(&track, &segmentation, &options),
                Err(MetricsError::InvalidOptions(_))
            ));
        }
    }

    #[test]
    fn json_uses_snake_case_and_partial_options() {
        let (track, segmentation) = one_leg(vec![point(0.0, 0.0), point(1.0, 3.0)]);
        let legs = leg_metrics(&track, &segmentation, &MetricsOptions::default()).unwrap();
        let value = serde_json::to_value(&legs[0]).unwrap();
        let speed = value["track"]["moving_speed_mps"].as_f64().unwrap();
        assert!((speed - 3.0).abs() < 1e-4);
        assert!(value["track"].get("distance_ratio").is_some());
        assert_eq!(value["missing"], serde_json::Value::Null);
        let options: MetricsOptions = serde_json::from_str(r#"{"stop_speed_mps": 0.8}"#).unwrap();
        assert_eq!(
            options,
            MetricsOptions {
                stop_speed_mps: 0.8,
                ..MetricsOptions::default()
            }
        );
    }
}
