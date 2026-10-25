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
//! - La velocidad equivalente en llano (#191) pesa cada trozo en movimiento por el coste de
//!   correr con su pendiente ([`running_cost`]).
//! - El tiempo de ataque (#189) es el que pasa el corredor cerca de la baliza antes de picarla.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::{ControlCode, Track, TrackPoint};
use crate::segmentation::{MissingLegTrack, Segmentation};

/// Radio medio de la Tierra (m), el de `docs/alineacion.md` y el generador del FIT sintético.
pub(crate) const EARTH_RADIUS_M: f64 = 6_371_008.8;
/// Por debajo de esta línea recta (m) no se da la relación distancia / línea recta.
const MIN_STRAIGHT_M: f64 = 1.0;
/// Mayor semiancho del suavizado de la altitud aceptado (s).
const MAX_ALTITUDE_SMOOTHING_S: f64 = 120.0;
/// La pendiente se mide en trozos en movimiento de al menos esta distancia (m): en un intervalo
/// de 1 s (unos 3 m) el ruido de la altitud daría pendientes enormes.
pub const GRADE_CHUNK_M: f64 = 20.0;
/// Pendiente máxima (fracción, ±) del modelo de coste; fuera, se recorta.
pub const MAX_GRADE: f64 = 0.45;

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
    /// Radio (m) de la zona de la baliza para el tiempo de ataque.
    pub attack_radius_m: f64,
}

impl Default for MetricsOptions {
    fn default() -> Self {
        Self {
            stop_speed_mps: 0.5,
            punch_grace_s: 5.0,
            max_gap_s: 10.0,
            altitude_smoothing_s: 5.0,
            attack_radius_m: 50.0,
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
    /// Velocidad equivalente en llano (m/s): la distancia en movimiento pesada por el coste de
    /// correr con la pendiente de cada trozo, entre el tiempo en movimiento. `None` sin
    /// movimiento o si a algún trozo le falta altitud. Ver `docs/metricas.md`.
    #[serde(default)]
    pub flat_speed_mps: Option<f64>,
    /// Subida acumulada con la altitud suavizada (m); `None` sin altitud.
    pub ascent_m: Option<f64>,
    /// Bajada acumulada con la altitud suavizada (m, positiva); `None` sin altitud.
    pub descent_m: Option<f64>,
    /// Pulso medio ponderado por tiempo (ppm); `None` sin pulso.
    pub heart_rate_bpm: Option<f64>,
    /// Cadencia media ponderada por tiempo (pasos/min); `None` sin cadencia.
    pub cadence_spm: Option<f64>,
    /// Tiempo de ataque (s): desde que el corredor entra en la zona de la baliza (a
    /// `attack_radius_m`) en su última aproximación hasta que la pica ([`attack_s`]). `None` si
    /// la línea recta del tramo mide menos de dos radios.
    #[serde(default)]
    pub attack_s: Option<f64>,
}

/// Qué velocidad se compara entre los tramos de una carrera (P8, P14; #191).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceSpeed {
    /// Equivalente en llano: la tienen todos los tramos con velocidad en movimiento.
    Flat,
    /// En movimiento: la carrera no tiene altitud (o le falta en algún tramo).
    Moving,
}

impl RaceSpeed {
    /// La de una carrera. Es la misma para todos sus tramos, para no comparar una velocidad en
    /// llano con una en movimiento.
    pub fn of(metrics: &[LegMetrics]) -> Self {
        let mut tracks = metrics
            .iter()
            .filter_map(|m| m.track.as_ref())
            .filter(|t| t.moving_speed_mps.is_some())
            .peekable();
        if tracks.peek().is_some() && tracks.all(|t| t.flat_speed_mps.is_some()) {
            Self::Flat
        } else {
            Self::Moving
        }
    }

    /// Esa velocidad en un tramo, si es un número positivo.
    pub fn of_track(self, track: &TrackMetrics) -> Option<f64> {
        match self {
            Self::Flat => track.flat_speed_mps,
            Self::Moving => track.moving_speed_mps,
        }
        .filter(|v| v.is_finite() && *v > 0.0)
    }
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
    let altitude = Smoothed::altitude(track, options);
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

pub(crate) fn validate(options: &MetricsOptions) -> Result<(), MetricsError> {
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
    if !(options.attack_radius_m.is_finite() && options.attack_radius_m > 0.0) {
        return Err(MetricsError::InvalidOptions(
            "attack_radius_m debe ser positivo",
        ));
    }
    Ok(())
}

/// Métricas de un sub-track; `None` si tiene menos de dos puntos.
fn track_metrics(
    points: &[TrackPoint],
    altitude: &Smoothed,
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
    let mut flat = FlatDistance::default();

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
            flat.close(altitude);
            continue;
        }
        if d / dt >= options.stop_speed_mps {
            moving_s += dt;
            moving_m += d;
            flat.add(a.time, b.time, d, altitude);
        } else {
            flat.close(altitude);
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

    flat.close(altitude);
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
        flat_speed_mps: flat
            .total()
            .filter(|_| moving_s > 0.0)
            .map(|m| m / moving_s),
        ascent_m,
        descent_m,
        heart_rate_bpm: heart_rate.value(),
        cadence_spm: cadence.value(),
        attack_s: attack_s(points, options.attack_radius_m),
    })
}

