//! Pérdida según desnivel (P13 de `docs/preguntas.md`: «¿Me frena el desnivel?»).
//!
//! Clasifica cada tramo en subida, llano o bajada por la subida y la bajada acumuladas del FIT
//! ([`crate::metrics`], altitud suavizada) por cada 100 m recorridos, y da por clase, sobre los
//! tramos que cuentan del histórico ([`crate::history::pattern_legs`]): tramos, errores, tasa de
//! error e IR medio ponderado por `ref_i`. Las definiciones están en `docs/historico.md`,
//! "Pérdida según desnivel (P13)".

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::history::{HistoryFilter, HistoryRace, pattern_legs};
use crate::metrics::{LegMetrics, TrackMetrics};
use crate::runner_report::LegReport;

/// Umbral por defecto (m por cada 100 m recorridos): sube o baja al menos esto para ser subida o
/// bajada. Es el único sitio donde vive el valor; [`SlopeConfig::default`] lo usa.
pub const DEFAULT_THRESHOLD_M_PER_100M: f64 = 4.0;

/// Distancia recorrida mínima (m) para clasificar un tramo: por debajo, un metro de error de la
/// altitud suavizada ya mueve la pendiente más que el umbral.
pub const MIN_DISTANCE_M: f64 = 50.0;

/// Parámetros de la clasificación.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SlopeConfig {
    /// Subida (o bajada) acumulada mínima, en m por cada 100 m recorridos, para que un tramo sea
    /// de subida (o de bajada). Tiene que ser un número mayor que 0.
    pub threshold_m_per_100m: f64,
}

impl Default for SlopeConfig {
    fn default() -> Self {
        Self {
            threshold_m_per_100m: DEFAULT_THRESHOLD_M_PER_100M,
        }
    }
}

impl SlopeConfig {
    /// Comprueba que el umbral es un número finito mayor que 0.
    pub fn validate(&self) -> Result<(), SlopeError> {
        let t = self.threshold_m_per_100m;
        if t.is_finite() && t > 0.0 {
            Ok(())
        } else {
            Err(SlopeError::InvalidThreshold)
        }
    }
}

/// Errores de P13.
#[derive(Debug, Error, PartialEq)]
pub enum SlopeError {
    #[error("el umbral de desnivel tiene que ser un número mayor que 0 (m por cada 100 m)")]
    InvalidThreshold,
}

/// Clase de desnivel de un tramo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlopeClass {
    Uphill,
    Flat,
    Downhill,
}

/// Clases en el orden en que salen: subida, llano y bajada.
pub const CLASSES: [SlopeClass; 3] = [SlopeClass::Uphill, SlopeClass::Flat, SlopeClass::Downhill];

/// Subida y bajada acumuladas por cada 100 m recorridos de un sub-track: `(subida, bajada)`.
/// `None` si no tiene altitud o recorre menos de [`MIN_DISTANCE_M`] (o algún valor no es
/// finito).
pub fn climb_per_100m(metrics: &TrackMetrics) -> Option<(f64, f64)> {
    let distance = metrics.distance_m;
    if !(distance.is_finite() && distance >= MIN_DISTANCE_M) {
        return None;
    }
    let (ascent, descent) = (metrics.ascent_m?, metrics.descent_m?);
    if !(ascent.is_finite() && descent.is_finite()) {
        return None;
    }
    Some((ascent / distance * 100.0, descent / distance * 100.0))
}

/// Clase de desnivel de un sub-track; `None` si no se puede clasificar ([`climb_per_100m`]) o el
/// umbral no es válido.
///
/// Subida si sube al menos el umbral por cada 100 m; bajada si baja al menos el umbral; llano en
/// otro caso. Si cumple las dos, gana la mayor, y a igualdad, subida (subir cuesta más que bajar
/// lo mismo).
pub fn classify(metrics: &TrackMetrics, config: &SlopeConfig) -> Option<SlopeClass> {
    config.validate().ok()?;
    let (up, down) = climb_per_100m(metrics)?;
    let t = config.threshold_m_per_100m;
    Some(match (up >= t, down >= t) {
        (true, true) if down > up => SlopeClass::Downhill,
        (true, _) => SlopeClass::Uphill,
        (false, true) => SlopeClass::Downhill,
        (false, false) => SlopeClass::Flat,
    })
}

