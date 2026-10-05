//! Mapa de una carrera: el track del reloj troceado en tramos, coloreado por ritmo (o pulso) y
//! con las balizas en su posición GPS.
//!
//! Todo se calcula aquí y en el núcleo; la interfaz solo dibuja lo que recibe (`docs/app.md`,
//! "Mapa"):
//!
//! 1. Se alinea el track guardado con las picadas del resultado
//!    (`tramos_core::alignment::align`) y se trocea en tramos
//!    (`tramos_core::segmentation::segment`). Las balizas son las posiciones de la
//!    segmentación: dónde estaba el corredor en el instante de cada picada.
//! 2. Cada intervalo entre dos puntos de un tramo lleva su ritmo, suavizado con la distancia
//!    recorrida en una ventana de ±[`PACE_WINDOW_S`] alrededor del intervalo, y su pulso (media
//!    de sus dos extremos).
//! 3. Los valores se reparten en [`CLASSES`] clases por cuantiles ponderados por tiempo: cada
//!    color ocupa más o menos el mismo tiempo de carrera.
//! 4. Los intervalos seguidos del mismo tramo y las mismas clases se juntan en un trozo de línea.

use chrono::{DateTime, Utc};
use serde::Serialize;
use thiserror::Error;
use tramos_core::alignment::{Alignment, AlignmentOptions, align};
use tramos_core::metrics::{LegMetrics, MetricsOptions, interval_distance_m, leg_metrics};
use tramos_core::model::{ControlCode, RaceResult, Track, TrackPoint};
use tramos_core::segmentation::{
    ControlRole, MissingLegTrack, MissingPosition, Segmentation, segment,
};
use tramos_store::{ResultId, Store, StoreError};

/// Número de clases de la escala de color.
pub const CLASSES: usize = 5;
/// Semiancho de la ventana con la que se suaviza el ritmo (s).
pub const PACE_WINDOW_S: f64 = 5.0;
/// Ritmo máximo (s/km): más lento, o parado, cuenta como este valor.
pub const MAX_PACE_S_PER_KM: f64 = 1200.0;
/// Fracción mínima del tiempo de carrera con pulso para ofrecer la escala de pulso.
pub const MIN_HEART_RATE_COVERAGE: f64 = 0.5;
/// Decimales de grado de las coordenadas: 6 son unos 10 cm.
const COORDINATE_SCALE: f64 = 1e6;

/// Errores al preparar el mapa. Los mensajes van a la interfaz, en español.
#[derive(Debug, Error)]
pub enum RaceMapError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("el resultado {0} no está en su carrera")]
    NoSuchResult(i64),
}

/// Lo que necesita la interfaz para dibujar el mapa de un resultado.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RaceMap {
    /// La carrera se importó sin FIT: no hay mapa.
    NoTrack,
    /// Hay track, pero ya no se puede alinear con las picadas (no debería pasar: solo se guarda
    /// si se alinea). `message` dice por qué, en español.
    NotAligned { message: String },
    /// Track, tramos y balizas listos para dibujar.
    Ready(MapTrack),
}

/// Track de la carrera (de la salida a la meta) preparado para el mapa.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapTrack {
    /// Rectángulo que contiene todos los tramos: `[oeste, sur, este, norte]` en grados.
    pub bounds: Option<Bounds>,
    /// Un tramo por cada par de picadas consecutivas, como en la segmentación.
    pub legs: Vec<MapLeg>,
    /// Trozos del track con su clase de color, en orden.
    pub pieces: Vec<TrackPiece>,
    /// Balizas situadas, en orden (salida, balizas del recorrido, meta).
    pub controls: Vec<MapControl>,
    /// Escala del ritmo, en s/km. `None` si no hay ningún intervalo con ritmo.
    pub pace: Option<ColorScale>,
    /// Escala del pulso, en ppm. `None` si el track no trae pulso en al menos la mitad del
    /// tiempo de carrera ([`MIN_HEART_RATE_COVERAGE`]).
    pub heart_rate: Option<ColorScale>,
    /// Avisos de la alineación y balizas que no se pueden situar, en español.
    pub warnings: Vec<String>,
}

