//! Vista de grupo (P15 de `docs/preguntas.md`): la entrenadora compara a sus corredores. Una
//! fila por corredor con lo principal de su histórico ([`group_row`]) y todos contra todos en las
//! carreras que comparten ([`compare`]). Las definiciones están en `docs/historico.md`, "Vista de
//! grupo (P15)".

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::common_errors::CommonErrors;
use crate::history::{History, HistoryStats};
use crate::leg_length::LegLengthStats;
use crate::race_format::RaceFormat;
use crate::slope::{SlopeHistory, SlopeStats};

/// Tramos mínimos de un cubo de duración para poder ser el «peor» de un corredor: con menos,
/// una tasa de error alta puede ser casualidad.
pub const MIN_BUCKET_LEGS: usize = 10;

/// El tipo de error más común de un corredor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopError {
    pub error_type: String,
    pub errors: usize,
    /// Parte de sus errores de orientación que son de ese tipo (0–1), contando los sin tipo.
    pub share: f64,
}

/// Lo principal del histórico de un corredor, para la tabla del grupo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupRow {
    /// Todas sus carreras que pasan el filtro: IR medio, tasa de error, pérdida media…
    pub stats: HistoryStats,
    /// Su tipo de error más común (P9); `None` si no tiene errores con tipo.
    pub top_error: Option<TopError>,
    /// Su cubo de duración con más tasa de error (P7) entre los que tienen al menos
    /// [`MIN_BUCKET_LEGS`] tramos; a igual tasa, el más corto.
    pub weakest_leg_length: Option<LegLengthStats>,
    /// Subida, llano y bajada (P13).
    pub slope: Vec<SlopeStats>,
}

/// La fila de un corredor a partir de su histórico, con el mismo filtro en todo.
pub fn group_row(
    history: &History,
    by_leg_length: &[LegLengthStats],
    slope: &SlopeHistory,
    errors: &CommonErrors,
) -> GroupRow {
    let total = &errors.total;
    // `by_type` va de más a menos errores.
    let top_error = total.by_type.first().map(|t| TopError {
        error_type: t.error_type.clone(),
        errors: t.errors,
        share: if total.errors == 0 {
            0.0
        } else {
            t.errors as f64 / total.errors as f64
        },
    });
    let mut weakest: Option<&LegLengthStats> = None;
    for bucket in by_leg_length.iter().filter(|b| b.legs >= MIN_BUCKET_LEGS) {
        let worse = match (weakest.and_then(|w| w.error_rate), bucket.error_rate) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(current), Some(rate)) => rate > current,
        };
        if worse {
            weakest = Some(bucket);
        }
    }
    GroupRow {
        stats: history.total,
        top_error,
        weakest_leg_length: weakest.copied(),
        slope: slope.by_class.clone(),
    }
}

/// Totales de un grupo de corredores (#120): todas las carreras y todos los tramos de sus
/// miembros juntos, con las mismas definiciones que el total del histórico de uno
/// ([`HistoryStats`]). Los de cada miembro se suman ponderados: el IR medio por sus carreras y
/// las tasas y pérdidas por sus tramos. La consistencia no se junta (`None`): la de cada uno es
/// ya una media de las carreras que la tienen, y no se sabe cuántas son.
pub fn group_total(members: &[HistoryStats]) -> HistoryStats {
    let mut races = 0;
    let mut legs = 0;
    let mut errors = 0;
    let mut performance_sum = 0.0;
    let mut loss_sum_s = 0.0;
    let mut loss_sum_pct = 0.0;
    for m in members {
        races += m.races;
        legs += m.legs;
        errors += m.errors;
        performance_sum += m.mean_performance.unwrap_or(0.0) * m.races as f64;
        loss_sum_s += m.mean_loss_s.unwrap_or(0.0) * m.legs as f64;
        loss_sum_pct += m.mean_loss_pct.unwrap_or(0.0) * m.legs as f64;
    }
    let per_race = |sum: f64| (races > 0).then(|| sum / races as f64);
    let per_leg = |sum: f64| (legs > 0).then(|| sum / legs as f64);
    HistoryStats {
        races,
        legs,
        errors,
        mean_performance: per_race(performance_sum),
        error_rate: per_leg(errors as f64),
        mean_loss_s: per_leg(loss_sum_s),
        mean_loss_pct: per_leg(loss_sum_pct),
        mean_consistency: None,
    }
}

/// Una carrera de un corredor del grupo, con sus números.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupEntry {
    /// Índice del corredor en el grupo.
    pub runner: usize,
    /// El mismo para todos los que corrieron la carrera (`crate::package::race_id`).
    pub race_id: String,
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
    /// Sus números en la carrera (`crate::history::race_stats`); `None` sin rendimiento
    /// habitual: no cuenta.
    pub stats: Option<HistoryStats>,
}