/// Clase del tramo `leg` del informe con las métricas del FIT de su carrera. Se empareja por
/// número de tramo y balizas; sin pareja (picadas que no casan con el recorrido) o sin sub-track,
/// `None`.
pub fn classify_leg(
    leg: &LegReport,
    metrics: &[LegMetrics],
    config: &SlopeConfig,
) -> Option<SlopeClass> {
    let m = metrics
        .iter()
        .find(|m| m.index == leg.index && m.from == leg.from && m.to == leg.to)?;
    classify(m.track.as_ref()?, config)
}

/// Una carrera del histórico con las métricas del FIT de cada tramo.
#[derive(Debug, Clone, Copy)]
pub struct SlopeRace<'a> {
    pub race: &'a HistoryRace,
    /// `None` si la carrera no tiene track (o no se puede alinear ni trocear).
    pub leg_metrics: Option<&'a [LegMetrics]>,
}

/// Números de una clase.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SlopeStats {
    pub class: SlopeClass,
    /// Tramos que cuentan de la clase (`n`).
    pub legs: usize,
    /// De ellos, los que son error.
    pub errors: usize,
    /// `errors / legs` (0–1).
    pub error_rate: Option<f64>,
    /// IR medio de los tramos de la clase, ponderado por `ref_i`: `Σ ref_i·IR_i / Σ ref_i`
    /// (1 = 100 %).
    pub mean_performance: Option<f64>,
}

/// P13 con un filtro: las tres clases y de dónde salen los tramos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlopeHistory {
    pub config: SlopeConfig,
    /// Subida, llano y bajada, siempre y en ese orden.
    pub by_class: Vec<SlopeStats>,
    /// Carreras con números (con rendimiento habitual) y track: las que aportan tramos.
    pub races_with_track: usize,
    /// Carreras con números sin track: no aportan tramos.
    pub races_without_track: usize,
    /// Tramos que cuentan de las carreras sin track.
    pub legs_without_track: usize,
    /// Tramos que cuentan de carreras con track que no se pueden clasificar (sin sub-track, sin
    /// altitud o menos de [`MIN_DISTANCE_M`] recorridos).
    pub unclassified_legs: usize,
}

/// Sumas de una clase.
#[derive(Debug, Default, Clone, Copy)]
struct Accumulator {
    legs: usize,
    errors: usize,
    weight: f64,
    weighted_performance: f64,
}

