//! Vista histórica: el agregado de muchas carreras de un corredor por formato (P6 de
//! `docs/preguntas.md`).
//!
//! Parte del informe de cada carrera ([`crate::runner_report`]), calculado con los umbrales del
//! usuario, y da por formato (sprint, media, larga y las carreras sin formato) y en total: número
//! de carreras, IR medio, tasa de error y pérdida media por tramo. Las definiciones exactas
//! están en `docs/historico.md`.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::race_format::RaceFormat;
use crate::runner_report::{LegReport, RunnerLostTime};

/// Formatos en el orden en que salen en el histórico.
pub const FORMATS: [RaceFormat; 3] = [RaceFormat::Sprint, RaceFormat::Middle, RaceFormat::Long];

/// Una carrera del corredor para el histórico.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryRace {
    /// Fecha local de la carrera.
    pub date: NaiveDate,
    /// Formato confirmado al importar; `None` si no se fijó.
    pub format: Option<RaceFormat>,
    /// Tiempo perdido del corredor en la carrera ([`crate::runner_report`]).
    pub lost_time: RunnerLostTime,
}

/// Qué carreras entran. Los campos ausentes no filtran.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryFilter {
    /// Desde esta fecha, incluida.
    pub from: Option<NaiveDate>,
    /// Hasta esta fecha, incluida.
    pub to: Option<NaiveDate>,
    /// Solo este formato. Las carreras sin formato solo entran sin este filtro.
    pub format: Option<RaceFormat>,
}

impl HistoryFilter {
    /// Si una carrera de fecha `date` y formato `format` pasa el filtro.
    pub fn includes(&self, date: NaiveDate, format: Option<RaceFormat>) -> bool {
        self.from.is_none_or(|from| date >= from)
            && self.to.is_none_or(|to| date <= to)
            && self.format.is_none_or(|f| format == Some(f))
    }
}

/// Números agregados de un grupo de carreras.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryStats {
    /// Carreras con números (con rendimiento habitual).
    pub races: usize,
    /// Tramos que cuentan (ver [`pattern_legs`]).
    pub legs: usize,
    /// Tramos que cuentan y son error.
    pub errors: usize,
    /// IR medio: media del rendimiento habitual de cada carrera (1 = 100 %).
    pub mean_performance: Option<f64>,
    /// `errors / legs` (0–1).
    pub error_rate: Option<f64>,
    /// Pérdida media por tramo (s): suma de `p_i` de los tramos con error entre `legs`. Los
    /// tramos sin error suman 0.
    pub mean_loss_s: Option<f64>,
    /// Lo mismo en % del tiempo esperado: media de `loss_pct` de los tramos con error y 0 en los
    /// demás. Comparable entre formatos, que tienen tramos de duraciones muy distintas.
    pub mean_loss_pct: Option<f64>,
}

/// Un formato (o las carreras sin formato, con `format: None`) y sus números.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FormatHistory {
    pub format: Option<RaceFormat>,
    pub stats: HistoryStats,
}

/// El histórico de un corredor con un filtro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub filter: HistoryFilter,
    /// Sprint, media y larga siempre (aunque no tengan carreras), salvo que se filtre por
    /// formato: entonces solo ese. Al final, las carreras sin formato, si hay alguna.
    pub by_format: Vec<FormatHistory>,
    /// Todas las carreras que pasan el filtro juntas, con o sin formato.
    pub total: HistoryStats,
    /// Carreras que pasan el filtro pero no tienen números (sin rendimiento habitual: no
    /// presentado o sin ningún split con referencia). No cuentan en ningún grupo.
    pub races_without_data: usize,
}

/// Tramos de una carrera que cuentan en el histórico (y en el resto de análisis de patrones):
/// los que tienen pérdida y no están excluidos de los patrones (ni el último ni los de
/// referencia corta, `docs/tiempo-perdido.md`, "Exclusiones").
pub fn pattern_legs(lost_time: &RunnerLostTime) -> impl Iterator<Item = &LegReport> {
    lost_time
        .legs
        .iter()
        .filter(|leg| !leg.excluded_from_patterns && leg.loss_s.is_some())
}

/// Sumas de un grupo, de las que salen las medias.
#[derive(Debug, Default)]
struct Accumulator {
    races: usize,
    performance_sum: f64,
    legs: usize,
    errors: usize,
    loss_sum_s: f64,
    loss_sum_pct: f64,
}

