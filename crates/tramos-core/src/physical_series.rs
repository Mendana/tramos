//! Serie de la carrera para las gráficas de la pestaña Físico (#213): el track de la salida a la
//! meta cada pocos segundos, con la altitud, la velocidad, el pulso y la cadencia suavizados, el
//! tramo y el estado de cada instante. El método está en `docs/rendimiento-fisico.md`
//! ("Serie de la carrera"). Resumen:
//!
//! - Cada instante de la serie cae en un intervalo del track, y el intervalo tiene el estado que
//!   le da [`crate::physical::race_physical`]: hueco, ataque, parado o traslado. La interfaz
//!   dibuja lo que recibe y no vuelve a decidir qué es cada cosa.
//! - La velocidad es la distancia recorrida en una ventana de ±[`SPEED_WINDOW_S`] (como el ritmo
//!   del mapa) sin cruzar huecos. La velocidad en llano la multiplica por el ajuste de la
//!   pendiente de esa zona ([`crate::metrics::grade_factor`]).
//! - El pulso y la cadencia son la media de las muestras a ±5 s.

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::metrics::{
    GRADE_CHUNK_M, MetricsError, MetricsOptions, Smoothed, attack_s, grade_factor,
    interval_distance_m, seconds_between, validate,
};
use crate::model::{ControlCode, Track, TrackPoint};
use crate::physical::HEART_RATE_SMOOTHING_S;
use crate::segmentation::Segmentation;

/// Semiancho (s) de la ventana con la que se mide la velocidad: el mismo que el ritmo del mapa.
pub const SPEED_WINDOW_S: f64 = 5.0;
/// Semiancho (s) de la media móvil de la cadencia.
pub const CADENCE_SMOOTHING_S: f64 = 5.0;
/// Intervalo mínimo (s) entre los instantes de la serie.
pub const MIN_STEP_S: f64 = 1.0;
/// Cuántas veces se puede ensanchar la ventana de la pendiente (×2 cada vez) hasta reunir los
/// [`GRADE_CHUNK_M`] de distancia que piden los trozos de pendiente.
const GRADE_WINDOW_DOUBLINGS: usize = 2;

/// Qué es un instante de la carrera, con las definiciones de `race_physical`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesState {
    /// En movimiento fuera de la zona de ataque: lo que cuenta como físico.
    Transit,
    /// Dentro de la zona de ataque de la baliza (parado o en movimiento).
    Attack,
    /// Parado (menos de `stop_speed_mps`) fuera de la zona de ataque.
    Stopped,
    /// Sin datos: hueco del track o tramo sin sub-track.
    Gap,
}

/// Un instante de la serie. Todo lo que no se puede dar sale `None`; en un hueco, todo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesSample {
    /// Segundos desde la salida.
    pub t_s: f64,
    /// Metros recorridos desde la salida, como `RacePhysical::distance_m`.
    pub distance_m: f64,
    /// Altitud suavizada (m); `None` sin altitud.
    pub altitude_m: Option<f64>,
    /// Velocidad real (m/s).
    pub speed_mps: Option<f64>,
    /// Velocidad equivalente en llano (m/s); `None` sin altitud o sin pendiente medible.
    pub flat_speed_mps: Option<f64>,
    /// Pulso suavizado (ppm); `None` sin pulso a ±[`HEART_RATE_SMOOTHING_S`].
    pub heart_rate_bpm: Option<f64>,
    /// Cadencia suavizada (pasos/min).
    pub cadence_spm: Option<f64>,
    /// Zancada (m por paso): velocidad real entre cadencia suavizada; `None` sin cadencia o
    /// parado (menos de `stop_speed_mps`), donde los pasos no avanzan.
    pub stride_m: Option<f64>,
    /// Número del tramo (empezando en 1); `None` en un hueco.
    pub leg: Option<usize>,
    pub state: SeriesState,
}

/// Un tramo con track, situado en la serie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesLeg {
    /// Número del tramo, empezando en 1, como en la tabla de tramos.
    pub index: usize,
    pub from: ControlCode,
    pub to: ControlCode,
    /// Principio y final (s desde la salida).
    pub from_s: f64,
    pub to_s: f64,
    /// Principio y final (m desde la salida).
    pub from_m: f64,
    pub to_m: f64,
}

/// Una picada situada en la serie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesControl {
    /// 0 = salida, 1 = primera baliza del recorrido, …, la última = meta.
    pub position: usize,
    pub code: ControlCode,
    pub t_s: f64,
    pub distance_m: f64,
}

