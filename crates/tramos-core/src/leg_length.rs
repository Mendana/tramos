//! Pérdida según la duración del tramo (P7 de `docs/preguntas.md`: «¿Tramos largos o cortos?»).
//!
//! Reparte los tramos que cuentan del histórico ([`crate::history::pattern_legs`]) en cubos
//! logarítmicos por su tiempo de referencia `ref_i` y da, por cubo, tramos, errores, tasa de
//! error y pérdida media. Se usa `ref_i` y no el split del corredor para que un error no cambie
//! el tramo de cubo. Las definiciones están en `docs/historico.md`, "Pérdida según duración del
//! tramo (P7)".

use serde::{Deserialize, Serialize};

use crate::history::{HistoryFilter, HistoryRace, pattern_legs};

/// Límites de los cubos en segundos de referencia: 20 s, 30 s, 1, 2, 4 y 8 min. Cada cubo va
/// de un límite, **incluido**, al siguiente, **excluido**; el último no tiene final.
pub const BUCKET_BOUNDS_S: [f64; 6] = [20.0, 30.0, 60.0, 120.0, 240.0, 480.0];

/// Número de cubos.
pub const BUCKETS: usize = BUCKET_BOUNDS_S.len();

/// Cubo de un tramo con referencia `reference_s`: índice en [`BUCKET_BOUNDS_S`], o `None` por
/// debajo de 20 s (excluido de los patrones) o si no es un número finito.
///
/// `[20, 30)` → 0, `[30, 60)` → 1, `[60, 120)` → 2, `[120, 240)` → 3, `[240, 480)` → 4 y
/// `[480, ∞)` → 5.
pub fn bucket_index(reference_s: f64) -> Option<usize> {
    if !reference_s.is_finite() {
        return None;
    }
    BUCKET_BOUNDS_S
        .iter()
        .rposition(|&bound| reference_s >= bound)
}

/// Números de un cubo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LegLengthStats {
    /// Inicio del cubo (s de referencia), incluido.
    pub from_s: f64,
    /// Final del cubo (s de referencia), excluido; `None` en el último.
    pub to_s: Option<f64>,
    /// Tramos que cuentan con la referencia en el cubo (`n`).
    pub legs: usize,
    /// De ellos, los que son error.
    pub errors: usize,
    /// `errors / legs` (0–1).
    pub error_rate: Option<f64>,
    /// Pérdida media por tramo (s): suma de `p_i` de los errores entre `legs`; los tramos sin
    /// error suman 0.
    pub mean_loss_s: Option<f64>,
    /// Lo mismo en % del tiempo esperado: suma de `loss_pct` de los errores entre `legs`.
    pub mean_loss_pct: Option<f64>,
}

/// Sumas de un cubo.
#[derive(Debug, Default, Clone, Copy)]
struct Accumulator {
    legs: usize,
    errors: usize,
    loss_sum_s: f64,
    loss_sum_pct: f64,
}

