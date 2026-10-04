//! Corte del track en tramos y posición de las balizas.
//!
//! A partir de una [`Alignment`], [`segment`] sitúa cada baliza en la posición del corredor en
//! el instante de su picada (interpolada entre los dos puntos del track que la rodean) y corta
//! el track en un sub-track por tramo. [`median_control_positions`] combina las posiciones de
//! varios corredores del mismo recorrido. El método completo está en `docs/segmentacion.md`.
//! Resumen:
//!
//! - Los tramos salen de las picadas consecutivas del resultado: con `n` balizas, `n + 1`
//!   tramos (salida → primera, …, última → meta).
//! - Los límites de cada tramo son puntos interpolados en el instante de la picada (hora del
//!   reloj) y están en los dos tramos contiguos: la distancia es continua.
//! - Un tramo cuyo principio o final no se puede situar sale sin sub-track y con el motivo; no
//!   es un error de toda la segmentación.

use std::collections::BTreeMap;

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::alignment::{AlignedPunch, Alignment, median};
use crate::model::{ControlCode, FINISH_CODE, START_CODE, Track, TrackPoint};

/// Errores que impiden segmentar: la alineación no corresponde al track que se pasa.
#[derive(Debug, Error, PartialEq)]
pub enum SegmentationError {
    #[error(
        "la picada {position} (baliza {code}) apunta al punto {index} del track, que solo tiene \
         {len} puntos: ¿la alineación es de otro track?"
    )]
    LocationOutsideTrack {
        position: usize,
        code: ControlCode,
        index: usize,
        len: usize,
    },

    #[error("la picada {position} (baliza {code}) tiene una fracción inválida: {fraction}")]
    InvalidFraction {
        position: usize,
        code: ControlCode,
        fraction: f64,
    },
}

/// Resultado de segmentar el track de un corredor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segmentation {
    /// Una entrada por picada, en el orden de `Alignment::punches`.
    pub controls: Vec<ControlPosition>,
    /// Un tramo por cada par de picadas consecutivas.
    pub legs: Vec<LegTrack>,
}

/// Posición de una baliza según el track del corredor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlPosition {
    /// Posición en la secuencia de picadas: 0 = salida, 1 = primera baliza del recorrido, …,
    /// la última = meta.
    pub position: usize,
    pub code: ControlCode,
    pub role: ControlRole,
    /// Instante de la picada en hora del reloj (`track_time` de la alineación).
    pub time: Option<DateTime<Utc>>,
    /// Posición del corredor en ese instante; `None` si no se puede situar (ver `missing`).
    pub point: Option<GeoPoint>,
    /// El instante cae en un hueco del track: la posición es una interpolación en línea recta
    /// entre dos puntos separados más de `max_gap_s` y no es fiable.
    pub in_gap: bool,
    /// Por qué no hay `point`.
    pub missing: Option<MissingPosition>,
}

/// Papel de la picada en el recorrido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlRole {
    /// Salida: su hora es la asignada, no una picada física.
    Start,
    /// Baliza del recorrido.
    Control,
    /// Meta.
    Finish,
}

/// Punto geográfico (WGS84).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    /// Latitud en grados.
    pub lat: f64,
    /// Longitud en grados.
    pub lon: f64,
    /// Altitud en metros, si el track la trae en los dos puntos que rodean el instante.
    pub altitude_m: Option<f64>,
}

/// Por qué una baliza no tiene posición.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingPosition {
    /// La picada no tiene hora en el cronometraje.
    NoPunchTime,
    /// La picada tiene hora, pero con el desfase cae fuera del track.
    OutsideTrack,
}

/// Sub-track de un tramo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegTrack {
    /// Número del tramo, empezando en 1 (el que sale de la salida).
    pub index: usize,
    pub from: ControlCode,
    pub to: ControlCode,
    /// Puntos del tramo: el límite inicial interpolado, los puntos del track estrictamente
    /// entre los dos límites y el límite final interpolado. `None` si falta algún límite.
    pub track: Option<Track>,
    /// Por qué no hay `track`.
    pub missing: Option<MissingLegTrack>,
}

/// Por qué un tramo no tiene sub-track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum MissingLegTrack {
    /// La picada de un extremo (`code`) no tiene hora.
    NoPunchTime { code: ControlCode },
    /// La picada de un extremo (`code`) cae fuera del track.
    OutsideTrack { code: ControlCode },
    /// El final del tramo es anterior a su principio.
    OutOfOrder,
}