/// La serie de una carrera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceSeries {
    /// Segundos entre un instante y el siguiente (el último puede quedar más cerca).
    pub step_s: f64,
    /// Duración de la serie (s): de la salida a la meta, con los huecos dentro.
    pub duration_s: f64,
    /// Un instante cada `step_s`, más el de la meta; vacía sin track.
    pub samples: Vec<SeriesSample>,
    /// Tramos con track, en orden.
    pub legs: Vec<SeriesLeg>,
    /// Picadas con hora que caen dentro de la serie, en orden.
    pub controls: Vec<SeriesControl>,
}

/// Intervalo entre dos puntos seguidos de un tramo, en segundos desde la salida.
struct Interval {
    start_s: f64,
    end_s: f64,
    /// Metros recorridos antes del intervalo.
    before_m: f64,
    distance_m: f64,
    state: SeriesState,
    leg: usize,
    /// Tramo continuo (sin huecos) al que pertenece.
    run: usize,
}

/// Los intervalos de la carrera y lo que hace falta para medir en cualquier instante.
struct Timeline {
    intervals: Vec<Interval>,
    /// Principio y final (s) de cada tramo continuo, por número de `Interval::run`.
    runs: Vec<(f64, f64)>,
    total_m: f64,
}

impl Timeline {
    /// Metros recorridos en el instante `t_s`, interpolados dentro de su intervalo.
    fn distance_at(&self, t_s: f64) -> f64 {
        let i = self.intervals.partition_point(|iv| iv.end_s <= t_s);
        match self.intervals.get(i) {
            None => self.total_m,
            Some(iv) if t_s <= iv.start_s => iv.before_m,
            Some(iv) => iv.before_m + iv.distance_m * (t_s - iv.start_s) / (iv.end_s - iv.start_s),
        }
    }

    /// El intervalo que contiene `t_s`; el último en la meta. `None` en un hueco entre tramos.
    fn interval_at(&self, t_s: f64) -> Option<&Interval> {
        let i = self.intervals.partition_point(|iv| iv.end_s <= t_s);
        let iv = self.intervals.get(i).or_else(|| self.intervals.last())?;
        (t_s >= iv.start_s).then_some(iv)
    }

    /// Ventana `[t − half, t + half]` recortada al tramo continuo del intervalo.
    fn window(&self, iv: &Interval, t_s: f64, half_s: f64) -> Option<(f64, f64)> {
        let &(first, last) = self.runs.get(iv.run)?;
        let (t0, t1) = ((t_s - half_s).max(first), (t_s + half_s).min(last));
        (t1 > t0).then_some((t0, t1))
    }

    /// Velocidad real en la ventana de `t_s` (m/s).
    fn speed(&self, iv: &Interval, t_s: f64) -> Option<f64> {
        let (t0, t1) = self.window(iv, t_s, SPEED_WINDOW_S)?;
        Some((self.distance_at(t1) - self.distance_at(t0)) / (t1 - t0))
    }

    /// Pendiente (fracción) de la zona de `t_s` con la altitud suavizada: la de la ventana de la
    /// velocidad o, si no llega a [`GRADE_CHUNK_M`], una más ancha (hasta el cuádruple).
    fn grade(
        &self,
        iv: &Interval,
        t_s: f64,
        altitude: &impl Fn(f64) -> Option<f64>,
    ) -> Option<f64> {
        (0..=GRADE_WINDOW_DOUBLINGS).find_map(|k| {
            let (t0, t1) = self.window(iv, t_s, SPEED_WINDOW_S * f64::from(1u32 << k))?;
            let distance = self.distance_at(t1) - self.distance_at(t0);
            (distance >= GRADE_CHUNK_M).then_some(())?;
            Some((altitude(t1)? - altitude(t0)?) / distance)
        })
    }
}