/// `[oeste, sur, este, norte]` en grados.
pub type Bounds = [f64; 4];

/// Un punto del mapa: `[longitud, latitud]`, como en GeoJSON.
pub type Coordinate = [f64; 2];

/// Un tramo en el mapa.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapLeg {
    /// Número del tramo, empezando en 1, como en la tabla de tramos.
    pub index: usize,
    pub from: ControlCode,
    pub to: ControlCode,
    /// Línea del tramo, de la baliza `from` a la `to`. Vacía si no tiene sub-track.
    pub coordinates: Vec<Coordinate>,
    pub bounds: Option<Bounds>,
    /// Por qué no tiene sub-track (`docs/segmentacion.md`).
    pub missing: Option<MissingLegTrack>,
}

/// Trozo del track con la misma clase de ritmo y de pulso, dentro de un tramo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackPiece {
    /// Tramo al que pertenece.
    pub leg: usize,
    pub coordinates: Vec<Coordinate>,
    /// Clase de ritmo, de 0 (más rápido) a `CLASSES − 1` (más lento); `None` en un hueco.
    pub pace_class: Option<usize>,
    /// Clase de pulso, de 0 (más bajo) a `CLASSES − 1` (más alto); `None` sin pulso o en un
    /// hueco.
    pub heart_rate_class: Option<usize>,
}

/// Una baliza situada en el mapa.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapControl {
    /// 0 = salida, 1 = primera baliza del recorrido, …, la última = meta. El tramo `position`
    /// acaba en esta baliza.
    pub position: usize,
    pub code: ControlCode,
    pub role: ControlRole,
    pub coordinate: Coordinate,
    /// La picada cae en un hueco del track: la posición es poco fiable.
    pub in_gap: bool,
}

/// Escala de color por clases.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColorScale {
    /// Límites de las clases, de menor a mayor: `CLASSES + 1` valores. La clase `k` va de
    /// `edges[k]` a `edges[k + 1]`; el primero es el mínimo y el último el máximo.
    pub edges: Vec<f64>,
}

/// Métricas del FIT de cada tramo del resultado `result` (`race_result` en su carrera). `None` si
/// no tiene track o ya no se puede alinear ni trocear: la carrera no aporta tramos a los análisis
/// que las usan (P2, P13).
pub(crate) fn stored_leg_metrics(
    store: &Store,
    result: ResultId,
    race_result: &RaceResult,
) -> Result<Option<Vec<LegMetrics>>, StoreError> {
    let Some(track) = store.load_track(result)? else {
        return Ok(None);
    };
    let Ok((_, segmentation)) = aligned_legs(&track, race_result) else {
        return Ok(None);
    };
    Ok(leg_metrics(&track, &segmentation, &MetricsOptions::default()).ok())
}

/// Alinea el track guardado de un resultado con sus picadas
/// (`tramos_core::alignment::align`, opciones por defecto) y lo trocea en tramos
/// (`tramos_core::segmentation::segment`). Lo usan el mapa y el histórico (P13). Si no se puede,
/// el motivo en español.
pub(crate) fn aligned_legs(
    track: &Track,
    result: &RaceResult,
) -> Result<(Alignment, Segmentation), String> {
    let alignment = align(track, result, &AlignmentOptions::default())
        .map_err(|err| format!("No se puede situar la carrera en el track: {err}"))?;
    let segmentation = segment(track, &alignment)
        .map_err(|err| format!("No se puede cortar el track en tramos: {err}"))?;
    Ok((alignment, segmentation))
}

