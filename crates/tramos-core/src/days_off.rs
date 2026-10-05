//! Días sin competir (P11 de `docs/preguntas.md`: «¿Entro peor en mapa tras días sin
//! competir?»).
//!
//! Reparte las carreras del histórico en cubos por los días desde la carrera anterior y da, por
//! cubo, el IR medio de los tres primeros tramos y la tasa de error del primer tercio de la
//! carrera. Las definiciones están en `docs/historico.md`, "Días sin competir (P11)".

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::history::{HistoryFilter, HistoryRace, pattern_legs, race_third};

/// Último día (incluido) de cada cubo salvo el último: hasta 7, 8–14, 15–30 y más de 30 días.
pub const BUCKET_LAST_DAYS: [i64; 3] = [7, 14, 30];

/// Número de cubos.
pub const BUCKETS: usize = BUCKET_LAST_DAYS.len() + 1;

/// Tramos del principio de la carrera para el IR de «entrada en mapa».
pub const FIRST_LEGS: usize = 3;

/// Cubo de una carrera con `days` días desde la anterior: `1..=7` → 0, `8..=14` → 1,
/// `15..=30` → 2 y `31..` → 3. `None` si `days` no es positivo.
pub fn bucket_index(days: i64) -> Option<usize> {
    if days < 1 {
        return None;
    }
    Some(
        BUCKET_LAST_DAYS
            .iter()
            .position(|&last| days <= last)
            .unwrap_or(BUCKET_LAST_DAYS.len()),
    )
}

/// Números de un cubo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DaysOffStats {
    /// Días desde la carrera anterior: primer día del cubo, incluido.
    pub from_days: i64,
    /// Último día, incluido; `None` en el último cubo.
    pub to_days: Option<i64>,
    /// Carreras del cubo.
    pub races: usize,
    /// Tramos con IR entre los tres primeros de esas carreras (`n` del IR).
    pub first_legs: usize,
    /// Media de sus `IR_i` (1 = 100 %).
    pub first_legs_performance: Option<f64>,
    /// Tramos que cuentan del primer tercio de esas carreras (`n` de la tasa).
    pub first_third_legs: usize,
    /// De ellos, los que son error.
    pub first_third_errors: usize,
    /// `first_third_errors / first_third_legs` (0–1).
    pub first_third_error_rate: Option<f64>,
}

/// Resultado de P11.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaysOff {
    /// Los cuatro cubos, en orden y aunque estén vacíos.
    pub buckets: Vec<DaysOffStats>,
    /// Carreras que pasan el filtro sin ninguna anterior (normalmente, la primera importada).
    pub without_previous: usize,
}

/// Sumas de un cubo.
#[derive(Debug, Default, Clone, Copy)]
struct Accumulator {
    races: usize,
    first_legs: usize,
    first_legs_ir_sum: f64,
    first_third_legs: usize,
    first_third_errors: usize,
}

impl Accumulator {
    fn add(&mut self, race: &HistoryRace) {
        self.races += 1;
        let lost = &race.lost_time;
        // Primer tercio por número de tramos del recorrido (con el último): 21 tramos → 7.
        let course_legs = lost.legs.len();
        for leg in pattern_legs(lost) {
            if leg.index <= FIRST_LEGS {
                if let Some(ir) = leg.performance_index {
                    self.first_legs += 1;
                    self.first_legs_ir_sum += ir;
                }
            }
            if race_third(leg.index, course_legs) == 0 {
                self.first_third_legs += 1;
                if leg.is_error {
                    self.first_third_errors += 1;
                }
            }
        }
    }
}

/// Días desde la carrera anterior a `date`: la de fecha más reciente **estrictamente** anterior
/// de `competed` (ordenadas). `None` si no hay ninguna.
fn days_since_previous(date: NaiveDate, competed: &[NaiveDate]) -> Option<i64> {
    let before = competed.partition_point(|d| *d < date);
    before
        .checked_sub(1)
        .and_then(|i| competed.get(i))
        .map(|prev| (date - *prev).num_days())
}