/// Serie de la carrera cada `step_s` segundos (al menos [`MIN_STEP_S`]): `segmentation` debe
/// salir de `track`. Las opciones son las de [`crate::physical::race_physical`]: deciden qué es
/// parado, hueco y ataque.
pub fn race_series(
    track: &Track,
    segmentation: &Segmentation,
    options: &MetricsOptions,
    step_s: f64,
) -> Result<RaceSeries, MetricsError> {
    validate(options)?;
    if !(step_s.is_finite() && step_s >= MIN_STEP_S) {
        return Err(MetricsError::InvalidOptions(
            "el intervalo de la serie debe ser de 1 s o más",
        ));
    }
    let legs: Vec<(usize, &[TrackPoint])> = segmentation
        .legs
        .iter()
        .filter_map(|leg| Some((leg.index, leg.track.as_ref()?.points.as_slice())))
        .filter(|(_, points)| points.len() >= 2)
        .collect();
    let empty = RaceSeries {
        step_s,
        duration_s: 0.0,
        samples: Vec::new(),
        legs: Vec::new(),
        controls: Vec::new(),
    };
    let Some(start) = series_start(segmentation) else {
        return Ok(empty);
    };
    let secs = |time: DateTime<Utc>| seconds_between(start, time);
    let at = |t_s: f64| start + TimeDelta::microseconds((t_s * 1e6).round() as i64);

    // Los intervalos, con el estado que les da `race_physical`.
    let mut timeline = Timeline {
        intervals: Vec::new(),
        runs: Vec::new(),
        total_m: 0.0,
    };
    let mut previous: Option<(f64, bool)> = None;
    for &(leg, points) in &legs {
        let Some(last) = points.last() else {
            continue;
        };
        let entry = attack_s(points, options.attack_radius_m)
            .map(|s| last.time - TimeDelta::microseconds((s * 1e6) as i64));
        for pair in points.windows(2) {
            let [a, b] = pair else { continue };
            let (dt, d) = (seconds_between(a.time, b.time), interval_distance_m(a, b));
            let before_m = timeline.total_m;
            timeline.total_m += d;
            if dt <= 0.0 {
                continue;
            }
            let gap = dt > options.max_gap_s;
            let state = if gap {
                SeriesState::Gap
            } else if entry.is_some_and(|e| a.time >= e) {
                SeriesState::Attack
            } else if d / dt >= options.stop_speed_mps {
                SeriesState::Transit
            } else {
                SeriesState::Stopped
            };
            let (start_s, end_s) = (secs(a.time), secs(b.time));
            // Un hueco, o un tramo sin track entre dos tramos, corta el tramo continuo.
            let apart = previous
                .is_none_or(|(end, was_gap)| gap || was_gap || start_s - end > options.max_gap_s);
            match timeline.runs.last_mut() {
                Some(run) if !apart => run.1 = end_s,
                _ => timeline.runs.push((start_s, end_s)),
            }
            timeline.intervals.push(Interval {
                start_s,
                end_s,
                before_m,
                distance_m: d,
                state,
                leg,
                run: timeline.runs.len().saturating_sub(1),
            });
            previous = Some((end_s, gap));
        }
    }
    let Some(duration_s) = timeline.intervals.last().map(|iv| iv.end_s) else {
        return Ok(empty);
    };

    let altitude = Smoothed::altitude(track, options);
    let altitude_at = |t_s: f64| altitude.at(at(t_s));
    let series_of = |value: fn(&TrackPoint) -> Option<f64>| -> Vec<(f64, f64)> {
        track
            .points
            .iter()
            .filter_map(|p| Some((secs(p.time), value(p)?)))
            .collect()
    };
    let heart_rate = series_of(|p| p.heart_rate_bpm.map(f64::from));
    let cadence = series_of(|p| p.cadence_spm);

    let mut times: Vec<f64> = (0..=(duration_s / step_s).floor() as usize)
        .map(|k| k as f64 * step_s)
        .collect();
    if times.last().is_none_or(|&t| duration_s - t > 1e-9) {
        times.push(duration_s);
    }
    let samples = times
        .into_iter()
        .map(|t_s| {
            let distance_m = timeline.distance_at(t_s);
            let Some(iv) = timeline
                .interval_at(t_s)
                .filter(|iv| iv.state != SeriesState::Gap)
            else {
                return SeriesSample {
                    t_s,
                    distance_m,
                    altitude_m: None,
                    speed_mps: None,
                    flat_speed_mps: None,
                    heart_rate_bpm: None,
                    cadence_spm: None,
                    stride_m: None,
                    leg: None,
                    state: SeriesState::Gap,
                };
            };
            let speed_mps = timeline.speed(iv, t_s);
            let altitude_m = altitude_at(t_s);
            let flat_speed_mps = speed_mps.zip(altitude_m).and_then(|(speed, _)| {
                let grade = timeline.grade(iv, t_s, &altitude_at)?;
                Some(speed * grade_factor(grade))
            });
            let cadence_spm = window_mean(&cadence, t_s, CADENCE_SMOOTHING_S);
            let stride_m = speed_mps
                .zip(cadence_spm)
                .filter(|&(speed, cadence)| speed >= options.stop_speed_mps && cadence > 0.0)
                .map(|(speed, cadence)| speed * 60.0 / cadence);
            SeriesSample {
                t_s,
                distance_m,
                altitude_m,
                speed_mps,
                flat_speed_mps,
                heart_rate_bpm: window_mean(&heart_rate, t_s, HEART_RATE_SMOOTHING_S),
                cadence_spm,
                stride_m,
                leg: Some(iv.leg),
                state: iv.state,
            }
        })
        .collect();

    let series_legs = segmentation
        .legs
        .iter()
        .filter_map(|leg| {
            let points = &leg.track.as_ref()?.points;
            let (first, last) = (points.first()?, points.last()?);
            (points.len() >= 2).then_some(())?;
            let (from_s, to_s) = (secs(first.time), secs(last.time));
            Some(SeriesLeg {
                index: leg.index,
                from: leg.from,
                to: leg.to,
                from_s,
                to_s,
                from_m: timeline.distance_at(from_s),
                to_m: timeline.distance_at(to_s),
            })
        })
        .collect();
    let controls = segmentation
        .controls
        .iter()
        .filter_map(|control| {
            let t_s = secs(control.time?);
            (0.0..=duration_s).contains(&t_s).then(|| SeriesControl {
                position: control.position,
                code: control.code,
                t_s,
                distance_m: timeline.distance_at(t_s),
            })
        })
        .collect();

    Ok(RaceSeries {
        step_s,
        duration_s,
        samples,
        legs: series_legs,
        controls,
    })
}

