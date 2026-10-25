//! Rendimiento físico de una carrera (#212): cómo de rápido corre el corredor, cuánto pulso le
//! cuesta, cómo se degrada a lo largo de la carrera y cómo sube y baja. El método está en
//! `docs/rendimiento-fisico.md`. Resumen:
//!
//! - Solo cuenta el **traslado**: los intervalos en movimiento fuera de la zona de ataque de su
//!   tramo ([`crate::metrics::attack_s`]). Paradas, huecos y ataques no son físico.
//! - El traslado se trocea como la velocidad en llano: trozos seguidos de al menos
//!   [`GRADE_CHUNK_M`] con su pendiente y su distancia equivalente en llano
//!   ([`crate::metrics::running_cost`]). Las mitades, las décimas y las clases de pendiente son
//!   grupos de trozos.
//! - La eficiencia son los metros (en llano) por latido, y el desacople, cuánto cae de la
//!   primera mitad del traslado a la segunda.

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::metrics::{
    GRADE_CHUNK_M, MetricsError, MetricsOptions, Smoothed, attack_s, interval_distance_m,
    running_cost, seconds_between, validate,
};
use crate::model::{Track, TrackPoint};
use crate::segmentation::Segmentation;

/// Semiancho (s) de la media móvil del pulso para el pulso máximo: un pico de un segundo del
/// sensor de muñeca no es el máximo del corredor.
pub const HEART_RATE_SMOOTHING_S: f64 = 5.0;
/// Fracción mínima del tiempo con pulso (o cadencia) para dar cifras de pulso (o cadencia).
pub const MIN_COVERAGE: f64 = 0.5;
/// Partes iguales de tiempo de traslado en las que se mide la deriva.
pub const BLOCKS: usize = 10;
/// Traslado mínimo (s) de cada mitad para comparar mitades y medir la deriva.
pub const MIN_HALF_S: f64 = 120.0;
/// Pendiente (fracción) desde la que un trozo cuenta como subida para la VAM.
pub const CLIMB_GRADE: f64 = 0.05;
/// Subida mínima (s) para dar la VAM.
pub const MIN_CLIMB_S: f64 = 60.0;
/// Límites (fracción) entre las clases de pendiente de [`GradeClass`].
pub const GRADE_LIMITS: [f64; 4] = [-0.08, -0.03, 0.03, 0.08];

/// Velocidad con la que se compara el traslado (eficiencia, mitades, décimas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedKind {
    /// Equivalente en llano: todos los trozos del traslado tienen altitud.
    Flat,
    /// Real: al track le falta altitud (o no hay traslado).
    Moving,
}

/// Clase de pendiente de un trozo de traslado. Un valor justo en un límite va a la clase de
/// arriba.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradeClass {
    /// Menos de −8 %.
    SteepDown,
    /// De −8 % a −3 %.
    Down,
    /// De −3 % a 3 %.
    Flat,
    /// De 3 % a 8 %.
    Up,
    /// 8 % o más.
    SteepUp,
}

impl GradeClass {
    /// Todas, de más bajada a más subida.
    pub const ALL: [Self; 5] = [
        Self::SteepDown,
        Self::Down,
        Self::Flat,
        Self::Up,
        Self::SteepUp,
    ];

    /// La clase de una pendiente (fracción: 0,1 = 10 % de subida).
    pub fn of(grade: f64) -> Self {
        let reached = GRADE_LIMITS.iter().filter(|&&limit| grade >= limit).count();
        Self::ALL[reached.min(Self::ALL.len() - 1)]
    }
}

/// Cifras de un conjunto de trozos de traslado (la carrera, una mitad, una décima, una clase de
/// pendiente).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitStats {
    /// Tiempo de traslado (s).
    pub transit_s: f64,
    /// Distancia de traslado (m).
    pub distance_m: f64,
    /// Velocidad real (m/s); `None` sin traslado.
    pub speed_mps: Option<f64>,
    /// Velocidad equivalente en llano (m/s); `None` sin traslado o si a algún trozo le falta
    /// altitud.
    pub flat_speed_mps: Option<f64>,
    /// Pulso medio ponderado por tiempo (ppm); `None` con pulso en menos de [`MIN_COVERAGE`]
    /// del traslado.
    pub heart_rate_bpm: Option<f64>,
    /// Metros por latido con la velocidad de la carrera ([`SpeedKind`]); `None` sin pulso.
    pub efficiency_m_per_beat: Option<f64>,
    /// Cadencia media (pasos/min); `None` con cadencia en menos de [`MIN_COVERAGE`] del
    /// traslado.
    pub cadence_spm: Option<f64>,
    /// Metros por paso (distancia real); `None` sin cadencia o con cadencia 0.
    pub stride_m: Option<f64>,
}

