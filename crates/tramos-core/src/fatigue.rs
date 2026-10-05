//! ¿El cansancio anticipa el error? (P14 de `docs/preguntas.md`): deriva del pulso frente a la
//! velocidad a lo largo de la carrera, pulso en el tramo previo a un error frente al previo a un
//! tramo limpio en la misma fase, y esfuerzo percibido de las etiquetas, por tercio de carrera.
//! Las definiciones están en `docs/historico.md`, "¿El cansancio anticipa el error? (P14)".
//!
//! Es un dato débil: el pulso de muñeca tiene retraso y saltos, y la app lo dice al enseñarlo.

use serde::{Deserialize, Serialize};

use crate::common_errors::{LegStatus, leg_status};
use crate::history::{HistoryFilter, HistoryRace, pattern_legs, race_third};
use crate::loss_breakdown::median;
use crate::metrics::{LegMetrics, TrackMetrics};
use crate::runner_report::LegReport;
use crate::taxonomy::LegTag;

/// Tercios de la carrera.
pub const THIRDS: usize = 3;

/// Tramos con dato que hacen falta en una carrera para fiarse de su mediana (de pulso o de
/// pulso/velocidad), como en P8.
pub const MIN_LEGS: usize = 3;

/// Una carrera del histórico con las métricas del FIT y las etiquetas de sus tramos.
#[derive(Debug, Clone, Copy)]
pub struct FatigueRace<'a> {
    pub race: &'a HistoryRace,
    /// `None` si la carrera no tiene track.
    pub leg_metrics: Option<&'a [LegMetrics]>,
    /// `(tramo, etiqueta)`, con el tramo desde 1 como `LegReport::index`.
    pub tags: &'a [(usize, LegTag)],
}

/// Deriva de un tercio: pulso frente a velocidad en los tramos limpios.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Drift {
    /// Tramos limpios con pulso y velocidad de carreras con mediana (n).
    pub legs: usize,
    /// Media de su pulso medio (ppm).
    pub mean_heart_rate_bpm: Option<f64>,
    /// Media de su velocidad en movimiento (m/s).
    pub mean_speed_mps: Option<f64>,
    /// Media de `(pulso / velocidad) / r_c`, con `r_c` la mediana de pulso / velocidad de los
    /// tramos limpios de su carrera: 1 = lo habitual en esa carrera; más = más pulso para la
    /// misma velocidad.
    pub mean_relative_ratio: Option<f64>,
}

/// Pulso del tramo anterior de un grupo de tramos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct HeartRateBefore {
    /// Tramos del grupo cuyo anterior tiene pulso (n).
    pub legs: usize,
    /// Media del pulso medio del tramo anterior (ppm).
    pub mean_heart_rate_bpm: Option<f64>,
    /// Media de `pulso del anterior − p_c`, con `p_c` la mediana del pulso de los tramos que
    /// cuentan de su carrera (ppm; positivo = más alto que lo habitual en esa carrera).
    pub mean_relative_bpm: Option<f64>,
}

/// Esfuerzo percibido (1–10) de un grupo de tramos etiquetados.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Effort {
    /// Tramos con esfuerzo apuntado (n).
    pub legs: usize,
    pub mean_effort: Option<f64>,
}

/// P14 en un tercio de carrera.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ThirdFatigue {
    /// 1, 2 o 3.
    pub third: usize,
    pub drift: Drift,
    /// Tramos del tercio que son error, con el pulso del tramo anterior.
    pub before_error: HeartRateBefore,
    /// Tramos del tercio limpios, con el pulso del tramo anterior.
    pub before_clean: HeartRateBefore,
    /// Esfuerzo apuntado en los errores, en los tramos limpios y en los físicos del tercio.
    pub effort_error: Effort,
    pub effort_clean: Effort,
    pub effort_physical: Effort,
}

/// P14 en el histórico.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fatigue {
    /// Los tres tercios, en orden.
    pub by_third: Vec<ThirdFatigue>,
    /// Carreras que cuentan con pulso en al menos [`MIN_LEGS`] tramos que cuentan: las que
    /// aportan al pulso.
    pub races_with_heart_rate: usize,
    /// Carreras que cuentan sin eso (sin FIT, sin pulso o con pocos tramos con pulso).
    pub races_without_heart_rate: usize,
    /// Errores (salvo el primer tramo de cada carrera) sin pulso del tramo anterior para comparar.
    pub errors_without_heart_rate: usize,
}