/// Agrega por cubo las carreras de `races` que pasan `filter` y tienen rendimiento habitual (las
/// mismas que [`crate::history::history`]).
///
/// `competed` son las fechas de **todas** las carreras en las que el corredor tomó la salida,
/// pasen o no el filtro y en cualquier orden: competir es competir, sea del formato que sea. Una
/// carrera no presentada no es «carrera anterior».
pub fn days_off(races: &[HistoryRace], competed: &[NaiveDate], filter: &HistoryFilter) -> DaysOff {
    let mut competed = competed.to_vec();
    competed.sort_unstable();
    let mut acc = [Accumulator::default(); BUCKETS];
    let mut without_previous = 0;
    let counted = races
        .iter()
        .filter(|r| filter.includes(r.date, r.format) && r.lost_time.usual_performance.is_some());
    for race in counted {
        let bucket = days_since_previous(race.date, &competed)
            .and_then(bucket_index)
            .and_then(|i| acc.get_mut(i));
        match bucket {
            Some(bucket) => bucket.add(race),
            None => without_previous += 1,
        }
    }

    let buckets = acc
        .iter()
        .enumerate()
        .map(|(i, a)| DaysOffStats {
            from_days: i
                .checked_sub(1)
                .and_then(|p| BUCKET_LAST_DAYS.get(p))
                .map_or(1, |last| last + 1),
            to_days: BUCKET_LAST_DAYS.get(i).copied(),
            races: a.races,
            first_legs: a.first_legs,
            first_legs_performance: (a.first_legs > 0)
                .then(|| a.first_legs_ir_sum / a.first_legs as f64),
            first_third_legs: a.first_third_legs,
            first_third_errors: a.first_third_errors,
            first_third_error_rate: (a.first_third_legs > 0)
                .then(|| a.first_third_errors as f64 / a.first_third_legs as f64),
        })
        .collect();
    DaysOff {
        buckets,
        without_previous,
    }
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

    /// Tramo: `Some((IR, error))` o `None` sin split; `short` = referencia corta.
    #[derive(Clone, Copy)]
    struct L {
        leg: Option<(f64, bool)>,
        short: bool,
    }

    const fn ok(ir: f64) -> L {
        L {
            leg: Some((ir, false)),
            short: false,
        }
    }

    const fn err(ir: f64) -> L {
        L {
            leg: Some((ir, true)),
            short: false,
        }
    }

    const fn short(l: L) -> L {
        L { short: true, ..l }
    }

    const NO_SPLIT: L = L {
        leg: None,
        short: false,
    };

    /// Carrera con los tramos `legs` más un último tramo (que no cuenta) con IR 1.
    fn race(date: &str, format: Option<RaceFormat>, ran: bool, legs: &[L]) -> HistoryRace {
        let mut all = legs.to_vec();
        all.push(ok(1.0));
        let n = all.len();
        let legs = all
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let last = i + 1 == n;
                LegReport {
                    index: i + 1,
                    from: 31,
                    to: 32,
                    split_s: l.leg.map(|_| 60.0),
                    elapsed_s: None,
                    place: None,
                    reference_s: Some(if l.short { 15.0 } else { 60.0 }),
                    reference_count: 1,
                    valid_splits: 4,
                    performance_index: l.leg.map(|(ir, _)| ir),
                    expected_s: l.leg.map(|_| 60.0),
                    loss_s: l.leg.map(|(_, e)| if e { 30.0 } else { 0.0 }),
                    loss_pct: l.leg.map(|(_, e)| if e { 50.0 } else { 0.0 }),
                    is_error: l.leg.is_some_and(|(_, e)| e),
                    gain_s: None,
                    cumulative_gain_s: None,
                    ideal_elapsed_s: None,
                    behind_ideal_s: None,
                    is_last: last,
                    short_reference: l.short,
                    excluded_from_patterns: last || l.short,
                }
            })
            .collect();
        HistoryRace {
            date: date.parse().unwrap(),
            format,
            lost_time: RunnerLostTime {
                total_s: None,
                usual_performance: ran.then_some(0.9),
                lost_time_s: None,
                error_count: 0,
                time_without_errors_s: None,
                ideal_time_s: None,
                behind_ideal_s: None,
                losing_streaks: Vec::new(),
                consistency: None,
                legs,
            },
        }
    }

    /// Fechas de las carreras en las que se tomó la salida: aquí, las que tienen habitual (X es
    /// un no presentado).
    fn competed(races: &[HistoryRace]) -> Vec<NaiveDate> {
        races
            .iter()
            .filter(|r| r.lost_time.usual_performance.is_some())
            .map(|r| r.date)
            .collect()
    }

    fn run(filter: &HistoryFilter) -> DaysOff {
        let all = races();
        days_off(&all, &competed(&all), filter)
    }

    #[test]
    fn buckets_are_inclusive_at_both_ends() {
        let cases = [
            (0, None),
            (-3, None),
            (1, Some(0)),
            (7, Some(0)),
            (8, Some(1)),
            (14, Some(1)),
            (15, Some(2)),
            (30, Some(2)),
            (31, Some(3)),
            (400, Some(3)),
        ];
        for (days, want) in cases {
            assert_eq!(bucket_index(days), want, "{days} días");
        }
    }

    /// Fechas sintéticas, sin orden (el cálculo no depende de él). Días desde la anterior:
    ///
    /// - A 1-mar, sprint: la primera → sin anterior.
    /// - X 4-mar, no presentado: no cuenta ni como anterior.
    /// - B 6-mar, media: 5 días (desde A; X no corrió) → hasta 7.
    /// - C 6-mar, sprint: el mismo día que B; la anterior es estrictamente anterior → 5 días.
    /// - D 16-mar, sprint: 10 días → 8–14.
    /// - E 15-abr, larga: 30 días → 15–30.
    /// - F 20-may, sprint: 35 días → más de 30.
    fn races() -> Vec<HistoryRace> {
        use RaceFormat::*;
        vec![
            // 6 tramos + el último = 7 → primer tercio, tramos 1–3.
            race(
                "2026-05-20",
                Some(Sprint),
                true,
                &[err(0.5), ok(0.8), ok(0.9), err(0.6), ok(1.0), ok(1.0)],
            ),
            race("2026-03-01", Some(Sprint), true, &[ok(1.0), ok(1.0)]),
            race("2026-03-04", Some(Long), false, &[NO_SPLIT, NO_SPLIT]),
            // 3 tramos + el último = 4 → primer tercio, tramos 1–2.
            race(
                "2026-03-06",
                Some(Middle),
                true,
                &[err(0.7), ok(0.9), ok(1.1)],
            ),
            race(
                "2026-03-06",
                Some(Sprint),
                true,
                &[ok(0.8), short(err(0.4)), ok(1.0)],
            ),
            // 8 tramos + el último = 9 → primer tercio, tramos 1–3. El 2 no tiene split.
            race(
                "2026-03-16",
                Some(Sprint),
                true,
                &[
                    ok(0.9),
                    NO_SPLIT,
                    err(0.7),
                    ok(1.0),
                    err(0.5),
                    ok(1.0),
                    ok(1.0),
                    ok(1.0),
                ],
            ),
            race(
                "2026-04-15",
                Some(Long),
                true,
                &[ok(1.2), ok(1.0), ok(0.8), ok(0.9), ok(0.9)],
            ),
        ]
    }

    #[test]
    fn aggregates_the_first_legs_by_days_since_the_previous_race() {
        let d = run(&HistoryFilter::default());
        let b = &d.buckets;
        assert_eq!(d.without_previous, 1);
        let ranges: Vec<_> = b.iter().map(|s| (s.from_days, s.to_days)).collect();
        assert_eq!(
            ranges,
            [(1, Some(7)), (8, Some(14)), (15, Some(30)), (31, None)]
        );

        // Hasta 7: B y C. Tres primeros: B 0,7 0,9 1,1; C 0,8 y 1,0 (el 2 es corto).
        // IR (0,7 + 0,9 + 1,1 + 0,8 + 1,0) / 5 = 0,9. Primer tercio (tramos 1–2): B 2 tramos
        // con 1 error; C solo el 1 (el 2 es corto) → 1 de 3.
        assert_eq!(
            (b[0].races, b[0].first_legs, b[0].first_third_legs),
            (2, 5, 3)
        );
        close(b[0].first_legs_performance, 0.9);
        assert_eq!(b[0].first_third_errors, 1);
        close(b[0].first_third_error_rate, 1.0 / 3.0);

        // 8–14: D. Tres primeros con IR: 0,9 y 0,7 → 0,8. Primer tercio (1–3): 2 tramos que
        // cuentan (el 2 no tiene split), 1 error.
        assert_eq!(
            (b[1].races, b[1].first_legs, b[1].first_third_legs),
            (1, 2, 2)
        );
        close(b[1].first_legs_performance, 0.8);
        close(b[1].first_third_error_rate, 0.5);

        // 15–30: E. (1,2 + 1,0 + 0,8) / 3 = 1,0. Primer tercio (6 tramos → 1–2): sin errores.
        assert_eq!(
            (b[2].races, b[2].first_legs, b[2].first_third_legs),
            (1, 3, 2)
        );
        close(b[2].first_legs_performance, 1.0);
        close(b[2].first_third_error_rate, 0.0);

        // Más de 30: F. (0,5 + 0,8 + 0,9) / 3. Primer tercio (1–3): 1 error de 3.
        assert_eq!(
            (b[3].races, b[3].first_legs, b[3].first_third_legs),
            (1, 3, 3)
        );
        close(b[3].first_legs_performance, 2.2 / 3.0);
        close(b[3].first_third_error_rate, 1.0 / 3.0);
    }

    #[test]
    fn days_count_against_races_left_out_by_the_filter() {
        // Solo sprint (A, C, D y F). A sigue sin anterior; C, a 5 días de A; D, a 10 del 6-mar
        // (B y C). La anterior de F es E (larga, fuera del filtro) → 35 días, no los 65 desde D.
        let filter = HistoryFilter {
            format: Some(RaceFormat::Sprint),
            ..HistoryFilter::default()
        };
        let d = run(&filter);
        let counts: Vec<_> = d.buckets.iter().map(|b| b.races).collect();
        assert_eq!(counts, [1, 1, 0, 1]);
        assert_eq!(d.without_previous, 1);
    }

    #[test]
    fn date_filter_keeps_the_gap_to_earlier_races() {
        // Desde el 1-abr entran E y F; E sigue a 30 días de D aunque D quede fuera.
        let filter = HistoryFilter {
            from: Some("2026-04-01".parse().unwrap()),
            ..HistoryFilter::default()
        };
        let d = run(&filter);
        let counts: Vec<_> = d.buckets.iter().map(|b| b.races).collect();
        assert_eq!(counts, [0, 0, 1, 1]);
        assert_eq!(d.without_previous, 0);
    }

    #[test]
    fn only_the_given_dates_count_as_previous_races() {
        // Si el filtro de la app solo trae D, los días salen igual de las fechas de salida.
        let all = races();
        let only_d: Vec<_> = all
            .iter()
            .filter(|r| r.date.to_string() == "2026-03-16")
            .cloned()
            .collect();
        let d = days_off(&only_d, &competed(&all), &HistoryFilter::default());
        let counts: Vec<_> = d.buckets.iter().map(|b| b.races).collect();
        assert_eq!(counts, [0, 1, 0, 0]);
        // Sin fechas de salida, D no tiene anterior.
        let d = days_off(&only_d, &[], &HistoryFilter::default());
        assert_eq!(d.without_previous, 1);
    }

    #[test]
    fn no_races_gives_four_empty_buckets() {
        let d = days_off(&[], &[], &HistoryFilter::default());
        assert_eq!(d.buckets.len(), BUCKETS);
        assert!(d.buckets.iter().all(|b| b.races == 0
            && b.first_legs_performance.is_none()
            && b.first_third_error_rate.is_none()));
        assert_eq!(d.without_previous, 0);
    }
}