/// Una décima del traslado, para las gráficas de deriva.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriftBlock {
    /// Número de la décima, de 0 a [`BLOCKS`] − 1.
    pub index: usize,
    /// Desde la salida (s) hasta el principio de su primer trozo.
    pub from_s: f64,
    /// Desde la salida (s) hasta el final de su último trozo.
    pub to_s: f64,
    #[serde(flatten)]
    pub stats: TransitStats,
}

/// Cifras del traslado en una clase de pendiente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradeStats {
    pub class: GradeClass,
    #[serde(flatten)]
    pub stats: TransitStats,
}

/// Tiempo en cada zona de pulso y carga de la carrera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeartRateZones {
    /// Segundos en cada zona (uno más que límites), de la salida a la meta sin huecos.
    pub seconds: Vec<f64>,
    /// Minutos en cada zona por su número (1, 2, 3…), sumados.
    pub load: f64,
}

/// Rendimiento físico de una carrera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RacePhysical {
    /// Suma de las duraciones de los tramos con track (s).
    pub duration_s: f64,
    /// Distancia de los tramos con track (m).
    pub distance_m: f64,
    /// Subida y bajada acumuladas con la altitud suavizada (m); `None` sin altitud.
    pub ascent_m: Option<f64>,
    pub descent_m: Option<f64>,
    /// En movimiento, parado (sin los primeros segundos de cada tramo) y en huecos (s), como en
    /// las métricas de tramo.
    pub moving_s: f64,
    pub stopped_s: f64,
    pub gap_s: f64,
    /// En las zonas de ataque, parado o en movimiento (s).
    pub attack_s: f64,
    /// Velocidad con la que se comparan el traslado, sus mitades y sus décimas.
    pub speed: SpeedKind,
    /// Cifras del traslado entero.
    pub transit: TransitStats,
    /// Pulso medio de la salida a la meta, parado incluido (ppm); `None` con pulso en menos de
    /// [`MIN_COVERAGE`] del tiempo sin huecos.
    pub heart_rate_bpm: Option<f64>,
    /// Mayor pulso suavizado ±[`HEART_RATE_SMOOTHING_S`] (ppm), con la misma condición.
    pub heart_rate_max_bpm: Option<f64>,
    /// Primera y segunda mitad del traslado; `None` si alguna tendría menos de [`MIN_HALF_S`].
    pub halves: Option<[TransitStats; 2]>,
    /// Caída de la eficiencia de la primera mitad a la segunda (%): positiva si la segunda
    /// cuesta más pulso por metro.
    pub decoupling_pct: Option<f64>,
    /// Cambio de la velocidad de la carrera ([`SpeedKind`]) de la primera mitad a la segunda (%).
    pub speed_change_pct: Option<f64>,
    /// Cambio del pulso medio de la primera mitad a la segunda (ppm).
    pub heart_rate_change_bpm: Option<f64>,
    /// Cambio de la zancada de la primera mitad a la segunda (%).
    pub stride_change_pct: Option<f64>,
    /// Décimas del traslado con algún trozo, en orden; vacío sin mitades.
    pub blocks: Vec<DriftBlock>,
    /// Las cinco clases de pendiente, de más bajada a más subida; vacío sin altitud.
    pub by_grade: Vec<GradeStats>,
    /// Metros subidos por hora en los trozos de [`CLIMB_GRADE`] o más; `None` con menos de
    /// [`MIN_CLIMB_S`] de subida.
    pub vam_m_per_h: Option<f64>,
    /// Tiempo de traslado en esos trozos (s).
    pub climb_s: f64,
    /// Tiempo en las zonas de pulso de Ajustes; `None` sin zonas o sin pulso.
    pub heart_rate_zones: Option<HeartRateZones>,
}