/// Tiempo de ataque de un sub-track (s): desde que entra en la zona de la baliza hasta que la
/// pica. La baliza es el último punto del sub-track (su picada).
///
/// La entrada es el primer instante en que baja de `radius_m` tras el último punto a más de dos
/// radios: así no cuenta desde el principio un tramo que sale cerca de su propia baliza o que
/// pasa cerca y se aleja. El instante se interpola entre los dos puntos que cruzan el radio.
/// `None` si la línea recta del tramo mide menos de dos radios: no hay aproximación que medir.
pub fn attack_s(points: &[TrackPoint], radius_m: f64) -> Option<f64> {
    let (first, control) = (points.first()?, points.last()?);
    if haversine_m(first, control) < 2.0 * radius_m {
        return None;
    }
    let distances: Vec<f64> = points.iter().map(|p| haversine_m(p, control)).collect();
    let far = distances.iter().rposition(|&d| d >= 2.0 * radius_m)?;
    let inside = far + distances.get(far..)?.iter().position(|&d| d < radius_m)?;
    let (before, after) = (points.get(inside.checked_sub(1)?)?, points.get(inside)?);
    let (d0, d1) = (distances[inside - 1], distances[inside]);
    let f = if d0 > d1 {
        (d0 - radius_m) / (d0 - d1)
    } else {
        0.0
    };
    let entry = seconds_between(before.time, after.time) * f;
    Some(seconds_between(before.time, control.time) - entry)
}

/// Coste energético de correr con pendiente `grade` (fracción: 0,1 = 10 % de subida), en J por
/// kg y metro: el polinomio de Minetti et al. (2002), válido entre −45 % y +45 %. Fuera de ese
/// rango se recorta. En llano vale 3,6.
pub fn running_cost(grade: f64) -> f64 {
    let i = grade.clamp(-MAX_GRADE, MAX_GRADE);
    ((((155.4 * i - 30.4) * i - 43.3) * i + 46.3) * i + 19.5) * i + 3.6
}

/// Distancia equivalente en llano de los trozos en movimiento de un tramo.
///
/// Los intervalos en movimiento seguidos se juntan en trozos de al menos [`GRADE_CHUNK_M`]; un
/// intervalo parado o un hueco cierra el trozo. Cada trozo vale su distancia por
/// `running_cost(pendiente) / running_cost(0)`, con la pendiente de la altitud suavizada entre
/// su principio y su final.
#[derive(Default)]
struct FlatDistance {
    /// Trozo abierto: principio, final y distancia.
    chunk: Option<(DateTime<Utc>, DateTime<Utc>, f64)>,
    equivalent_m: f64,
    chunks: usize,
    /// Algún trozo sin altitud en sus extremos.
    missing_altitude: bool,
}

impl FlatDistance {
    fn add(&mut self, from: DateTime<Utc>, to: DateTime<Utc>, d: f64, altitude: &Smoothed) {
        let (start, _, total) = self.chunk.unwrap_or((from, to, 0.0));
        self.chunk = Some((start, to, total + d));
        if total + d >= GRADE_CHUNK_M {
            self.close(altitude);
        }
    }

    fn close(&mut self, altitude: &Smoothed) {
        let Some((start, end, d)) = self.chunk.take() else {
            return;
        };
        if d <= 0.0 {
            return;
        }
        match altitude.at(start).zip(altitude.at(end)) {
            Some((a, b)) => {
                self.equivalent_m += d * running_cost((b - a) / d) / running_cost(0.0);
                self.chunks += 1;
            }
            None => self.missing_altitude = true,
        }
    }