impl Accumulator {
    /// Suma una carrera con rendimiento habitual `usual`.
    fn add(&mut self, usual: f64, lost_time: &RunnerLostTime) {
        self.races += 1;
        self.performance_sum += usual;
        for leg in pattern_legs(lost_time) {
            self.legs += 1;
            if leg.is_error {
                self.errors += 1;
                self.loss_sum_s += leg.loss_s.unwrap_or(0.0);
                self.loss_sum_pct += leg.loss_pct.unwrap_or(0.0);
            }
        }
    }

    fn stats(&self) -> HistoryStats {
        let per_race = |sum: f64| (self.races > 0).then(|| sum / self.races as f64);
        let per_leg = |sum: f64| (self.legs > 0).then(|| sum / self.legs as f64);
        HistoryStats {
            races: self.races,
            legs: self.legs,
            errors: self.errors,
            mean_performance: per_race(self.performance_sum),
            error_rate: per_leg(self.errors as f64),
            mean_loss_s: per_leg(self.loss_sum_s),
            mean_loss_pct: per_leg(self.loss_sum_pct),
        }
    }
}

/// Agrega las carreras que pasan `filter` por formato y en total.
pub fn history(races: &[HistoryRace], filter: &HistoryFilter) -> History {
    let mut formats: [Accumulator; 3] = Default::default();
    let mut unassigned = Accumulator::default();
    let mut total = Accumulator::default();
    let mut races_without_data = 0;

    for race in races.iter().filter(|r| filter.includes(r.date, r.format)) {
        let Some(usual) = race.lost_time.usual_performance else {
            races_without_data += 1;
            continue;
        };
        let group = match race.format {
            Some(format) => FORMATS
                .iter()
                .position(|f| *f == format)
                .and_then(|i| formats.get_mut(i)),
            None => Some(&mut unassigned),
        };
        if let Some(group) = group {
            group.add(usual, &race.lost_time);
        }
        total.add(usual, &race.lost_time);
    }

    let mut by_format: Vec<FormatHistory> = FORMATS
        .iter()
        .zip(&formats)
        .filter(|(format, _)| filter.format.is_none_or(|f| f == **format))
        .map(|(format, acc)| FormatHistory {
            format: Some(*format),
            stats: acc.stats(),
        })
        .collect();
    if unassigned.races > 0 {
        by_format.push(FormatHistory {
            format: None,
            stats: unassigned.stats(),
        });
    }

    History {
        filter: *filter,
        by_format,
        total: total.stats(),
        races_without_data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    fn close(actual: Option<f64>, expected: f64) {
        let Some(a) = actual else {
            panic!("se esperaba {expected} y no hay valor");
        };
        assert!((a - expected).abs() < EPS, "{a} != {expected}");
    }

    /// Tramo con la pérdida `loss` (s y %) y sus marcas. `None` = sin split o sin referencia.
    #[derive(Clone, Copy)]
    struct L {
        loss: Option<(f64, f64)>,
        error: bool,
        last: bool,
        short: bool,
    }

    const fn ok(loss_s: f64, loss_pct: f64) -> L {
        L {
            loss: Some((loss_s, loss_pct)),
            error: false,
            last: false,
            short: false,
        }
    }

    const fn err(loss_s: f64, loss_pct: f64) -> L {
        L {
            error: true,
            ..ok(loss_s, loss_pct)
        }
    }

    const fn last(l: L) -> L {
        L { last: true, ..l }
    }

    const fn short(l: L) -> L {
        L { short: true, ..l }
    }

    const NO_SPLIT: L = L {
        loss: None,
        error: false,
        last: false,
        short: false,
    };

    fn race(date: &str, format: Option<RaceFormat>, usual: Option<f64>, legs: &[L]) -> HistoryRace {
        let legs = legs
            .iter()
            .enumerate()
            .map(|(i, l)| LegReport {
                index: i + 1,
                from: 31,
                to: 32,
                split_s: l.loss.map(|_| 60.0),
                elapsed_s: None,
                place: None,
                reference_s: Some(if l.short { 15.0 } else { 60.0 }),
                reference_count: 1,
                valid_splits: 4,
                performance_index: l.loss.map(|_| 1.0),
                expected_s: l.loss.map(|_| 60.0),
                loss_s: l.loss.map(|(s, _)| s),
                loss_pct: l.loss.map(|(_, p)| p),
                is_error: l.error,
                gain_s: None,
                cumulative_gain_s: None,
                ideal_elapsed_s: None,
                behind_ideal_s: None,
                is_last: l.last,
                short_reference: l.short,
                excluded_from_patterns: l.last || l.short,
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

    /// Cinco carreras sintéticas. Por carrera, tramos que cuentan / errores / pérdida de los
    /// errores (s y %):
    ///
    /// - A, sprint, 1-mar, habitual 0,90: 3 / 1 / 30 s y 50 %. El último tramo es error pero no
    ///   cuenta.
    /// - B, sprint, 10-abr, habitual 0,80: 2 / 1 / 60 s y 100 %. No cuentan el de referencia
    ///   corta (error), el que no tiene split ni el último.
    /// - C, media, 20-may, habitual 1,00: 4 / 2 / 65 s y 32,5 %.
    /// - D, sin formato, 15-jun, habitual 0,70: 1 / 1 / 20 s y 25 %.
    /// - E, larga, 1-jul, sin habitual (no presentado): sin números.
    fn races() -> Vec<HistoryRace> {
        use RaceFormat::*;
        vec![
            race(
                "2026-03-01",
                Some(Sprint),
                Some(0.90),
                &[
                    err(30.0, 50.0),
                    ok(-5.0, -10.0),
                    ok(10.0, 20.0),
                    last(err(20.0, 40.0)),
                ],
            ),
            race(
                "2026-04-10",
                Some(Sprint),
                Some(0.80),
                &[
                    err(60.0, 100.0),
                    short(err(16.0, 80.0)),
                    NO_SPLIT,
                    ok(0.0, 0.0),
                    last(ok(0.0, 0.0)),
                ],
            ),
            race(
                "2026-05-20",
                Some(Middle),
                Some(1.00),
                &[
                    err(40.0, 20.0),
                    ok(-10.0, -8.0),
                    ok(5.0, 4.0),
                    err(25.0, 12.5),
                    last(ok(2.0, 3.0)),
                ],
            ),
            race(
                "2026-06-15",
                None,
                Some(0.70),
                &[err(20.0, 25.0), last(ok(1.0, 1.0))],
            ),
            race("2026-07-01", Some(Long), None, &[NO_SPLIT, last(NO_SPLIT)]),
        ]
    }

    fn group(h: &History, format: Option<RaceFormat>) -> HistoryStats {
        h.by_format
            .iter()
            .find(|g| g.format == format)
            .map(|g| g.stats)
            .unwrap()
    }

    /// Comprueba carreras, tramos y errores, y IR medio, tasa de error y pérdida media (s y %).
    fn check(s: HistoryStats, counts: (usize, usize, usize), means: [f64; 4]) {
        let [ir, rate, loss_s, loss_pct] = means;
        assert_eq!((s.races, s.legs, s.errors), counts);
        close(s.mean_performance, ir);
        close(s.error_rate, rate);
        close(s.mean_loss_s, loss_s);
        close(s.mean_loss_pct, loss_pct);
    }

    #[test]
    fn aggregates_by_format_and_in_total() {
        let h = history(&races(), &HistoryFilter::default());
        let formats: Vec<_> = h.by_format.iter().map(|g| g.format).collect();
        use RaceFormat::*;
        assert_eq!(formats, [Some(Sprint), Some(Middle), Some(Long), None]);

        // Sprint: A y B. IR (0,90 + 0,80) / 2; 2 errores en 5 tramos; (30 + 60) / 5 s;
        // (50 + 100) / 5 %.
        check(group(&h, Some(Sprint)), (2, 5, 2), [0.85, 0.4, 18.0, 30.0]);
        // Media: C. 2 de 4; 65 / 4 s; 32,5 / 4 %.
        check(group(&h, Some(Middle)), (1, 4, 2), [1.0, 0.5, 16.25, 8.125]);
        // Larga: E no tiene números; el grupo sale vacío.
        let long = group(&h, Some(Long));
        assert_eq!(long, HistoryStats::default());
        // Sin formato: D.
        check(group(&h, None), (1, 1, 1), [0.70, 1.0, 20.0, 25.0]);
        // Total: A, B, C y D. IR (0,9 + 0,8 + 1 + 0,7) / 4; 5 de 10; 175 / 10 s; 207,5 / 10 %.
        check(h.total, (4, 10, 5), [0.85, 0.5, 17.5, 20.75]);
        assert_eq!(h.races_without_data, 1);
    }

    #[test]
    fn date_range_is_inclusive() {
        let filter = HistoryFilter {
            from: Some("2026-04-10".parse().unwrap()),
            to: Some("2026-05-20".parse().unwrap()),
            format: None,
        };
        let h = history(&races(), &filter);
        assert_eq!(h.filter, filter);
        // Solo B y C; sin carreras sin formato, ese grupo no sale.
        assert_eq!(h.by_format.len(), 3);
        check(
            group(&h, Some(RaceFormat::Sprint)),
            (1, 2, 1),
            [0.8, 0.5, 30.0, 50.0],
        );
        check(
            group(&h, Some(RaceFormat::Middle)),
            (1, 4, 2),
            [1.0, 0.5, 16.25, 8.125],
        );
        // Total: (0,8 + 1) / 2; 3 de 6; 125 / 6 s; 132,5 / 6 %.
        check(h.total, (2, 6, 3), [0.9, 0.5, 125.0 / 6.0, 132.5 / 6.0]);
        assert_eq!(h.races_without_data, 0);

        // Un solo día también es un rango.
        let day = "2026-03-01".parse().unwrap();
        let h = history(
            &races(),
            &HistoryFilter {
                from: Some(day),
                to: Some(day),
                format: None,
            },
        );
        check(h.total, (1, 3, 1), [0.9, 1.0 / 3.0, 10.0, 50.0 / 3.0]);
    }

    #[test]
    fn format_filter_keeps_only_that_format() {
        let h = history(
            &races(),
            &HistoryFilter {
                format: Some(RaceFormat::Sprint),
                ..HistoryFilter::default()
            },
        );
        assert_eq!(h.by_format.len(), 1);
        assert_eq!(h.by_format[0].format, Some(RaceFormat::Sprint));
        // Sin la carrera sin formato (D) ni la larga (E).
        assert_eq!(h.total, h.by_format[0].stats);
        check(h.total, (2, 5, 2), [0.85, 0.4, 18.0, 30.0]);
        assert_eq!(h.races_without_data, 0);

        let h = history(
            &races(),
            &HistoryFilter {
                format: Some(RaceFormat::Long),
                ..HistoryFilter::default()
            },
        );
        assert_eq!(h.total, HistoryStats::default());
        assert_eq!(h.races_without_data, 1);
    }

    #[test]
    fn nothing_matches() {
        // Fechas al revés: ninguna carrera.
        let h = history(
            &races(),
            &HistoryFilter {
                from: Some("2026-06-01".parse().unwrap()),
                to: Some("2026-05-01".parse().unwrap()),
                format: None,
            },
        );
        assert_eq!(h.total, HistoryStats::default());
        assert_eq!(h.by_format.len(), 3);
        assert!(
            h.by_format
                .iter()
                .all(|g| g.stats == HistoryStats::default())
        );

        let h = history(&[], &HistoryFilter::default());
        assert_eq!(h.total.mean_performance, None);
        assert_eq!(h.total.error_rate, None);
        assert_eq!(h.races_without_data, 0);
    }

    #[test]
    fn race_without_counting_legs_still_counts_for_performance() {
        // Solo el último tramo: la carrera cuenta para el IR, pero no aporta tramos.
        let only_last = race(
            "2026-01-01",
            Some(RaceFormat::Long),
            Some(0.95),
            &[last(err(30.0, 30.0))],
        );
        let h = history(&[only_last], &HistoryFilter::default());
        let long = group(&h, Some(RaceFormat::Long));
        assert_eq!((long.races, long.legs, long.errors), (1, 0, 0));
        close(long.mean_performance, 0.95);
        assert_eq!(long.error_rate, None);
        assert_eq!(long.mean_loss_s, None);
    }

    #[test]
    fn pattern_legs_skip_excluded_and_legs_without_loss() {
        let r = &races()[1];
        let counted: Vec<usize> = pattern_legs(&r.lost_time).map(|l| l.index).collect();
        assert_eq!(counted, [1, 4]);
    }

    #[test]
    fn filter_deserializes_with_missing_fields() {
        let f: HistoryFilter = serde_json::from_str(r#"{"from": "2026-01-01"}"#).unwrap();
        assert_eq!(f.from, Some("2026-01-01".parse().unwrap()));
        assert_eq!((f.to, f.format), (None, None));
        let f: HistoryFilter =
            serde_json::from_str(r#"{"from": null, "to": "2026-12-31", "format": "middle"}"#)
                .unwrap();
        assert_eq!(f.format, Some(RaceFormat::Middle));
    }
}