/// Mapa del resultado `result_id`.
pub fn race_map(store: &Store, result_id: i64) -> Result<RaceMap, RaceMapError> {
    let result = ResultId(result_id);
    let (event_id, at) = store.result_ref(result)?;
    let Some(track) = store.load_track(result)? else {
        return Ok(RaceMap::NoTrack);
    };
    let event = store.load_event(event_id)?;
    let race_result = at
        .get(&event)
        .ok_or(RaceMapError::NoSuchResult(result_id))?;
    let (alignment, segmentation) = match aligned_legs(&track, race_result) {
        Ok(done) => done,
        Err(message) => return Ok(RaceMap::NotAligned { message }),
    };

    let mut warnings: Vec<String> = alignment.warnings.into_iter().map(|w| w.message).collect();
    let mut controls = Vec::new();
    for control in &segmentation.controls {
        match control.point {
            Some(point) => controls.push(MapControl {
                position: control.position,
                code: control.code,
                role: control.role,
                coordinate: coordinate(point.lon, point.lat),
                in_gap: control.in_gap,
            }),
            None => warnings.push(format!(
                "La baliza {} no aparece en el mapa: {}.",
                control.code,
                match control.missing {
                    Some(MissingPosition::NoPunchTime) => "la picada no tiene hora",
                    Some(MissingPosition::OutsideTrack) | None => "su picada cae fuera del track",
                }
            )),
        }
    }

    let options = MetricsOptions::default();
    let distance = CumulativeDistance::new(&track);
    let mut legs = Vec::with_capacity(segmentation.legs.len());
    let mut intervals = Vec::new();
    for leg in &segmentation.legs {
        let points = leg.track.as_ref().map_or(&[][..], |t| t.points.as_slice());
        for pair in points.windows(2) {
            intervals.push(interval(leg.index, &pair[0], &pair[1], &distance, &options));
        }
        let coordinates: Vec<Coordinate> =
            points.iter().map(|p| coordinate(p.lon, p.lat)).collect();
        legs.push(MapLeg {
            index: leg.index,
            from: leg.from,
            to: leg.to,
            bounds: bounds(&coordinates),
            coordinates,
            missing: leg.missing,
        });
    }

    let pace = scale(
        intervals
            .iter()
            .filter_map(|i| Some((i.pace?, i.duration_s))),
    );
    let race_s: f64 = intervals
        .iter()
        .filter(|i| i.pace.is_some())
        .map(|i| i.duration_s)
        .sum();
    let heart_rate_s: f64 = intervals
        .iter()
        .filter(|i| i.pace.is_some() && i.heart_rate.is_some())
        .map(|i| i.duration_s)
        .sum();
    let heart_rate = if race_s > 0.0 && heart_rate_s / race_s >= MIN_HEART_RATE_COVERAGE {
        scale(intervals.iter().filter_map(|i| {
            i.pace?;
            Some((i.heart_rate?, i.duration_s))
        }))
    } else {
        None
    };

    let pieces = pieces(&intervals, pace.as_ref(), heart_rate.as_ref());
    let all: Vec<Coordinate> = legs
        .iter()
        .flat_map(|l| l.coordinates.iter().copied())
        .collect();
    Ok(RaceMap::Ready(MapTrack {
        bounds: bounds(&all),
        legs,
        pieces,
        controls,
        pace,
        heart_rate,
        warnings,
    }))
}

/// Un intervalo entre dos puntos consecutivos de un tramo.
#[derive(Debug, Clone, PartialEq)]
struct Interval {
    leg: usize,
    from: Coordinate,
    to: Coordinate,
    duration_s: f64,
    /// Ritmo suavizado (s/km); `None` en un hueco.
    pace: Option<f64>,
    /// Media del pulso de los dos extremos; `None` si falta en alguno o en un hueco.
    heart_rate: Option<f64>,
}

fn interval(
    leg: usize,
    a: &TrackPoint,
    b: &TrackPoint,
    distance: &CumulativeDistance,
    options: &MetricsOptions,
) -> Interval {
    let duration_s = seconds_between(a.time, b.time);
    let in_gap = duration_s > options.max_gap_s;
    let heart_rate = match (a.heart_rate_bpm, b.heart_rate_bpm) {
        (Some(x), Some(y)) if !in_gap => Some((f64::from(x) + f64::from(y)) / 2.0),
        _ => None,
    };
    Interval {
        leg,
        from: coordinate(a.lon, a.lat),
        to: coordinate(b.lon, b.lat),
        duration_s,
        pace: if in_gap {
            None
        } else {
            distance.pace(a.time, b.time, PACE_WINDOW_S, options.stop_speed_mps)
        },
        heart_rate,
    }
}

