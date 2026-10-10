//! Consistencia (P10 de `docs/preguntas.md`): la desviación típica de los `IR_i` de una carrera,
//! ponderada por `ref_i`. Menor = más consistente. Ver `docs/tiempo-perdido.md`, "Consistencia".

use crate::history::{HistoryFilter, HistoryRace, pattern_legs};
use crate::runner_report::RunnerLostTime;

/// Tramos mínimos para que la consistencia tenga sentido: con uno solo siempre sería 0.
pub const MIN_CONSISTENCY_LEGS: usize = 2;

/// Desviación típica ponderada (de población) de `(valor, peso)`: la media es la ponderada y la
/// varianza `Σ w·(x − media)² / Σ w`. `None` con menos de [`MIN_CONSISTENCY_LEGS`] valores o si
/// los pesos no suman algo positivo.
pub fn weighted_std_dev(values: &[(f64, f64)]) -> Option<f64> {
    if values.len() < MIN_CONSISTENCY_LEGS {
        return None;
    }
    let total: f64 = values.iter().map(|&(_, w)| w).sum();
    if total <= 0.0 {
        return None;
    }
    let mean = values.iter().map(|&(x, w)| w * x).sum::<f64>() / total;
    let variance = values
        .iter()
        .map(|&(x, w)| w * (x - mean).powi(2))
        .sum::<f64>()
        / total;
    Some(variance.sqrt())
}

/// Consistencia de una carrera (1 = 100 puntos de IR): la desviación típica de `IR_i` ponderada
/// por `ref_i` en los tramos que cuentan para los patrones ([`pattern_legs`]: ni el último ni los
/// de referencia corta). `None` si quedan menos de [`MIN_CONSISTENCY_LEGS`].
pub fn race_consistency(lost_time: &RunnerLostTime) -> Option<f64> {
    let values: Vec<(f64, f64)> = pattern_legs(lost_time)
        .filter_map(|leg| Some((leg.performance_index?, leg.reference_s?)))
        .collect();
    weighted_std_dev(&values)
}