/// Instante de la salida de la serie: el primer punto del primer tramo con track.
fn series_start(segmentation: &Segmentation) -> Option<DateTime<Utc>> {
    segmentation
        .legs
        .iter()
        .filter_map(|leg| leg.track.as_ref())
        .find(|track| track.points.len() >= 2)
        .and_then(|track| track.points.first())
        .map(|p| p.time)
}

/// Posición `(latitud, longitud)` en grados de cada instante de `series`, en su orden: la del
/// track interpolada en línea recta entre los dos puntos que rodean el instante, como hace la
/// segmentación con las picadas. `None` en los instantes de hueco, donde no hay track que
/// interpolar. `series` debe salir de `race_series` con este `track` y esta `segmentation`.
pub fn series_positions(
    track: &Track,
    segmentation: &Segmentation,
    series: &RaceSeries,
) -> Vec<Option<(f64, f64)>> {
    let Some(start) = series_start(segmentation) else {
        return vec![None; series.samples.len()];
    };
    series
        .samples
        .iter()
        .map(|sample| {
            if sample.state == SeriesState::Gap {
                return None;
            }
            let time = start + TimeDelta::microseconds((sample.t_s * 1e6).round() as i64);
            let after = track.points.partition_point(|p| p.time <= time);
            let next = track.points.get(after);
            let Some(previous) = after.checked_sub(1).and_then(|i| track.points.get(i)) else {
                return next.map(|p| (p.lat, p.lon));
            };
            let Some(next) = next else {
                return Some((previous.lat, previous.lon));
            };
            let span = seconds_between(previous.time, next.time);
            let f = if span > 0.0 {
                seconds_between(previous.time, time) / span
            } else {
                0.0
            };
            Some((
                previous.lat + f * (next.lat - previous.lat),
                previous.lon + f * (next.lon - previous.lon),
            ))
        })
        .collect()
}

