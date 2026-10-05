//! ¿Qué hago después de fallar? (P8 de `docs/preguntas.md`): encadenamiento de errores,
//! recuperación acelerada y rachas limpias, sobre los tramos que cuentan del histórico. Las
//! definiciones están en `docs/historico.md`, "Después de fallar (P8)".

use serde::{Deserialize, Serialize};

use crate::history::{HistoryFilter, TrackedRace, pattern_legs};
use crate::loss_breakdown::median;
use crate::metrics::LegMetrics;
use crate::runner_report::LegReport;

/// Cuánto más rápido (en movimiento) que su mediana en tramos limpios tiene que ir el corredor
/// en el tramo siguiente a un error para que cuente como «acelerar»: +5 %.
pub const ACCELERATION: f64 = 0.05;

/// Tramos limpios con velocidad en movimiento que hacen falta en una carrera para fiarse de su
/// mediana.
pub const MIN_CLEAN_LEGS: usize = 3;

/// Primer número de tramos limpios seguidos de cada cubo de rachas: 0, 1–2, 3–5, 6–10 y más
/// de 10.
pub const STREAK_STARTS: [usize; 5] = [0, 1, 3, 6, 11];

/// Tramos y errores de un grupo de tramos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rate {
    pub legs: usize,
    pub errors: usize,
    /// `errors / legs` (0–1); `None` sin tramos.
    pub error_rate: Option<f64>,
}

impl Rate {
    fn add(&mut self, leg: &LegReport) {
        self.legs += 1;
        if leg.is_error {
            self.errors += 1;
        }
        self.error_rate = Some(self.errors as f64 / self.legs as f64);
    }
}

/// Tasa de error de los tramos con cierta racha previa de tramos limpios.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StreakBucket {
    /// Tramos limpios seguidos justo antes: de `from` a `to`, incluidos (`to` = `None`: sin fin).
    pub from: usize,
    pub to: Option<usize>,
    pub rate: Rate,
}

/// P8 en el histórico.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AfterError {
    /// Tramos cuyo tramo anterior fue un error.
    pub after_error: Rate,
    /// Tramos cuyo tramo anterior fue limpio.
    pub after_clean: Rate,
    /// Tramos tras un error yendo más de un 5 % más rápido que la mediana de los tramos limpios
    /// de la carrera.
    pub accelerated: Rate,
    /// Tramos tras un error sin acelerar tanto.
    pub not_accelerated: Rate,
    /// Tramos tras un error sin velocidad para comparar (sin FIT, sin sub-track o carrera con
    /// pocos tramos limpios).
    pub after_error_without_speed: usize,
    /// Los cinco cubos de rachas limpias, en orden.
    pub streaks: Vec<StreakBucket>,
}