    /// Distancia equivalente (m); `None` sin trozos o si a alguno le faltaba altitud.
    fn total(&self) -> Option<f64> {
        (self.chunks > 0 && !self.missing_altitude).then_some(self.equivalent_m)
    }
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

pub(crate) fn seconds_between(a: DateTime<Utc>, b: DateTime<Utc>) -> f64 {
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

/// Serie del track entero (altitud, pulso) suavizada con una media móvil en el tiempo.
pub(crate) struct Smoothed {
    /// Instantes y valores suavizados de los puntos con valor, en orden.
    times: Vec<DateTime<Utc>>,
    values: Vec<f64>,
}

impl Smoothed {
    /// La altitud del track con el suavizado de `options` (`altitude_smoothing_s`).
    pub(crate) fn altitude(track: &Track, options: &MetricsOptions) -> Self {
        Self::new(
            track
                .points
                .iter()
                .filter_map(|p| Some((p.time, p.altitude_m?))),
            options.altitude_smoothing_s,
            options.max_gap_s,
        )
    }

    /// Media de los valores a `window_s` o menos de cada punto, sin cruzar huecos de más de
    /// `max_gap_s` (a un lado y otro de un hueco el terreno puede ser otro). `samples` va en
    /// orden de tiempo.
    pub(crate) fn new(
        samples: impl IntoIterator<Item = (DateTime<Utc>, f64)>,
        window_s: f64,
        max_gap_s: f64,
    ) -> Self {
        let raw: Vec<(DateTime<Utc>, f64)> =
            samples.into_iter().filter(|(_, v)| v.is_finite()).collect();
        // Tramo continuo (sin huecos) de cada punto, para no promediar a través de un hueco.
        let mut run = Vec::with_capacity(raw.len());
        let mut current = 0usize;
        let mut previous: Option<DateTime<Utc>> = None;
        for &(t, _) in &raw {
            if previous.is_some_and(|p| seconds_between(p, t) > max_gap_s) {
                current += 1;
            }
            run.push(current);
            previous = Some(t);
        }

        let w = window_s;
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

    /// Mayor valor suavizado de los puntos entre `from` y `to` (incluidos); `None` si no hay.
    pub(crate) fn max_between(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Option<f64> {
        self.times
            .iter()
            .zip(&self.values)
            .filter(|(t, _)| (from..=to).contains(*t))
            .map(|(_, v)| *v)
            .reduce(f64::max)
    }

    /// Valor suavizado en `t`, interpolado entre los dos puntos que lo rodean; `None` fuera
    /// de la serie o sin valores.
    pub(crate) fn at(&self, t: DateTime<Utc>) -> Option<f64> {
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
    pub(crate) fn climb(&self, points: &[TrackPoint]) -> Option<(f64, f64)> {
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

    /// 40 s a 3 m/s hacia el norte con la altitud subiendo `climb` m por segundo.
    fn slope_run(climb: f64) -> Vec<TrackPoint> {
        (0..=40)
            .map(|s| TrackPoint {
                altitude_m: Some(500.0 + climb * s as f64),
                ..point(s as f64, 3.0 * s as f64)
            })
            .collect()
    }

    /// Sin suavizar: en una pendiente constante la suavizada recorta los bordes.
    fn raw_altitude() -> MetricsOptions {
        MetricsOptions {
            altitude_smoothing_s: 0.0,
            ..MetricsOptions::default()
        }
    }

    #[test]
    fn running_cost_is_minettis_polynomial() {
        assert!(close(running_cost(0.0), 3.6));
        // A mano: 155,4·10⁻⁵ − 30,4·10⁻⁴ − 43,3·10⁻³ + 46,3·10⁻² + 19,5·10⁻¹ + 3,6.
        assert!(close(running_cost(0.1), 5.968214));
        assert!(close(running_cost(-0.1), 2.151706));
        // Fuera de ±45 % se recorta.
        assert!(close(running_cost(0.8), running_cost(MAX_GRADE)));
        assert!(close(running_cost(-0.8), running_cost(-MAX_GRADE)));
    }

    #[test]
    fn flat_speed_weighs_the_slope() {
        // Llano: igual que la velocidad en movimiento.
        let m = run(slope_run(0.0), &raw_altitude());
        assert!((m.flat_speed_mps.unwrap() - 3.0).abs() < 1e-4);
        // Subida del 10 % (0,3 m por cada 3 m): 3 · 5,968214 / 3,6.
        let m = run(slope_run(0.3), &raw_altitude());
        assert!((m.flat_speed_mps.unwrap() - 4.973512).abs() < 1e-3);
        assert!((m.moving_speed_mps.unwrap() - 3.0).abs() < 1e-4);
        // Bajada del 10 %: 3 · 2,151706 / 3,6.
        let m = run(slope_run(-0.3), &raw_altitude());
        assert!((m.flat_speed_mps.unwrap() - 1.793088).abs() < 1e-3);
        // Pendiente del 100 %: se recorta al 45 %.
        let m = run(slope_run(3.0), &raw_altitude());
        let clipped = 3.0 * running_cost(MAX_GRADE) / 3.6;
        assert!((m.flat_speed_mps.unwrap() - clipped).abs() < 1e-3);
    }

    #[test]
    fn flat_speed_ignores_stops_and_needs_altitude() {
        // 0–10 s subiendo al 10 %, 20 s parado y 10 s más en llano: el parón no cuenta y cada
        // trozo lleva su pendiente. 30 m a 5,968214 / 3,6 y 30 m a 1, en 20 s.
        let mut points: Vec<TrackPoint> = (0..=10)
            .map(|s| TrackPoint {
                altitude_m: Some(0.3 * s as f64),
                ..point(s as f64, 3.0 * s as f64)
            })
            .collect();
        points.extend((11..=30).map(|s| TrackPoint {
            altitude_m: Some(3.0),
            ..point(s as f64, 30.0)
        }));
        points.extend((31..=40).map(|s| TrackPoint {
            altitude_m: Some(3.0),
            ..point(s as f64, 30.0 + 3.0 * (s - 30) as f64)
        }));
        let m = run(points.clone(), &raw_altitude());
        let expected = (30.0 * 5.968214 / 3.6 + 30.0) / 20.0;
        assert!((m.flat_speed_mps.unwrap() - expected).abs() < 1e-3);

        // Sin altitud en un trozo, no hay velocidad en llano (no se inventa la pendiente).
        for p in &mut points[35..] {
            p.altitude_m = None;
        }
        assert_eq!(run(points, &raw_altitude()).flat_speed_mps, None);
        // Ni en un track sin altitud.
        assert_eq!(
            run(
                slope_run(0.0)
                    .into_iter()
                    .map(|p| TrackPoint {
                        altitude_m: None,
                        ..p
                    })
                    .collect(),
                &raw_altitude()
            )
            .flat_speed_mps,
            None
        );
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
        let altitude = Smoothed::altitude(&track, &MetricsOptions::default());
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
    fn attack_counts_from_entering_the_control_zone() {
        // 200 m en línea recta a 4 m/s (50 s) y, a 50 m de la baliza, 40 s buscando en
        // círculos a 20 m antes de llegar. La zona (50 m) empieza a los 150 m, a los 37,5 s; se
        // pica a los 90 s: 52,5 s de ataque.
        let mut points: Vec<TrackPoint> =
            (0..=45).map(|s| point(s as f64, 4.0 * s as f64)).collect();
        points.extend(
            (46..=85).map(|s| point(s as f64, 180.0 + if s % 2 == 0 { 5.0 } else { -5.0 })),
        );
        points.extend((86..=90).map(|s| point(s as f64, 180.0 + 4.0 * (s - 85) as f64)));
        let m = run(points, &MetricsOptions::default());
        assert!(close(m.attack_s.unwrap(), 52.5), "{:?}", m.attack_s);

        // Limpio: entra a los 37,5 s y pica a los 50 s.
        let clean: Vec<TrackPoint> = (0..=50).map(|s| point(s as f64, 4.0 * s as f64)).collect();
        assert!(close(
            run(clean, &MetricsOptions::default()).attack_s.unwrap(),
            12.5
        ));
    }

    #[test]
    fn attack_starts_after_the_last_time_far_from_the_control() {
        // Sale hacia la baliza, pasa a 30 m (dentro de la zona) sin verla, se aleja a 150 m y
        // vuelve: el ataque cuenta desde la segunda entrada, no desde la primera.
        let mut points: Vec<TrackPoint> =
            (0..=17).map(|s| point(s as f64, 10.0 * s as f64)).collect();
        // A 170 m (30 m de la baliza en 200 m), se da la vuelta hasta 50 m del origen.
        points.extend((18..=29).map(|s| point(s as f64, 170.0 - 10.0 * (s - 17) as f64)));
        // Y vuelve hasta la baliza, a 200 m.
        points.extend((30..=44).map(|s| point(s as f64, 50.0 + 10.0 * (s - 29) as f64)));
        let m = run(points, &MetricsOptions::default());
        // Segunda entrada en la zona a los 150 m de la salida (50 m de la baliza), a los 39 s;
        // pica a los 44 s.
        assert!(close(m.attack_s.unwrap(), 5.0), "{:?}", m.attack_s);
    }

    #[test]
    fn short_legs_have_no_attack() {
        // 90 m en línea recta: menos de dos radios (100 m).
        let points: Vec<TrackPoint> = (0..=30).map(|s| point(s as f64, 3.0 * s as f64)).collect();
        assert_eq!(
            run(points.clone(), &MetricsOptions::default()).attack_s,
            None
        );
        // Con un radio de 20 m sí: entra a los 70 m (23,3 s) y pica a los 30 s.
        let small = MetricsOptions {
            attack_radius_m: 20.0,
            ..MetricsOptions::default()
        };
        let a = run(points, &small).attack_s.unwrap();
        assert!((a - 20.0 / 3.0).abs() < 1e-3, "{a}");
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
            MetricsOptions {
                attack_radius_m: 0.0,
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
