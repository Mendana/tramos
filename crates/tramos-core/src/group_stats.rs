//! Estadísticas de un grupo de atletas (#145): los análisis de Estadísticas con todos los
//! miembros del grupo juntos. Cada miembro llega con su histórico ya calculado con sus umbrales
//! y se suma con su peso, como en [`crate::group_compare::pool_side`]: los recuentos se suman y
//! cada media se pondera por el número de casos de la que sale. Así cada análisis sale igual que
//! si todas las carreras fueran de un solo corredor.
//!
//! Solo se juntan los análisis que se pueden juntar **sin aproximar**. La consistencia no: la de
//! cada miembro es ya una media de las carreras que la tienen, y no se sabe cuántas son. Las
//! definiciones están en `docs/historico.md`, "Estadísticas de un grupo (#145)".

use serde::{Deserialize, Serialize};

use crate::after_error::{AfterError, Rate, StreakBucket};
use crate::common_errors::{
    CommonErrors, ErrorTypes, FormatErrorTypes, LengthErrorTypes, SubtypeCount, TypeCount,
    sort_types,
};
use crate::days_off::{DaysOff, DaysOffStats};
use crate::fatigue::{Drift, Effort, Fatigue, HeartRateBefore, ThirdFatigue};
use crate::group::group_total;
use crate::history::{FORMATS, FormatHistory, History, HistoryFilter, HistoryStats};
use crate::leg_length::LegLengthStats;
use crate::loss_breakdown::BreakdownHistory;
use crate::slope::{SlopeConfig, SlopeHistory, SlopeStats};

/// Los análisis del histórico de un miembro, con el filtro del grupo y sus umbrales.
#[derive(Debug, Clone, Copy)]
pub struct MemberAnalyses<'a> {
    pub history: &'a History,
    /// P7.
    pub by_leg_length: &'a [LegLengthStats],
    /// P13.
    pub by_slope: &'a SlopeHistory,
    /// P2.
    pub loss_breakdown: &'a BreakdownHistory,
    /// P8.
    pub after_error: &'a AfterError,
    /// P9.
    pub common_errors: &'a CommonErrors,
    /// P14.
    pub fatigue: &'a Fatigue,
    /// P11.
    pub days_off: &'a DaysOff,
}

/// Los análisis de Estadísticas con todos los miembros de un grupo juntos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupStats {
    /// Miembros con alguna carrera que cuenta.
    pub runners: usize,
    /// Por formato y total ([`group_total`]); la consistencia, a `None`.
    pub history: History,
    pub by_leg_length: Vec<LegLengthStats>,
    pub by_slope: SlopeHistory,
    pub loss_breakdown: BreakdownHistory,
    pub after_error: AfterError,
    pub common_errors: CommonErrors,
    pub fatigue: Fatigue,
    pub days_off: DaysOff,
}