/// Rendimiento físico de una carrera: `segmentation` debe salir de `track`.
///
/// `heart_rate_limits` son los límites de las zonas de pulso (ppm, de menor a mayor), si el
/// usuario las ha definido. Las opciones son las de las métricas de tramo: deciden qué es parado,
/// hueco y ataque, y cómo se suaviza la altitud.
pub fn race_physical(
    track: &Track,
    segmentation: &Segmentation,
    options: &MetricsOptions,
    heart_rate_limits: Option<&[f64]>,
) -> Result<RacePhysical, MetricsError> {
    validate(options)?;
    if let Some(limits) = heart_rate_limits
        && !(limits.iter().all(|l| l.is_finite()) && limits.windows(2).all(|w| w[0] < w[1]))
    {
        return Err(MetricsError::InvalidOptions(
            "los límites de las zonas de pulso deben ir de menor a mayor",
        ));
    }
    let altitude = Smoothed::altitude(track, options);
    let legs: Vec<&[TrackPoint]> = segmentation
        .legs
        .iter()
        .filter_map(|leg| leg.track.as_ref())
        .map(|t| t.points.as_slice())
        .filter(|points| points.len() >= 2)
        .collect();

    let mut chunker = Chunker::new(&altitude);
    let mut totals = Totals::default();
    let mut zone_s = vec![0.0; heart_rate_limits.map_or(0, |l| l.len() + 1)];
    for points in &legs {
        let (Some(first), Some(last)) = (points.first(), points.last()) else {
            continue;
        };
        totals.duration_s += seconds_between(first.time, last.time);
        if let Some((up, down)) = altitude.climb(points) {
            totals.climb = Some(totals.climb.map_or((up, down), |(u, d)| (u + up, d + down)));
        }
        let entry = attack_s(points, options.attack_radius_m)
            .map(|s| last.time - TimeDelta::microseconds((s * 1e6) as i64));

        for pair in points.windows(2) {
            let [a, b] = pair else { continue };
            let dt = seconds_between(a.time, b.time);
            let d = interval_distance_m(a, b);
            totals.distance_m += d;
            if dt <= 0.0 {
                continue;
            }
            if dt > options.max_gap_s {
                totals.gap_s += dt;
                chunker.close();
                continue;
            }
            totals.timed_s += dt;
            if let Some(hr) = midpoint(
                a.heart_rate_bpm.map(f64::from),
                b.heart_rate_bpm.map(f64::from),
            ) {
                totals.beats += hr * dt / 60.0;
                totals.heart_rate_s += dt;
                // La zona es cuántos límites alcanza, como en el mapa.
                if let Some(limits) = heart_rate_limits
                    && let Some(s) = zone_s.get_mut(limits.iter().filter(|&&l| hr >= l).count())
                {
                    *s += dt;
                }
            }
            // Un intervalo es de ataque si empieza dentro de la zona de la baliza.
            let attacking = entry.is_some_and(|e| a.time >= e);
            if attacking {
                totals.attack_s += dt;
            }
            if d / dt >= options.stop_speed_mps {
                totals.moving_s += dt;
                if attacking {
                    chunker.close();
                } else {
                    chunker.add(a, b, dt, d);
                }
            } else {
                chunker.close();
                // Solo la parte del intervalo posterior a los primeros `punch_grace_s` del tramo.
                let start = seconds_between(first.time, a.time).max(options.punch_grace_s);
                totals.stopped_s += (seconds_between(first.time, b.time) - start).max(0.0);
            }
        }
        chunker.close();
    }

    let transit_s = chunker.transit_s;
    let chunks = chunker.done;
    let speed = if !chunks.is_empty() && chunks.iter().all(|c| c.rise_m.is_some()) {
        SpeedKind::Flat
    } else {
        SpeedKind::Moving
    };

    let halves = (transit_s >= 2.0 * MIN_HALF_S).then(|| {
        let half = transit_s / 2.0;
        let (first, second): (Vec<&Chunk>, Vec<&Chunk>) =
            chunks.iter().partition(|c| c.midpoint_s() < half);
        [stats(first, speed), stats(second, speed)]
    });
    let change = |of: fn(&TransitStats) -> Option<f64>| {
        let [first, second] = halves.as_ref()?;
        Some((of(first)?, of(second)?))
    };
    let compared = match speed {
        SpeedKind::Flat => |s: &TransitStats| s.flat_speed_mps,
        SpeedKind::Moving => |s: &TransitStats| s.speed_mps,
    };
    let relative = |(a, b): (f64, f64)| (a > 0.0).then(|| (b - a) / a * 100.0);

    let blocks = if halves.is_some() {
        let block_s = transit_s / BLOCKS as f64;
        let race_start = legs.first().and_then(|p| p.first()).map(|p| p.time);
        (0..BLOCKS)
            .filter_map(|index| {
                let members: Vec<&Chunk> = chunks
                    .iter()
                    .filter(|c| ((c.midpoint_s() / block_s) as usize).min(BLOCKS - 1) == index)
                    .collect();
                let start = race_start?;
                let from_s = seconds_between(start, members.first()?.start);
                let to_s = seconds_between(start, members.last()?.end);
                Some(DriftBlock {
                    index,
                    from_s,
                    to_s,
                    stats: stats(members, speed),
                })
            })
            .collect()
    } else {
        Vec::new()
    };

    let by_grade = match speed {
        SpeedKind::Flat => GradeClass::ALL
            .iter()
            .map(|&class| GradeStats {
                class,
                stats: stats(
                    chunks
                        .iter()
                        .filter(|c| c.grade().is_some_and(|g| GradeClass::of(g) == class)),
                    speed,
                ),
            })
            .collect(),
        SpeedKind::Moving => Vec::new(),
    };

    let climbing: Vec<&Chunk> = chunks
        .iter()
        .filter(|c| c.grade().is_some_and(|g| g >= CLIMB_GRADE))
        .collect();
    let climb_s: f64 = climbing.iter().map(|c| c.duration_s).sum();
    let climb_m: f64 = climbing.iter().filter_map(|c| c.rise_m).sum();

    let has_heart_rate =
        totals.timed_s > 0.0 && totals.heart_rate_s >= MIN_COVERAGE * totals.timed_s;
    let heart_rate_max_bpm = legs
        .first()
        .and_then(|p| p.first())
        .zip(legs.last().and_then(|p| p.last()))
        .and_then(|(first, last)| {
            Smoothed::new(
                track
                    .points
                    .iter()
                    .filter_map(|p| Some((p.time, f64::from(p.heart_rate_bpm?)))),
                HEART_RATE_SMOOTHING_S,
                options.max_gap_s,
            )
            .max_between(first.time, last.time)
        })
        .filter(|_| has_heart_rate);
    let heart_rate_zones = heart_rate_limits
        .filter(|_| has_heart_rate)
        .map(|_| HeartRateZones {
            load: zone_s
                .iter()
                .enumerate()
                .map(|(k, s)| s / 60.0 * (k + 1) as f64)
                .sum(),
            seconds: zone_s,
        });

    Ok(RacePhysical {
        duration_s: totals.duration_s,
        distance_m: totals.distance_m,
        ascent_m: totals.climb.map(|(up, _)| up),
        descent_m: totals.climb.map(|(_, down)| down),
        moving_s: totals.moving_s,
        stopped_s: totals.stopped_s,
        gap_s: totals.gap_s,
        attack_s: totals.attack_s,
        speed,
        transit: stats(&chunks, speed),
        heart_rate_bpm: has_heart_rate.then(|| totals.beats * 60.0 / totals.heart_rate_s),
        heart_rate_max_bpm,
        decoupling_pct: change(|s| s.efficiency_m_per_beat)
            .and_then(|(a, b)| relative((a, b)).map(|pct| -pct)),
        speed_change_pct: change(compared).and_then(relative),
        heart_rate_change_bpm: change(|s| s.heart_rate_bpm).map(|(a, b)| b - a),
        stride_change_pct: change(|s| s.stride_m).and_then(relative),
        halves,
        blocks,
        by_grade,
        vam_m_per_h: (climb_s >= MIN_CLIMB_S).then(|| climb_m / climb_s * 3600.0),
        climb_s,
        heart_rate_zones,
    })
}