/// Sumas de una media.
#[derive(Debug, Default, Clone, Copy)]
struct Mean {
    n: usize,
    sum: f64,
}

impl Mean {
    fn add(&mut self, value: f64) {
        self.n += 1;
        self.sum += value;
    }

    fn get(&self) -> Option<f64> {
        (self.n > 0).then(|| self.sum / self.n as f64)
    }
}

/// Sumas de un tercio.
#[derive(Debug, Default, Clone, Copy)]
struct Accumulator {
    drift_hr: Mean,
    drift_speed: Mean,
    drift_ratio: Mean,
    before_error: (Mean, Mean),
    before_clean: (Mean, Mean),
    effort_error: Mean,
    effort_clean: Mean,
    effort_physical: Mean,
}

fn heart_rate_before(sums: &(Mean, Mean)) -> HeartRateBefore {
    HeartRateBefore {
        legs: sums.0.n,
        mean_heart_rate_bpm: sums.0.get(),
        mean_relative_bpm: sums.1.get(),
    }
}

fn effort(sums: &Mean) -> Effort {
    Effort {
        legs: sums.n,
        mean_effort: sums.get(),
    }
}

impl Accumulator {
    fn finish(&self, third: usize) -> ThirdFatigue {
        ThirdFatigue {
            third,
            drift: Drift {
                legs: self.drift_ratio.n,
                mean_heart_rate_bpm: self.drift_hr.get(),
                mean_speed_mps: self.drift_speed.get(),
                mean_relative_ratio: self.drift_ratio.get(),
            },
            before_error: heart_rate_before(&self.before_error),
            before_clean: heart_rate_before(&self.before_clean),
            effort_error: effort(&self.effort_error),
            effort_clean: effort(&self.effort_clean),
            effort_physical: effort(&self.effort_physical),
        }
    }
}

/// Métricas del FIT del tramo `leg`, si casan su número y sus balizas y tiene sub-track.
fn track_of<'a>(leg: &LegReport, metrics: Option<&'a [LegMetrics]>) -> Option<&'a TrackMetrics> {
    metrics?
        .iter()
        .find(|m| m.index == leg.index && m.from == leg.from && m.to == leg.to)?
        .track
        .as_ref()
}