fn moving_speed(leg: &LegReport, metrics: &[LegMetrics]) -> Option<f64> {
    metrics
        .iter()
        .find(|m| m.index == leg.index && m.from == leg.from && m.to == leg.to)?
        .track
        .as_ref()?
        .moving_speed_mps
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// Cubo de rachas de `clean` tramos limpios seguidos.
fn streak_bucket(clean: usize) -> usize {
    STREAK_STARTS
        .iter()
        .rposition(|&start| clean >= start)
        .unwrap_or(0)
}

/// Calcula P8 con las carreras que pasan `filter` y tienen rendimiento habitual (las mismas que
/// [`crate::history::history`]).
///
/// En cada carrera, los tramos que cuentan ([`pattern_legs`]) se toman en orden y el «tramo
/// siguiente» es el siguiente de esa secuencia: los excluidos (último, referencia corta, sin
/// split) ni cuentan ni cortan. El primer tramo de cada carrera no tiene anterior y no entra.
pub fn after_error(races: &[TrackedRace<'_>], filter: &HistoryFilter) -> AfterError {
    let mut out = AfterError {
        after_error: Rate::default(),
        after_clean: Rate::default(),
        accelerated: Rate::default(),
        not_accelerated: Rate::default(),
        after_error_without_speed: 0,
        streaks: Vec::new(),
    };
    let mut streaks = [Rate::default(); STREAK_STARTS.len()];
    let counted = races.iter().filter(|r| {
        filter.includes(r.race.date, r.race.format) && r.race.lost_time.usual_performance.is_some()
    });
    for r in counted {
        let legs: Vec<&LegReport> = pattern_legs(&r.race.lost_time).collect();
        let usual_speed = r.leg_metrics.and_then(|metrics| {
            let mut speeds: Vec<f64> = legs
                .iter()
                .filter(|l| !l.is_error)
                .filter_map(|l| moving_speed(l, metrics))
                .collect();
            if speeds.len() < MIN_CLEAN_LEGS {
                return None;
            }
            median(&mut speeds)
        });

        let mut clean_run = 0;
        for pair in legs.windows(2) {
            let &[prev, next] = pair else {
                continue;
            };
            clean_run = if prev.is_error { 0 } else { clean_run + 1 };
            if let Some(bucket) = streaks.get_mut(streak_bucket(clean_run)) {
                bucket.add(next);
            }
            if !prev.is_error {
                out.after_clean.add(next);
                continue;
            }
            out.after_error.add(next);
            let speed = r
                .leg_metrics
                .and_then(|metrics| moving_speed(next, metrics))
                .zip(usual_speed);
            match speed {
                Some((v, usual)) if v > usual * (1.0 + ACCELERATION) => out.accelerated.add(next),
                Some(_) => out.not_accelerated.add(next),
                None => out.after_error_without_speed += 1,
            }
        }
    }
    out.streaks = STREAK_STARTS
        .iter()
        .enumerate()
        .zip(streaks)
        .map(|((i, &from), rate)| StreakBucket {
            from,
            to: STREAK_STARTS.get(i + 1).map(|next| next - 1),
            rate,
        })
        .collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::HistoryRace;
    use crate::metrics::TrackMetrics;
    use crate::runner_report::RunnerLostTime;

    const EPS: f64 = 1e-9;

    fn close(actual: Option<f64>, expected: f64) {
        let Some(a) = actual else {
            panic!("se esperaba {expected} y no hay valor");
        };
        assert!((a - expected).abs() < EPS, "{a} != {expected}");
    }

    /// Una carrera a partir de una cadena: `x` = error, `o` = limpio, `s` = referencia corta
    /// (no cuenta), más un último tramo que no cuenta. `speeds`: velocidad en movimiento de cada
    /// tramo de la cadena (`None` = sin FIT).
    fn race(pattern: &str, speeds: Option<&[f64]>) -> (HistoryRace, Option<Vec<LegMetrics>>) {
        let mut kinds: Vec<char> = pattern.chars().collect();
        kinds.push('l');
        let n = kinds.len();
        let legs = kinds
            .iter()
            .enumerate()
            .map(|(i, &k)| LegReport {
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
                // Un tramo corto con error no cuenta: no debe cortar la racha.
                is_error: k == 'x' || k == 's',
                gain_s: None,
                cumulative_gain_s: None,
                ideal_elapsed_s: None,
                behind_ideal_s: None,
                is_last: i + 1 == n,
                short_reference: k == 's',
                excluded_from_patterns: k == 's' || i + 1 == n,
            })
            .collect();
        let metrics = speeds.map(|speeds| {
            speeds
                .iter()
                .enumerate()
                .map(|(i, &v)| LegMetrics {
                    index: i + 1,
                    from: 31,
                    to: 32,
                    track: Some(TrackMetrics {
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
                        heart_rate_bpm: None,
                        cadence_spm: None,
                    }),
                    missing: None,
                })
                .collect()
        });
        let race = HistoryRace {
            date: "2026-05-01".parse().unwrap(),
            format: None,
            lost_time: RunnerLostTime {
                total_s: None,
                usual_performance: Some(0.9),
                lost_time_s: None,
                error_count: 0,
                time_without_errors_s: None,
                ideal_time_s: None,
                behind_ideal_s: None,
                losing_streaks: Vec::new(),
                consistency: None,
                legs,
            },
        };
        (race, metrics)
    }

    fn run(data: &[(HistoryRace, Option<Vec<LegMetrics>>)]) -> AfterError {
        let tracked: Vec<TrackedRace<'_>> = data
            .iter()
            .map(|(race, metrics)| TrackedRace {
                race,
                leg_metrics: metrics.as_deref(),
            })
            .collect();
        after_error(&tracked, &HistoryFilter::default())
    }

    #[test]
    fn chaining_compares_the_leg_after_an_error_with_the_leg_after_a_clean_one() {
        // A: o x x o o x o → pares (o,x) (x,x) (x,o) (o,o) (o,x) (x,o).
        //    Tras error: x, o, o → 1 de 3. Tras limpio: x, o, x → 2 de 3.
        // B: x o s x → la corta no cuenta ni corta: pares (x,o) (o,x).
        //    Tras error: o → 0 de 1. Tras limpio: x → 1 de 1.
        let a = run(&[race("oxxooxo", None), race("xosx", None)]);
        assert_eq!((a.after_error.legs, a.after_error.errors), (4, 1));
        close(a.after_error.error_rate, 0.25);
        assert_eq!((a.after_clean.legs, a.after_clean.errors), (4, 3));
        close(a.after_clean.error_rate, 0.75);
        // Sin FIT no hay velocidad: los cuatro tramos tras error, sin comparar.
        assert_eq!(a.after_error_without_speed, 4);
        assert_eq!(a.accelerated.legs + a.not_accelerated.legs, 0);
    }

    #[test]
    fn clean_streaks_count_the_clean_legs_right_before() {
        // o o o o x o x x o o o o o o o o o o o o x
        // Rachas previas de cada tramo desde el 2.º: 1 2 3 4 0 1 0 0 1 2 … 11 12 (el último, x).
        let mut pattern = String::from("ooooxoxx");
        pattern.push_str(&"o".repeat(12));
        pattern.push('x');
        let a = run(&[race(&pattern, None)]);
        let b: Vec<(usize, Option<usize>, usize, usize)> = a
            .streaks
            .iter()
            .map(|s| (s.from, s.to, s.rate.legs, s.rate.errors))
            .collect();
        assert_eq!(
            b,
            [
                // 0: los tramos 6, 8 y 9 (tras x); 8 es error.
                (0, Some(0), 3, 1),
                // 1–2: tramos 2, 3, 7 (x), 10, 11.
                (1, Some(2), 5, 1),
                // 3–5: tramos 4, 5 (x), 12, 13, 14.
                (3, Some(5), 5, 1),
                // 6–10: tramos 15–19.
                (6, Some(10), 5, 0),
                // más de 10: tramos 20 y 21 (x).
                (11, None, 2, 1),
            ]
        );
        // El cubo 0 es justo «tras un error».
        assert_eq!(a.streaks[0].rate, a.after_error);
    }

    #[test]
    fn recovery_compares_the_speed_with_the_median_of_clean_legs() {
        // Limpios (tramos 1, 3, 4 y 7) a 3,0, 3,4, 3,2 y 3,2 → mediana 3,2; +5 % = 3,36.
        // Tramo 3 (tras el error 2) a 3,4: acelera y es limpio. Tramo 6 (tras el error 5) a
        // 3,3: no acelera y es error. Tramo 7 (tras el error 6) a 3,2: no acelera, limpio.
        let (r, m) = race("oxooxxo", Some(&[3.0, 2.0, 3.4, 3.2, 2.5, 3.3, 3.2, 3.0]));
        let a = run(&[(r, m)]);
        assert_eq!((a.accelerated.legs, a.accelerated.errors), (1, 0));
        assert_eq!((a.not_accelerated.legs, a.not_accelerated.errors), (2, 1));
        assert_eq!(a.after_error_without_speed, 0);
        close(a.not_accelerated.error_rate, 0.5);
    }

    #[test]
    fn too_few_clean_legs_leave_the_speed_out() {
        // Solo dos limpios con velocidad: sin mediana.
        let (r, m) = race("oxo", Some(&[3.0, 2.0, 3.4, 3.0]));
        let a = run(&[(r, m)]);
        assert_eq!(a.after_error.legs, 1);
        assert_eq!(a.after_error_without_speed, 1);
    }

    #[test]
    fn no_races_gives_empty_rates_and_five_buckets() {
        let a = run(&[]);
        assert_eq!(a.after_error, Rate::default());
        assert_eq!(a.streaks.len(), 5);
        assert!(a.streaks.iter().all(|s| s.rate.error_rate.is_none()));
    }
}