/// Media de los dos extremos de un intervalo, si los tienen los dos.
fn midpoint(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    Some((a? + b?) / 2.0)
}

/// Sumas de la carrera entera, fuera del traslado.
#[derive(Default)]
struct Totals {
    duration_s: f64,
    distance_m: f64,
    climb: Option<(f64, f64)>,
    moving_s: f64,
    stopped_s: f64,
    gap_s: f64,
    attack_s: f64,
    /// Tiempo sin huecos (s).
    timed_s: f64,
    /// Latidos y tiempo con pulso, sin huecos.
    beats: f64,
    heart_rate_s: f64,
}

/// Trozo de traslado: intervalos en movimiento seguidos, fuera del ataque, de al menos
/// [`GRADE_CHUNK_M`] (o menos si los corta una parada, un hueco, el ataque o el final del tramo).
struct Chunk {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    /// Traslado anterior al trozo (s).
    transit_before_s: f64,
    duration_s: f64,
    distance_m: f64,
    /// Diferencia de altitud suavizada entre el final y el principio (m); `None` sin altitud.
    rise_m: Option<f64>,
    /// Distancia equivalente en llano (m); `None` sin altitud.
    flat_m: Option<f64>,
    /// Latidos y tiempo con pulso.
    beats: f64,
    heart_rate_s: f64,
    /// Pasos y tiempo con cadencia.
    steps: f64,
    cadence_s: f64,
}

impl Chunk {
    fn grade(&self) -> Option<f64> {
        self.rise_m.map(|rise| rise / self.distance_m)
    }

    /// Mitad del trozo en tiempo de traslado (s): decide su mitad y su décima.
    fn midpoint_s(&self) -> f64 {
        self.transit_before_s + self.duration_s / 2.0
    }

    /// Pulso medio (ppm), si hay pulso en al menos [`MIN_COVERAGE`] del trozo.
    fn heart_rate_bpm(&self) -> Option<f64> {
        (self.heart_rate_s > 0.0 && self.heart_rate_s >= MIN_COVERAGE * self.duration_s)
            .then(|| self.beats * 60.0 / self.heart_rate_s)
    }