/// Agrega por clase de desnivel los tramos que cuentan de las carreras que pasan `filter` (el
/// mismo filtro y las mismas carreras que [`crate::history::history`]).
pub fn slope(
    races: &[SlopeRace<'_>],
    filter: &HistoryFilter,
    config: &SlopeConfig,
) -> Result<SlopeHistory, SlopeError> {
    config.validate()?;
    let mut acc = [Accumulator::default(); CLASSES.len()];
    let mut out = SlopeHistory {
        config: *config,
        by_class: Vec::new(),
        races_with_track: 0,
        races_without_track: 0,
        legs_without_track: 0,
        unclassified_legs: 0,
    };
    let counted = races.iter().filter(|r| {
        // Como en el histórico, una carrera sin rendimiento habitual no tiene números.
        filter.includes(r.race.date, r.race.format) && r.race.lost_time.usual_performance.is_some()
    });
    for r in counted {
        let legs = pattern_legs(&r.race.lost_time);
        let Some(metrics) = r.leg_metrics else {
            out.races_without_track += 1;
            out.legs_without_track += legs.count();
            continue;
        };
        out.races_with_track += 1;
        for leg in legs {
            let class = classify_leg(leg, metrics, config);
            let Some(a) = class
                .and_then(|c| CLASSES.iter().position(|k| *k == c))
                .and_then(|i| acc.get_mut(i))
            else {
                out.unclassified_legs += 1;
                continue;
            };
            a.legs += 1;
            if leg.is_error {
                a.errors += 1;
            }
            if let (Some(ir), Some(reference)) = (leg.performance_index, leg.reference_s) {
                a.weight += reference;
                a.weighted_performance += reference * ir;
            }
        }
    }
    out.by_class = CLASSES
        .iter()
        .zip(&acc)
        .map(|(class, a)| SlopeStats {
            class: *class,
            legs: a.legs,
            errors: a.errors,
            error_rate: (a.legs > 0).then(|| a.errors as f64 / a.legs as f64),
            mean_performance: (a.weight > 0.0).then(|| a.weighted_performance / a.weight),
        })
        .collect();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{MetricsOptions, leg_metrics};
    use crate::model::{Track, TrackPoint};
    use crate::race_format::RaceFormat;
    use crate::runner_report::RunnerLostTime;
    use crate::segmentation::{LegTrack, MissingLegTrack, Segmentation};
    use chrono::{DateTime, TimeDelta, Utc};

    const EPS: f64 = 1e-9;

    fn close(actual: Option<f64>, expected: f64) {
        let Some(a) = actual else {
            panic!("se esperaba {expected} y no hay valor");
        };
        assert!((a - expected).abs() < EPS, "{a} != {expected}");
    }

    /// Métricas de un sub-track de `distance` m con esa subida y bajada (m).
    fn tm(distance: f64, ascent: Option<f64>, descent: Option<f64>) -> TrackMetrics {
        TrackMetrics {
            duration_s: 60.0,
            distance_m: distance,
            straight_m: distance,
            distance_ratio: Some(1.0),
            moving_s: 60.0,
            stopped_s: 0.0,
            gap_s: 0.0,
            moving_speed_mps: Some(distance / 60.0),
            ascent_m: ascent,
            descent_m: descent,
            heart_rate_bpm: None,
            cadence_spm: None,
        }
    }

    fn class(distance: f64, ascent: f64, descent: f64) -> Option<SlopeClass> {
        classify(
            &tm(distance, Some(ascent), Some(descent)),
            &SlopeConfig::default(),
        )
    }

    #[test]
    fn default_threshold_is_four_meters_per_hundred() {
        assert_eq!(SlopeConfig::default().threshold_m_per_100m, 4.0);
        // En JSON, un campo que falta toma el valor por defecto.
        let config: SlopeConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config, SlopeConfig::default());
    }

    /// Perfiles sintéticos (criterio de aceptación de #29): (distancia, subida, bajada) → clase.
    #[test]
    fn classifies_synthetic_profiles() {
        use SlopeClass::*;
        let cases = [
            // Llano: 1 y 1,5 m/100 m.
            (200.0, 2.0, 3.0, Some(Flat)),
            // Subida justo en el umbral (4 m/100 m): entra.
            (200.0, 8.0, 1.0, Some(Uphill)),
            // Justo por debajo (3,99): llano.
            (200.0, 7.98, 0.0, Some(Flat)),
            // Bajada en el umbral: 10 m en 250 m.
            (250.0, 0.0, 10.0, Some(Downhill)),
            // Sube 6 y baja 5 por cada 100 m: las dos pasan y gana la mayor.
            (200.0, 12.0, 10.0, Some(Uphill)),
            (200.0, 10.0, 12.0, Some(Downhill)),
            // Empate con las dos por encima: subida.
            (200.0, 10.0, 10.0, Some(Uphill)),
            // Sube 3,9 y baja 3,9: ninguna pasa, llano aunque el total sea grande.
            (1000.0, 39.0, 39.0, Some(Flat)),
            // Una cuesta fuerte de sprint: 15 m en 120 m (12,5 m/100 m).
            (120.0, 15.0, 0.5, Some(Uphill)),
            // Distancia mínima: 50 m se clasifica, menos no.
            (50.0, 2.0, 0.0, Some(Uphill)),
            (49.9, 2.0, 0.0, None),
            (0.0, 0.0, 0.0, None),
            (f64::NAN, 1.0, 1.0, None),
            (200.0, f64::INFINITY, 0.0, None),
        ];
        for (distance, ascent, descent, expected) in cases {
            assert_eq!(
                class(distance, ascent, descent),
                expected,
                "{distance} m, +{ascent} −{descent}"
            );
        }
        // Sin altitud no se clasifica.
        let config = SlopeConfig::default();
        assert_eq!(classify(&tm(200.0, None, None), &config), None);
        assert_eq!(
            climb_per_100m(&tm(200.0, Some(8.0), Some(3.0))),
            Some((4.0, 1.5))
        );
    }

    #[test]
    fn threshold_is_configurable() {
        let gentle = tm(200.0, Some(6.0), Some(2.0)); // 3 y 1 m/100 m
        let rolling = tm(200.0, Some(12.0), Some(10.0)); // 6 y 5
        let at = |t: f64| SlopeConfig {
            threshold_m_per_100m: t,
        };
        assert_eq!(classify(&gentle, &at(4.0)), Some(SlopeClass::Flat));
        assert_eq!(classify(&gentle, &at(3.0)), Some(SlopeClass::Uphill));
        assert_eq!(classify(&gentle, &at(0.5)), Some(SlopeClass::Uphill));
        assert_eq!(classify(&rolling, &at(5.5)), Some(SlopeClass::Uphill));
        assert_eq!(classify(&rolling, &at(10.0)), Some(SlopeClass::Flat));
        // Umbral inválido: ningún tramo se clasifica y el agregado es un error.
        for t in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(classify(&gentle, &at(t)), None, "umbral {t}");
            assert_eq!(at(t).validate(), Err(SlopeError::InvalidThreshold));
            assert_eq!(
                slope(&[], &HistoryFilter::default(), &at(t)),
                Err(SlopeError::InvalidThreshold)
            );
        }
    }

    // --- Perfil sintético de punta a punta: track → métricas → clase. ---

    /// Metros por grado de latitud (radio de `docs/alineacion.md`).
    const M_PER_DEG: f64 = 6_371_008.8 * std::f64::consts::PI / 180.0;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-03T16:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    /// Punto a `s` s de `t0`, `north_m` m al norte de (40°, −4°) y a `altitude` m.
    fn point(s: i64, north_m: f64, altitude: f64) -> TrackPoint {
        TrackPoint {
            time: t0() + TimeDelta::seconds(s),
            lat: 40.0 + north_m / M_PER_DEG,
            lon: -4.0,
            altitude_m: Some(altitude),
            heart_rate_bpm: None,
            cadence_spm: None,
            distance_m: None,
        }
    }

    /// Cuatro tramos de 60 s a 4 m/s (240 m) hacia el norte, con un punto por segundo:
    ///
    /// 1. sube 15 m (6,25 m/100 m) → subida;
    /// 2. llano;
    /// 3. baja 15 m → bajada;
    /// 4. sube 18 m en 120 m y baja 14 m en los otros 120 (7,5 y 5,8 m/100 m) → subida, la
    ///    mayor.
    ///
    /// Más un quinto tramo sin sub-track, que no se clasifica.
    #[test]
    fn classifies_a_synthetic_track_end_to_end() {
        let profile = |s: i64| -> f64 {
            let x = s as f64;
            match s {
                0..=60 => 100.0 + 15.0 * x / 60.0,
                61..=120 => 115.0,
                121..=180 => 115.0 - 15.0 * (x - 120.0) / 60.0,
                181..=210 => 100.0 + 18.0 * (x - 180.0) / 30.0,
                _ => 118.0 - 14.0 * (x - 210.0) / 30.0,
            }
        };
        let points: Vec<TrackPoint> = (0..=240)
            .map(|s| point(s, 4.0 * s as f64, profile(s)))
            .collect();
        let track = Track {
            points: points.clone(),
            sport: None,
        };
        let mut legs: Vec<LegTrack> = (0..4)
            .map(|i| LegTrack {
                index: i + 1,
                from: 31 + i as u16,
                to: 32 + i as u16,
                track: Some(Track {
                    points: points[i * 60..=(i + 1) * 60].to_vec(),
                    sport: None,
                }),
                missing: None,
            })
            .collect();
        legs.push(LegTrack {
            index: 5,
            from: 35,
            to: 36,
            track: None,
            missing: Some(MissingLegTrack::OutOfOrder),
        });
        let segmentation = Segmentation {
            controls: Vec::new(),
            legs,
        };
        let metrics = leg_metrics(&track, &segmentation, &MetricsOptions::default()).unwrap();
        let config = SlopeConfig::default();
        let classes: Vec<_> = metrics
            .iter()
            .map(|m| m.track.as_ref().and_then(|t| classify(t, &config)))
            .collect();
        use SlopeClass::*;
        assert_eq!(
            classes,
            [Some(Uphill), Some(Flat), Some(Downhill), Some(Uphill), None]
        );
        // El suavizado (±5 s) redondea las esquinas del perfil y le quita desnivel al tramo (aquí,
        // de 18 a unos 15 m de subida), pero subida y bajada siguen por encima del umbral.
        let (up, down) = climb_per_100m(metrics[3].track.as_ref().unwrap()).unwrap();
        assert!(up < 7.5 && up > 6.0, "subida {up}");
        assert!(down < 14.0 / 2.4 && down > 4.5, "bajada {down}");
    }

    // --- Agregado por clase. ---

    /// Tramo del informe con referencia, IR y error, y las métricas de su sub-track
    /// (`None` = sin sub-track).
    #[derive(Clone, Copy)]
    struct L {
        reference: f64,
        ir: f64,
        error: bool,
        last: bool,
        /// (distancia, subida, bajada); subida y bajada `None` = sin altitud.
        track: Option<(f64, Option<f64>, Option<f64>)>,
        /// Las balizas de las métricas no casan con las del informe.
        mismatch: bool,
    }

    const fn leg(reference: f64, ir: f64, error: bool, d: f64, up: f64, down: f64) -> L {
        L {
            reference,
            ir,
            error,
            last: false,
            track: Some((d, Some(up), Some(down))),
            mismatch: false,
        }
    }

    const fn up(reference: f64, ir: f64, error: bool) -> L {
        leg(reference, ir, error, 200.0, 10.0, 0.0)
    }

    const fn flat(reference: f64, ir: f64) -> L {
        leg(reference, ir, false, 150.0, 1.0, 1.0)
    }

    fn race(
        date: &str,
        format: Option<RaceFormat>,
        usual: Option<f64>,
        legs: &[L],
    ) -> (HistoryRace, Vec<LegMetrics>) {
        let reports = legs
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let short = l.reference < 20.0;
                LegReport {
                    index: i + 1,
                    from: 31 + i as u16,
                    to: 32 + i as u16,
                    split_s: Some(l.reference / l.ir),
                    elapsed_s: None,
                    place: None,
                    reference_s: Some(l.reference),
                    reference_count: 1,
                    valid_splits: 4,
                    performance_index: Some(l.ir),
                    expected_s: Some(l.reference),
                    loss_s: Some(if l.error { 30.0 } else { 0.0 }),
                    loss_pct: Some(if l.error { 30.0 } else { 0.0 }),
                    is_error: l.error,
                    gain_s: None,
                    cumulative_gain_s: None,
                    ideal_elapsed_s: None,
                    behind_ideal_s: None,
                    is_last: l.last,
                    short_reference: short,
                    excluded_from_patterns: l.last || short,
                }
            })
            .collect();
        let metrics = legs
            .iter()
            .enumerate()
            .map(|(i, l)| LegMetrics {
                index: i + 1,
                from: if l.mismatch { 99 } else { 31 + i as u16 },
                to: 32 + i as u16,
                track: l.track.map(|(d, a, b)| tm(d, a, b)),
                missing: l.track.is_none().then_some(MissingLegTrack::OutOfOrder),
            })
            .collect();
        let race = HistoryRace {
            date: date.parse().unwrap(),
            format,
            lost_time: RunnerLostTime {
                total_s: None,
                usual_performance: usual,
                lost_time_s: None,
                error_count: 0,
                time_without_errors_s: None,
                ideal_time_s: None,
                behind_ideal_s: None,
                losing_streaks: Vec::new(),
                consistency: None,
                legs: reports,
            },
        };
        (race, metrics)
    }

    /// Cuatro carreras sintéticas (referencia en s, IR, error y perfil):
    ///
    /// - A, sprint, 1-mar, con track: 40 s, IR 1,0, subida; 60 s, 0,5, error, subida (6 y
    ///   1 m/100 m); 30 s, 0,9, llano; 50 s, 0,8, error, bajada (4,5 m/100 m); 45 s sin altitud
    ///   (sin clasificar); uno de 15 s en subida (referencia corta, no cuenta) y el último, que
    ///   no cuenta.
    /// - B, media, 1-abr, con track: 100 s, 0,8, llano (2 y 1); 200 s, 0,6, error, sube 6 y baja
    ///   5 (subida); 120 s, 1,2, sube 5 y baja 6 (bajada); 90 s sin sub-track y 80 s con las
    ///   balizas cambiadas (sin clasificar), y el último.
    /// - C, larga, 1-may, sin track: dos tramos que cuentan y el último.
    /// - D, larga, 1-jun, con track y sin rendimiento habitual: no cuenta.
    fn races() -> Vec<(HistoryRace, Option<Vec<LegMetrics>>)> {
        use RaceFormat::*;
        let last = |l: L| L { last: true, ..l };
        let with = |(r, m): (HistoryRace, Vec<LegMetrics>)| (r, Some(m));
        let without = |(r, _): (HistoryRace, Vec<LegMetrics>)| (r, None);
        vec![
            with(race(
                "2026-03-01",
                Some(Sprint),
                Some(0.9),
                &[
                    up(40.0, 1.0, false),
                    leg(60.0, 0.5, true, 200.0, 12.0, 2.0),
                    flat(30.0, 0.9),
                    leg(50.0, 0.8, true, 200.0, 0.0, 9.0),
                    L {
                        track: Some((200.0, None, None)),
                        ..flat(45.0, 1.1)
                    },
                    up(15.0, 0.4, true),
                    last(flat(30.0, 1.0)),
                ],
            )),
            with(race(
                "2026-04-01",
                Some(Middle),
                Some(1.0),
                &[
                    leg(100.0, 0.8, false, 400.0, 8.0, 4.0),
                    leg(200.0, 0.6, true, 500.0, 30.0, 25.0),
                    leg(120.0, 1.2, false, 400.0, 20.0, 24.0),
                    L {
                        track: None,
                        ..up(90.0, 1.0, false)
                    },
                    L {
                        mismatch: true,
                        ..up(80.0, 0.9, false)
                    },
                    last(up(60.0, 1.0, false)),
                ],
            )),
            without(race(
                "2026-05-01",
                Some(Long),
                Some(0.95),
                &[
                    up(300.0, 0.9, true),
                    flat(400.0, 1.0),
                    last(flat(60.0, 1.0)),
                ],
            )),
            with(race(
                "2026-06-01",
                Some(Long),
                None,
                &[up(300.0, 0.9, true), last(flat(60.0, 1.0))],
            )),
        ]
    }

    fn run(filter: &HistoryFilter, config: &SlopeConfig) -> SlopeHistory {
        let data = races();
        let input: Vec<SlopeRace<'_>> = data
            .iter()
            .map(|(race, metrics)| SlopeRace {
                race,
                leg_metrics: metrics.as_deref(),
            })
            .collect();
        slope(&input, filter, config).unwrap()
    }

    /// Comprueba clase, tramos, errores, tasa de error e IR medio.
    fn check(s: &SlopeStats, class: SlopeClass, counts: (usize, usize), rate: f64, ir: f64) {
        assert_eq!(s.class, class);
        assert_eq!((s.legs, s.errors), counts, "{class:?}");
        close(s.error_rate, rate);
        close(s.mean_performance, ir);
    }

    fn empty(s: &SlopeStats) {
        assert_eq!((s.legs, s.errors), (0, 0), "{:?}", s.class);
        assert_eq!((s.error_rate, s.mean_performance), (None, None));
    }

    #[test]
    fn aggregates_counted_legs_by_class() {
        use SlopeClass::*;
        let h = run(&HistoryFilter::default(), &SlopeConfig::default());
        assert_eq!(h.config, SlopeConfig::default());
        assert_eq!(h.by_class.len(), 3);
        // Subida: A 40 s (1,0), A 60 s (0,5, error), B 200 s (0,6, error).
        // IR = (40·1,0 + 60·0,5 + 200·0,6) / 300 = 190 / 300.
        check(&h.by_class[0], Uphill, (3, 2), 2.0 / 3.0, 190.0 / 300.0);
        // Llano: A 30 s (0,9), B 100 s (0,8). IR = (27 + 80) / 130.
        check(&h.by_class[1], Flat, (2, 0), 0.0, 107.0 / 130.0);
        // Bajada: A 50 s (0,8, error), B 120 s (1,2). IR = (40 + 144) / 170.
        check(&h.by_class[2], Downhill, (2, 1), 0.5, 184.0 / 170.0);
        // A y B con track; C sin track aporta 2 tramos sin clasificar; D no cuenta.
        assert_eq!((h.races_with_track, h.races_without_track), (2, 1));
        assert_eq!(h.legs_without_track, 2);
        // A: el de 45 s sin altitud; B: el de 90 s sin sub-track y el de balizas cambiadas.
        assert_eq!(h.unclassified_legs, 3);
        // Las clases, los sin clasificar y los sin track suman los tramos que cuentan (12).
        let classified: usize = h.by_class.iter().map(|s| s.legs).sum();
        assert_eq!(classified + h.unclassified_legs + h.legs_without_track, 12);
    }

    #[test]
    fn a_higher_threshold_moves_legs_to_flat() {
        use SlopeClass::*;
        let h = run(
            &HistoryFilter::default(),
            &SlopeConfig {
                threshold_m_per_100m: 5.0,
            },
        );
        // A 40 s (5 m/100 m) sigue en subida (el umbral entra); A 50 s (baja 4,5) pasa a llano.
        check(&h.by_class[0], Uphill, (3, 2), 2.0 / 3.0, 190.0 / 300.0);
        // Llano: A 30 s, A 50 s (error) y B 100 s. IR = (27 + 40 + 80) / 180.
        check(&h.by_class[1], Flat, (3, 1), 1.0 / 3.0, 147.0 / 180.0);
        // Bajada: B 120 s (sube 5 y baja 6).
        check(&h.by_class[2], Downhill, (1, 0), 0.0, 1.2);
    }

    #[test]
    fn format_and_date_filters_apply() {
        use SlopeClass::*;
        let config = SlopeConfig::default();
        let sprint = run(
            &HistoryFilter {
                format: Some(RaceFormat::Sprint),
                ..HistoryFilter::default()
            },
            &config,
        );
        check(&sprint.by_class[0], Uphill, (2, 1), 0.5, 70.0 / 100.0);
        check(&sprint.by_class[1], Flat, (1, 0), 0.0, 0.9);
        check(&sprint.by_class[2], Downhill, (1, 1), 1.0, 0.8);
        assert_eq!(
            (sprint.races_with_track, sprint.races_without_track),
            (1, 0)
        );
        assert_eq!(
            (sprint.unclassified_legs, sprint.legs_without_track),
            (1, 0)
        );

        // Del 1-abr al 1-jun, incluidos: B, C y D (que no cuenta).
        let spring = run(
            &HistoryFilter {
                from: Some("2026-04-01".parse().unwrap()),
                to: Some("2026-06-01".parse().unwrap()),
                format: None,
            },
            &config,
        );
        check(&spring.by_class[0], Uphill, (1, 1), 1.0, 0.6);
        check(&spring.by_class[1], Flat, (1, 0), 0.0, 0.8);
        check(&spring.by_class[2], Downhill, (1, 0), 0.0, 1.2);
        assert_eq!(
            (spring.races_with_track, spring.races_without_track),
            (1, 1)
        );
        assert_eq!(
            (spring.unclassified_legs, spring.legs_without_track),
            (2, 2)
        );

        // Sin carreras, tres clases vacías y en orden.
        let none = slope(&[], &HistoryFilter::default(), &config).unwrap();
        let classes: Vec<_> = none.by_class.iter().map(|s| s.class).collect();
        assert_eq!(classes, CLASSES);
        none.by_class.iter().for_each(empty);
        assert_eq!((none.races_with_track, none.races_without_track), (0, 0));
    }

    #[test]
    fn json_uses_snake_case_class_names() {
        let json = serde_json::to_value(CLASSES).unwrap();
        assert_eq!(json, serde_json::json!(["uphill", "flat", "downhill"]));
    }
}