/// Distancia acumulada a lo largo del track entero, para medir la velocidad en cualquier
/// ventana de tiempo (también la que cruza el límite entre dos tramos).
struct CumulativeDistance {
    start: Option<DateTime<Utc>>,
    /// Segundos desde el primer punto.
    times: Vec<f64>,
    /// Metros desde el primer punto (`interval_distance_m` del núcleo).
    distances: Vec<f64>,
}

impl CumulativeDistance {
    fn new(track: &Track) -> Self {
        let start = track.points.first().map(|p| p.time);
        let mut times = Vec::with_capacity(track.points.len());
        let mut distances = Vec::with_capacity(track.points.len());
        let mut total = 0.0;
        for (i, p) in track.points.iter().enumerate() {
            if i > 0 {
                total += interval_distance_m(&track.points[i - 1], p);
            }
            times.push(start.map_or(0.0, |s| seconds_between(s, p.time)));
            distances.push(total);
        }
        Self {
            start,
            times,
            distances,
        }
    }

    /// Distancia acumulada en el instante `t` (s desde el primer punto), interpolada entre los
    /// dos puntos que lo rodean y limitada a los extremos del track.
    fn at(&self, t: f64) -> Option<f64> {
        let (&first, &last) = (self.times.first()?, self.times.last()?);
        if t <= first {
            return self.distances.first().copied();
        }
        if t >= last {
            return self.distances.last().copied();
        }
        let i = self.times.partition_point(|&x| x <= t);
        let (t0, t1) = (self.times[i - 1], self.times[i]);
        let (d0, d1) = (self.distances[i - 1], self.distances[i]);
        if t1 <= t0 {
            return Some(d1);
        }
        Some(d0 + (d1 - d0) * (t - t0) / (t1 - t0))
    }

    /// Ritmo (s/km) en la ventana `[a − window_s, b + window_s]`, recortada al track. Por debajo
    /// de `stop_speed_mps` (parado) es [`MAX_PACE_S_PER_KM`].
    fn pace(
        &self,
        a: DateTime<Utc>,
        b: DateTime<Utc>,
        window_s: f64,
        stop_speed_mps: f64,
    ) -> Option<f64> {
        let start = self.start?;
        let first = *self.times.first()?;
        let last = *self.times.last()?;
        let t0 = (seconds_between(start, a) - window_s).max(first);
        let t1 = (seconds_between(start, b) + window_s).min(last);
        if t1 <= t0 {
            return None;
        }
        let speed = (self.at(t1)? - self.at(t0)?) / (t1 - t0);
        if speed < stop_speed_mps {
            return Some(MAX_PACE_S_PER_KM);
        }
        Some((1000.0 / speed).min(MAX_PACE_S_PER_KM))
    }
}

/// Escala de [`CLASSES`] clases por cuantiles ponderados (`(valor, peso)`); `None` sin valores.
fn scale(values: impl Iterator<Item = (f64, f64)>) -> Option<ColorScale> {
    let mut values: Vec<(f64, f64)> = values
        .filter(|(v, w)| v.is_finite() && w.is_finite() && *w >= 0.0)
        .collect();
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = values.iter().map(|(_, w)| w).sum();
    let mut edges = Vec::with_capacity(CLASSES + 1);
    edges.push(values[0].0);
    for k in 1..CLASSES {
        edges.push(weighted_quantile(&values, total, k as f64 / CLASSES as f64));
    }
    edges.push(values[values.len() - 1].0);
    Some(ColorScale { edges })
}

/// Primer valor (de `sorted`, ordenado) cuyo peso acumulado llega a `q` del total. Con peso total
/// 0, el cuantil sin ponderar.
fn weighted_quantile(sorted: &[(f64, f64)], total: f64, q: f64) -> f64 {
    if total > 0.0 {
        let target = q * total;
        let mut cumulative = 0.0;
        for (value, weight) in sorted {
            cumulative += weight;
            if cumulative >= target {
                return *value;
            }
        }
    }
    let i = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[i.min(sorted.len() - 1)].0
}

