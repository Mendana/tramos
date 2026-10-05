//! ¿Lento o desorientado? (P2 de `docs/preguntas.md`). Versión inicial, heurística: reparte la
//! pérdida de cada tramo en **desvío** (metros de más respecto a lo habitual del corredor en la
//! carrera), **paradas** y **ritmo** (el resto), con las métricas del FIT
//! ([`crate::metrics`]). Las definiciones y todos los parámetros están en `docs/tiempo-perdido.md`,
//! "¿Lento o desorientado? (P2)".

use serde::{Deserialize, Serialize};

use crate::history::{HistoryFilter, TrackedRace, pattern_legs};
use crate::metrics::LegMetrics;
use crate::runner_report::{LegReport, RunnerLostTime};

/// Tramos sin error con relación distancia / línea recta que hacen falta para fiarse de la
/// relación habitual `r0` (su mediana). Con menos, no se reparte la pérdida de la carrera.
///
/// Es el único parámetro propio de P2. Los demás son los de las métricas
/// ([`crate::metrics::MetricsOptions`]): velocidad de parado (0,5 m/s) y segundos tras la picada
/// que no cuentan como parado (5 s).
pub const MIN_CLEAN_LEGS: usize = 3;

/// Reparto de la pérdida de un tramo. `loss_s = detour_s + stopped_s + pace_s`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LegBreakdown {
    /// Número del tramo (desde 1).
    pub index: usize,
    /// Pérdida `p_i` (s).
    pub loss_s: f64,
    pub is_error: bool,
    /// Desvío: `(d_run − r0 · d_line) / v_mov` (s). Negativo si el tramo fue más directo de lo
    /// habitual.
    pub detour_s: f64,
    /// Paradas: tiempo parado del tramo (s), sin los primeros segundos tras la picada.
    pub stopped_s: f64,
    /// Ritmo: el resto, `p_i − desvío − paradas` (s).
    pub pace_s: f64,
}

/// Sumas del reparto de un conjunto de tramos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct BreakdownTotals {
    /// Tramos sumados.
    pub legs: usize,
    pub loss_s: f64,
    pub detour_s: f64,
    pub stopped_s: f64,
    pub pace_s: f64,
}

impl BreakdownTotals {
    fn add(&mut self, leg: &LegBreakdown) {
        self.legs += 1;
        self.loss_s += leg.loss_s;
        self.detour_s += leg.detour_s;
        self.stopped_s += leg.stopped_s;
        self.pace_s += leg.pace_s;
    }

    fn merge(&mut self, other: &BreakdownTotals) {
        self.legs += other.legs;
        self.loss_s += other.loss_s;
        self.detour_s += other.detour_s;
        self.stopped_s += other.stopped_s;
        self.pace_s += other.pace_s;
    }
}

/// P2 de una carrera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceBreakdown {
    /// `r0`: mediana de `d_run / d_line` en los tramos sin error; `None` con menos de
    /// [`MIN_CLEAN_LEGS`].
    pub usual_ratio: Option<f64>,
    /// Tramos sin error con relación de los que sale `r0`.
    pub clean_legs: usize,
    /// Los tramos que cuentan ([`pattern_legs`]) que se pueden repartir, en orden. Vacío sin
    /// `r0`.
    pub legs: Vec<LegBreakdown>,
    /// Suma de los tramos con error de `legs`.
    pub errors: BreakdownTotals,
    /// Tramos con error que cuentan pero no se pueden repartir (sin sub-track, sin movimiento o
    /// sin `r0`).
    pub errors_without_breakdown: usize,
}

/// Métricas del FIT del tramo `leg`, si casan su número y sus balizas.
fn metrics_of<'a>(leg: &LegReport, metrics: &'a [LegMetrics]) -> Option<&'a LegMetrics> {
    metrics
        .iter()
        .find(|m| m.index == leg.index && m.from == leg.from && m.to == leg.to)
}

/// Mediana simple (con un número par de valores, la media de los dos centrales).
pub(crate) fn median(values: &mut [f64]) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let mid = n / 2;
    match n {
        0 => None,
        _ if n % 2 == 1 => values.get(mid).copied(),
        _ => Some((values.get(mid - 1)? + values.get(mid)?) / 2.0),
    }
}