/// Posición de una baliza combinando varios corredores (mediana de cada coordenada).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MedianControlPosition {
    /// Posición en la secuencia de picadas (0 = salida), como en [`ControlPosition`].
    pub position: usize,
    pub code: ControlCode,
    /// Mediana de las latitudes (grados).
    pub lat: f64,
    /// Mediana de las longitudes (grados).
    pub lon: f64,
    /// Mediana de las altitudes de los corredores que la tienen.
    pub altitude_m: Option<f64>,
    /// Corredores que aportan posición a esta baliza.
    pub runners: usize,
}

/// Límite de un tramo ya situado en el track.
struct Boundary {
    /// Índice del punto del track anterior (o igual) al instante.
    index: usize,
    point: TrackPoint,
}

/// Sitúa las balizas y corta el track en tramos según la alineación.
///
/// Falla solo si la alineación no corresponde al track (una picada apunta a un punto que no
/// existe o tiene una fracción fuera de 0–1). Las picadas que no se pueden situar dejan sin
/// sub-track sus dos tramos, con el motivo.
pub fn segment(track: &Track, alignment: &Alignment) -> Result<Segmentation, SegmentationError> {
    let mut boundaries: Vec<Result<Boundary, MissingPosition>> =
        Vec::with_capacity(alignment.punches.len());
    let mut controls = Vec::with_capacity(alignment.punches.len());

    for (position, punch) in alignment.punches.iter().enumerate() {
        let boundary = locate_punch(track, position, punch)?;
        let in_gap = punch.location.is_some_and(|l| l.in_gap);
        controls.push(ControlPosition {
            position,
            code: punch.code,
            role: role(punch.code),
            time: boundary.as_ref().ok().map(|b| b.point.time),
            point: boundary.as_ref().ok().map(|b| GeoPoint {
                lat: b.point.lat,
                lon: b.point.lon,
                altitude_m: b.point.altitude_m,
            }),
            in_gap,
            missing: boundary.as_ref().err().copied(),
        });
        boundaries.push(boundary);
    }

    let legs = alignment
        .punches
        .windows(2)
        .zip(boundaries.windows(2))
        .enumerate()
        .map(|(i, (punches, ends))| {
            let (from, to) = (&punches[0], &punches[1]);
            let cut = match (&ends[0], &ends[1]) {
                (Err(reason), _) => Err(missing_leg(*reason, from.code)),
                (_, Err(reason)) => Err(missing_leg(*reason, to.code)),
                (Ok(start), Ok(end)) => cut(track, start, end),
            };
            LegTrack {
                index: i + 1,
                from: from.code,
                to: to.code,
                missing: cut.as_ref().err().copied(),
                track: cut.ok(),
            }
        })
        .collect();

    Ok(Segmentation { controls, legs })
}

/// Combina las posiciones de las balizas de varios corredores del mismo recorrido.
///
/// Agrupa por posición en la secuencia de picadas y código (la misma baliza en el mismo punto
/// del recorrido) y da la mediana de la latitud, de la longitud y de la altitud por separado.
/// No cuentan las balizas sin posición ni las que caen en un hueco del track. Las balizas que
/// nadie sitúa no salen. El resultado va ordenado por posición y, a igual posición, por código.
pub fn median_control_positions<'a, I>(runners: I) -> Vec<MedianControlPosition>
where
    I: IntoIterator<Item = &'a [ControlPosition]>,
{
    let mut groups: BTreeMap<(usize, ControlCode), Vec<GeoPoint>> = BTreeMap::new();
    for controls in runners {
        for control in controls {
            let Some(point) = control.point else { continue };
            if control.in_gap || !point.lat.is_finite() || !point.lon.is_finite() {
                continue;
            }
            groups
                .entry((control.position, control.code))
                .or_default()
                .push(point);
        }
    }

    groups
        .into_iter()
        .filter_map(|((position, code), points)| {
            Some(MedianControlPosition {
                position,
                code,
                lat: median(points.iter().map(|p| p.lat))?,
                lon: median(points.iter().map(|p| p.lon))?,
                altitude_m: median(points.iter().filter_map(|p| p.altitude_m)),
                runners: points.len(),
            })
        })
        .collect()
}