/// Agrega por cubo los tramos que cuentan de las carreras que pasan `filter` (el mismo filtro y
/// las mismas carreras que [`crate::history::history`]). Devuelve siempre los seis cubos, en
/// orden y aunque estén vacíos (con las medias a `None`).
pub fn leg_length(races: &[HistoryRace], filter: &HistoryFilter) -> Vec<LegLengthStats> {
    let mut acc = [Accumulator::default(); BUCKETS];
    let counted = races.iter().filter(|r| {
        // Como en el histórico, una carrera sin rendimiento habitual no tiene números.
        filter.includes(r.date, r.format) && r.lost_time.usual_performance.is_some()
    });
    for race in counted {
        for leg in pattern_legs(&race.lost_time) {
            let Some(bucket) = leg
                .reference_s
                .and_then(bucket_index)
                .and_then(|i| acc.get_mut(i))
            else {
                continue;
            };
            bucket.legs += 1;
            if leg.is_error {
                bucket.errors += 1;
                bucket.loss_sum_s += leg.loss_s.unwrap_or(0.0);
                bucket.loss_sum_pct += leg.loss_pct.unwrap_or(0.0);
            }
        }
    }

    acc.iter()
        .zip(BUCKET_BOUNDS_S)
        .enumerate()
        .map(|(i, (a, from_s))| {
            let per_leg = |sum: f64| (a.legs > 0).then(|| sum / a.legs as f64);
            LegLengthStats {
                from_s,
                to_s: BUCKET_BOUNDS_S.get(i + 1).copied(),
                legs: a.legs,
                errors: a.errors,
                error_rate: per_leg(a.errors as f64),
                mean_loss_s: per_leg(a.loss_sum_s),
                mean_loss_pct: per_leg(a.loss_sum_pct),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race_format::RaceFormat;
    use crate::runner_report::{LegReport, RunnerLostTime};

    const EPS: f64 = 1e-9;

    fn close(actual: Option<f64>, expected: f64) {
        let Some(a) = actual else {
            panic!("se esperaba {expected} y no hay valor");
        };
        assert!((a - expected).abs() < EPS, "{a} != {expected}");
    }

    #[test]
    fn bucket_bounds_include_the_start_and_exclude_the_end() {
        let cases = [
            (0.0, None),
            (19.99, None),
            (20.0, Some(0)),
            (29.99, Some(0)),
            (30.0, Some(1)),
            (59.99, Some(1)),
            (60.0, Some(2)),
            (119.99, Some(2)),
            (120.0, Some(3)),
            (239.99, Some(3)),
            (240.0, Some(4)),
            (479.99, Some(4)),
            (480.0, Some(5)),
            (3600.0, Some(5)),
            (-30.0, None),
            (f64::NAN, None),
            (f64::INFINITY, None),
        ];
        for (reference, expected) in cases {
            assert_eq!(bucket_index(reference), expected, "ref = {reference}");
        }
    }

    /// Tramo con referencia `reference` (s) y pérdida `loss` (s y %); `None` = sin split.
    #[derive(Clone, Copy)]
    struct L {
        reference: f64,
        loss: Option<(f64, f64)>,
        error: bool,
        last: bool,
    }

    const fn ok(reference: f64) -> L {
        L {
            reference,
            loss: Some((0.0, 0.0)),
            error: false,
            last: false,
        }
    }

    const fn err(reference: f64, loss_s: f64, loss_pct: f64) -> L {
        L {
            reference,
            loss: Some((loss_s, loss_pct)),
            error: true,
            last: false,
        }
    }

    const fn last(l: L) -> L {
        L { last: true, ..l }
    }

    const fn no_split(reference: f64) -> L {
        L {
            reference,
            loss: None,
            error: false,
            last: false,
        }
    }

    fn race(date: &str, format: Option<RaceFormat>, usual: Option<f64>, legs: &[L]) -> HistoryRace {
        let legs = legs
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let short = l.reference < 20.0;
                LegReport {
                    index: i + 1,
                    from: 31,
                    to: 32,
                    split_s: l.loss.map(|_| l.reference),
                    elapsed_s: None,
                    place: None,
                    reference_s: Some(l.reference),
                    reference_count: 1,
                    valid_splits: 4,
                    performance_index: l.loss.map(|_| 1.0),
                    expected_s: l.loss.map(|_| l.reference),
                    loss_s: l.loss.map(|(s, _)| s),
                    loss_pct: l.loss.map(|(_, p)| p),
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
        HistoryRace {
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
                legs,
            },
        }
    }

    /// Tres carreras sintéticas:
    ///
    /// - A, sprint, 1-mar: 25 s (error, 15 s y 60 %), 28 s, 45 s (error, 20 s y 40 %), 50 s y un
    ///   tramo de 15 s con error, que no cuenta (referencia corta). El último (300 s, error) no
    ///   cuenta.
    /// - B, media, 1-abr: 90 s, 100 s (error, 30 s y 30 %), 200 s, 300 s (error, 60 s y 20 %),
    ///   un tramo de 400 s sin split (no cuenta) y el último, 60 s.
    /// - C, larga, 1-may: 500 s (error, 100 s y 20 %), 600 s y el último, 30 s.
    /// - D, larga, 1-jun, sin rendimiento habitual: no cuenta.
    fn races() -> Vec<HistoryRace> {
        use RaceFormat::*;
        vec![
            race(
                "2026-03-01",
                Some(Sprint),
                Some(0.9),
                &[
                    err(25.0, 15.0, 60.0),
                    ok(28.0),
                    err(45.0, 20.0, 40.0),
                    ok(50.0),
                    err(15.0, 10.0, 66.0),
                    last(err(300.0, 90.0, 30.0)),
                ],
            ),
            race(
                "2026-04-01",
                Some(Middle),
                Some(1.0),
                &[
                    ok(90.0),
                    err(100.0, 30.0, 30.0),
                    ok(200.0),
                    err(300.0, 60.0, 20.0),
                    no_split(400.0),
                    last(ok(60.0)),
                ],
            ),
            race(
                "2026-05-01",
                Some(Long),
                Some(0.95),
                &[err(500.0, 100.0, 20.0), ok(600.0), last(ok(30.0))],
            ),
            race(
                "2026-06-01",
                Some(Long),
                None,
                &[no_split(500.0), last(no_split(30.0))],
            ),
        ]
    }

    /// Comprueba tramos y errores, y tasa de error y pérdida media (s y %).
    fn check(s: &LegLengthStats, counts: (usize, usize), means: [f64; 3]) {
        let [rate, loss_s, loss_pct] = means;
        assert_eq!((s.legs, s.errors), counts, "cubo {}", s.from_s);
        close(s.error_rate, rate);
        close(s.mean_loss_s, loss_s);
        close(s.mean_loss_pct, loss_pct);
    }

    fn empty(s: &LegLengthStats) {
        assert_eq!((s.legs, s.errors), (0, 0), "cubo {}", s.from_s);
        assert_eq!(
            (s.error_rate, s.mean_loss_s, s.mean_loss_pct),
            (None, None, None)
        );
    }

    #[test]
    fn aggregates_counted_legs_by_bucket() {
        let b = leg_length(&races(), &HistoryFilter::default());
        let bounds: Vec<_> = b.iter().map(|s| (s.from_s, s.to_s)).collect();
        assert_eq!(
            bounds,
            [
                (20.0, Some(30.0)),
                (30.0, Some(60.0)),
                (60.0, Some(120.0)),
                (120.0, Some(240.0)),
                (240.0, Some(480.0)),
                (480.0, None),
            ]
        );
        // 20–30 s: 25 y 28 (A); 1 error de 2; 15 / 2 s; 60 / 2 %.
        check(&b[0], (2, 1), [0.5, 7.5, 30.0]);
        // 30–60 s: 45 y 50 (A); el último de B (60 s) no cuenta.
        check(&b[1], (2, 1), [0.5, 10.0, 20.0]);
        // 1–2 min: 90 y 100 (B).
        check(&b[2], (2, 1), [0.5, 15.0, 15.0]);
        // 2–4 min: 200 (B), sin error.
        check(&b[3], (1, 0), [0.0, 0.0, 0.0]);
        // 4–8 min: 300 (B); el último de A (300 s) no cuenta, ni el de 400 s sin split.
        check(&b[4], (1, 1), [1.0, 60.0, 20.0]);
        // Más de 8 min: 500 y 600 (C); D no tiene habitual.
        check(&b[5], (2, 1), [0.5, 50.0, 10.0]);
        // Todos los tramos que cuentan caen en algún cubo.
        let total: usize = b.iter().map(|s| s.legs).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn format_and_date_filters_apply() {
        let sprint = leg_length(
            &races(),
            &HistoryFilter {
                format: Some(RaceFormat::Sprint),
                ..HistoryFilter::default()
            },
        );
        assert_eq!(sprint.len(), BUCKETS);
        check(&sprint[0], (2, 1), [0.5, 7.5, 30.0]);
        check(&sprint[1], (2, 1), [0.5, 10.0, 20.0]);
        sprint[2..].iter().for_each(empty);

        // Del 1-abr al 1-may, incluidos: B y C.
        let spring = leg_length(
            &races(),
            &HistoryFilter {
                from: Some("2026-04-01".parse().unwrap()),
                to: Some("2026-05-01".parse().unwrap()),
                format: None,
            },
        );
        spring[..2].iter().for_each(empty);
        check(&spring[2], (2, 1), [0.5, 15.0, 15.0]);
        check(&spring[5], (2, 1), [0.5, 50.0, 10.0]);

        // Sin carreras, seis cubos vacíos.
        let none = leg_length(&[], &HistoryFilter::default());
        assert_eq!(none.len(), BUCKETS);
        none.iter().for_each(empty);
    }
}