/// Pulso medio del tramo (ppm), si es un número positivo.
fn heart_rate(leg: &LegReport, metrics: Option<&[LegMetrics]>) -> Option<f64> {
    track_of(leg, metrics)?
        .heart_rate_bpm
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// Pulso medio y velocidad en movimiento del tramo, si los dos son números positivos.
fn heart_rate_and_speed(leg: &LegReport, metrics: Option<&[LegMetrics]>) -> Option<(f64, f64)> {
    let speed = track_of(leg, metrics)?
        .moving_speed_mps
        .filter(|v| v.is_finite() && *v > 0.0)?;
    Some((heart_rate(leg, metrics)?, speed))
}

/// Calcula P14 con las carreras que pasan `filter` y tienen rendimiento habitual (las mismas que
/// [`crate::history::history`]), sobre sus tramos que cuentan ([`pattern_legs`]).
///
/// Qué fue cada tramo lo dice [`leg_status`]: manda la etiqueta y lo «físico» no es error ni
/// limpio. El tercio es el del tramo ([`race_third`]). El «tramo anterior» es el anterior de la
/// secuencia de tramos que cuentan, como en P8.
pub fn fatigue(races: &[FatigueRace<'_>], filter: &HistoryFilter) -> Fatigue {
    let mut thirds = [Accumulator::default(); THIRDS];
    let mut out = Fatigue {
        by_third: Vec::new(),
        races_with_heart_rate: 0,
        races_without_heart_rate: 0,
        errors_without_heart_rate: 0,
    };
    let counted = races.iter().filter(|r| {
        filter.includes(r.race.date, r.race.format) && r.race.lost_time.usual_performance.is_some()
    });
    for r in counted {
        let lost = &r.race.lost_time;
        let course_legs = lost.legs.len();
        let legs: Vec<(&LegReport, LegStatus)> = pattern_legs(lost)
            .map(|leg| {
                let tag = r
                    .tags
                    .iter()
                    .find(|(index, _)| *index == leg.index)
                    .map(|(_, tag)| tag);
                (leg, leg_status(leg, tag))
            })
            .collect();
        let third_of = |leg: &LegReport| race_third(leg.index, course_legs);

        // Esfuerzo percibido: no necesita FIT.
        for &(leg, status) in &legs {
            let Some(value) = r
                .tags
                .iter()
                .find(|(index, _)| *index == leg.index)
                .and_then(|(_, tag)| tag.effort)
            else {
                continue;
            };
            if let Some(acc) = thirds.get_mut(third_of(leg)) {
                let sums = match status {
                    LegStatus::Error { .. } => &mut acc.effort_error,
                    LegStatus::Clean => &mut acc.effort_clean,
                    LegStatus::Physical => &mut acc.effort_physical,
                };
                sums.add(f64::from(value));
            }
        }

        // Mediana del pulso de la carrera (`p_c`): sin ella, la carrera no aporta pulso.
        let mut rates: Vec<f64> = legs
            .iter()
            .filter_map(|(leg, _)| heart_rate(leg, r.leg_metrics))
            .collect();
        let usual_rate = if rates.len() >= MIN_LEGS {
            median(&mut rates)
        } else {
            None
        };
        let Some(usual_rate) = usual_rate else {
            out.races_without_heart_rate += 1;
            out.errors_without_heart_rate += legs
                .iter()
                .skip(1)
                .filter(|(_, s)| matches!(s, LegStatus::Error { .. }))
                .count();
            continue;
        };
        out.races_with_heart_rate += 1;

        // Deriva: pulso / velocidad de los tramos limpios frente a su mediana (`r_c`).
        let clean: Vec<(&LegReport, f64, f64)> = legs
            .iter()
            .filter(|(_, s)| *s == LegStatus::Clean)
            .filter_map(|&(leg, _)| {
                let (hr, speed) = heart_rate_and_speed(leg, r.leg_metrics)?;
                Some((leg, hr, speed))
            })
            .collect();
        let mut ratios: Vec<f64> = clean.iter().map(|(_, hr, v)| hr / v).collect();
        let usual_ratio = if ratios.len() >= MIN_LEGS {
            median(&mut ratios).filter(|r| *r > 0.0)
        } else {
            None
        };
        if let Some(usual_ratio) = usual_ratio {
            for &(leg, hr, speed) in &clean {
                if let Some(acc) = thirds.get_mut(third_of(leg)) {
                    acc.drift_hr.add(hr);
                    acc.drift_speed.add(speed);
                    acc.drift_ratio.add(hr / speed / usual_ratio);
                }
            }
        }

        // Antes del error: pulso del tramo anterior, en el tercio del tramo que sigue.
        for pair in legs.windows(2) {
            let &[(prev, _), (next, status)] = pair else {
                continue;
            };
            let Some(hr) = heart_rate(prev, r.leg_metrics) else {
                if matches!(status, LegStatus::Error { .. }) {
                    out.errors_without_heart_rate += 1;
                }
                continue;
            };
            let Some(acc) = thirds.get_mut(third_of(next)) else {
                continue;
            };
            let sums = match status {
                LegStatus::Error { .. } => &mut acc.before_error,
                LegStatus::Clean => &mut acc.before_clean,
                LegStatus::Physical => continue,
            };
            sums.0.add(hr);
            sums.1.add(hr - usual_rate);
        }
    }
    out.by_third = thirds
        .iter()
        .enumerate()
        .map(|(i, acc)| acc.finish(i + 1))
        .collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race_format::RaceFormat;
    use crate::runner_report::RunnerLostTime;
    use crate::taxonomy::Confirmation;

    const EPS: f64 = 1e-9;

    fn close(actual: Option<f64>, expected: f64) {
        let Some(a) = actual else {
            panic!("se esperaba {expected} y no hay valor");
        };
        assert!((a - expected).abs() < EPS, "{a} != {expected}");
    }

    /// Tramo: `o` limpio, `x` error (según el cálculo), `s` referencia corta (no cuenta); con
    /// pulso y velocidad del FIT (`None` = sin sub-track).
    #[derive(Clone, Copy)]
    struct L(char, Option<(f64, f64)>);

    /// Una carrera con los tramos `legs` más un último que no cuenta. `with_fit`: si tiene
    /// métricas del FIT.
    fn race(
        date: &str,
        format: Option<RaceFormat>,
        usual: Option<f64>,
        legs: &[L],
        with_fit: bool,
    ) -> (HistoryRace, Option<Vec<LegMetrics>>) {
        let mut all: Vec<L> = legs.to_vec();
        all.push(L('o', Some((150.0, 3.0))));
        let n = all.len();
        let reports = all
            .iter()
            .enumerate()
            .map(|(i, &L(k, _))| LegReport {
                index: i + 1,
                from: 31,
                to: 32,
                split_s: Some(60.0),
                elapsed_s: None,
                place: None,
                reference_s: Some(if k == 's' { 15.0 } else { 60.0 }),
                reference_count: 1,
                valid_splits: 4,
                performance_index: Some(1.0),
                expected_s: Some(60.0),
                loss_s: Some(if k == 'x' { 30.0 } else { 0.0 }),
                loss_pct: Some(0.0),
                is_error: k == 'x',
                gain_s: None,
                cumulative_gain_s: None,
                ideal_elapsed_s: None,
                behind_ideal_s: None,
                is_last: i + 1 == n,
                short_reference: k == 's',
                excluded_from_patterns: k == 's' || i + 1 == n,
            })
            .collect();
        let metrics = with_fit.then(|| {
            all.iter()
                .enumerate()
                .map(|(i, &L(_, data))| LegMetrics {
                    index: i + 1,
                    from: 31,
                    to: 32,
                    track: data.map(|(hr, v)| TrackMetrics {
                        duration_s: 60.0,
                        distance_m: v * 60.0,
                        straight_m: v * 50.0,
                        distance_ratio: Some(1.2),
                        moving_s: 60.0,
                        stopped_s: 0.0,
                        gap_s: 0.0,
                        moving_speed_mps: Some(v),
                        ascent_m: None,
                        descent_m: None,
                        heart_rate_bpm: Some(hr),
                        cadence_spm: None,
                    }),
                    missing: None,
                })
                .collect()
        });
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

    const fn o(hr: f64, v: f64) -> L {
        L('o', Some((hr, v)))
    }

    const fn x(hr: f64, v: f64) -> L {
        L('x', Some((hr, v)))
    }

    fn tag(confirmation: Option<Confirmation>, effort: u8) -> LegTag {
        LegTag {
            confirmation,
            effort: Some(effort),
            ..LegTag::default()
        }
    }

    type Data = Vec<(HistoryRace, Option<Vec<LegMetrics>>, Vec<(usize, LegTag)>)>;

    /// Carreras sintéticas:
    ///
    /// - A, sprint, con FIT: 9 tramos que cuentan + el último (10: tercios 1–4, 5–7 y 8–10).
    ///   Limpios 1, 2, 4, 5, 7 y 8 con pulso / velocidad 45, 50, 50, 50, 55 y 60 (mediana
    ///   `r_c` = 50); errores 3, 6 y 9. Pulsos 135, 150, 158, 155, 160, 170, 165, 168 y 172
    ///   (mediana `p_c` = 160). Esfuerzo: 2 (No, 5), 3 (8), 6 (9), 9 (Sí, 7) y el último (10, no
    ///   cuenta).
    /// - B, media, sin FIT: 1 o, 2 x (Físico, 9), 3 o, 4 x (6), 5 x (No, 4), 6 corto (10, no
    ///   cuenta), 7 o + el último (8: tercios 1–3, 4–6 y 7–8).
    /// - C, sin formato, con FIT: 1 o 150/3,0; 2 x (Físico) 160/2,0; 3 x (No) 155/3,1; 4 o sin
    ///   sub-track; 5 x 170/2,5 + el último (6: tercios 1–2, 3–4 y 5–6). Pulsos 150, 160, 155 y
    ///   170: `p_c` = 157,5. Solo dos limpios con velocidad: sin deriva.
    /// - D, larga, con FIT pero sin rendimiento habitual: no cuenta.
    fn data() -> Data {
        use Confirmation::*;
        let a = race(
            "2026-03-01",
            Some(RaceFormat::Sprint),
            Some(0.9),
            &[
                o(135.0, 3.0),
                o(150.0, 3.0),
                x(158.0, 2.6),
                o(155.0, 3.1),
                o(160.0, 3.2),
                x(170.0, 2.4),
                o(165.0, 3.0),
                o(168.0, 2.8),
                x(172.0, 2.0),
            ],
            true,
        );
        let b = race(
            "2026-04-01",
            Some(RaceFormat::Middle),
            Some(0.85),
            &[
                L('o', None),
                L('x', None),
                L('o', None),
                L('x', None),
                L('x', None),
                L('s', None),
                L('o', None),
            ],
            false,
        );
        let c = race(
            "2026-05-01",
            None,
            Some(0.8),
            &[
                o(150.0, 3.0),
                x(160.0, 2.0),
                x(155.0, 3.1),
                L('o', None),
                x(170.0, 2.5),
            ],
            true,
        );
        let d = race(
            "2026-06-01",
            Some(RaceFormat::Long),
            None,
            &[x(190.0, 2.0), o(120.0, 3.0), x(190.0, 2.0), x(190.0, 2.0)],
            true,
        );
        vec![
            (
                a.0,
                a.1,
                vec![
                    (2, tag(Some(NoError), 5)),
                    (3, tag(None, 8)),
                    (6, tag(None, 9)),
                    (9, tag(Some(Error), 7)),
                    (10, tag(Some(Error), 10)),
                ],
            ),
            (
                b.0,
                b.1,
                vec![
                    (2, tag(Some(Physical), 9)),
                    (4, tag(None, 6)),
                    (5, tag(Some(NoError), 4)),
                    (6, tag(Some(Error), 10)),
                ],
            ),
            (
                c.0,
                c.1,
                vec![
                    (
                        2,
                        LegTag {
                            confirmation: Some(Physical),
                            ..LegTag::default()
                        },
                    ),
                    (
                        3,
                        LegTag {
                            confirmation: Some(NoError),
                            ..LegTag::default()
                        },
                    ),
                ],
            ),
            (d.0, d.1, vec![(1, tag(Some(Error), 10))]),
        ]
    }

    fn run(data: &Data, filter: &HistoryFilter) -> Fatigue {
        let races: Vec<FatigueRace<'_>> = data
            .iter()
            .map(|(race, metrics, tags)| FatigueRace {
                race,
                leg_metrics: metrics.as_deref(),
                tags,
            })
            .collect();
        fatigue(&races, filter)
    }

    #[test]
    fn counts_races_with_heart_rate() {
        let f = run(&data(), &HistoryFilter::default());
        // A y C con pulso; B sin FIT; D no cuenta.
        assert_eq!(
            (f.races_with_heart_rate, f.races_without_heart_rate),
            (2, 1)
        );
        // B: el error 4 (el 2 es físico); C: el error 5 sigue a un tramo sin sub-track.
        assert_eq!(f.errors_without_heart_rate, 2);
        let thirds: Vec<usize> = f.by_third.iter().map(|t| t.third).collect();
        assert_eq!(thirds, [1, 2, 3]);
    }

    #[test]
    fn drift_compares_heart_rate_per_speed_with_the_race_median() {
        let f = run(&data(), &HistoryFilter::default());
        let [t1, t2, t3] = [f.by_third[0], f.by_third[1], f.by_third[2]];
        // 1.er tercio de A: limpios 1, 2 y 4 → 45/50, 50/50 y 50/50.
        assert_eq!(t1.drift.legs, 3);
        close(t1.drift.mean_relative_ratio, (0.9 + 1.0 + 1.0) / 3.0);
        close(t1.drift.mean_heart_rate_bpm, (135.0 + 150.0 + 155.0) / 3.0);
        close(t1.drift.mean_speed_mps, (3.0 + 3.0 + 3.1) / 3.0);
        // 2.º: 5 y 7 → 1,0 y 1,1. C no tiene mediana (dos limpios con velocidad).
        assert_eq!(t2.drift.legs, 2);
        close(t2.drift.mean_relative_ratio, 1.05);
        // 3.º: 8 → 60/50.
        assert_eq!(t3.drift.legs, 1);
        close(t3.drift.mean_relative_ratio, 1.2);
        close(t3.drift.mean_heart_rate_bpm, 168.0);
    }

    #[test]
    fn heart_rate_before_an_error_against_before_a_clean_leg_in_the_same_third() {
        let f = run(&data(), &HistoryFilter::default());
        let [t1, t2, t3] = [f.by_third[0], f.by_third[1], f.by_third[2]];
        // 1.er tercio. Antes de error: A 2→3 (150, −10). Antes de limpio: A 1→2 (135, −25) y
        // 3→4 (158, −2).
        assert_eq!(t1.before_error.legs, 1);
        close(t1.before_error.mean_heart_rate_bpm, 150.0);
        close(t1.before_error.mean_relative_bpm, -10.0);
        assert_eq!(t1.before_clean.legs, 2);
        close(t1.before_clean.mean_heart_rate_bpm, 146.5);
        close(t1.before_clean.mean_relative_bpm, -13.5);
        // 2.º. Antes de error: A 5→6 (160, 0). Antes de limpio: A 4→5 (155, −5), 6→7 (170, +10)
        // y C 2→3 (160, +2,5) y 3→4 (155, −2,5). C 1→2 no entra: 2 es físico.
        assert_eq!(t2.before_error.legs, 1);
        close(t2.before_error.mean_relative_bpm, 0.0);
        assert_eq!(t2.before_clean.legs, 4);
        close(t2.before_clean.mean_heart_rate_bpm, 160.0);
        close(t2.before_clean.mean_relative_bpm, 1.25);
        // 3.º. Antes de error: A 8→9 (168, +8); C 4→5 no tiene pulso del anterior. Antes de
        // limpio: A 7→8 (165, +5).
        assert_eq!(t3.before_error.legs, 1);
        close(t3.before_error.mean_relative_bpm, 8.0);
        assert_eq!(t3.before_clean.legs, 1);
        close(t3.before_clean.mean_heart_rate_bpm, 165.0);
    }

    #[test]
    fn effort_comes_from_the_tags_of_the_counted_legs() {
        let f = run(&data(), &HistoryFilter::default());
        let [t1, t2, t3] = [f.by_third[0], f.by_third[1], f.by_third[2]];
        // 1.er tercio: error A3 (8), limpio A2 (5, «No»), físico B2 (9).
        assert_eq!(
            (
                t1.effort_error.legs,
                t1.effort_clean.legs,
                t1.effort_physical.legs
            ),
            (1, 1, 1)
        );
        close(t1.effort_error.mean_effort, 8.0);
        close(t1.effort_clean.mean_effort, 5.0);
        close(t1.effort_physical.mean_effort, 9.0);
        // 2.º: errores A6 (9) y B4 (6); limpio B5 (4, «No» a un error del cálculo). B6 es corto.
        assert_eq!(t2.effort_error.legs, 2);
        close(t2.effort_error.mean_effort, 7.5);
        close(t2.effort_clean.mean_effort, 4.0);
        assert_eq!(t2.effort_physical, Effort::default());
        // 3.º: error A9 (7). El último de A no cuenta.
        assert_eq!(t3.effort_error.legs, 1);
        close(t3.effort_error.mean_effort, 7.0);
        assert_eq!(t3.effort_clean.legs, 0);
    }

    #[test]
    fn filter_leaves_races_out() {
        let data = data();
        let f = run(
            &data,
            &HistoryFilter {
                format: Some(RaceFormat::Middle),
                ..HistoryFilter::default()
            },
        );
        // Solo B: sin pulso, pero con esfuerzo.
        assert_eq!(
            (f.races_with_heart_rate, f.races_without_heart_rate),
            (0, 1)
        );
        assert!(
            f.by_third
                .iter()
                .all(|t| t.drift.legs == 0 && t.before_error.legs == 0 && t.before_clean.legs == 0)
        );
        assert_eq!(f.by_third[0].effort_physical.legs, 1);
    }

    #[test]
    fn no_races_gives_three_empty_thirds() {
        let f = run(&Vec::new(), &HistoryFilter::default());
        assert_eq!(f.by_third.len(), 3);
        assert!(
            f.by_third
                .iter()
                .all(|t| t.drift.mean_relative_ratio.is_none()
                    && t.before_error.mean_heart_rate_bpm.is_none()
                    && t.effort_error.mean_effort.is_none())
        );
    }
}