    /// Cadencia media (pasos/min), con la misma condición.
    fn cadence_spm(&self) -> Option<f64> {
        (self.cadence_s > 0.0 && self.cadence_s >= MIN_COVERAGE * self.duration_s)
            .then(|| self.steps * 60.0 / self.cadence_s)
    }

    /// Metros con los que se compara: en llano o reales.
    fn compared_m(&self, speed: SpeedKind) -> f64 {
        match speed {
            SpeedKind::Flat => self.flat_m.unwrap_or(self.distance_m),
            SpeedKind::Moving => self.distance_m,
        }
    }
}

/// Junta los intervalos de traslado en trozos.
struct Chunker<'a> {
    altitude: &'a Smoothed,
    open: Option<Chunk>,
    done: Vec<Chunk>,
    /// Traslado acumulado (s).
    transit_s: f64,
}

impl<'a> Chunker<'a> {
    fn new(altitude: &'a Smoothed) -> Self {
        Self {
            altitude,
            open: None,
            done: Vec::new(),
            transit_s: 0.0,
        }
    }

    fn add(&mut self, a: &TrackPoint, b: &TrackPoint, dt: f64, d: f64) {
        let chunk = self.open.get_or_insert(Chunk {
            start: a.time,
            end: a.time,
            transit_before_s: self.transit_s,
            duration_s: 0.0,
            distance_m: 0.0,
            rise_m: None,
            flat_m: None,
            beats: 0.0,
            heart_rate_s: 0.0,
            steps: 0.0,
            cadence_s: 0.0,
        });
        chunk.end = b.time;
        chunk.duration_s += dt;
        chunk.distance_m += d;
        if let Some(hr) = midpoint(
            a.heart_rate_bpm.map(f64::from),
            b.heart_rate_bpm.map(f64::from),
        ) {
            chunk.beats += hr * dt / 60.0;
            chunk.heart_rate_s += dt;
        }
        if let Some(cadence) = midpoint(a.cadence_spm, b.cadence_spm) {
            chunk.steps += cadence * dt / 60.0;
            chunk.cadence_s += dt;
        }
        let full = chunk.distance_m >= GRADE_CHUNK_M;
        self.transit_s += dt;
        if full {
            self.close();
        }
    }

    /// Cierra el trozo abierto con su pendiente y su distancia en llano.
    fn close(&mut self) {
        let Some(mut chunk) = self.open.take() else {
            return;
        };
        if chunk.distance_m <= 0.0 {
            return;
        }
        chunk.rise_m = self
            .altitude
            .at(chunk.start)
            .zip(self.altitude.at(chunk.end))
            .map(|(a, b)| b - a);
        chunk.flat_m = chunk
            .grade()
            .map(|grade| chunk.distance_m * running_cost(grade) / running_cost(0.0));
        self.done.push(chunk);
    }
}