/// Junta los análisis de los miembros de un grupo, todos con el mismo `filter`.
pub fn group_stats(filter: &HistoryFilter, members: &[MemberAnalyses<'_>]) -> GroupStats {
    let histories: Vec<&History> = members.iter().map(|m| m.history).collect();
    let leg_lengths: Vec<&[LegLengthStats]> = members.iter().map(|m| m.by_leg_length).collect();
    let slopes: Vec<&SlopeHistory> = members.iter().map(|m| m.by_slope).collect();
    let breakdowns: Vec<&BreakdownHistory> = members.iter().map(|m| m.loss_breakdown).collect();
    let after_errors: Vec<&AfterError> = members.iter().map(|m| m.after_error).collect();
    let common: Vec<&CommonErrors> = members.iter().map(|m| m.common_errors).collect();
    let fatigues: Vec<&Fatigue> = members.iter().map(|m| m.fatigue).collect();
    let days_off: Vec<&DaysOff> = members.iter().map(|m| m.days_off).collect();
    GroupStats {
        runners: members.iter().filter(|m| m.history.total.races > 0).count(),
        history: pool_history(filter, &histories),
        by_leg_length: pool_leg_length(&leg_lengths),
        by_slope: pool_slope(&slopes),
        loss_breakdown: pool_breakdown(&breakdowns),
        after_error: pool_after_error(&after_errors),
        common_errors: pool_common_errors(&common),
        fatigue: pool_fatigue(&fatigues),
        days_off: pool_days_off(&days_off),
    }
}

/// Agrupa por clave, en el orden en que aparece cada una.
fn by_key<'a, T, K: PartialEq>(
    items: impl IntoIterator<Item = &'a T>,
    key: impl Fn(&T) -> K,
) -> Vec<Vec<&'a T>>
where
    T: 'a,
{
    let mut groups: Vec<(K, Vec<&'a T>)> = Vec::new();
    for item in items {
        let k = key(item);
        match groups.iter_mut().find(|(g, _)| *g == k) {
            Some((_, list)) => list.push(item),
            None => groups.push((k, vec![item])),
        }
    }
    groups.into_iter().map(|(_, list)| list).collect()
}

/// Suma de una media ponderada por su número de casos.
#[derive(Debug, Default, Clone, Copy)]
struct Weighted {
    n: usize,
    sum: f64,
}

impl Weighted {
    /// Una media `mean` de `n` casos (`None` solo con `n = 0`).
    fn add(&mut self, mean: Option<f64>, n: usize) {
        self.n += n;
        self.sum += mean.unwrap_or(0.0) * n as f64;
    }

    fn mean(&self) -> Option<f64> {
        (self.n > 0).then(|| self.sum / self.n as f64)
    }
}

fn rate(errors: usize, legs: usize) -> Option<f64> {
    (legs > 0).then(|| errors as f64 / legs as f64)
}

/// Por formato y total, con [`group_total`]: el IR medio ponderado por carreras, y la tasa de
/// error y la pérdida media por tramos. Sprint, media y larga siempre (o solo el del filtro) y,
/// al final, sin formato si algún miembro tiene carreras sin formato.
pub fn pool_history(filter: &HistoryFilter, members: &[&History]) -> History {
    let without_format = members.iter().any(|h| {
        h.by_format
            .iter()
            .any(|g| g.format.is_none() && g.stats.races > 0)
    });
    let formats = FORMATS
        .iter()
        .filter(|f| filter.format.is_none_or(|g| g == **f))
        .map(|f| Some(*f))
        .chain(without_format.then_some(None));
    let by_format = formats
        .map(|format| {
            let stats: Vec<HistoryStats> = members
                .iter()
                .flat_map(|h| &h.by_format)
                .filter(|g| g.format == format)
                .map(|g| g.stats)
                .collect();
            FormatHistory {
                format,
                stats: group_total(&stats),
            }
        })
        .collect();
    let totals: Vec<HistoryStats> = members.iter().map(|h| h.total).collect();
    History {
        filter: *filter,
        by_format,
        total: group_total(&totals),
        races_without_data: members.iter().map(|h| h.races_without_data).sum(),
    }
}

/// P7: los mismos cubos de todos; en cada uno se suman tramos y errores, y la pérdida media se
/// pondera por tramos.
pub fn pool_leg_length(members: &[&[LegLengthStats]]) -> Vec<LegLengthStats> {
    by_key(members.iter().flat_map(|m| m.iter()), |b| b.from_s)
        .into_iter()
        .filter_map(|same| {
            let first = **same.first()?;
            let (mut legs, mut errors) = (0, 0);
            let (mut loss_s, mut loss_pct) = (Weighted::default(), Weighted::default());
            for b in &same {
                legs += b.legs;
                errors += b.errors;
                loss_s.add(b.mean_loss_s, b.legs);
                loss_pct.add(b.mean_loss_pct, b.legs);
            }
            Some(LegLengthStats {
                legs,
                errors,
                error_rate: rate(errors, legs),
                mean_loss_s: loss_s.mean(),
                mean_loss_pct: loss_pct.mean(),
                ..first
            })
        })
        .collect()
}

/// P13 por clase: se suman tramos y errores, y el IR medio se pondera por `reference_s`
/// (`Σ ref_i`): sale igual que si todos los tramos fueran de un solo corredor.
pub fn pool_slope_classes(members: &[&[SlopeStats]]) -> Vec<SlopeStats> {
    by_key(members.iter().flat_map(|m| m.iter()), |c| c.class)
        .into_iter()
        .filter_map(|same| {
            let first = **same.first()?;
            let (mut legs, mut errors, mut reference_s, mut weighted) = (0, 0, 0.0, 0.0);
            for c in &same {
                legs += c.legs;
                errors += c.errors;
                reference_s += c.reference_s;
                weighted += c.mean_performance.unwrap_or(0.0) * c.reference_s;
            }
            Some(SlopeStats {
                class: first.class,
                legs,
                errors,
                error_rate: rate(errors, legs),
                mean_performance: (reference_s > 0.0).then(|| weighted / reference_s),
                reference_s,
            })
        })
        .collect()
}

/// P13 entero: las clases ([`pool_slope_classes`]) y los recuentos de carreras y tramos sumados.
pub fn pool_slope(members: &[&SlopeHistory]) -> SlopeHistory {
    let classes: Vec<&[SlopeStats]> = members.iter().map(|s| s.by_class.as_slice()).collect();
    SlopeHistory {
        config: members
            .first()
            .map_or_else(SlopeConfig::default, |s| s.config),
        by_class: pool_slope_classes(&classes),
        races_with_track: members.iter().map(|s| s.races_with_track).sum(),
        races_without_track: members.iter().map(|s| s.races_without_track).sum(),
        legs_without_track: members.iter().map(|s| s.legs_without_track).sum(),
        unclassified_legs: members.iter().map(|s| s.unclassified_legs).sum(),
    }
}

/// P2: los errores repartidos y los recuentos, sumados.
pub fn pool_breakdown(members: &[&BreakdownHistory]) -> BreakdownHistory {
    let mut out = BreakdownHistory::default();
    for m in members {
        out.errors.merge(&m.errors);
        out.races_with_track += m.races_with_track;
        out.races_without_track += m.races_without_track;
        out.errors_without_breakdown += m.errors_without_breakdown;
    }
    out
}

fn pool_rate(rates: impl IntoIterator<Item = Rate>) -> Rate {
    let (mut legs, mut errors) = (0, 0);
    for r in rates {
        legs += r.legs;
        errors += r.errors;
    }
    Rate {
        legs,
        errors,
        error_rate: rate(errors, legs),
    }
}

/// P8: en cada grupo de tramos (tras un error, tras un limpio, acelerando o no, y cada cubo de
/// rachas) se suman tramos y errores.
pub fn pool_after_error(members: &[&AfterError]) -> AfterError {
    let pooled = |get: fn(&AfterError) -> Rate| pool_rate(members.iter().map(|m| get(m)));
    AfterError {
        after_error: pooled(|m| m.after_error),
        after_clean: pooled(|m| m.after_clean),
        accelerated: pooled(|m| m.accelerated),
        not_accelerated: pooled(|m| m.not_accelerated),
        after_error_without_speed: members.iter().map(|m| m.after_error_without_speed).sum(),
        streaks: by_key(members.iter().flat_map(|m| &m.streaks), |s| s.from)
            .into_iter()
            .filter_map(|same| {
                let first = **same.first()?;
                Some(StreakBucket {
                    rate: pool_rate(same.iter().map(|s| s.rate)),
                    ..first
                })
            })
            .collect(),
    }
}

/// P9: un reparto de errores. Todo son recuentos y sumas de pérdida: se suman, también por tipo
/// y por subtipo, y se ordenan como los de un corredor.
pub fn pool_error_types(members: &[&ErrorTypes]) -> ErrorTypes {
    let mut by_type: Vec<TypeCount> = by_key(members.iter().flat_map(|m| &m.by_type), |t| {
        t.error_type.clone()
    })
    .into_iter()
    .filter_map(|same| {
        let first = same.first()?;
        let subtypes = by_key(same.iter().copied().flat_map(|t| &t.subtypes), |s| {
            s.subtype.clone()
        })
        .into_iter()
        .filter_map(|subs| {
            Some(SubtypeCount {
                subtype: subs.first()?.subtype.clone(),
                errors: subs.iter().map(|s| s.errors).sum(),
                loss_s: subs.iter().map(|s| s.loss_s).sum(),
            })
        })
        .collect();
        Some(TypeCount {
            error_type: first.error_type.clone(),
            errors: same.iter().map(|t| t.errors).sum(),
            loss_s: same.iter().map(|t| t.loss_s).sum(),
            subtypes,
        })
    })
    .collect();
    sort_types(&mut by_type);
    ErrorTypes {
        legs: members.iter().map(|m| m.legs).sum(),
        errors: members.iter().map(|m| m.errors).sum(),
        loss_s: members.iter().map(|m| m.loss_s).sum(),
        untyped: members.iter().map(|m| m.untyped).sum(),
        untyped_loss_s: members.iter().map(|m| m.untyped_loss_s).sum(),
        unreviewed: members.iter().map(|m| m.unreviewed).sum(),
        by_type,
        physical_legs: members.iter().map(|m| m.physical_legs).sum(),
        physical_loss_s: members.iter().map(|m| m.physical_loss_s).sum(),
    }
}

/// P9 por cubo de duración: cada cubo, con [`pool_error_types`].
fn pool_length_error_types(members: &[&[LengthErrorTypes]]) -> Vec<LengthErrorTypes> {
    by_key(members.iter().flat_map(|m| m.iter()), |b| b.from_s)
        .into_iter()
        .filter_map(|same| {
            let first = same.first()?;
            let types: Vec<&ErrorTypes> = same.iter().map(|b| &b.types).collect();
            Some(LengthErrorTypes {
                from_s: first.from_s,
                to_s: first.to_s,
                types: pool_error_types(&types),
            })
        })
        .collect()
}

/// P9 entero: el total, por cubo, por formato y por formato y cubo.
pub fn pool_common_errors(members: &[&CommonErrors]) -> CommonErrors {
    let totals: Vec<&ErrorTypes> = members.iter().map(|m| &m.total).collect();
    let lengths: Vec<&[LengthErrorTypes]> =
        members.iter().map(|m| m.by_leg_length.as_slice()).collect();
    CommonErrors {
        total: pool_error_types(&totals),
        by_leg_length: pool_length_error_types(&lengths),
        by_format: by_key(members.iter().flat_map(|m| &m.by_format), |f| f.format)
            .into_iter()
            .filter_map(|same| {
                let format = same.first()?.format;
                let totals: Vec<&ErrorTypes> = same.iter().map(|f| &f.total).collect();
                let lengths: Vec<&[LengthErrorTypes]> =
                    same.iter().map(|f| f.by_leg_length.as_slice()).collect();
                Some(FormatErrorTypes {
                    format,
                    total: pool_error_types(&totals),
                    by_leg_length: pool_length_error_types(&lengths),
                })
            })
            .collect(),
    }
}

/// P11: cada miembro, con sus días desde su carrera anterior. En cada cubo se suman carreras,
/// tramos y errores; el IR de entrada en mapa se pondera por `first_legs`.
pub fn pool_days_off(members: &[&DaysOff]) -> DaysOff {
    DaysOff {
        buckets: by_key(members.iter().flat_map(|m| &m.buckets), |b| b.from_days)
            .into_iter()
            .filter_map(|same| {
                let first = **same.first()?;
                let mut performance = Weighted::default();
                let (mut races, mut legs, mut errors) = (0, 0, 0);
                for b in &same {
                    races += b.races;
                    performance.add(b.first_legs_performance, b.first_legs);
                    legs += b.first_third_legs;
                    errors += b.first_third_errors;
                }
                Some(DaysOffStats {
                    races,
                    first_legs: performance.n,
                    first_legs_performance: performance.mean(),
                    first_third_legs: legs,
                    first_third_errors: errors,
                    first_third_error_rate: rate(errors, legs),
                    ..first
                })
            })
            .collect(),
        without_previous: members.iter().map(|m| m.without_previous).sum(),
    }
}

fn pool_heart_rate_before(values: &[HeartRateBefore]) -> HeartRateBefore {
    let (mut bpm, mut relative) = (Weighted::default(), Weighted::default());
    for v in values {
        bpm.add(v.mean_heart_rate_bpm, v.legs);
        relative.add(v.mean_relative_bpm, v.legs);
    }
    HeartRateBefore {
        legs: bpm.n,
        mean_heart_rate_bpm: bpm.mean(),
        mean_relative_bpm: relative.mean(),
    }
}

fn pool_effort(values: &[Effort]) -> Effort {
    let mut effort = Weighted::default();
    for v in values {
        effort.add(v.mean_effort, v.legs);
    }
    Effort {
        legs: effort.n,
        mean_effort: effort.mean(),
    }
}

/// P14: por tercio, cada media (ya relativa a la carrera de cada uno) ponderada por sus tramos,
/// y los recuentos de carreras y errores sumados.
pub fn pool_fatigue(members: &[&Fatigue]) -> Fatigue {
    let by_third = by_key(members.iter().flat_map(|m| &m.by_third), |t| t.third)
        .into_iter()
        .filter_map(|same| {
            let third = same.first()?.third;
            let mut bpm = Weighted::default();
            let mut speed = Weighted::default();
            let mut ratio = Weighted::default();
            for t in &same {
                bpm.add(t.drift.mean_heart_rate_bpm, t.drift.legs);
                speed.add(t.drift.mean_speed_mps, t.drift.legs);
                ratio.add(t.drift.mean_relative_ratio, t.drift.legs);
            }
            let each = |get: fn(&ThirdFatigue) -> HeartRateBefore| -> Vec<HeartRateBefore> {
                same.iter().map(|t| get(t)).collect()
            };
            let efforts = |get: fn(&ThirdFatigue) -> Effort| -> Vec<Effort> {
                same.iter().map(|t| get(t)).collect()
            };
            Some(ThirdFatigue {
                third,
                drift: Drift {
                    legs: ratio.n,
                    mean_heart_rate_bpm: bpm.mean(),
                    mean_speed_mps: speed.mean(),
                    mean_relative_ratio: ratio.mean(),
                },
                before_error: pool_heart_rate_before(&each(|t| t.before_error)),
                before_clean: pool_heart_rate_before(&each(|t| t.before_clean)),
                effort_error: pool_effort(&efforts(|t| t.effort_error)),
                effort_clean: pool_effort(&efforts(|t| t.effort_clean)),
                effort_physical: pool_effort(&efforts(|t| t.effort_physical)),
            })
        })
        .collect();
    Fatigue {
        by_third,
        races_with_heart_rate: members.iter().map(|m| m.races_with_heart_rate).sum(),
        races_without_heart_rate: members.iter().map(|m| m.races_without_heart_rate).sum(),
        errors_without_heart_rate: members.iter().map(|m| m.errors_without_heart_rate).sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loss_breakdown::BreakdownTotals;
    use crate::race_format::RaceFormat;
    use crate::slope::SlopeClass;

    fn close(actual: Option<f64>, expected: f64) {
        let actual = actual.unwrap();
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    /// (carreras, tramos, errores, IR, pérdida s, pérdida %).
    fn stats(races: usize, legs: usize, errors: usize, ir: f64, s: f64, pct: f64) -> HistoryStats {
        HistoryStats {
            races,
            legs,
            errors,
            mean_performance: Some(ir),
            error_rate: rate(errors, legs),
            mean_loss_s: Some(s),
            mean_loss_pct: Some(pct),
            mean_consistency: Some(0.1),
        }
    }

    fn history(by_format: &[(Option<RaceFormat>, HistoryStats)], total: HistoryStats) -> History {
        History {
            filter: HistoryFilter::default(),
            by_format: by_format
                .iter()
                .map(|(format, stats)| FormatHistory {
                    format: *format,
                    stats: *stats,
                })
                .collect(),
            total,
            races_without_data: 1,
        }
    }

    const SPRINT: Option<RaceFormat> = Some(RaceFormat::Sprint);
    const MIDDLE: Option<RaceFormat> = Some(RaceFormat::Middle);
    const LONG: Option<RaceFormat> = Some(RaceFormat::Long);

    /// Ana: 2 sprints. Bea: 1 sprint, 2 medias y 1 sin formato.
    fn two_histories() -> (History, History) {
        let empty = HistoryStats::default();
        let ana = history(
            &[
                (SPRINT, stats(2, 20, 4, 0.9, 6.0, 5.0)),
                (MIDDLE, empty),
                (LONG, empty),
            ],
            stats(2, 20, 4, 0.9, 6.0, 5.0),
        );
        let bea = history(
            &[
                (SPRINT, stats(1, 10, 1, 0.8, 3.0, 2.0)),
                (MIDDLE, stats(2, 30, 6, 1.0, 4.0, 3.0)),
                (LONG, empty),
                (None, stats(1, 5, 2, 0.7, 10.0, 8.0)),
            ],
            // (0,8 + 2 + 0,7) / 4; (30 + 120 + 50) / 45 s; (20 + 90 + 40) / 45 %.
            stats(4, 45, 9, 0.875, 200.0 / 45.0, 150.0 / 45.0),
        );
        (ana, bea)
    }

    #[test]
    fn formats_and_total_pool_races_and_legs_with_their_weights() {
        let (ana, bea) = two_histories();
        let pooled = pool_history(&HistoryFilter::default(), &[&ana, &bea]);
        let formats: Vec<Option<RaceFormat>> = pooled.by_format.iter().map(|g| g.format).collect();
        assert_eq!(formats, [SPRINT, MIDDLE, LONG, None]);

        // Sprint: 3 carreras, IR (0,9 × 2 + 0,8) / 3; 30 tramos, 5 errores; pérdida
        // (6 × 20 + 3 × 10) / 30 = 5 s y (5 × 20 + 2 × 10) / 30 = 4 %.
        let sprint = pooled.by_format[0].stats;
        assert_eq!((sprint.races, sprint.legs, sprint.errors), (3, 30, 5));
        close(sprint.mean_performance, 2.6 / 3.0);
        close(sprint.error_rate, 5.0 / 30.0);
        close(sprint.mean_loss_s, 5.0);
        close(sprint.mean_loss_pct, 4.0);
        // Media: solo Bea. Larga: nadie. Sin formato: Bea.
        close(pooled.by_format[1].stats.mean_performance, 1.0);
        assert_eq!(pooled.by_format[2].stats, HistoryStats::default());
        assert_eq!(pooled.by_format[3].stats.races, 1);

        // Total: 6 carreras, IR (0,9 × 2 + 0,875 × 4) / 6 = 5,3 / 6; 65 tramos, 13 errores;
        // pérdida (6 × 20 + 200) / 65 s y (5 × 20 + 150) / 65 %.
        let total = pooled.total;
        assert_eq!((total.races, total.legs, total.errors), (6, 65, 13));
        close(total.mean_performance, 5.3 / 6.0);
        close(total.error_rate, 0.2);
        close(total.mean_loss_s, 320.0 / 65.0);
        close(total.mean_loss_pct, 250.0 / 65.0);
        assert_eq!(pooled.races_without_data, 2);
        // La consistencia no se junta.
        assert!(
            pooled
                .by_format
                .iter()
                .all(|g| g.stats.mean_consistency.is_none())
        );
        assert_eq!(total.mean_consistency, None);

        // Sin nadie con carreras sin formato no sale ese grupo; con filtro, solo su formato.
        let alone = pool_history(&HistoryFilter::default(), &[&ana]);
        assert_eq!(alone.by_format.len(), 3);
        let filter = HistoryFilter {
            format: MIDDLE,
            ..HistoryFilter::default()
        };
        let middle = pool_history(&filter, &[&ana, &bea]);
        assert_eq!(middle.by_format.len(), 2);
        assert_eq!(middle.by_format[0].format, MIDDLE);
        assert_eq!(middle.filter, filter);
    }

    fn bucket(from_s: f64, legs: usize, errors: usize, s: f64, pct: f64) -> LegLengthStats {
        LegLengthStats {
            from_s,
            to_s: Some(from_s * 2.0),
            legs,
            errors,
            error_rate: rate(errors, legs),
            mean_loss_s: (legs > 0).then_some(s),
            mean_loss_pct: (legs > 0).then_some(pct),
        }
    }

    #[test]
    fn leg_length_buckets_add_legs_and_weight_the_loss() {
        let ana = [bucket(20.0, 10, 2, 6.0, 5.0), bucket(30.0, 0, 0, 0.0, 0.0)];
        let bea = [bucket(20.0, 30, 3, 2.0, 1.0), bucket(30.0, 4, 1, 5.0, 3.0)];
        let pooled = pool_leg_length(&[&ana, &bea]);
        // 20–30 s: 40 tramos, 5 errores (12,5 %); (6 × 10 + 2 × 30) / 40 = 3 s y
        // (5 × 10 + 1 × 30) / 40 = 2 %.
        assert_eq!((pooled[0].legs, pooled[0].errors), (40, 5));
        close(pooled[0].error_rate, 0.125);
        close(pooled[0].mean_loss_s, 3.0);
        close(pooled[0].mean_loss_pct, 2.0);
        assert_eq!(pooled[0].to_s, Some(40.0));
        // 30–60 s: solo Bea.
        close(pooled[1].mean_loss_s, 5.0);
        close(pooled[1].error_rate, 0.25);
    }

    fn class(c: SlopeClass, legs: usize, errors: usize, ir: Option<f64>, r: f64) -> SlopeStats {
        SlopeStats {
            class: c,
            legs,
            errors,
            error_rate: rate(errors, legs),
            mean_performance: ir,
            reference_s: r,
        }
    }

    #[test]
    fn slope_weights_the_performance_by_reference_and_adds_the_counts() {
        let ana = SlopeHistory {
            config: SlopeConfig::default(),
            by_class: vec![
                class(SlopeClass::Uphill, 4, 1, Some(0.8), 400.0),
                class(SlopeClass::Flat, 0, 0, None, 0.0),
                class(SlopeClass::Downhill, 2, 1, Some(1.1), 100.0),
            ],
            races_with_track: 2,
            races_without_track: 1,
            legs_without_track: 12,
            unclassified_legs: 3,
        };
        let bea = SlopeHistory {
            config: SlopeConfig::default(),
            by_class: vec![
                class(SlopeClass::Uphill, 6, 3, Some(0.9), 100.0),
                class(SlopeClass::Flat, 2, 0, Some(1.0), 50.0),
                class(SlopeClass::Downhill, 0, 0, None, 0.0),
            ],
            races_with_track: 1,
            races_without_track: 0,
            legs_without_track: 0,
            unclassified_legs: 1,
        };
        let pooled = pool_slope(&[&ana, &bea]);
        // Subida: 10 tramos, 4 errores; IR (0,8 × 400 + 0,9 × 100) / 500 = 0,82.
        let up = pooled.by_class[0];
        assert_eq!((up.class, up.legs, up.errors), (SlopeClass::Uphill, 10, 4));
        close(up.error_rate, 0.4);
        close(up.mean_performance, 0.82);
        assert_eq!(up.reference_s, 500.0);
        // Llano, solo Bea; bajada, solo Ana.
        close(pooled.by_class[1].mean_performance, 1.0);
        close(pooled.by_class[2].mean_performance, 1.1);
        close(pooled.by_class[2].error_rate, 0.5);
        assert_eq!(
            (
                pooled.races_with_track,
                pooled.races_without_track,
                pooled.legs_without_track,
                pooled.unclassified_legs
            ),
            (3, 1, 12, 4)
        );
    }

    #[test]
    fn the_breakdown_adds_the_errors_of_everyone() {
        let ana = BreakdownHistory {
            errors: BreakdownTotals {
                legs: 3,
                loss_s: 90.0,
                detour_s: 50.0,
                stopped_s: 10.0,
                pace_s: 30.0,
            },
            races_with_track: 2,
            races_without_track: 1,
            errors_without_breakdown: 2,
        };
        let bea = BreakdownHistory {
            errors: BreakdownTotals {
                legs: 1,
                loss_s: 30.0,
                detour_s: 0.0,
                stopped_s: 20.0,
                pace_s: 10.0,
            },
            races_with_track: 1,
            races_without_track: 0,
            errors_without_breakdown: 1,
        };
        let pooled = pool_breakdown(&[&ana, &bea]);
        // Desvío 50 de 120 s (41,7 %), paradas 30 (25 %) y ritmo 40 (33,3 %).
        assert_eq!(
            pooled.errors,
            BreakdownTotals {
                legs: 4,
                loss_s: 120.0,
                detour_s: 50.0,
                stopped_s: 30.0,
                pace_s: 40.0,
            }
        );
        assert_eq!(
            (
                pooled.races_with_track,
                pooled.races_without_track,
                pooled.errors_without_breakdown
            ),
            (3, 1, 3)
        );
    }

    fn r(legs: usize, errors: usize) -> Rate {
        Rate {
            legs,
            errors,
            error_rate: rate(errors, legs),
        }
    }

    fn streaks(rates: [Rate; 5]) -> Vec<StreakBucket> {
        let starts = crate::after_error::STREAK_STARTS;
        rates
            .iter()
            .enumerate()
            .map(|(i, rate)| StreakBucket {
                from: starts[i],
                to: starts.get(i + 1).map(|n| n - 1),
                rate: *rate,
            })
            .collect()
    }

    #[test]
    fn after_an_error_adds_legs_and_errors_of_each_group() {
        let ana = AfterError {
            after_error: r(4, 1),
            after_clean: r(10, 2),
            accelerated: r(2, 0),
            not_accelerated: r(1, 1),
            after_error_without_speed: 1,
            streaks: streaks([r(4, 1), r(6, 1), r(4, 1), r(0, 0), r(0, 0)]),
        };
        let bea = AfterError {
            after_error: r(2, 1),
            after_clean: r(6, 0),
            accelerated: r(0, 0),
            not_accelerated: r(2, 1),
            after_error_without_speed: 0,
            streaks: streaks([r(2, 1), r(3, 0), r(3, 0), r(0, 0), r(0, 0)]),
        };
        let pooled = pool_after_error(&[&ana, &bea]);
        // Tras un error, 2 de 6; tras un limpio, 2 de 16.
        assert_eq!(pooled.after_error, r(6, 2));
        close(pooled.after_error.error_rate, 1.0 / 3.0);
        close(pooled.after_clean.error_rate, 0.125);
        // Acelerando, 0 de 2; sin acelerar, 2 de 3.
        close(pooled.accelerated.error_rate, 0.0);
        close(pooled.not_accelerated.error_rate, 2.0 / 3.0);
        assert_eq!(pooled.after_error_without_speed, 1);
        // Rachas: 0 → 2 de 6; 1–2 → 1 de 9; 6–10, vacío.
        assert_eq!(pooled.streaks.len(), 5);
        assert_eq!(pooled.streaks[0].rate, r(6, 2));
        close(pooled.streaks[1].rate.error_rate, 1.0 / 9.0);
        assert_eq!((pooled.streaks[1].from, pooled.streaks[1].to), (1, Some(2)));
        assert_eq!(pooled.streaks[3].rate.error_rate, None);
    }

    fn sub(subtype: Option<&str>, errors: usize, loss_s: f64) -> SubtypeCount {
        SubtypeCount {
            subtype: subtype.map(str::to_string),
            errors,
            loss_s,
        }
    }

    /// Un reparto: tramos, errores sin tipo (todos sin revisar) con su pérdida, los tipos con sus
    /// subtipos y los físicos con su pérdida.
    fn types(
        legs: usize,
        untyped: (usize, f64),
        by_type: &[(&str, Vec<SubtypeCount>)],
        physical: (usize, f64),
    ) -> ErrorTypes {
        let by_type: Vec<TypeCount> = by_type
            .iter()
            .map(|(t, subs)| TypeCount {
                error_type: t.to_string(),
                errors: subs.iter().map(|s| s.errors).sum(),
                loss_s: subs.iter().map(|s| s.loss_s).sum(),
                subtypes: subs.clone(),
            })
            .collect();
        let typed: usize = by_type.iter().map(|t| t.errors).sum();
        let typed_loss: f64 = by_type.iter().map(|t| t.loss_s).sum();
        ErrorTypes {
            legs,
            errors: typed + untyped.0,
            loss_s: typed_loss + untyped.1,
            untyped: untyped.0,
            untyped_loss_s: untyped.1,
            unreviewed: untyped.0,
            by_type,
            physical_legs: physical.0,
            physical_loss_s: physical.1,
        }
    }

    fn no_errors() -> ErrorTypes {
        types(0, (0, 0.0), &[], (0, 0.0))
    }

    /// Todo en un formato y en el cubo de 20 s.
    fn common(format: Option<RaceFormat>, total: ErrorTypes) -> CommonErrors {
        let lengths = |t: &ErrorTypes| {
            vec![
                LengthErrorTypes {
                    from_s: 20.0,
                    to_s: Some(30.0),
                    types: t.clone(),
                },
                LengthErrorTypes {
                    from_s: 30.0,
                    to_s: Some(60.0),
                    types: no_errors(),
                },
            ]
        };
        CommonErrors {
            by_leg_length: lengths(&total),
            by_format: [SPRINT, MIDDLE, LONG, None]
                .into_iter()
                .map(|f| {
                    let t = if f == format {
                        total.clone()
                    } else {
                        no_errors()
                    };
                    FormatErrorTypes {
                        format: f,
                        by_leg_length: lengths(&t),
                        total: t,
                    }
                })
                .collect(),
            total,
        }
    }

    #[test]
    fn error_types_add_types_and_subtypes_and_sort_again() {
        // Ana, en sprint: 10 tramos; navegación 2 (paralelo 40 s, sin subtipo 20 s), ataque 1
        // (20 s), uno sin tipo (20 s) y un físico (8 s).
        let ana = common(
            SPRINT,
            types(
                10,
                (1, 20.0),
                &[
                    (
                        "navigation",
                        vec![sub(Some("parallel"), 1, 40.0), sub(None, 1, 20.0)],
                    ),
                    ("attack", vec![sub(None, 1, 20.0)]),
                ],
                (1, 8.0),
            ),
        );
        // Bea, en media: 8 tramos; ataque 2 (pasarse 30 s, sin subtipo 20 s) y navegación 1
        // (paralelo 20 s).
        let bea = common(
            MIDDLE,
            types(
                8,
                (0, 0.0),
                &[
                    (
                        "attack",
                        vec![sub(Some("overshoot"), 1, 30.0), sub(None, 1, 20.0)],
                    ),
                    ("navigation", vec![sub(Some("parallel"), 1, 20.0)]),
                ],
                (0, 0.0),
            ),
        );
        let pooled = pool_common_errors(&[&ana, &bea]);
        let total = &pooled.total;
        // 18 tramos, 7 errores (170 s), 1 sin tipo y sin revisar (20 s), 1 físico (8 s).
        assert_eq!(
            (total.legs, total.errors, total.untyped, total.unreviewed),
            (18, 7, 1, 1)
        );
        assert_eq!((total.loss_s, total.untyped_loss_s), (170.0, 20.0));
        assert_eq!((total.physical_legs, total.physical_loss_s), (1, 8.0));
        // Navegación 3 errores y 80 s; ataque 3 y 70 s: a igualdad, más pérdida primero.
        let by_type: Vec<(&str, usize, f64)> = total
            .by_type
            .iter()
            .map(|t| (t.error_type.as_str(), t.errors, t.loss_s))
            .collect();
        assert_eq!(by_type, [("navigation", 3, 80.0), ("attack", 3, 70.0)]);
        // Paralelo 2 (60 s) y sin subtipo 1; en ataque, «sin subtipo» (2) va al final.
        assert_eq!(
            total.by_type[0].subtypes,
            [sub(Some("parallel"), 2, 60.0), sub(None, 1, 20.0)]
        );
        assert_eq!(
            total.by_type[1].subtypes,
            [sub(Some("overshoot"), 1, 30.0), sub(None, 2, 40.0)]
        );

        // Por cubo: todo en el de 20 s, igual que el total.
        assert_eq!(pooled.by_leg_length[0].types, *total);
        assert_eq!(pooled.by_leg_length[1].types.errors, 0);
        // Por formato: sprint es lo de Ana; media, lo de Bea.
        assert_eq!(pooled.by_format.len(), 4);
        assert_eq!(pooled.by_format[0].total, ana.total);
        assert_eq!(pooled.by_format[1].total, bea.total);
        assert_eq!(pooled.by_format[1].by_leg_length[0].types, bea.total);
        assert_eq!(pooled.by_format[3].format, None);
    }

    fn days(
        from_days: i64,
        races: usize,
        first: (usize, Option<f64>),
        third: (usize, usize),
    ) -> DaysOffStats {
        DaysOffStats {
            from_days,
            to_days: Some(from_days + 6),
            races,
            first_legs: first.0,
            first_legs_performance: first.1,
            first_third_legs: third.0,
            first_third_errors: third.1,
            first_third_error_rate: rate(third.1, third.0),
        }
    }

    #[test]
    fn days_off_weights_the_entry_performance_by_first_legs() {
        let ana = DaysOff {
            buckets: vec![
                days(1, 2, (6, Some(0.9)), (8, 2)),
                days(8, 1, (3, Some(1.0)), (4, 0)),
                days(15, 0, (0, None), (0, 0)),
                days(31, 0, (0, None), (0, 0)),
            ],
            without_previous: 1,
        };
        let bea = DaysOff {
            buckets: vec![
                days(1, 1, (2, Some(0.75)), (5, 3)),
                days(8, 0, (0, None), (0, 0)),
                days(15, 1, (3, Some(0.8)), (4, 1)),
                days(31, 0, (0, None), (0, 0)),
            ],
            without_previous: 1,
        };
        let pooled = pool_days_off(&[&ana, &bea]);
        // ≤ 7 días: 3 carreras; IR (0,9 × 6 + 0,75 × 2) / 8 = 0,8625; primer tercio 5 de 13.
        let week = pooled.buckets[0];
        assert_eq!((week.races, week.first_legs), (3, 8));
        close(week.first_legs_performance, 0.8625);
        assert_eq!((week.first_third_legs, week.first_third_errors), (13, 5));
        close(week.first_third_error_rate, 5.0 / 13.0);
        // 8–14, solo Ana; 15–30, solo Bea; > 30, vacío.
        close(pooled.buckets[1].first_legs_performance, 1.0);
        close(pooled.buckets[2].first_third_error_rate, 0.25);
        assert_eq!(pooled.buckets[3].first_legs_performance, None);
        assert_eq!(pooled.buckets[3].from_days, 31);
        assert_eq!(pooled.without_previous, 2);
    }

    fn third(
        n: usize,
        drift: Drift,
        before_error: HeartRateBefore,
        effort: Effort,
    ) -> ThirdFatigue {
        ThirdFatigue {
            third: n,
            drift,
            before_error,
            before_clean: HeartRateBefore::default(),
            effort_error: effort,
            effort_clean: Effort::default(),
            effort_physical: Effort::default(),
        }
    }

    fn drift(legs: usize, hr: f64, speed: f64, ratio: f64) -> Drift {
        Drift {
            legs,
            mean_heart_rate_bpm: Some(hr),
            mean_speed_mps: Some(speed),
            mean_relative_ratio: Some(ratio),
        }
    }

    fn before(legs: usize, hr: f64, relative: f64) -> HeartRateBefore {
        HeartRateBefore {
            legs,
            mean_heart_rate_bpm: Some(hr),
            mean_relative_bpm: Some(relative),
        }
    }

    fn effort(legs: usize, mean: f64) -> Effort {
        Effort {
            legs,
            mean_effort: Some(mean),
        }
    }

    #[test]
    fn fatigue_weights_each_mean_by_its_legs() {
        let empty = |n| {
            third(
                n,
                Drift::default(),
                HeartRateBefore::default(),
                Effort::default(),
            )
        };
        let ana = Fatigue {
            by_third: vec![
                third(
                    1,
                    drift(3, 150.0, 3.0, 0.95),
                    before(1, 160.0, -10.0),
                    effort(1, 8.0),
                ),
                empty(2),
                empty(3),
            ],
            races_with_heart_rate: 1,
            races_without_heart_rate: 1,
            errors_without_heart_rate: 2,
        };
        let bea = Fatigue {
            by_third: vec![
                third(
                    1,
                    drift(1, 170.0, 2.5, 1.15),
                    before(1, 150.0, 6.0),
                    effort(2, 6.5),
                ),
                empty(2),
                empty(3),
            ],
            races_with_heart_rate: 1,
            races_without_heart_rate: 0,
            errors_without_heart_rate: 0,
        };
        let pooled = pool_fatigue(&[&ana, &bea]);
        let first = pooled.by_third[0];
        // Deriva: 4 tramos; pulso (150 × 3 + 170) / 4 = 155; velocidad (3 × 3 + 2,5) / 4 =
        // 2,875; relativa (0,95 × 3 + 1,15) / 4 = 1.
        assert_eq!(first.drift.legs, 4);
        close(first.drift.mean_heart_rate_bpm, 155.0);
        close(first.drift.mean_speed_mps, 2.875);
        close(first.drift.mean_relative_ratio, 1.0);
        // Antes del error: (160 + 150) / 2 = 155 ppm; (−10 + 6) / 2 = −2 ppm.
        assert_eq!(first.before_error.legs, 2);
        close(first.before_error.mean_heart_rate_bpm, 155.0);
        close(first.before_error.mean_relative_bpm, -2.0);
        assert_eq!(first.before_clean, HeartRateBefore::default());
        // Esfuerzo en los errores: (8 + 6,5 × 2) / 3 = 7.
        assert_eq!(first.effort_error.legs, 3);
        close(first.effort_error.mean_effort, 7.0);
        assert_eq!(pooled.by_third[2].drift.mean_relative_ratio, None);
        assert_eq!(
            (
                pooled.races_with_heart_rate,
                pooled.races_without_heart_rate,
                pooled.errors_without_heart_rate
            ),
            (2, 1, 2)
        );
    }

    #[test]
    fn the_group_counts_the_members_with_races() {
        let (ana, _) = two_histories();
        let nobody = history(&[], HistoryStats::default());
        let lengths = [bucket(20.0, 10, 2, 6.0, 5.0)];
        let slope = pool_slope(&[]);
        let breakdown = BreakdownHistory::default();
        let after = pool_after_error(&[]);
        let errors = common(SPRINT, no_errors());
        let fatigue = pool_fatigue(&[]);
        let days_off = pool_days_off(&[]);
        let member = |history| MemberAnalyses {
            history,
            by_leg_length: &lengths,
            by_slope: &slope,
            loss_breakdown: &breakdown,
            after_error: &after,
            common_errors: &errors,
            fatigue: &fatigue,
            days_off: &days_off,
        };
        let group = group_stats(&HistoryFilter::default(), &[member(&ana), member(&nobody)]);
        assert_eq!(group.runners, 1);
        assert_eq!(group.history.total.races, 2);
        assert_eq!(group.by_leg_length[0].legs, 20);
        assert_eq!(group.history.races_without_data, 2);
    }
}