fn role(code: ControlCode) -> ControlRole {
    match code {
        START_CODE => ControlRole::Start,
        FINISH_CODE => ControlRole::Finish,
        _ => ControlRole::Control,
    }
}

fn missing_leg(reason: MissingPosition, code: ControlCode) -> MissingLegTrack {
    match reason {
        MissingPosition::NoPunchTime => MissingLegTrack::NoPunchTime { code },
        MissingPosition::OutsideTrack => MissingLegTrack::OutsideTrack { code },
    }
}

/// Sitúa una picada en el track: el punto interpolado en su instante.
fn locate_punch(
    track: &Track,
    position: usize,
    punch: &AlignedPunch,
) -> Result<Result<Boundary, MissingPosition>, SegmentationError> {
    let Some(location) = punch.location else {
        return Ok(Err(if punch.punch_time.is_none() {
            MissingPosition::NoPunchTime
        } else {
            MissingPosition::OutsideTrack
        }));
    };
    let outside = || SegmentationError::LocationOutsideTrack {
        position,
        code: punch.code,
        index: location.index,
        len: track.points.len(),
    };
    let fraction = location.fraction;
    if !(0.0..=1.0).contains(&fraction) {
        return Err(SegmentationError::InvalidFraction {
            position,
            code: punch.code,
            fraction,
        });
    }
    let a = track.points.get(location.index).ok_or_else(outside)?;
    let point = if fraction == 0.0 {
        let mut p = a.clone();
        if let Some(t) = punch.track_time {
            p.time = t;
        }
        p
    } else {
        let b = track.points.get(location.index + 1).ok_or_else(outside)?;
        interpolate(a, b, fraction, punch.track_time)
    };
    Ok(Ok(Boundary {
        index: location.index,
        point,
    }))
}

/// Punto entre `a` y `b` a la fracción `f` (0 = `a`). Las magnitudes opcionales solo se
/// interpolan si las tienen los dos puntos. El instante es `time` si se conoce (el de la
/// picada en hora del reloj) y si no, el interpolado.
fn interpolate(a: &TrackPoint, b: &TrackPoint, f: f64, time: Option<DateTime<Utc>>) -> TrackPoint {
    let lerp = |x: f64, y: f64| x + f * (y - x);
    let lerp_opt = |x: Option<f64>, y: Option<f64>| Some(lerp(x?, y?));
    let time = time.unwrap_or_else(|| {
        let span = (b.time - a.time).num_microseconds().unwrap_or(0) as f64;
        a.time + TimeDelta::microseconds((f * span).round() as i64)
    });
    TrackPoint {
        time,
        lat: lerp(a.lat, b.lat),
        lon: lerp(a.lon, b.lon),
        altitude_m: lerp_opt(a.altitude_m, b.altitude_m),
        heart_rate_bpm: lerp_opt(
            a.heart_rate_bpm.map(f64::from),
            b.heart_rate_bpm.map(f64::from),
        )
        .map(|hr| hr.round().clamp(0.0, 255.0) as u8),
        cadence_spm: lerp_opt(a.cadence_spm, b.cadence_spm),
        distance_m: lerp_opt(a.distance_m, b.distance_m),
    }
}