/// Clase de `value`: el número de límites interiores que supera, de 0 a `CLASSES − 1`.
fn class(scale: &ColorScale, value: f64) -> usize {
    let inner = scale
        .edges
        .get(1..scale.edges.len().saturating_sub(1))
        .unwrap_or_default();
    inner.iter().filter(|&&edge| value > edge).count()
}

/// Junta los intervalos seguidos del mismo tramo y las mismas clases en trozos de línea.
fn pieces(
    intervals: &[Interval],
    pace: Option<&ColorScale>,
    heart_rate: Option<&ColorScale>,
) -> Vec<TrackPiece> {
    let mut pieces: Vec<TrackPiece> = Vec::new();
    for i in intervals {
        let pace_class = pace.zip(i.pace).map(|(s, v)| class(s, v));
        let heart_rate_class = heart_rate.zip(i.heart_rate).map(|(s, v)| class(s, v));
        if let Some(last) = pieces.last_mut()
            && last.leg == i.leg
            && last.pace_class == pace_class
            && last.heart_rate_class == heart_rate_class
            && last.coordinates.last() == Some(&i.from)
        {
            last.coordinates.push(i.to);
            continue;
        }
        pieces.push(TrackPiece {
            leg: i.leg,
            coordinates: vec![i.from, i.to],
            pace_class,
            heart_rate_class,
        });
    }
    pieces
}

fn bounds(coordinates: &[Coordinate]) -> Option<Bounds> {
    let first = coordinates.first()?;
    let mut b = [first[0], first[1], first[0], first[1]];
    for c in coordinates {
        b[0] = b[0].min(c[0]);
        b[1] = b[1].min(c[1]);
        b[2] = b[2].max(c[0]);
        b[3] = b[3].max(c[1]);
    }
    Some(b)
}

fn coordinate(lon: f64, lat: f64) -> Coordinate {
    [
        (lon * COORDINATE_SCALE).round() / COORDINATE_SCALE,
        (lat * COORDINATE_SCALE).round() / COORDINATE_SCALE,
    ]
}