/// Media de las muestras `(t, valor)` (en orden de `t`) a `half_s` o menos de `t_s`; `None` si no
/// hay ninguna.
fn window_mean(samples: &[(f64, f64)], t_s: f64, half_s: f64) -> Option<f64> {
    let lo = samples.partition_point(|&(t, _)| t < t_s - half_s);
    let hi = samples.partition_point(|&(t, _)| t <= t_s + half_s);
    let window = samples.get(lo..hi)?;
    (!window.is_empty()).then(|| window.iter().map(|&(_, v)| v).sum::<f64>() / window.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::EARTH_RADIUS_M;
    use crate::physical::race_physical;
    use crate::segmentation::{ControlPosition, ControlRole, LegTrack};

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

    /// Punto quieto con la distancia del reloj: sin línea recta, no hay ataque.
    fn watch(s: f64, distance_m: f64) -> TrackPoint {
        TrackPoint {
            distance_m: Some(distance_m),
            ..point(s, 0.0)
        }
    }

    fn leg(index: usize, points: Vec<TrackPoint>) -> LegTrack {
        LegTrack {
            index,
            from: 30 + index as ControlCode,
            to: 31 + index as ControlCode,
            track: Some(Track {
                points,
                sport: None,
            }),
            missing: None,
        }
    }

    fn one_leg(points: Vec<TrackPoint>) -> (Track, Segmentation) {
        let track = Track {
            points: points.clone(),
            sport: None,
        };
        let segmentation = Segmentation {
            controls: Vec::new(),
            legs: vec![leg(1, points)],
        };
        (track, segmentation)
    }

    /// Sin suavizar: en una pendiente constante la suavizada recorta los bordes.
    fn raw_altitude() -> MetricsOptions {
        MetricsOptions {
            altitude_smoothing_s: 0.0,
            ..MetricsOptions::default()
        }
    }

    fn series(points: Vec<TrackPoint>, step_s: f64) -> RaceSeries {
        let (track, segmentation) = one_leg(points);
        race_series(&track, &segmentation, &raw_altitude(), step_s).unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    /// 60 s a 4 m/s en llano a 500 m, con pulso 150 y cadencia 180.
    fn steady() -> Vec<TrackPoint> {
        (0..=60)
            .map(|s| TrackPoint {
                altitude_m: Some(500.0),
                heart_rate_bpm: Some(150),
                cadence_spm: Some(180.0),
                ..watch(s as f64, 4.0 * s as f64)
            })
            .collect()
    }

    #[test]
    fn steady_run_every_five_seconds() {
        let s = series(steady(), 5.0);
        assert!(close(s.step_s, 5.0));
        assert!(close(s.duration_s, 60.0));
        assert_eq!(s.samples.len(), 13);
        for (k, sample) in s.samples.iter().enumerate() {
            assert!(close(sample.t_s, 5.0 * k as f64));
            assert!(close(sample.distance_m, 20.0 * k as f64));
            assert!(close(sample.speed_mps.unwrap(), 4.0));
            // En llano, la velocidad en llano es la real.
            assert!(close(sample.flat_speed_mps.unwrap(), 4.0));
            assert!(close(sample.altitude_m.unwrap(), 500.0));
            assert!(close(sample.heart_rate_bpm.unwrap(), 150.0));
            assert!(close(sample.cadence_spm.unwrap(), 180.0));
            // 4 m/s a 180 pasos/min: 4 · 60 / 180 m por paso.
            assert!(close(sample.stride_m.unwrap(), 4.0 * 60.0 / 180.0));
            assert_eq!(sample.leg, Some(1));
            assert_eq!(sample.state, SeriesState::Transit);
        }
        assert_eq!(s.legs.len(), 1);
        assert_eq!((s.legs[0].index, s.legs[0].from, s.legs[0].to), (1, 31, 32));
        assert!(close(s.legs[0].from_s, 0.0) && close(s.legs[0].to_s, 60.0));
        assert!(close(s.legs[0].from_m, 0.0) && close(s.legs[0].to_m, 240.0));
    }

    #[test]
    fn the_last_instant_is_the_finish() {
        // 62 s: 0, 5, …, 60 y la meta en 62.
        let points = (0..=62).map(|s| watch(s as f64, 4.0 * s as f64)).collect();
        let s = series(points, 5.0);
        assert_eq!(s.samples.len(), 14);
        let last = s.samples.last().unwrap();
        assert!(close(last.t_s, 62.0));
        assert!(close(last.distance_m, 248.0));
        assert_eq!(last.state, SeriesState::Transit);
    }

    #[test]
    fn smooths_heart_rate_and_cadence_at_five_seconds() {
        // Pulso 150 hasta el segundo 29 y 170 desde el 30; cadencia 180 y 190.
        let points = (0..=60)
            .map(|s| TrackPoint {
                heart_rate_bpm: Some(if s < 30 { 150 } else { 170 }),
                cadence_spm: Some(if s < 30 { 180.0 } else { 190.0 }),
                ..watch(s as f64, 4.0 * s as f64)
            })
            .collect();
        let s = series(points, 5.0);
        let at = |t: usize| &s.samples[t / 5];
        assert!(close(at(20).heart_rate_bpm.unwrap(), 150.0));
        // A ±5 s del segundo 30: 25–29 a 150 (5 muestras) y 30–35 a 170 (6).
        assert!(close(
            at(30).heart_rate_bpm.unwrap(),
            (5.0 * 150.0 + 6.0 * 170.0) / 11.0
        ));
        assert!(close(
            at(30).cadence_spm.unwrap(),
            (5.0 * 180.0 + 6.0 * 190.0) / 11.0
        ));
        assert!(close(at(45).heart_rate_bpm.unwrap(), 170.0));
    }

    #[test]
    fn stride_needs_movement() {
        // 30 s a 4 m/s con 180 pasos/min y 30 s parado, con la cadencia del reloj a 60.
        let points = (0..=60)
            .map(|s| TrackPoint {
                cadence_spm: Some(if s <= 30 { 180.0 } else { 60.0 }),
                ..watch(s as f64, 4.0 * s.min(30) as f64)
            })
            .collect();
        let s = series(points, 10.0);
        assert!(close(s.samples[1].stride_m.unwrap(), 4.0 * 60.0 / 180.0));
        assert!(close(s.samples[5].speed_mps.unwrap(), 0.0));
        assert!(close(s.samples[5].cadence_spm.unwrap(), 60.0));
        assert_eq!(s.samples[5].stride_m, None);
    }

    #[test]
    fn missing_data_is_none_not_a_number() {
        let points = (0..=60).map(|s| watch(s as f64, 4.0 * s as f64)).collect();
        let s = series(points, 10.0);
        for sample in &s.samples {
            assert_eq!(sample.altitude_m, None);
            assert_eq!(sample.flat_speed_mps, None);
            assert_eq!(sample.heart_rate_bpm, None);
            assert_eq!(sample.cadence_spm, None);
            assert_eq!(sample.stride_m, None);
            assert!(close(sample.speed_mps.unwrap(), 4.0));
        }
    }

    /// Como `transit_leaves_out_stops_and_the_attack` de `physical.rs`: 0–60 s a 3 m/s hacia el
    /// norte (180 m), 10 s parado y 70–90 s a 3 m/s hasta la baliza, a 240 m. La zona de ataque
    /// (50 m) empieza en el metro 190, a 73,33 s: el intervalo de 73 a 74 aún es traslado.
    fn stop_and_attack() -> Vec<TrackPoint> {
        let mut points: Vec<TrackPoint> =
            (0..=60).map(|s| point(s as f64, 3.0 * s as f64)).collect();
        points.extend((61..=70).map(|s| point(s as f64, 180.0)));
        points.extend((71..=90).map(|s| point(s as f64, 180.0 + 3.0 * (s - 70) as f64)));
        points
    }

    #[test]
    fn states_are_those_of_race_physical() {
        let (track, segmentation) = one_leg(stop_and_attack());
        let options = raw_altitude();
        let s = race_series(&track, &segmentation, &options, 1.0).unwrap();
        assert_eq!(s.samples.len(), 91);
        let state = |t: usize| s.samples[t].state;
        assert!((0..60).all(|t| state(t) == SeriesState::Transit));
        assert!((60..70).all(|t| state(t) == SeriesState::Stopped));
        assert!((70..74).all(|t| state(t) == SeriesState::Transit));
        // Del segundo 74 a la meta, ataque.
        assert!((74..=90).all(|t| state(t) == SeriesState::Attack));
        // Parado, la velocidad es 0 m/s.
        assert!(close(s.samples[65].speed_mps.unwrap(), 0.0));
        assert!(close(s.samples[30].speed_mps.unwrap(), 3.0));

        // Cada intervalo de 1 s es un instante: los segundos de cada estado son los de
        // `race_physical` (sin el instante de la meta, que cierra el último intervalo).
        let physical = race_physical(&track, &segmentation, &options, None).unwrap();
        let seconds = |wanted: SeriesState| {
            s.samples[..s.samples.len() - 1]
                .iter()
                .filter(|x| x.state == wanted)
                .count() as f64
        };
        assert!(close(
            seconds(SeriesState::Transit),
            physical.transit.transit_s
        ));
        assert!(close(seconds(SeriesState::Attack), physical.attack_s));
        // Esta parada es lejos de la picada de salida: `stopped_s` la cuenta entera.
        assert!(close(seconds(SeriesState::Stopped), physical.stopped_s));
        assert!(close(seconds(SeriesState::Gap), physical.gap_s));
    }

    #[test]
    fn gaps_have_no_data_and_do_not_leak_into_the_speed() {
        // 0–5 s a 2 m/s, hueco de 20 s (40 m en recta), 25–30 s a 2 m/s.
        let mut points: Vec<TrackPoint> =
            (0..=5).map(|s| point(s as f64, 2.0 * s as f64)).collect();
        points.extend((25..=30).map(|s| point(s as f64, 50.0 + 2.0 * (s - 25) as f64)));
        let (track, segmentation) = one_leg(points);
        let options = raw_altitude();
        let s = race_series(&track, &segmentation, &options, 1.0).unwrap();
        assert_eq!(s.samples.len(), 31);
        for t in 5..25 {
            let sample = &s.samples[t];
            assert_eq!(sample.state, SeriesState::Gap);
            assert_eq!((sample.speed_mps, sample.leg), (None, None));
            assert_eq!(sample.altitude_m, None);
        }
        // La ventana de ±5 s se recorta al tramo continuo: el hueco no entra en la velocidad.
        assert!(close(s.samples[3].speed_mps.unwrap(), 2.0));
        assert!(close(s.samples[26].speed_mps.unwrap(), 2.0));
        // La distancia sí cuenta el hueco, como `RacePhysical::distance_m`: 10 m + 40 m.
        assert!((s.samples[15].distance_m - 30.0).abs() < 1e-3);
        assert!((s.samples[25].distance_m - 50.0).abs() < 1e-3);
        assert!((s.samples[30].distance_m - 60.0).abs() < 1e-3);
        let physical = race_physical(&track, &segmentation, &options, None).unwrap();
        assert!(close(physical.gap_s, 20.0));
        assert_eq!(
            s.samples
                .iter()
                .filter(|x| x.state == SeriesState::Gap)
                .count(),
            20
        );
    }

    #[test]
    fn flat_speed_weights_the_grade() {
        // 100 s a 4 m/s en llano y 100 s subiendo al 10 % (0,4 m/s).
        let altitude = |s: f64| 500.0 + 0.4 * (s - 100.0).max(0.0);
        let points = (0..=200)
            .map(|s| TrackPoint {
                altitude_m: Some(altitude(s as f64)),
                ..watch(s as f64, 4.0 * s as f64)
            })
            .collect();
        let s = series(points, 10.0);
        let at = |t: usize| &s.samples[t / 10];
        assert!(close(at(50).flat_speed_mps.unwrap(), 4.0));
        assert!(close(at(50).altitude_m.unwrap(), 500.0));
        // Subida del 10 %: 4 · 1,459 = 5,836 m/s en llano.
        assert!((at(150).flat_speed_mps.unwrap() - 5.836).abs() < 1e-9);
        assert!(close(at(150).speed_mps.unwrap(), 4.0));
        assert!(close(at(150).altitude_m.unwrap(), 520.0));
        // En el cambio de pendiente, la ventana ve medio llano y media subida: 2 m en 40 m.
        let expected = 4.0 * grade_factor(0.05);
        assert!((at(100).flat_speed_mps.unwrap() - expected).abs() < 1e-9);
    }

    #[test]
    fn slow_walking_widens_the_grade_window() {
        // 1 m/s al 10 %: ±5 s son 10 m y ±10 s, 20 m: la pendiente se mide a ±10 s.
        let points = (0..=120)
            .map(|s| TrackPoint {
                altitude_m: Some(500.0 + 0.1 * s as f64),
                ..watch(s as f64, s as f64)
            })
            .collect();
        let s = series(points, 20.0);
        let sample = &s.samples[3];
        assert!(close(sample.speed_mps.unwrap(), 1.0));
        let expected = grade_factor(0.1);
        assert!((sample.flat_speed_mps.unwrap() - expected).abs() < 1e-9);
    }

    #[test]
    fn legs_and_controls_are_placed_in_the_series() {
        // Dos tramos de 30 s a 4 m/s; la baliza intermedia se pica en el segundo 30.
        let all: Vec<TrackPoint> = (0..=60).map(|s| watch(s as f64, 4.0 * s as f64)).collect();
        let control = |position: usize, code: ControlCode, role: ControlRole, s: Option<f64>| {
            ControlPosition {
                position,
                code,
                role,
                time: s.map(|s| point(s, 0.0).time),
                point: None,
                in_gap: false,
                missing: None,
            }
        };
        let track = Track {
            points: all.clone(),
            sport: None,
        };
        let segmentation = Segmentation {
            controls: vec![
                control(0, 31, ControlRole::Start, Some(0.0)),
                control(1, 32, ControlRole::Control, Some(30.0)),
                control(2, 33, ControlRole::Finish, Some(60.0)),
                control(3, 34, ControlRole::Control, None),
                control(4, 35, ControlRole::Control, Some(90.0)),
            ],
            legs: vec![leg(1, all[..=30].to_vec()), leg(2, all[30..].to_vec())],
        };
        let s = race_series(&track, &segmentation, &raw_altitude(), 10.0).unwrap();
        assert_eq!(s.legs.len(), 2);
        assert!(close(s.legs[1].from_s, 30.0) && close(s.legs[1].to_s, 60.0));
        assert!(close(s.legs[1].from_m, 120.0) && close(s.legs[1].to_m, 240.0));
        // Sin la picada sin hora ni la que cae fuera de la serie.
        let controls: Vec<(usize, ControlCode, f64, f64)> = s
            .controls
            .iter()
            .map(|c| (c.position, c.code, c.t_s, c.distance_m))
            .collect();
        assert_eq!(
            controls,
            vec![
                (0, 31, 0.0, 0.0),
                (1, 32, 30.0, 120.0),
                (2, 33, 60.0, 240.0)
            ]
        );
        // El tramo de cada instante; en la picada, el que empieza en ella.
        let legs: Vec<Option<usize>> = s.samples.iter().map(|x| x.leg).collect();
        assert_eq!(legs, [1, 1, 1, 2, 2, 2, 2].map(Some).to_vec());
    }

    #[test]
    fn a_leg_without_track_is_a_gap() {
        // Tramo 1 de 0 a 20 s, tramo 2 sin track y tramo 3 de 40 a 60 s.
        let between = |from: i64, to: i64| -> Vec<TrackPoint> {
            (from..=to)
                .map(|s| watch(s as f64, 4.0 * s as f64))
                .collect()
        };
        let track = Track {
            points: between(0, 60),
            sport: None,
        };
        let mut missing = leg(2, Vec::new());
        missing.track = None;
        let segmentation = Segmentation {
            controls: Vec::new(),
            legs: vec![leg(1, between(0, 20)), missing, leg(3, between(40, 60))],
        };
        let s = race_series(&track, &segmentation, &raw_altitude(), 10.0).unwrap();
        let states: Vec<SeriesState> = s.samples.iter().map(|x| x.state).collect();
        use SeriesState::{Gap, Transit};
        assert_eq!(
            states,
            [Transit, Transit, Gap, Gap, Transit, Transit, Transit]
        );
        // El tramo sin track no está entre los tramos de la serie.
        assert_eq!(s.legs.iter().map(|l| l.index).collect::<Vec<_>>(), [1, 3]);
        // La velocidad de después del hueco no mira lo de antes: 4 m/s.
        assert!(close(s.samples[4].speed_mps.unwrap(), 4.0));
    }

    /// La posición de cada instante se interpola en el track; en un hueco no hay.
    #[test]
    fn positions_are_interpolated_in_the_track_and_empty_in_gaps() {
        // 0–5 s a 2 m/s hacia el norte, hueco de 20 s y 25–30 s a 2 m/s (50 m al norte en el 25).
        let mut points: Vec<TrackPoint> =
            (0..=5).map(|s| point(s as f64, 2.0 * s as f64)).collect();
        points.extend((25..=30).map(|s| point(s as f64, 50.0 + 2.0 * (s - 25) as f64)));
        let (track, segmentation) = one_leg(points);
        let s = race_series(&track, &segmentation, &raw_altitude(), 1.5).unwrap();
        let positions = series_positions(&track, &segmentation, &s);
        assert_eq!(positions.len(), s.samples.len());
        let north = |m: f64| 40.0 + m / M_PER_DEG;
        // t = 0 en el primer punto; t = 1,5 entre los de 1 s y 2 s: 3 m al norte.
        let (lat, lon) = positions[0].unwrap();
        assert!(close(lat, north(0.0)) && close(lon, -4.0));
        assert!(close(positions[1].unwrap().0, north(3.0)));
        // t = 25,5 (índice 17): 1 m después del punto de 25 s.
        assert!(close(s.samples[17].t_s, 25.5));
        assert!(close(positions[17].unwrap().0, north(51.0)));
        // t = 7,5 está en el hueco.
        assert_eq!(s.samples[5].state, SeriesState::Gap);
        assert_eq!(positions[5], None);
        assert!(
            positions
                .iter()
                .zip(&s.samples)
                .all(|(p, x)| (x.state == SeriesState::Gap) == p.is_none())
        );
    }

    #[test]
    fn without_track_there_are_no_positions() {
        let track = Track {
            points: Vec::new(),
            sport: None,
        };
        let segmentation = Segmentation {
            controls: Vec::new(),
            legs: Vec::new(),
        };
        let s = race_series(&track, &segmentation, &MetricsOptions::default(), 5.0).unwrap();
        assert!(series_positions(&track, &segmentation, &s).is_empty());
    }

    #[test]
    fn without_track_the_series_is_empty() {
        let track = Track {
            points: Vec::new(),
            sport: None,
        };
        let segmentation = Segmentation {
            controls: Vec::new(),
            legs: Vec::new(),
        };
        let s = race_series(&track, &segmentation, &MetricsOptions::default(), 5.0).unwrap();
        assert!(s.samples.is_empty() && s.legs.is_empty() && s.controls.is_empty());
        assert!(close(s.duration_s, 0.0));
    }

    #[test]
    fn the_step_must_be_at_least_one_second() {
        let (track, segmentation) = one_leg(steady());
        for step in [0.0, 0.5, -5.0, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                race_series(&track, &segmentation, &MetricsOptions::default(), step),
                Err(MetricsError::InvalidOptions(_))
            ));
        }
        assert!(race_series(&track, &segmentation, &MetricsOptions::default(), 1.0).is_ok());
    }

    #[test]
    fn json_uses_snake_case() {
        let s = series(steady(), 30.0);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["samples"][0]["state"], "transit");
        assert!(json["samples"][1]["flat_speed_mps"].is_number());
        assert!(json["legs"][0]["from_m"].is_number());
        let back: RaceSeries = serde_json::from_value(json).unwrap();
        assert_eq!(back, s);
    }
}