/// Sub-track entre dos límites: el límite inicial, los puntos del track estrictamente entre
/// los dos instantes y el límite final.
fn cut(track: &Track, start: &Boundary, end: &Boundary) -> Result<Track, MissingLegTrack> {
    if end.point.time < start.point.time {
        return Err(MissingLegTrack::OutOfOrder);
    }
    let mut points = vec![start.point.clone()];
    // `points[start.index] <= inicio < points[start.index + 1]` y `points[end.index] <= final`.
    let inner = track
        .points
        .get(start.index + 1..=end.index)
        .unwrap_or_default();
    points.extend(
        inner
            .iter()
            .filter(|p| p.time > start.point.time && p.time < end.point.time)
            .cloned(),
    );
    points.push(end.point.clone());
    Ok(Track { points })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alignment::{AlignmentQuality, Coverage, PunchUsage, TrackLocation};
    use chrono::TimeZone;
    use serde_json::json;

    fn t0() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 3, 16, 0, 0).unwrap()
    }

    fn at(seconds: f64) -> DateTime<Utc> {
        t0() + TimeDelta::milliseconds((seconds * 1000.0).round() as i64)
    }

    fn point(s: f64, lat: f64, lon: f64, altitude_m: Option<f64>) -> TrackPoint {
        TrackPoint {
            time: at(s),
            lat,
            lon,
            altitude_m,
            heart_rate_bpm: Some(150),
            cadence_spm: None,
            distance_m: Some(s * 3.0),
        }
    }

    /// Cuatro puntos cada 10 s: norte, oeste y sur. El tercero no tiene altitud.
    fn square() -> Track {
        Track {
            points: vec![
                point(0.0, 40.000, -3.000, Some(100.0)),
                point(10.0, 40.001, -3.000, Some(110.0)),
                point(20.0, 40.001, -3.002, None),
                point(30.0, 40.000, -3.002, Some(130.0)),
            ],
        }
    }

    /// Picada en el instante `s` (hora del reloj; desfase 0) situada en `(index, fraction)`.
    fn punch(code: ControlCode, s: Option<f64>, location: Option<(usize, f64)>) -> AlignedPunch {
        AlignedPunch {
            code,
            punch_time: s.map(at),
            track_time: location.and(s.map(at)),
            location: location.map(|(index, fraction)| TrackLocation {
                index,
                fraction,
                in_gap: false,
            }),
            usage: if code == START_CODE {
                PunchUsage::Start
            } else if s.is_none() {
                PunchUsage::NoTime
            } else {
                PunchUsage::Used
            },
            local_offset_s: None,
        }
    }

    fn alignment(punches: Vec<AlignedPunch>) -> Alignment {
        Alignment {
            offset_s: 0.0,
            offset_estimated: true,
            confidence: 1.0,
            quality: AlignmentQuality {
                controls_used: 0,
                support: 1.0,
                margin: 1.0,
                runner_up_offset_s: None,
            },
            coverage: Coverage {
                track_start: at(0.0),
                track_end: at(30.0),
                race_start: at(5.0),
                race_finish: at(30.0),
                missing_start_s: 0.0,
                missing_end_s: 0.0,
                gaps: vec![],
            },
            punches,
            warnings: vec![],
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Salida a los 5 s, baliza 31 a los 15 s y meta a los 30 s (justo en el último punto).
    fn three_punches() -> Alignment {
        alignment(vec![
            punch(START_CODE, Some(5.0), Some((0, 0.5))),
            punch(31, Some(15.0), Some((1, 0.5))),
            punch(FINISH_CODE, Some(30.0), Some((3, 0.0))),
        ])
    }

    #[test]
    fn controls_are_interpolated_at_the_punch_instant() {
        let s = segment(&square(), &three_punches()).unwrap();
        assert_eq!(s.controls.len(), 3);

        // A mitad de camino entre el punto 0 y el 1.
        let start = &s.controls[0];
        assert_eq!(start.role, ControlRole::Start);
        assert_eq!(start.time, Some(at(5.0)));
        let p = start.point.unwrap();
        assert!(close(p.lat, 40.0005) && close(p.lon, -3.0));
        assert!(close(p.altitude_m.unwrap(), 105.0));

        // Entre el punto 1 y el 2: el 2 no tiene altitud, así que tampoco la baliza.
        let c31 = &s.controls[1];
        assert_eq!(
            (c31.position, c31.code, c31.role),
            (1, 31, ControlRole::Control)
        );
        let p = c31.point.unwrap();
        assert!(close(p.lat, 40.001) && close(p.lon, -3.001));
        assert_eq!(p.altitude_m, None);

        // Justo en el último punto.
        let finish = &s.controls[2];
        assert_eq!(finish.role, ControlRole::Finish);
        let p = finish.point.unwrap();
        assert!(close(p.lat, 40.0) && close(p.lon, -3.002));
        assert_eq!(p.altitude_m, Some(130.0));
        assert!(s.controls.iter().all(|c| c.missing.is_none() && !c.in_gap));
    }

    #[test]
    fn legs_share_their_boundaries() {
        let track = square();
        let s = segment(&track, &three_punches()).unwrap();
        assert_eq!(s.legs.len(), 2);
        let leg1 = s.legs[0].track.as_ref().unwrap();
        let leg2 = s.legs[1].track.as_ref().unwrap();
        assert_eq!(
            (s.legs[0].index, s.legs[0].from, s.legs[0].to),
            (1, START_CODE, 31)
        );
        assert_eq!(
            (s.legs[1].index, s.legs[1].from, s.legs[1].to),
            (2, 31, FINISH_CODE)
        );

        // Tramo 1: límite a los 5 s, punto 1 (10 s) y límite a los 15 s.
        let times: Vec<_> = leg1.points.iter().map(|p| p.time).collect();
        assert_eq!(times, vec![at(5.0), at(10.0), at(15.0)]);
        assert_eq!(leg1.points[1], track.points[1]);
        // Tramo 2: límite a los 15 s, punto 2 (20 s) y la meta, sin repetir el punto 3.
        let times: Vec<_> = leg2.points.iter().map(|p| p.time).collect();
        assert_eq!(times, vec![at(15.0), at(20.0), at(30.0)]);
        assert_eq!(leg2.points[2], track.points[3]);

        // El límite compartido es el mismo punto en los dos tramos.
        assert_eq!(leg1.points.last(), leg2.points.first());
        // Las magnitudes opcionales también se interpolan: distancia 3 m/s × 15 s.
        let boundary = &leg2.points[0];
        assert!(close(boundary.distance_m.unwrap(), 45.0));
        assert_eq!(boundary.heart_rate_bpm, Some(150));
        assert_eq!(boundary.cadence_spm, None);
    }

    #[test]
    fn punch_without_time_leaves_its_two_legs_without_track() {
        let a = alignment(vec![
            punch(START_CODE, Some(5.0), Some((0, 0.5))),
            punch(31, None, None),
            punch(45, Some(25.0), Some((2, 0.5))),
            punch(FINISH_CODE, Some(30.0), Some((3, 0.0))),
        ]);
        let s = segment(&square(), &a).unwrap();
        assert_eq!(s.controls[1].missing, Some(MissingPosition::NoPunchTime));
        assert_eq!(s.controls[1].point, None);
        assert_eq!(s.controls[1].time, None);
        assert_eq!(s.legs.len(), 3);
        for leg in &s.legs[..2] {
            assert_eq!(leg.track, None);
            assert_eq!(leg.missing, Some(MissingLegTrack::NoPunchTime { code: 31 }));
        }
        let leg3 = s.legs[2].track.as_ref().unwrap();
        let times: Vec<_> = leg3.points.iter().map(|p| p.time).collect();
        assert_eq!(times, vec![at(25.0), at(30.0)]);
    }

    #[test]
    fn punch_outside_track_and_out_of_order() {
        let a = alignment(vec![
            punch(START_CODE, Some(-20.0), None),
            punch(31, Some(15.0), Some((1, 0.5))),
            punch(45, Some(12.0), Some((1, 0.2))),
        ]);
        let s = segment(&square(), &a).unwrap();
        assert_eq!(s.controls[0].missing, Some(MissingPosition::OutsideTrack));
        assert_eq!(
            s.legs[0].missing,
            Some(MissingLegTrack::OutsideTrack { code: START_CODE })
        );
        assert_eq!(s.legs[1].missing, Some(MissingLegTrack::OutOfOrder));
        assert_eq!(s.legs[1].track, None);
    }

    #[test]
    fn location_that_does_not_fit_the_track_is_an_error() {
        let a = alignment(vec![punch(31, Some(15.0), Some((3, 0.5)))]);
        assert_eq!(
            segment(&square(), &a),
            Err(SegmentationError::LocationOutsideTrack {
                position: 0,
                code: 31,
                index: 3,
                len: 4
            })
        );
        let a = alignment(vec![punch(31, Some(15.0), Some((1, 1.5)))]);
        assert!(matches!(
            segment(&square(), &a),
            Err(SegmentationError::InvalidFraction { .. })
        ));
        // Sin picadas no hay tramos.
        let s = segment(&square(), &alignment(vec![])).unwrap();
        assert!(s.controls.is_empty() && s.legs.is_empty());
    }

    #[test]
    fn punch_in_gap_is_cut_but_flagged() {
        // Hueco de 40 s entre el punto 1 y el 2.
        let track = Track {
            points: vec![
                point(0.0, 40.000, -3.000, None),
                point(10.0, 40.001, -3.000, None),
                point(50.0, 40.001, -3.004, None),
            ],
        };
        let mut gap = punch(31, Some(20.0), Some((1, 0.25)));
        if let Some(l) = gap.location.as_mut() {
            l.in_gap = true;
        }
        let a = alignment(vec![
            punch(START_CODE, Some(0.0), Some((0, 0.0))),
            gap,
            punch(FINISH_CODE, Some(50.0), Some((2, 0.0))),
        ]);
        let s = segment(&track, &a).unwrap();
        let c = &s.controls[1];
        assert!(c.in_gap);
        assert!(close(c.point.unwrap().lon, -3.001));
        assert!(s.legs.iter().all(|l| l.track.is_some()));
        // La mediana no la usa.
        let combined = median_control_positions([s.controls.as_slice()]);
        let codes: Vec<_> = combined.iter().map(|m| m.code).collect();
        assert_eq!(codes, vec![START_CODE, FINISH_CODE]);
    }

    fn located(
        position: usize,
        code: ControlCode,
        lat: f64,
        lon: f64,
        alt: Option<f64>,
    ) -> ControlPosition {
        ControlPosition {
            position,
            code,
            role: role(code),
            time: Some(at(0.0)),
            point: Some(GeoPoint {
                lat,
                lon,
                altitude_m: alt,
            }),
            in_gap: false,
            missing: None,
        }
    }

    #[test]
    fn median_of_odd_and_even_number_of_runners() {
        let a = vec![
            located(1, 31, 40.0, -3.0, Some(100.0)),
            located(2, 45, 41.0, -4.0, None),
        ];
        let b = vec![
            located(1, 31, 40.3, -3.9, Some(104.0)),
            located(2, 45, 41.2, -4.4, Some(200.0)),
        ];
        let c = vec![
            located(1, 31, 40.1, -3.1, None),
            located(2, 45, 41.4, -4.2, Some(210.0)),
        ];
        let mut d = vec![
            located(1, 31, 40.2, -3.2, Some(108.0)),
            // Otra baliza en la misma posición: no se mezcla con la 45.
            located(2, 46, 50.0, -5.0, None),
        ];
        d.push(ControlPosition {
            point: None,
            missing: Some(MissingPosition::NoPunchTime),
            ..located(3, FINISH_CODE, 0.0, 0.0, None)
        });

        let runners = [a.as_slice(), b.as_slice(), c.as_slice(), d.as_slice()];
        let m = median_control_positions(runners);
        assert_eq!(m.len(), 3);

        // Baliza 31, cuatro corredores (par): media de los dos centrales.
        assert_eq!((m[0].position, m[0].code, m[0].runners), (1, 31, 4));
        assert!(close(m[0].lat, 40.15) && close(m[0].lon, -3.15));
        // Altitudes 100, 104 y 108 (impar): 104.
        assert_eq!(m[0].altitude_m, Some(104.0));

        // Baliza 45, tres corredores (impar).
        assert_eq!((m[1].position, m[1].code, m[1].runners), (2, 45, 3));
        assert!(close(m[1].lat, 41.2) && close(m[1].lon, -4.2));
        // Altitudes 200 y 210 (par).
        assert_eq!(m[1].altitude_m, Some(205.0));

        // Baliza 46, un corredor. La meta sin posición no sale.
        assert_eq!((m[2].position, m[2].code, m[2].runners), (2, 46, 1));
        assert_eq!(m[2].altitude_m, None);

        assert!(median_control_positions(std::iter::empty()).is_empty());
    }

    #[test]
    fn json_uses_snake_case() {
        let a = alignment(vec![
            punch(START_CODE, Some(5.0), Some((0, 0.5))),
            punch(31, None, None),
        ]);
        let s = segment(&square(), &a).unwrap();
        let value = serde_json::to_value(&s).unwrap();
        assert_eq!(value["controls"][0]["role"], json!("start"));
        assert_eq!(value["controls"][0]["time"], json!("2026-10-03T16:00:05Z"));
        assert_eq!(value["controls"][1]["missing"], json!("no_punch_time"));
        assert_eq!(
            value["legs"][0]["missing"],
            json!({"reason": "no_punch_time", "code": 31})
        );
        assert_eq!(value["legs"][0]["track"], json!(null));
        let back: Segmentation = serde_json::from_value(value).unwrap();
        assert_eq!(back, s);
    }
}