/// Reparte la pérdida de los tramos que cuentan de una carrera con las métricas de su track.
pub fn race_breakdown(lost: &RunnerLostTime, metrics: &[LegMetrics]) -> RaceBreakdown {
    let mut ratios: Vec<f64> = pattern_legs(lost)
        .filter(|leg| !leg.is_error)
        .filter_map(|leg| metrics_of(leg, metrics)?.track.as_ref()?.distance_ratio)
        .filter(|r| r.is_finite())
        .collect();
    let clean_legs = ratios.len();
    let usual_ratio = if clean_legs >= MIN_CLEAN_LEGS {
        median(&mut ratios)
    } else {
        None
    };

    let mut legs = Vec::new();
    let mut errors = BreakdownTotals::default();
    let mut errors_without_breakdown = 0;
    for leg in pattern_legs(lost) {
        let breakdown = usual_ratio.and_then(|r0| {
            let loss_s = leg.loss_s?;
            let track = metrics_of(leg, metrics)?.track.as_ref()?;
            let v = track.moving_speed_mps.filter(|v| *v > 0.0)?;
            let detour_s = (track.distance_m - r0 * track.straight_m) / v;
            let stopped_s = track.stopped_s;
            Some(LegBreakdown {
                index: leg.index,
                loss_s,
                is_error: leg.is_error,
                detour_s,
                stopped_s,
                pace_s: loss_s - detour_s - stopped_s,
            })
        });
        match breakdown {
            Some(b) => {
                if b.is_error {
                    errors.add(&b);
                }
                legs.push(b);
            }
            None if leg.is_error => errors_without_breakdown += 1,
            None => {}
        }
    }
    RaceBreakdown {
        usual_ratio,
        clean_legs,
        legs,
        errors,
        errors_without_breakdown,
    }
}

/// P2 en el histórico: el reparto de la pérdida de los errores de todas las carreras.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct BreakdownHistory {
    /// Suma de los errores repartidos de todas las carreras con track.
    pub errors: BreakdownTotals,
    /// Carreras con números y track.
    pub races_with_track: usize,
    /// Carreras con números sin track: no aportan.
    pub races_without_track: usize,
    /// Errores que cuentan sin reparto: de carreras sin track o sin `r0`, o sin sub-track.
    pub errors_without_breakdown: usize,
}