/// La consistencia de cada carrera que pasa `filter` y que la tiene, de la más antigua a la más
/// reciente (a igual fecha, en el orden en que vienen). Cuenta las mismas carreras que
/// `mean_consistency` del histórico: las que tienen rendimiento habitual. Es la serie de la
/// gráfica de consistencia y de la que sale la frase de P10 (`docs/frases.md`).
pub fn consistency_series(races: &[HistoryRace], filter: &HistoryFilter) -> Vec<f64> {
    let mut series: Vec<(chrono::NaiveDate, f64)> = races
        .iter()
        .filter(|r| filter.includes(r.date, r.format))
        .filter(|r| r.lost_time.usual_performance.is_some())
        .filter_map(|r| Some((r.date, r.lost_time.consistency?)))
        .collect();
    // Estable: a igual fecha se respeta el orden de entrada.
    series.sort_by_key(|(date, _)| *date);
    series.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner_report::LegReport;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn equal_weights_give_the_usual_standard_deviation() {
        // IR 0,9 y 1,1: media 1, desviación 0,1.
        let sd = weighted_std_dev(&[(0.9, 60.0), (1.1, 60.0)]).unwrap();
        assert!(close(sd, 0.1), "{sd}");
    }

    #[test]
    fn weights_pull_the_mean_and_the_spread() {
        // IR 1,0 (ref 60), 0,8 (ref 120) y 1,2 (ref 60):
        // media = (60 + 96 + 72) / 240 = 0,95;
        // varianza = (60·0,05² + 120·0,15² + 60·0,25²) / 240 = 6,6 / 240 = 0,0275.
        let sd = weighted_std_dev(&[(1.0, 60.0), (0.8, 120.0), (1.2, 60.0)]).unwrap();
        assert!(close(sd, 0.0275_f64.sqrt()), "{sd}");
        assert!(close(sd, 0.165_831_239_517_769_99), "{sd}");
    }

    #[test]
    fn identical_performances_are_perfectly_consistent() {
        let sd = weighted_std_dev(&[(0.95, 30.0), (0.95, 90.0), (0.95, 45.0)]).unwrap();
        assert!(close(sd, 0.0), "{sd}");
    }

    #[test]
    fn fewer_than_two_legs_or_no_weight_has_no_value() {
        assert_eq!(weighted_std_dev(&[]), None);
        assert_eq!(weighted_std_dev(&[(1.0, 60.0)]), None);
        assert_eq!(weighted_std_dev(&[(1.0, 0.0), (0.8, 0.0)]), None);
    }

    /// Tramo con `IR` y referencia `reference`; `excluded` = último o de referencia corta.
    fn leg(index: usize, ir: Option<f64>, reference: f64, excluded: bool) -> LegReport {
        LegReport {
            index,
            from: 31,
            to: 32,
            split_s: ir.map(|ir| reference / ir),
            elapsed_s: None,
            place: None,
            reference_s: Some(reference),
            reference_count: 1,
            valid_splits: 4,
            performance_index: ir,
            expected_s: ir.map(|_| reference),
            loss_s: ir.map(|_| 0.0),
            loss_pct: ir.map(|_| 0.0),
            is_error: false,
            gain_s: None,
            cumulative_gain_s: None,
            ideal_elapsed_s: None,
            behind_ideal_s: None,
            is_last: false,
            short_reference: excluded,
            excluded_from_patterns: excluded,
        }
    }

    fn lost_time(legs: Vec<LegReport>) -> RunnerLostTime {
        RunnerLostTime {
            total_s: None,
            usual_performance: Some(1.0),
            lost_time_s: None,
            error_count: 0,
            time_without_errors_s: None,
            ideal_time_s: None,
            behind_ideal_s: None,
            losing_streaks: Vec::new(),
            consistency: None,
            legs,
        }
    }

    #[test]
    fn race_consistency_uses_only_the_pattern_legs() {
        // Cuentan los tramos 1, 3 y 5 (el ejemplo ponderado de arriba: 0,0275 de varianza). El 2
        // no tiene split, el 4 tiene referencia corta y el 6 es el último: no entran.
        let mut legs = vec![
            leg(1, Some(1.0), 60.0, false),
            leg(2, None, 60.0, false),
            leg(3, Some(0.8), 120.0, false),
            leg(4, Some(0.4), 15.0, true),
            leg(5, Some(1.2), 60.0, false),
            leg(6, Some(2.0), 10.0, true),
        ];
        legs[5].is_last = true;
        let sd = race_consistency(&lost_time(legs)).unwrap();
        assert!(close(sd, 0.0275_f64.sqrt()), "{sd}");
    }

    #[test]
    fn race_with_a_single_pattern_leg_has_no_consistency() {
        let legs = vec![
            leg(1, Some(0.9), 60.0, false),
            leg(2, Some(1.3), 10.0, true),
        ];
        assert_eq!(race_consistency(&lost_time(legs)), None);
    }

    fn race(date: &str, consistency: Option<f64>, usual: Option<f64>) -> HistoryRace {
        let mut lost = lost_time(Vec::new());
        lost.consistency = consistency;
        lost.usual_performance = usual;
        HistoryRace {
            date: date.parse().unwrap(),
            format: None,
            lost_time: lost,
        }
    }

    #[test]
    fn series_is_chronological_and_skips_races_without_a_value() {
        // Entran fuera de orden. La de 2026-03-01 no tiene consistencia y la de 2026-04-01 no
        // tiene rendimiento habitual: no cuentan, como en la media del histórico.
        let races = [
            race("2026-05-01", Some(0.30), Some(0.9)),
            race("2026-01-01", Some(0.10), Some(0.9)),
            race("2026-03-01", None, Some(0.9)),
            race("2026-04-01", Some(0.40), None),
            race("2026-02-01", Some(0.20), Some(0.9)),
        ];
        assert_eq!(
            consistency_series(&races, &HistoryFilter::default()),
            [0.10, 0.20, 0.30]
        );
    }

    #[test]
    fn series_respects_the_filter() {
        let races = [
            race("2026-01-01", Some(0.10), Some(0.9)),
            race("2026-02-01", Some(0.20), Some(0.9)),
            race("2026-03-01", Some(0.30), Some(0.9)),
        ];
        let filter = HistoryFilter {
            from: Some("2026-02-01".parse().unwrap()),
            to: Some("2026-02-28".parse().unwrap()),
            format: None,
        };
        assert_eq!(consistency_series(&races, &filter), [0.20]);
    }
}