/// Cifras de un conjunto de trozos.
fn stats<'a>(chunks: impl IntoIterator<Item = &'a Chunk>, speed: SpeedKind) -> TransitStats {
    let chunks: Vec<&Chunk> = chunks.into_iter().collect();
    let transit_s: f64 = chunks.iter().map(|c| c.duration_s).sum();
    let distance_m: f64 = chunks.iter().map(|c| c.distance_m).sum();
    let per_second = |m: f64| (transit_s > 0.0).then(|| m / transit_s);
    let flat_m: Option<f64> = if chunks.is_empty() {
        None
    } else {
        chunks.iter().map(|c| c.flat_m).sum()
    };

    let with_heart_rate: Vec<(&Chunk, f64)> = chunks
        .iter()
        .filter_map(|c| Some((*c, c.heart_rate_bpm()?)))
        .collect();
    let heart_rate_s: f64 = with_heart_rate.iter().map(|(c, _)| c.duration_s).sum();
    let has_heart_rate = heart_rate_s > 0.0 && heart_rate_s >= MIN_COVERAGE * transit_s;
    let beats: f64 = with_heart_rate
        .iter()
        .map(|(c, hr)| hr * c.duration_s / 60.0)
        .sum();
    let compared_m: f64 = with_heart_rate
        .iter()
        .map(|(c, _)| c.compared_m(speed))
        .sum();

    let with_cadence: Vec<(&Chunk, f64)> = chunks
        .iter()
        .filter_map(|c| Some((*c, c.cadence_spm()?)))
        .collect();
    let cadence_s: f64 = with_cadence.iter().map(|(c, _)| c.duration_s).sum();
    let has_cadence = cadence_s > 0.0 && cadence_s >= MIN_COVERAGE * transit_s;
    let steps: f64 = with_cadence
        .iter()
        .map(|(c, cadence)| cadence * c.duration_s / 60.0)
        .sum();
    let stepped_m: f64 = with_cadence.iter().map(|(c, _)| c.distance_m).sum();

    TransitStats {
        transit_s,
        distance_m,
        speed_mps: per_second(distance_m),
        flat_speed_mps: flat_m.and_then(per_second),
        heart_rate_bpm: has_heart_rate.then(|| beats * 60.0 / heart_rate_s),
        efficiency_m_per_beat: (has_heart_rate && beats > 0.0).then(|| compared_m / beats),
        cadence_spm: has_cadence.then(|| steps * 60.0 / cadence_s),
        stride_m: (has_cadence && steps > 0.0).then(|| stepped_m / steps),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::EARTH_RADIUS_M;
    use crate::segmentation::LegTrack;

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

    /// Punto quieto en (40°, −4°) con la distancia del reloj: sin línea recta, no hay ataque.
    fn watch(s: f64, distance_m: f64) -> TrackPoint {
        TrackPoint {
            distance_m: Some(distance_m),
            ..point(s, 0.0)
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

    fn run(points: Vec<TrackPoint>, limits: Option<&[f64]>) -> RacePhysical {
        let (track, segmentation) = one_leg(points);
        race_physical(&track, &segmentation, &raw_altitude(), limits).unwrap()
    }

    /// Sin suavizar: en una pendiente constante la suavizada recorta los bordes.
    fn raw_altitude() -> MetricsOptions {
        MetricsOptions {
            altitude_smoothing_s: 0.0,
            ..MetricsOptions::default()
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn transit_leaves_out_stops_and_the_attack() {
        // 0–60 s a 3 m/s hacia el norte (180 m), 10 s parado y 70–90 s a 3 m/s hasta la baliza,
        // a 240 m. La zona de ataque (50 m) empieza en el metro 190, a 73,33 s: los intervalos
        // que empiezan desde 74 s (16 s) son ataque; el de 73 a 74 aún es traslado.
        let mut points: Vec<TrackPoint> =
            (0..=60).map(|s| point(s as f64, 3.0 * s as f64)).collect();
        points.extend((61..=70).map(|s| point(s as f64, 180.0)));
        points.extend((71..=90).map(|s| point(s as f64, 180.0 + 3.0 * (s - 70) as f64)));
        let p = run(points, None);
        assert!(close(p.duration_s, 90.0));
        assert!((p.distance_m - 240.0).abs() < 1e-3);
        assert!(close(p.moving_s, 80.0));
        assert!(close(p.stopped_s, 10.0));
        assert!(close(p.attack_s, 16.0));
        assert!(close(p.transit.transit_s, 64.0));
        assert!((p.transit.distance_m - 192.0).abs() < 1e-3);
        assert!((p.transit.speed_mps.unwrap() - 3.0).abs() < 1e-4);
        // Sin altitud: se compara la velocidad real, y no hay pendientes ni pulso.
        assert_eq!(p.speed, SpeedKind::Moving);
        assert_eq!(p.transit.flat_speed_mps, None);
        assert!(p.by_grade.is_empty());
        assert_eq!(p.ascent_m, None);
        assert_eq!(p.vam_m_per_h, None);
        assert_eq!(p.heart_rate_bpm, None);
        assert_eq!(p.transit.efficiency_m_per_beat, None);
        assert_eq!(p.transit.cadence_spm, None);
        // 64 s de traslado no dan para dos mitades de 2 min.
        assert_eq!(p.halves, None);
        assert_eq!(p.decoupling_pct, None);
        assert!(p.blocks.is_empty());
    }

    /// 600 s a 4 m/s en llano (trozos de 5 s, 20 m). Pulso y cadencia de cada punto según su
    /// segundo.
    fn steady_run(heart_rate: impl Fn(i64) -> u8, cadence: impl Fn(i64) -> f64) -> Vec<TrackPoint> {
        (0..=600)
            .map(|s| TrackPoint {
                altitude_m: Some(500.0),
                heart_rate_bpm: Some(heart_rate(s)),
                cadence_spm: Some(cadence(s)),
                ..watch(s as f64, 4.0 * s as f64)
            })
            .collect()
    }

    #[test]
    fn decoupling_compares_the_two_halves() {
        // Pulso 150 hasta el segundo 299 y 165 desde el 300; cadencia 180 y luego 190. El
        // intervalo 299–300 (primera mitad) vale la media de sus extremos: 157,5 y 185.
        let p = run(
            steady_run(
                |s| if s < 300 { 150 } else { 165 },
                |s| if s < 300 { 180.0 } else { 190.0 },
            ),
            None,
        );
        assert_eq!(p.speed, SpeedKind::Flat);
        let [first, second] = p.halves.clone().unwrap();
        assert!(close(first.transit_s, 300.0));
        assert!(close(second.transit_s, 300.0));
        // Primera mitad: (299 · 150 + 157,5) / 300 = 150,025 ppm; 240 m/min / 150,025.
        let hr1 = (299.0 * 150.0 + 157.5) / 300.0;
        assert!(close(first.heart_rate_bpm.unwrap(), hr1));
        assert!(close(first.efficiency_m_per_beat.unwrap(), 240.0 / hr1));
        assert!(close(second.heart_rate_bpm.unwrap(), 165.0));
        assert!(close(second.efficiency_m_per_beat.unwrap(), 240.0 / 165.0));
        // Desacople: (240/150,025 − 240/165) / (240/150,025) = 1 − 150,025/165 = 9,076 %.
        assert!(close(
            p.decoupling_pct.unwrap(),
            (1.0 - hr1 / 165.0) * 100.0
        ));
        assert!(close(p.heart_rate_change_bpm.unwrap(), 165.0 - hr1));
        assert!(close(p.speed_change_pct.unwrap(), 0.0));
        // Zancada: 240 m/min entre 180,0167 pasos/min, y 240 / 190 en la segunda.
        let cad1 = (299.0 * 180.0 + 185.0) / 300.0;
        assert!(close(first.stride_m.unwrap(), 240.0 / cad1));
        assert!(close(second.stride_m.unwrap(), 240.0 / 190.0));
        assert!(close(
            p.stride_change_pct.unwrap(),
            (cad1 / 190.0 - 1.0) * 100.0
        ));
        // En la carrera: 2400 m entre los latidos de las dos mitades.
        let beats = (hr1 + 165.0) * 5.0;
        assert!(close(
            p.transit.efficiency_m_per_beat.unwrap(),
            2400.0 / beats
        ));
        assert!(close(p.heart_rate_bpm.unwrap(), (hr1 + 165.0) / 2.0));
        assert!(close(p.heart_rate_max_bpm.unwrap(), 165.0));
    }

    #[test]
    fn drift_blocks_split_the_transit_in_tenths() {
        let p = run(
            steady_run(|s| if s < 300 { 150 } else { 165 }, |_| 180.0),
            None,
        );
        assert_eq!(p.blocks.len(), BLOCKS);
        for (k, block) in p.blocks.iter().enumerate() {
            assert_eq!(block.index, k);
            assert!(close(block.from_s, 60.0 * k as f64));
            assert!(close(block.to_s, 60.0 * (k + 1) as f64));
            assert!(close(block.stats.transit_s, 60.0));
            assert!(close(block.stats.flat_speed_mps.unwrap(), 4.0));
        }
        assert!(close(p.blocks[0].stats.efficiency_m_per_beat.unwrap(), 1.6));
        assert!(close(
            p.blocks[9].stats.efficiency_m_per_beat.unwrap(),
            240.0 / 165.0
        ));
        assert!(close(p.blocks[0].stats.stride_m.unwrap(), 240.0 / 180.0));
    }

    #[test]
    fn grade_classes_and_vam() {
        // 100 s a 4 m/s en llano, 100 s subiendo al 10 % (0,4 m/s) y 100 s bajando al 5 %
        // (0,2 m/s). Los cambios caen en los bordes de los trozos de 5 s.
        let altitude = |s: f64| {
            if s <= 100.0 {
                500.0
            } else if s <= 200.0 {
                500.0 + 0.4 * (s - 100.0)
            } else {
                540.0 - 0.2 * (s - 200.0)
            }
        };
        let points = (0..=300)
            .map(|s| TrackPoint {
                altitude_m: Some(altitude(s as f64)),
                ..watch(s as f64, 4.0 * s as f64)
            })
            .collect();
        let p = run(points, None);
        assert_eq!(p.speed, SpeedKind::Flat);
        assert!(close(p.ascent_m.unwrap(), 40.0));
        assert!(close(p.descent_m.unwrap(), 20.0));
        let class = |c: GradeClass| &p.by_grade.iter().find(|g| g.class == c).unwrap().stats;
        assert_eq!(p.by_grade.len(), 5);
        assert!(close(class(GradeClass::Flat).transit_s, 100.0));
        assert!(close(class(GradeClass::Flat).flat_speed_mps.unwrap(), 4.0));
        // Subida del 10 %: 4 · 5,968214 / 3,6 = 6,631349 m/s en llano.
        let up = class(GradeClass::SteepUp);
        assert!(close(up.transit_s, 100.0));
        assert!(close(up.speed_mps.unwrap(), 4.0));
        assert!((up.flat_speed_mps.unwrap() - 6.631349).abs() < 1e-5);
        // Bajada del 5 %: 4 · 2,745924 / 3,6 = 3,051027 m/s en llano.
        let down = class(GradeClass::Down);
        assert!(close(down.transit_s, 100.0));
        assert!((down.flat_speed_mps.unwrap() - 3.051027).abs() < 1e-5);
        assert!(close(class(GradeClass::Up).transit_s, 0.0));
        assert_eq!(class(GradeClass::SteepDown).speed_mps, None);
        // VAM: 40 m en los 100 s de subida.
        assert!(close(p.climb_s, 100.0));
        assert!(close(p.vam_m_per_h.unwrap(), 1440.0));
    }

    #[test]
    fn grade_class_limits_go_up() {
        assert_eq!(GradeClass::of(-0.1), GradeClass::SteepDown);
        assert_eq!(GradeClass::of(-0.08), GradeClass::Down);
        assert_eq!(GradeClass::of(-0.03), GradeClass::Flat);
        assert_eq!(GradeClass::of(0.0), GradeClass::Flat);
        assert_eq!(GradeClass::of(0.03), GradeClass::Up);
        assert_eq!(GradeClass::of(0.08), GradeClass::SteepUp);
    }

    #[test]
    fn heart_rate_zones_and_load() {
        // 240 s a 4 m/s: pulso 130 hasta el segundo 59, 150 hasta el 179 y 170 después. Con
        // límites 140 y 160, el intervalo 59–60 (media 140) cae en la zona 1 y el 179–180 (160),
        // en la 2: 59 s, 120 s y 61 s.
        let points = (0..=240)
            .map(|s| TrackPoint {
                heart_rate_bpm: Some(if s < 60 {
                    130
                } else if s < 180 {
                    150
                } else {
                    170
                }),
                ..watch(s as f64, 4.0 * s as f64)
            })
            .collect();
        let p = run(points, Some(&[140.0, 160.0]));
        let zones = p.heart_rate_zones.unwrap();
        assert_eq!(zones.seconds.len(), 3);
        assert!(close(zones.seconds[0], 59.0));
        assert!(close(zones.seconds[1], 120.0));
        assert!(close(zones.seconds[2], 61.0));
        // Carga: (59 · 1 + 120 · 2 + 61 · 3) / 60 = 482 / 60.
        assert!(close(zones.load, 482.0 / 60.0));
        assert!(close(p.heart_rate_max_bpm.unwrap(), 170.0));
    }

    #[test]
    fn zones_need_heart_rate() {
        let points = (0..=240).map(|s| watch(s as f64, 4.0 * s as f64)).collect();
        let p = run(points, Some(&[140.0, 160.0]));
        assert_eq!(p.heart_rate_zones, None);
        assert_eq!(p.heart_rate_max_bpm, None);
    }

    #[test]
    fn little_heart_rate_gives_no_figures() {
        // Pulso solo en el primer tercio: menos de la mitad del traslado.
        let points = (0..=300)
            .map(|s| TrackPoint {
                heart_rate_bpm: (s <= 100).then_some(150),
                ..watch(s as f64, 4.0 * s as f64)
            })
            .collect();
        let p = run(points, None);
        assert_eq!(p.heart_rate_bpm, None);
        assert_eq!(p.transit.heart_rate_bpm, None);
        assert_eq!(p.transit.efficiency_m_per_beat, None);
        let [first, _] = p.halves.unwrap();
        // En la primera mitad (150 s) sí hay pulso en 100 s.
        assert!(close(first.heart_rate_bpm.unwrap(), 150.0));
    }

    #[test]
    fn heart_rate_limits_must_go_up() {
        let (track, segmentation) = one_leg(vec![point(0.0, 0.0), point(1.0, 3.0)]);
        let options = MetricsOptions::default();
        for limits in [&[160.0, 140.0][..], &[140.0, 140.0], &[f64::NAN]] {
            assert!(matches!(
                race_physical(&track, &segmentation, &options, Some(limits)),
                Err(MetricsError::InvalidOptions(_))
            ));
        }
        assert!(race_physical(&track, &segmentation, &options, Some(&[])).is_ok());
    }

    #[test]
    fn json_uses_snake_case() {
        let p = run(steady_run(|_| 150, |_| 180.0), Some(&[140.0]));
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["speed"], "flat");
        assert_eq!(json["by_grade"][2]["class"], "flat");
        assert!(json["by_grade"][2]["transit_s"].is_number());
        assert!(json["blocks"][0]["efficiency_m_per_beat"].is_number());
        assert!(json["halves"][1]["stride_m"].is_number());
        let back: RacePhysical = serde_json::from_value(json).unwrap();
        assert_eq!(back, p);
    }
}