fn seconds_between(a: DateTime<Utc>, b: DateTime<Utc>) -> f64 {
    (b - a).num_microseconds().map_or_else(
        || (b - a).num_milliseconds() as f64 / 1e3,
        |us| us as f64 / 1e6,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::{ImportRequest, import, preview};
    use chrono::TimeZone;
    use tramos_core::identify::RunnerIdentity;

    fn fixture(path: &str) -> String {
        format!("{}/../../fixtures/{path}", env!("CARGO_MANIFEST_DIR"))
    }

    fn identity() -> RunnerIdentity {
        RunnerIdentity {
            si_card: Some(143),
            full_name: Some("N143 Apellido143".into()),
        }
    }

    /// Importa el fixture de Baltanás como el corredor de la tarjeta 143, con o sin el FIT
    /// sintético.
    fn imported(with_fit: bool) -> (Store, i64) {
        let mut store = Store::open_in_memory().unwrap();
        let spl = fixture("spl/baltanas-anon.spl");
        let fit = with_fit.then(|| fixture("fit/baltanas-sintetico.fit"));
        let p = preview(&store, &spl, fit.as_deref(), &identity()).unwrap();
        let outcome = import(
            &mut store,
            &ImportRequest {
                spl_path: spl,
                fit_path: fit,
                result: p.candidates[0].result,
                format: p.suggested_format,
                identity: identity(),
            },
        )
        .unwrap();
        (store, outcome.result_id)
    }

    fn ready(store: &Store, result_id: i64) -> MapTrack {
        match race_map(store, result_id).unwrap() {
            RaceMap::Ready(map) => map,
            other => panic!("se esperaba un mapa: {other:?}"),
        }
    }

    /// Distancia aproximada en metros entre dos coordenadas cercanas.
    fn meters(a: Coordinate, b: Coordinate) -> f64 {
        let m_per_deg = 6_371_008.8 * std::f64::consts::PI / 180.0;
        let dx = (a[0] - b[0]) * m_per_deg * a[1].to_radians().cos();
        let dy = (a[1] - b[1]) * m_per_deg;
        dx.hypot(dy)
    }

    fn truth() -> serde_json::Value {
        let text = std::fs::read_to_string(fixture("fit/baltanas-sintetico.truth.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn a_race_without_fit_has_no_map() {
        let (store, result_id) = imported(false);
        assert_eq!(race_map(&store, result_id).unwrap(), RaceMap::NoTrack);
        let json = serde_json::to_value(RaceMap::NoTrack).unwrap();
        assert_eq!(json, serde_json::json!({"status": "no_track"}));
    }

    #[test]
    fn missing_result_is_an_error() {
        let store = Store::open_in_memory().unwrap();
        assert!(matches!(
            race_map(&store, 42),
            Err(RaceMapError::Store(StoreError::ResultNotFound(42)))
        ));
    }

    /// Las balizas están donde dice la verdad del FIT sintético y cada tramo va de su baliza de
    /// salida a la de llegada.
    #[test]
    fn controls_and_legs_match_the_synthetic_truth() {
        let (store, result_id) = imported(true);
        let map = ready(&store, result_id);
        let truth = truth();
        let expected = truth["controls"].as_array().unwrap();

        assert_eq!(map.controls.len(), 22);
        assert_eq!(map.controls.len(), expected.len());
        for (control, t) in map.controls.iter().zip(expected) {
            assert_eq!(u64::from(control.code), t["code"].as_u64().unwrap());
            let at = [t["lon"].as_f64().unwrap(), t["lat"].as_f64().unwrap()];
            assert!(
                meters(control.coordinate, at) < 10.0,
                "baliza {} a {} m",
                control.code,
                meters(control.coordinate, at)
            );
        }
        assert_eq!(map.controls[0].role, ControlRole::Start);
        assert_eq!(map.controls[21].role, ControlRole::Finish);
        assert!(
            map.controls
                .iter()
                .enumerate()
                .all(|(i, c)| c.position == i)
        );

        assert_eq!(map.legs.len(), 21);
        for leg in &map.legs {
            assert!(leg.missing.is_none());
            let from = map.controls[leg.index - 1].coordinate;
            let to = map.controls[leg.index].coordinate;
            assert_eq!(leg.coordinates.first(), Some(&from), "tramo {}", leg.index);
            assert_eq!(leg.coordinates.last(), Some(&to), "tramo {}", leg.index);
            let b = leg.bounds.unwrap();
            assert!(b[0] <= b[2] && b[1] <= b[3]);
        }
        let all = map.bounds.unwrap();
        for c in &map.controls {
            let [lon, lat] = c.coordinate;
            assert!(all[0] <= lon && lon <= all[2] && all[1] <= lat && lat <= all[3]);
        }
        assert!(map.warnings.is_empty(), "{:?}", map.warnings);
    }

    /// Los trozos de cada tramo, uno detrás de otro, dibujan el tramo entero.
    #[test]
    fn pieces_cover_every_leg() {
        let (store, result_id) = imported(true);
        let map = ready(&store, result_id);
        for leg in &map.legs {
            let mut joined: Vec<Coordinate> = Vec::new();
            for piece in map.pieces.iter().filter(|p| p.leg == leg.index) {
                assert!(piece.coordinates.len() >= 2);
                if let Some(last) = joined.last() {
                    assert_eq!(piece.coordinates.first(), Some(last));
                    joined.extend_from_slice(&piece.coordinates[1..]);
                } else {
                    joined.extend_from_slice(&piece.coordinates);
                }
            }
            assert_eq!(joined, leg.coordinates, "tramo {}", leg.index);
        }
        // Sin huecos, todos los trozos tienen ritmo y pulso.
        assert!(map.pieces.iter().all(|p| p.pace_class.is_some()));
        assert!(map.pieces.iter().all(|p| p.heart_rate_class.is_some()));
        // Juntar intervalos sirve: muchos menos trozos que puntos.
        let points: usize = map.legs.iter().map(|l| l.coordinates.len()).sum();
        assert!(map.pieces.len() < points / 2, "{} trozos", map.pieces.len());
    }

    /// La parada de 30 s del tramo 9 sale en la clase más lenta, y las escalas son crecientes.
    #[test]
    fn the_stop_is_in_the_slowest_class() {
        let (store, result_id) = imported(true);
        let map = ready(&store, result_id);
        let stop = &truth()["stop"];
        assert_eq!(stop["leg"].as_u64(), Some(9));
        let at = [stop["lon"].as_f64().unwrap(), stop["lat"].as_f64().unwrap()];

        let near_stop: Vec<&TrackPiece> = map
            .pieces
            .iter()
            .filter(|p| p.coordinates.iter().any(|&c| meters(c, at) < 3.0))
            .collect();
        assert!(!near_stop.is_empty());
        assert!(near_stop.iter().all(|p| p.leg == 9));
        assert!(near_stop.iter().any(|p| p.pace_class == Some(CLASSES - 1)));

        let pace = map.pace.unwrap();
        assert_eq!(pace.edges.len(), CLASSES + 1);
        assert!(pace.edges.windows(2).all(|w| w[0] <= w[1]));
        // La parada lleva el máximo; correr, unos minutos por km.
        assert_eq!(pace.edges[CLASSES], MAX_PACE_S_PER_KM);
        assert!(
            pace.edges[1] > 120.0 && pace.edges[CLASSES - 1] < 600.0,
            "{pace:?}"
        );
        // Cada clase de ritmo aparece en el track.
        for k in 0..CLASSES {
            assert!(
                map.pieces.iter().any(|p| p.pace_class == Some(k)),
                "clase {k}"
            );
        }

        let heart_rate = map.heart_rate.unwrap();
        assert!(heart_rate.edges.windows(2).all(|w| w[0] <= w[1]));
        assert!(heart_rate.edges[0] > 60.0 && heart_rate.edges[CLASSES] < 220.0);
    }

    #[test]
    fn map_json_uses_snake_case_and_a_status_tag() {
        let (store, result_id) = imported(true);
        let json = serde_json::to_value(race_map(&store, result_id).unwrap()).unwrap();
        assert_eq!(json["status"], "ready");
        assert_eq!(json["controls"][0]["role"], "start");
        assert!(json["legs"][0]["coordinates"][0].is_array());
        assert!(json["pieces"][0]["pace_class"].is_u64());
        assert_eq!(json["pace"]["edges"].as_array().unwrap().len(), CLASSES + 1);
    }

    fn point(t: i64, north_m: f64, distance_m: Option<f64>, hr: Option<u8>) -> TrackPoint {
        // 1 m hacia el norte son 1 / 111 195 grados de latitud con el radio del núcleo.
        let m_per_deg = 6_371_008.8 * std::f64::consts::PI / 180.0;
        TrackPoint {
            time: Utc.with_ymd_and_hms(2026, 10, 3, 16, 0, 0).unwrap()
                + chrono::TimeDelta::seconds(t),
            lat: 42.0 + north_m / m_per_deg,
            lon: -4.0,
            altitude_m: None,
            heart_rate_bpm: hr,
            cadence_spm: None,
            distance_m,
        }
    }

    /// Track a mano: 4 m/s durante 20 s, parado 10 s y 2 m/s otros 20 s.
    fn hand_track() -> Track {
        let mut points = Vec::new();
        let mut d = 0.0;
        for t in 0..=50 {
            if t > 0 {
                d += if t <= 20 {
                    4.0
                } else if t <= 30 {
                    0.0
                } else {
                    2.0
                };
            }
            points.push(point(t, d, Some(d), Some(150)));
        }
        Track {
            points,
            sport: None,
        }
    }

    #[test]
    fn pace_is_smoothed_distance_over_time() {
        let track = hand_track();
        let distance = CumulativeDistance::new(&track);
        let p = |a: usize, b: usize| {
            distance
                .pace(
                    track.points[a].time,
                    track.points[b].time,
                    PACE_WINDOW_S,
                    0.5,
                )
                .unwrap()
        };
        // Lejos de los cambios: 4 m/s = 250 s/km y 2 m/s = 500 s/km.
        assert!((p(8, 9) - 250.0).abs() < 1e-9);
        assert!((p(40, 41) - 500.0).abs() < 1e-9);
        // En plena parada (24–25, ventana 19–30): 4 m en 11 s, por debajo de 0,5 m/s.
        assert_eq!(p(24, 25), MAX_PACE_S_PER_KM);
        // Ventana 15–26: 20 m en 11 s.
        assert!((p(20, 21) - 1000.0 / (20.0 / 11.0)).abs() < 1e-9);
        // En el borde la ventana se recorta: 0–6 s, 24 m.
        assert!((p(0, 1) - 250.0).abs() < 1e-9);
        // Sin distancia del reloj, la del GPS da lo mismo (a menos de 1 mm por metro).
        let mut gps = hand_track();
        for point in &mut gps.points {
            point.distance_m = None;
        }
        let gps_distance = CumulativeDistance::new(&gps);
        let pace = gps_distance
            .pace(gps.points[8].time, gps.points[9].time, PACE_WINDOW_S, 0.5)
            .unwrap();
        assert!((pace - 250.0).abs() < 0.25, "{pace}");
    }

    #[test]
    fn quantile_classes_by_hand() {
        // 10 valores con el mismo peso: los cuantiles 0,2, 0,4… son 2, 4, 6 y 8.
        let s = scale((1..=10).map(|v| (f64::from(v), 1.0))).unwrap();
        assert_eq!(s.edges, vec![1.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(class(&s, 1.0), 0);
        assert_eq!(class(&s, 2.0), 0);
        assert_eq!(class(&s, 3.0), 1);
        assert_eq!(class(&s, 8.0), 3);
        assert_eq!(class(&s, 9.0), 4);
        assert_eq!(class(&s, 100.0), 4);

        // El peso cuenta: el 1 dura 9 veces más que el 10, así que los cuatro límites son 1.
        let s = scale([(1.0, 9.0), (10.0, 1.0)].into_iter()).unwrap();
        assert_eq!(s.edges, vec![1.0, 1.0, 1.0, 1.0, 1.0, 10.0]);
        assert_eq!(class(&s, 1.0), 0);
        assert_eq!(class(&s, 10.0), 4);

        // Un único valor: todo en la primera clase.
        let s = scale([(5.0, 1.0)].into_iter()).unwrap();
        assert_eq!(class(&s, 5.0), 0);
        assert!(scale(std::iter::empty()).is_none());
        assert!(scale([(f64::NAN, 1.0)].into_iter()).is_none());
    }

    #[test]
    fn gaps_have_no_colour_and_pieces_split_by_class() {
        let mut track = hand_track();
        // Un hueco de 20 s entre los puntos 40 y 41.
        for p in track.points.iter_mut().skip(41) {
            p.time += chrono::TimeDelta::seconds(19);
        }
        let distance = CumulativeDistance::new(&track);
        let options = MetricsOptions::default();
        let intervals: Vec<Interval> = track
            .points
            .windows(2)
            .map(|w| interval(1, &w[0], &w[1], &distance, &options))
            .collect();
        assert!(intervals[40].pace.is_none() && intervals[40].heart_rate.is_none());
        assert_eq!(intervals[39].heart_rate, Some(150.0));

        let pace = scale(
            intervals
                .iter()
                .filter_map(|i| Some((i.pace?, i.duration_s))),
        )
        .unwrap();
        let pieces = pieces(&intervals, Some(&pace), None);
        // Los trozos van seguidos, sin repetir puntos, y el hueco va aparte sin clase.
        let total: usize = pieces.iter().map(|p| p.coordinates.len() - 1).sum();
        assert_eq!(total, intervals.len());
        let gap: Vec<&TrackPiece> = pieces.iter().filter(|p| p.pace_class.is_none()).collect();
        assert_eq!(gap.len(), 1);
        assert_eq!(gap[0].coordinates.len(), 2);
        for w in pieces.windows(2) {
            assert_eq!(w[0].coordinates.last(), w[1].coordinates.first());
            assert!(
                w[0].pace_class != w[1].pace_class
                    || w[0].heart_rate_class != w[1].heart_rate_class
            );
        }
    }
}