/// Suma el reparto de los errores de las carreras que pasan `filter` y tienen rendimiento
/// habitual (las mismas que [`crate::history::history`]).
pub fn breakdown_history(races: &[TrackedRace<'_>], filter: &HistoryFilter) -> BreakdownHistory {
    let mut out = BreakdownHistory::default();
    let counted = races.iter().filter(|r| {
        filter.includes(r.race.date, r.race.format) && r.race.lost_time.usual_performance.is_some()
    });
    for r in counted {
        let lost = &r.race.lost_time;
        let Some(metrics) = r.leg_metrics else {
            out.races_without_track += 1;
            out.errors_without_breakdown += pattern_legs(lost).filter(|l| l.is_error).count();
            continue;
        };
        out.races_with_track += 1;
        let race = race_breakdown(lost, metrics);
        out.errors.merge(&race.errors);
        out.errors_without_breakdown += race.errors_without_breakdown;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::HistoryRace;
    use crate::metrics::TrackMetrics;

    const EPS: f64 = 1e-9;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < EPS, "{a} != {b}");
    }

    /// Un tramo de prueba: pérdida, error y métricas (recorrido, línea recta, velocidad en
    /// movimiento, parado). `None` en las métricas = sin sub-track.
    struct T {
        loss: f64,
        error: bool,
        track: Option<(f64, f64, f64, f64)>,
    }

    const fn t(loss: f64, error: bool, run: f64, line: f64, v: f64, stopped: f64) -> T {
        T {
            loss,
            error,
            track: Some((run, line, v, stopped)),
        }
    }

    /// Carrera con los tramos `legs` más un último tramo (que no cuenta, con un rodeo enorme
    /// que no debe influir en nada).
    fn race(legs: &[T]) -> (RunnerLostTime, Vec<LegMetrics>) {
        let mut all: Vec<&T> = legs.iter().collect();
        let last = t(50.0, true, 900.0, 100.0, 3.0, 20.0);
        all.push(&last);
        let n = all.len();
        let reports = all
            .iter()
            .enumerate()
            .map(|(i, l)| LegReport {
                index: i + 1,
                from: 31,
                to: 32,
                split_s: Some(100.0),
                elapsed_s: None,
                place: None,
                reference_s: Some(90.0),
                reference_count: 1,
                valid_splits: 4,
                performance_index: Some(0.9),
                expected_s: Some(100.0 - l.loss),
                loss_s: Some(l.loss),
                loss_pct: Some(10.0),
                is_error: l.error,
                gain_s: None,
                cumulative_gain_s: None,
                ideal_elapsed_s: None,
                behind_ideal_s: None,
                is_last: i + 1 == n,
                short_reference: false,
                excluded_from_patterns: i + 1 == n,
            })
            .collect();
        let metrics = all
            .iter()
            .enumerate()
            .map(|(i, l)| LegMetrics {
                index: i + 1,
                from: 31,
                to: 32,
                track: l.track.map(|(run, line, v, stopped)| TrackMetrics {
                    duration_s: 100.0,
                    distance_m: run,
                    straight_m: line,
                    distance_ratio: (line >= 1.0).then(|| run / line),
                    moving_s: 100.0 - stopped,
                    stopped_s: stopped,
                    gap_s: 0.0,
                    moving_speed_mps: (v > 0.0).then_some(v),
                    ascent_m: None,
                    descent_m: None,
                    heart_rate_bpm: None,
                    cadence_spm: None,
                }),
                missing: None,
            })
            .collect();
        let lost = RunnerLostTime {
            total_s: None,
            usual_performance: Some(0.9),
            lost_time_s: None,
            error_count: 0,
            time_without_errors_s: None,
            ideal_time_s: None,
            behind_ideal_s: None,
            losing_streaks: Vec::new(),
            consistency: None,
            legs: reports,
        };
        (lost, metrics)
    }

    /// Tres tramos limpios con relación 1,2, 1,1 y 1,4 → `r0` = 1,2. Un error con rodeo: 600 m
    /// recorridos frente a 200 m en línea recta, a 3 m/s, 20 s parado y 150 s perdidos.
    fn example() -> (RunnerLostTime, Vec<LegMetrics>) {
        race(&[
            t(-2.0, false, 240.0, 200.0, 4.0, 0.0),
            t(3.0, false, 330.0, 300.0, 4.0, 0.0),
            t(150.0, true, 600.0, 200.0, 3.0, 20.0),
            t(5.0, false, 140.0, 100.0, 4.0, 1.0),
        ])
    }

    #[test]
    fn detour_and_stops_explain_a_wrong_route() {
        let (lost, metrics) = example();
        let b = race_breakdown(&lost, &metrics);
        assert_eq!(b.clean_legs, 3);
        close(b.usual_ratio.unwrap(), 1.2);
        assert_eq!(b.legs.len(), 4);
        // Error: desvío (600 − 1,2 · 200) / 3 = 120 s; paradas 20 s; ritmo 150 − 140 = 10 s.
        let e = b.legs[2];
        assert_eq!((e.index, e.is_error), (3, true));
        close(e.detour_s, 120.0);
        close(e.stopped_s, 20.0);
        close(e.pace_s, 10.0);
        // Tramo limpio: (140 − 120) / 4 = 5 s de desvío, 1 s parado, ritmo 5 − 6 = −1 s.
        let c = b.legs[3];
        close(c.detour_s, 5.0);
        close(c.pace_s, -1.0);
        // Los totales son solo los errores que cuentan (el último, aun siendo error, no).
        assert_eq!(b.errors.legs, 1);
        close(b.errors.loss_s, 150.0);
        close(b.errors.detour_s, 120.0);
        close(b.errors.stopped_s, 20.0);
        close(b.errors.pace_s, 10.0);
        assert_eq!(b.errors_without_breakdown, 0);
    }

    #[test]
    fn a_slow_leg_on_the_usual_line_is_pace() {
        // Error de 40 s por la línea habitual (240 / 200 = r0), sin parar: todo ritmo.
        let (lost, metrics) = race(&[
            t(0.0, false, 240.0, 200.0, 4.0, 0.0),
            t(0.0, false, 240.0, 200.0, 4.0, 0.0),
            t(0.0, false, 240.0, 200.0, 4.0, 0.0),
            t(40.0, true, 240.0, 200.0, 2.5, 0.0),
        ]);
        let e = race_breakdown(&lost, &metrics).legs[3];
        close(e.detour_s, 0.0);
        close(e.stopped_s, 0.0);
        close(e.pace_s, 40.0);
    }

    #[test]
    fn median_of_an_even_number_of_clean_legs() {
        let (lost, metrics) = race(&[
            t(0.0, false, 110.0, 100.0, 4.0, 0.0),
            t(0.0, false, 130.0, 100.0, 4.0, 0.0),
            t(0.0, false, 150.0, 100.0, 4.0, 0.0),
            t(0.0, false, 170.0, 100.0, 4.0, 0.0),
        ]);
        close(race_breakdown(&lost, &metrics).usual_ratio.unwrap(), 1.4);
    }

    #[test]
    fn too_few_clean_legs_or_missing_tracks_leave_errors_without_breakdown() {
        // Dos limpios con relación: no hay r0 y nada se reparte.
        let (lost, metrics) = race(&[
            t(0.0, false, 120.0, 100.0, 4.0, 0.0),
            t(0.0, false, 120.0, 100.0, 4.0, 0.0),
            t(60.0, true, 300.0, 100.0, 3.0, 0.0),
        ]);
        let b = race_breakdown(&lost, &metrics);
        assert_eq!((b.clean_legs, b.usual_ratio), (2, None));
        assert!(b.legs.is_empty());
        assert_eq!(b.errors_without_breakdown, 1);

        // Con r0, un error sin sub-track o sin movimiento no se reparte.
        let (lost, metrics) = race(&[
            t(0.0, false, 120.0, 100.0, 4.0, 0.0),
            t(0.0, false, 120.0, 100.0, 4.0, 0.0),
            t(0.0, false, 120.0, 100.0, 4.0, 0.0),
            T {
                loss: 30.0,
                error: true,
                track: None,
            },
            t(30.0, true, 0.0, 100.0, 0.0, 100.0),
        ]);
        let b = race_breakdown(&lost, &metrics);
        assert_eq!(b.legs.len(), 3);
        assert_eq!(b.errors_without_breakdown, 2);
        assert_eq!(b.errors, BreakdownTotals::default());
    }

    #[test]
    fn history_adds_the_errors_of_the_races_with_track() {
        let (lost, metrics) = example();
        let with = HistoryRace {
            date: "2026-05-01".parse().unwrap(),
            format: None,
            lost_time: lost.clone(),
        };
        let without = HistoryRace {
            date: "2026-06-01".parse().unwrap(),
            format: None,
            lost_time: lost,
        };
        let races = [
            TrackedRace {
                race: &with,
                leg_metrics: Some(&metrics),
            },
            TrackedRace {
                race: &with,
                leg_metrics: Some(&metrics),
            },
            TrackedRace {
                race: &without,
                leg_metrics: None,
            },
        ];
        let h = breakdown_history(&races, &HistoryFilter::default());
        assert_eq!((h.races_with_track, h.races_without_track), (2, 1));
        assert_eq!(h.errors.legs, 2);
        close(h.errors.loss_s, 300.0);
        close(h.errors.detour_s, 240.0);
        close(h.errors.stopped_s, 40.0);
        close(h.errors.pace_s, 20.0);
        // El error de la carrera sin track cuenta como sin reparto.
        assert_eq!(h.errors_without_breakdown, 1);

        let only_june = HistoryFilter {
            from: Some("2026-06-01".parse().unwrap()),
            ..HistoryFilter::default()
        };
        let h = breakdown_history(&races, &only_june);
        assert_eq!((h.races_with_track, h.errors.legs), (0, 0));
    }
}