/// Un corredor en una carrera compartida.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedResult {
    pub runner: usize,
    pub stats: HistoryStats,
}

/// Una carrera que han corrido al menos dos corredores del grupo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedRace {
    pub race_id: String,
    pub date: NaiveDate,
    pub name: Option<String>,
    pub format: Option<RaceFormat>,
    /// De más a menos IR (a igual IR, por orden en el grupo).
    pub results: Vec<SharedResult>,
}

/// Un corredor frente a otro en las carreras que comparten.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeadToHead {
    pub runner: usize,
    pub other: usize,
    /// Carreras que han corrido los dos.
    pub races: usize,
    /// En cuántas tuvo `runner` más IR que `other`, y en cuántas menos (los empates, en
    /// ninguna).
    pub better: usize,
    pub worse: usize,
    /// Media de `IR(runner) − IR(other)` en esas carreras (1 = 100 puntos).
    pub mean_difference: f64,
}

/// Todos contra todos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupComparison {
    /// De la más reciente a la más antigua.
    pub shared_races: Vec<SharedRace>,
    /// Cada par ordenado de corredores con alguna carrera en común, por `runner` y `other`.
    pub head_to_head: Vec<HeadToHead>,
}

/// Cruza las carreras de los corredores por `race_id`. Solo cuentan las que tienen números; una
/// carrera es compartida si la tienen al menos dos corredores. Si un corredor aparece dos veces
/// en la misma carrera, cuenta la primera.
pub fn compare(entries: &[GroupEntry]) -> GroupComparison {
    let mut shared: Vec<SharedRace> = Vec::new();
    for entry in entries {
        let Some(stats) = entry.stats.filter(|s| s.mean_performance.is_some()) else {
            continue;
        };
        let result = SharedResult {
            runner: entry.runner,
            stats,
        };
        match shared.iter_mut().find(|r| r.race_id == entry.race_id) {
            Some(race) if race.results.iter().any(|r| r.runner == entry.runner) => {}
            Some(race) => race.results.push(result),
            None => shared.push(SharedRace {
                race_id: entry.race_id.clone(),
                date: entry.date,
                name: entry.name.clone(),
                format: entry.format,
                results: vec![result],
            }),
        }
    }
    shared.retain(|race| race.results.len() >= 2);
    for race in &mut shared {
        race.results.sort_by(|a, b| {
            let ir = |r: &SharedResult| r.stats.mean_performance.unwrap_or(f64::NEG_INFINITY);
            ir(b).total_cmp(&ir(a)).then(a.runner.cmp(&b.runner))
        });
    }
    shared.sort_by(|a, b| b.date.cmp(&a.date).then(a.race_id.cmp(&b.race_id)));

    let mut pairs: Vec<(HeadToHead, f64)> = Vec::new();
    for race in &shared {
        for a in &race.results {
            for b in race.results.iter().filter(|b| b.runner != a.runner) {
                let (Some(ir_a), Some(ir_b)) = (a.stats.mean_performance, b.stats.mean_performance)
                else {
                    continue;
                };
                let index = match pairs
                    .iter()
                    .position(|(p, _)| p.runner == a.runner && p.other == b.runner)
                {
                    Some(i) => i,
                    None => {
                        pairs.push((
                            HeadToHead {
                                runner: a.runner,
                                other: b.runner,
                                races: 0,
                                better: 0,
                                worse: 0,
                                mean_difference: 0.0,
                            },
                            0.0,
                        ));
                        pairs.len() - 1
                    }
                };
                let (pair, sum) = &mut pairs[index];
                pair.races += 1;
                if ir_a > ir_b {
                    pair.better += 1;
                } else if ir_a < ir_b {
                    pair.worse += 1;
                }
                *sum += ir_a - ir_b;
            }
        }
    }
    let mut head_to_head: Vec<HeadToHead> = pairs
        .into_iter()
        .map(|(mut pair, sum)| {
            pair.mean_difference = sum / pair.races as f64;
            pair
        })
        .collect();
    head_to_head.sort_by_key(|p| (p.runner, p.other));
    GroupComparison {
        shared_races: shared,
        head_to_head,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common_errors::{ErrorTypes, TypeCount};
    use crate::history::HistoryFilter;
    use crate::slope::{SlopeClass, SlopeConfig};

    /// Un atleta sintético con sus totales: carreras, tramos, errores, IR medio y pérdida media
    /// por tramo en segundos y en %.
    fn athlete(
        races: usize,
        legs: usize,
        errors: usize,
        ir: f64,
        s: f64,
        pct: f64,
    ) -> HistoryStats {
        HistoryStats {
            races,
            legs,
            errors,
            mean_performance: Some(ir),
            error_rate: Some(errors as f64 / legs as f64),
            mean_loss_s: Some(s),
            mean_loss_pct: Some(pct),
            mean_consistency: Some(0.05),
        }
    }

    fn close_to(actual: Option<f64>, expected: f64) {
        let actual = actual.unwrap();
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    /// Tres atletas sintéticos en dos grupos: A = {0, 1} y B = {1, 2} (1 está en los dos).
    #[test]
    fn group_totals_pool_the_races_and_legs_of_their_members() {
        let athletes = [
            athlete(2, 20, 4, 0.90, 6.0, 5.0),
            athlete(3, 30, 3, 0.80, 3.0, 2.0),
            athlete(1, 10, 5, 0.70, 12.0, 10.0),
        ];

        // A: IR (0,90 × 2 + 0,80 × 3) / 5 = 0,84; errores 7 / 50; pérdida (6 × 20 + 3 × 30) / 50.
        let a = group_total(&[athletes[0], athletes[1]]);
        assert_eq!((a.races, a.legs, a.errors), (5, 50, 7));
        close_to(a.mean_performance, 0.84);
        close_to(a.error_rate, 0.14);
        close_to(a.mean_loss_s, 4.2);
        close_to(a.mean_loss_pct, 3.2);
        assert_eq!(a.mean_consistency, None);

        // B: IR (0,80 × 3 + 0,70) / 4 = 0,775; errores 8 / 40; pérdida (3 × 30 + 12 × 10) / 40.
        let b = group_total(&[athletes[1], athletes[2]]);
        assert_eq!((b.races, b.legs, b.errors), (4, 40, 8));
        close_to(b.mean_performance, 0.775);
        close_to(b.error_rate, 0.2);
        close_to(b.mean_loss_s, 5.25);
        close_to(b.mean_loss_pct, 4.0);

        // Uno sin carreras no cambia nada; un grupo sin nadie no tiene medias.
        let with_empty = group_total(&[athletes[0], athletes[1], HistoryStats::default()]);
        assert_eq!(with_empty, a);
        let empty = group_total(&[]);
        assert_eq!(
            (empty.races, empty.mean_performance, empty.error_rate),
            (0, None, None)
        );
    }

    fn stats(ir: Option<f64>) -> Option<HistoryStats> {
        Some(HistoryStats {
            races: 1,
            legs: 10,
            mean_performance: ir,
            ..HistoryStats::default()
        })
    }

    fn entry(runner: usize, race: &str, day: u32, ir: Option<f64>) -> GroupEntry {
        GroupEntry {
            runner,
            race_id: race.into(),
            date: NaiveDate::from_ymd_opt(2026, 10, day).unwrap(),
            name: Some(format!("Carrera {race}")),
            format: None,
            stats: stats(ir),
        }
    }

    /// Tres corredores sintéticos (0, 1 y 2):
    /// - R1 (día 1): los tres, con IR 0,90, 0,80 y 0,85.
    /// - R2 (día 2): 0 y 1, con 0,70 y 0,75.
    /// - R3 (día 3): solo 2 (no es compartida).
    /// - R4 (día 4): 0 con IR y 1 sin rendimiento habitual (no es compartida).
    /// - R5 (día 5): 1 y 2 empatan a 0,80.
    fn three_runners() -> Vec<GroupEntry> {
        vec![
            entry(0, "R1", 1, Some(0.90)),
            entry(1, "R1", 1, Some(0.80)),
            entry(2, "R1", 1, Some(0.85)),
            entry(0, "R2", 2, Some(0.70)),
            entry(1, "R2", 2, Some(0.75)),
            entry(2, "R3", 3, Some(0.95)),
            entry(0, "R4", 4, Some(0.90)),
            entry(1, "R4", 4, None),
            entry(1, "R5", 5, Some(0.80)),
            entry(2, "R5", 5, Some(0.80)),
            // Repetido: cuenta el primero.
            entry(2, "R5", 5, Some(0.10)),
        ]
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn shared_races_are_the_ones_with_two_runners_with_numbers() {
        let comparison = compare(&three_runners());
        let races: Vec<(&str, Vec<usize>)> = comparison
            .shared_races
            .iter()
            .map(|r| {
                (
                    r.race_id.as_str(),
                    r.results.iter().map(|x| x.runner).collect(),
                )
            })
            .collect();
        // De la más reciente a la más antigua; dentro, de más a menos IR (empate: por orden).
        assert_eq!(
            races,
            [
                ("R5", vec![1, 2]),
                ("R2", vec![1, 0]),
                ("R1", vec![0, 2, 1])
            ]
        );
    }

    #[test]
    fn head_to_head_counts_wins_losses_and_the_mean_difference() {
        let comparison = compare(&three_runners());
        let pairs: Vec<(usize, usize, usize, usize, usize)> = comparison
            .head_to_head
            .iter()
            .map(|p| (p.runner, p.other, p.races, p.better, p.worse))
            .collect();
        assert_eq!(
            pairs,
            [
                // 0 frente a 1: R1 (0,90 > 0,80) y R2 (0,70 < 0,75).
                (0, 1, 2, 1, 1),
                // 0 frente a 2: solo R1 (0,90 > 0,85).
                (0, 2, 1, 1, 0),
                (1, 0, 2, 1, 1),
                // 1 frente a 2: R1 (0,80 < 0,85) y R5 (empate).
                (1, 2, 2, 0, 1),
                (2, 0, 1, 0, 1),
                (2, 1, 2, 1, 0),
            ]
        );
        let diff = |runner: usize, other: usize| {
            comparison
                .head_to_head
                .iter()
                .find(|p| p.runner == runner && p.other == other)
                .unwrap()
                .mean_difference
        };
        // ((0,90 − 0,80) + (0,70 − 0,75)) / 2 = 0,025.
        assert!(close(diff(0, 1), 0.025));
        assert!(close(diff(1, 0), -0.025));
        // ((0,80 − 0,85) + 0) / 2 = −0,025.
        assert!(close(diff(1, 2), -0.025));
        assert!(close(diff(0, 2), 0.05));
    }

    #[test]
    fn no_shared_races_no_comparison() {
        let comparison = compare(&[entry(0, "R1", 1, Some(0.9)), entry(1, "R2", 1, Some(0.8))]);
        assert!(comparison.shared_races.is_empty());
        assert!(comparison.head_to_head.is_empty());
    }

    fn bucket(from_s: f64, legs: usize, errors: usize) -> LegLengthStats {
        LegLengthStats {
            from_s,
            to_s: None,
            legs,
            errors,
            error_rate: (legs > 0).then(|| errors as f64 / legs as f64),
            mean_loss_s: None,
            mean_loss_pct: None,
        }
    }

    fn type_count(key: &str, errors: usize) -> TypeCount {
        TypeCount {
            error_type: key.into(),
            errors,
            loss_s: 0.0,
            subtypes: Vec::new(),
        }
    }

    #[test]
    fn the_row_takes_the_top_error_and_the_weakest_bucket_with_enough_legs() {
        let total = HistoryStats {
            races: 4,
            legs: 60,
            errors: 6,
            mean_performance: Some(0.88),
            error_rate: Some(0.1),
            ..HistoryStats::default()
        };
        let history = History {
            filter: HistoryFilter::default(),
            by_format: Vec::new(),
            total,
            races_without_data: 0,
        };
        let by_leg_length = [
            // 3 de 5: mucha tasa pero pocos tramos, no cuenta.
            bucket(0.0, 5, 3),
            bucket(30.0, 20, 2),
            bucket(60.0, 20, 4),
            // Igual tasa que el anterior: gana el más corto.
            bucket(120.0, 10, 2),
            bucket(240.0, 0, 0),
        ];
        let slope = SlopeHistory {
            config: SlopeConfig::default(),
            by_class: vec![SlopeStats {
                class: SlopeClass::Uphill,
                legs: 3,
                errors: 1,
                error_rate: Some(1.0 / 3.0),
                mean_performance: Some(0.8),
                reference_s: 300.0,
            }],
            races_with_track: 1,
            races_without_track: 0,
            legs_without_track: 0,
            unclassified_legs: 0,
        };
        let errors = CommonErrors {
            total: ErrorTypes {
                legs: 60,
                errors: 6,
                loss_s: 0.0,
                untyped: 1,
                untyped_loss_s: 0.0,
                unreviewed: 1,
                by_type: vec![type_count("navigation", 3), type_count("attack", 2)],
                physical_legs: 0,
                physical_loss_s: 0.0,
            },
            by_leg_length: Vec::new(),
            by_format: Vec::new(),
        };

        let row = group_row(&history, &by_leg_length, &slope, &errors);
        assert_eq!(row.stats, total);
        assert_eq!(
            row.top_error,
            Some(TopError {
                error_type: "navigation".into(),
                errors: 3,
                share: 0.5
            })
        );
        assert_eq!(row.weakest_leg_length.map(|b| b.from_s), Some(60.0));
        assert_eq!(row.slope, slope.by_class);

        // Sin errores con tipo ni cubos con bastantes tramos.
        let empty = CommonErrors {
            total: ErrorTypes {
                by_type: Vec::new(),
                ..errors.total.clone()
            },
            ..errors.clone()
        };
        let row = group_row(&history, &by_leg_length[..1], &slope, &empty);
        assert_eq!((row.top_error, row.weakest_leg_length), (None, None));
    }
}
