//! Comparar dos grupos de atletas (#121): IR medio, tasa de error, tipos de error, duración del
//! tramo (P7) y desnivel (P13) de un grupo frente a otro. Las dos dudas de la comparación son
//! opciones ([`CompareOptions`]): qué pasa con quien está en los dos grupos y qué carreras
//! entran. Las definiciones están en `docs/historico.md`, "Comparar grupos".
//!
//! Cada lado junta a sus miembros como [`crate::group::group_total`]: sus carreras y tramos,
//! cada uno con sus umbrales, sumados con su peso.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::common_errors::ErrorTypes;
use crate::group::group_total;
use crate::group_stats::{pool_error_types, pool_leg_length, pool_slope_classes};
use crate::history::HistoryStats;
use crate::leg_length::LegLengthStats;
use crate::slope::SlopeStats;

/// Qué se hace con un atleta que está en los dos grupos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Overlap {
    /// Cuenta en los dos, entero.
    #[default]
    CountInBoth,
    /// No cuenta en ninguno de los dos.
    Exclude,
}

/// Qué carreras entran.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RaceSelection {
    /// Solo las que han corrido miembros de los dos grupos: se comparan en el mismo terreno.
    #[default]
    Shared,
    /// Todas las de cada grupo (con el filtro del histórico).
    All,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompareOptions {
    pub overlap: Overlap,
    pub races: RaceSelection,
}

/// Quién cuenta en cada lado.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sides {
    pub a: Vec<String>,
    pub b: Vec<String>,
    /// Los que están en los dos grupos (cuenten o no), en orden.
    pub in_both: Vec<String>,
}

/// Los miembros que cuentan en cada lado, sin repetir y en orden. Con [`Overlap::Exclude`], los
/// que están en los dos no cuentan en ninguno.
pub fn sides(a: &[String], b: &[String], overlap: Overlap) -> Sides {
    let a: BTreeSet<&String> = a.iter().collect();
    let b: BTreeSet<&String> = b.iter().collect();
    let in_both: Vec<String> = a.intersection(&b).map(|r| (*r).clone()).collect();
    let keep = |side: &BTreeSet<&String>| -> Vec<String> {
        side.iter()
            .filter(|r| overlap == Overlap::CountInBoth || !in_both.contains(r))
            .map(|r| (*r).clone())
            .collect()
    };
    Sides {
        a: keep(&a),
        b: keep(&b),
        in_both,
    }
}

/// Las carreras (`race_id`) de un miembro: quién y cuáles.
pub type MemberRaces<'a> = (&'a str, &'a [String]);

/// Carreras que han corrido un miembro de `a` y **otro** de `b`: alguien que está en los dos
/// grupos no hace por sí solo que una carrera sea de los dos.
pub fn races_of_both(a: &[MemberRaces<'_>], b: &[MemberRaces<'_>]) -> BTreeSet<String> {
    let (in_a, in_b) = (runners_by_race(a), runners_by_race(b));
    in_a.iter()
        .filter(|(race, ra)| {
            in_b.get(*race)
                .is_some_and(|rb| ra.iter().any(|x| rb.iter().any(|y| x != y)))
        })
        .map(|(race, _)| (*race).to_string())
        .collect()
}

/// Quién ha corrido cada carrera de un lado.
fn runners_by_race<'a>(side: &[MemberRaces<'a>]) -> BTreeMap<&'a str, BTreeSet<&'a str>> {
    let mut by_race: BTreeMap<&'a str, BTreeSet<&'a str>> = BTreeMap::new();
    for (runner, races) in side {
        for race in races.iter() {
            by_race.entry(race.as_str()).or_default().insert(runner);
        }
    }
    by_race
}

/// Lo de un miembro que entra en la comparación: su histórico con las carreras que entran.
#[derive(Debug, Clone, Copy)]
pub struct MemberStats<'a> {
    /// El total de su histórico (`History::total`).
    pub total: HistoryStats,
    /// P7 (`crate::leg_length`).
    pub by_leg_length: &'a [LegLengthStats],
    /// P13 (`SlopeHistory::by_class`).
    pub by_slope: &'a [SlopeStats],
    /// P9, el reparto de todos sus errores (`CommonErrors::total`).
    pub errors: &'a ErrorTypes,
}

/// Un tipo de error en un grupo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeShare {
    pub error_type: String,
    pub errors: usize,
    /// Parte de los errores de orientación del grupo (0–1), contando los sin tipo.
    pub share: f64,
}

/// Un grupo, con sus miembros juntos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupSide {
    /// Miembros con alguna carrera que cuenta.
    pub runners: usize,
    /// Carreras y tramos de todos ([`group_total`]).
    pub stats: HistoryStats,
    /// P7 de todos: los mismos cubos, con tramos y errores sumados.
    pub by_leg_length: Vec<LegLengthStats>,
    /// P13 de todos: las tres clases, con el IR ponderado por `reference_s`.
    pub by_slope: Vec<SlopeStats>,
    /// Errores de orientación de todos (con tipo y sin él).
    pub orientation_errors: usize,
    /// De ellos, sin tipo.
    pub untyped: usize,
    /// Errores con tipo, de más a menos (a igualdad, más pérdida primero y luego por clave, como
    /// en P9).
    pub by_type: Vec<TypeShare>,
}

/// Junta a los miembros de un grupo.
pub fn pool_side(members: &[MemberStats<'_>]) -> GroupSide {
    let counted: Vec<&MemberStats<'_>> = members.iter().filter(|m| m.total.races > 0).collect();
    let stats = group_total(&counted.iter().map(|m| m.total).collect::<Vec<_>>());

    // P7 y P13, como en las estadísticas de un grupo (`crate::group_stats`).
    let lengths: Vec<&[LegLengthStats]> = counted.iter().map(|m| m.by_leg_length).collect();
    let by_leg_length = pool_leg_length(&lengths);
    let classes: Vec<&[SlopeStats]> = counted.iter().map(|m| m.by_slope).collect();
    let by_slope = pool_slope_classes(&classes);

    // P9: errores por tipo, sobre todos los de orientación.
    let errors: Vec<&ErrorTypes> = counted.iter().map(|m| m.errors).collect();
    let errors = pool_error_types(&errors);
    let orientation_errors = errors.errors;
    let untyped = errors.untyped;
    let by_type: Vec<TypeShare> = errors
        .by_type
        .into_iter()
        .map(|t| TypeShare {
            share: if orientation_errors == 0 {
                0.0
            } else {
                t.errors as f64 / orientation_errors as f64
            },
            error_type: t.error_type,
            errors: t.errors,
        })
        .collect();

    GroupSide {
        runners: counted.len(),
        stats,
        by_leg_length,
        by_slope,
        orientation_errors,
        untyped,
        by_type,
    }
}

/// Un grupo frente a otro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupsComparison {
    pub options: CompareOptions,
    pub a: GroupSide,
    pub b: GroupSide,
    /// Atletas que están en los dos grupos (cuenten o no, según `options.overlap`).
    pub in_both: usize,
    /// Con [`RaceSelection::Shared`], cuántas carreras han corrido los dos grupos; con
    /// [`RaceSelection::All`], `None`.
    pub shared_races: Option<usize>,
    /// IR medio de `a` menos el de `b` (1 = 100 puntos).
    pub performance_difference: Option<f64>,
    /// Tasa de error de `a` menos la de `b` (0–1).
    pub error_rate_difference: Option<f64>,
    /// Pérdida media por tramo (%) de `a` menos la de `b`.
    pub loss_pct_difference: Option<f64>,
}

/// La comparación de dos lados ya juntados, con sus diferencias (`a − b`).
pub fn compare_groups(
    options: CompareOptions,
    a: GroupSide,
    b: GroupSide,
    in_both: usize,
    shared_races: Option<usize>,
) -> GroupsComparison {
    let difference = |x: Option<f64>, y: Option<f64>| Some(x? - y?);
    GroupsComparison {
        options,
        performance_difference: difference(a.stats.mean_performance, b.stats.mean_performance),
        error_rate_difference: difference(a.stats.error_rate, b.stats.error_rate),
        loss_pct_difference: difference(a.stats.mean_loss_pct, b.stats.mean_loss_pct),
        a,
        b,
        in_both,
        shared_races,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common_errors::TypeCount;
    use crate::slope::SlopeClass;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn close(actual: Option<f64>, expected: f64) {
        let actual = actual.unwrap();
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    #[test]
    fn whoever_is_in_both_groups_counts_in_both_or_in_neither() {
        let a = ids(&["ana", "berta", "ana"]);
        let b = ids(&["carla", "berta"]);
        let both = sides(&a, &b, Overlap::CountInBoth);
        assert_eq!(both.a, ids(&["ana", "berta"]));
        assert_eq!(both.b, ids(&["berta", "carla"]));
        assert_eq!(both.in_both, ids(&["berta"]));

        let apart = sides(&a, &b, Overlap::Exclude);
        assert_eq!(apart.a, ids(&["ana"]));
        assert_eq!(apart.b, ids(&["carla"]));
        assert_eq!(apart.in_both, ids(&["berta"]));
    }

    #[test]
    fn a_race_of_both_needs_two_different_runners() {
        let (r1, r2, r3) = (ids(&["r1", "r2"]), ids(&["r2"]), ids(&["r3"]));
        // A: ana (r1, r2) y berta (r3); B: berta (r3) y carla (r2).
        let a: Vec<MemberRaces<'_>> = vec![("ana", &r1), ("berta", &r3)];
        let b: Vec<MemberRaces<'_>> = vec![("berta", &r3), ("carla", &r2)];
        let races = races_of_both(&a, &b);
        // r1: solo A. r2: ana y carla. r3: solo berta, que está en los dos: no cuenta.
        assert_eq!(races, BTreeSet::from(["r2".to_string()]));
        assert!(races_of_both(&a, &[]).is_empty());
    }

    fn bucket(
        from_s: f64,
        legs: usize,
        errors: usize,
        loss_s: f64,
        loss_pct: f64,
    ) -> LegLengthStats {
        LegLengthStats {
            from_s,
            to_s: Some(from_s + 60.0),
            legs,
            errors,
            error_rate: (legs > 0).then(|| errors as f64 / legs as f64),
            mean_loss_s: (legs > 0).then_some(loss_s),
            mean_loss_pct: (legs > 0).then_some(loss_pct),
        }
    }

    fn class(
        c: SlopeClass,
        legs: usize,
        errors: usize,
        ir: Option<f64>,
        reference_s: f64,
    ) -> SlopeStats {
        SlopeStats {
            class: c,
            legs,
            errors,
            error_rate: (legs > 0).then(|| errors as f64 / legs as f64),
            mean_performance: ir,
            reference_s,
        }
    }

    fn errors(total: usize, untyped: usize, by_type: &[(&str, usize)]) -> ErrorTypes {
        ErrorTypes {
            legs: 0,
            errors: total,
            loss_s: 0.0,
            untyped,
            untyped_loss_s: 0.0,
            unreviewed: 0,
            by_type: by_type
                .iter()
                .map(|(t, n)| TypeCount {
                    error_type: t.to_string(),
                    errors: *n,
                    loss_s: 0.0,
                    subtypes: Vec::new(),
                })
                .collect(),
            physical_legs: 0,
            physical_loss_s: 0.0,
        }
    }

    fn total(races: usize, legs: usize, errors: usize, ir: f64) -> HistoryStats {
        HistoryStats {
            races,
            legs,
            errors,
            mean_performance: Some(ir),
            error_rate: Some(errors as f64 / legs as f64),
            mean_loss_s: Some(5.0),
            mean_loss_pct: Some(4.0),
            mean_consistency: None,
        }
    }

    /// Dos miembros sintéticos de un grupo y uno sin carreras.
    #[test]
    fn a_side_pools_its_members_with_their_weights() {
        let legs_1 = [bucket(20.0, 10, 2, 6.0, 5.0), bucket(80.0, 0, 0, 0.0, 0.0)];
        let legs_2 = [bucket(20.0, 30, 3, 2.0, 1.0), bucket(80.0, 5, 1, 4.0, 3.0)];
        let slope_1 = [
            class(SlopeClass::Uphill, 4, 1, Some(0.80), 400.0),
            class(SlopeClass::Flat, 0, 0, None, 0.0),
        ];
        let slope_2 = [
            class(SlopeClass::Uphill, 6, 3, Some(0.90), 100.0),
            class(SlopeClass::Flat, 2, 0, Some(1.0), 50.0),
        ];
        let errors_1 = errors(4, 1, &[("navigation", 2), ("attack", 1)]);
        let errors_2 = errors(6, 2, &[("attack", 3), ("route", 1)]);
        let empty = errors(0, 0, &[]);
        let members = [
            MemberStats {
                total: total(2, 10, 2, 0.9),
                by_leg_length: &legs_1,
                by_slope: &slope_1,
                errors: &errors_1,
            },
            MemberStats {
                total: total(3, 35, 4, 0.8),
                by_leg_length: &legs_2,
                by_slope: &slope_2,
                errors: &errors_2,
            },
            MemberStats {
                total: HistoryStats::default(),
                by_leg_length: &[],
                by_slope: &[],
                errors: &empty,
            },
        ];
        let side = pool_side(&members);
        assert_eq!(side.runners, 2);
        assert_eq!(
            (side.stats.races, side.stats.legs, side.stats.errors),
            (5, 45, 6)
        );
        // (0,9 × 2 + 0,8 × 3) / 5.
        close(side.stats.mean_performance, 0.84);

        // Cubo de 20 s: 40 tramos, 5 errores; pérdida (6 × 10 + 2 × 30) / 40 = 3 s y
        // (5 × 10 + 1 × 30) / 40 = 2 %.
        let first = side.by_leg_length[0];
        assert_eq!((first.from_s, first.legs, first.errors), (20.0, 40, 5));
        close(first.error_rate, 0.125);
        close(first.mean_loss_s, 3.0);
        close(first.mean_loss_pct, 2.0);
        let second = side.by_leg_length[1];
        assert_eq!((second.legs, second.errors), (5, 1));
        close(second.mean_loss_s, 4.0);

        // Subida: IR (0,8 × 400 + 0,9 × 100) / 500 = 0,82; 10 tramos, 4 errores.
        let up = &side.by_slope[0];
        assert_eq!((up.class, up.legs, up.errors), (SlopeClass::Uphill, 10, 4));
        close(up.mean_performance, 0.82);
        close(up.error_rate, 0.4);
        assert_eq!(up.reference_s, 500.0);
        // Llano: solo el segundo tiene tramos con IR.
        close(side.by_slope[1].mean_performance, 1.0);

        // Tipos: 10 errores de orientación, 3 sin tipo; attack 4, navigation 2, route 1.
        assert_eq!((side.orientation_errors, side.untyped), (10, 3));
        let types: Vec<(&str, usize)> = side
            .by_type
            .iter()
            .map(|t| (t.error_type.as_str(), t.errors))
            .collect();
        assert_eq!(types, [("attack", 4), ("navigation", 2), ("route", 1)]);
        assert!((side.by_type[0].share - 0.4).abs() < 1e-9);
    }

    #[test]
    fn the_comparison_gives_the_differences_a_minus_b() {
        let empty = errors(0, 0, &[]);
        let side = |races, legs, errs, ir| {
            pool_side(&[MemberStats {
                total: total(races, legs, errs, ir),
                by_leg_length: &[],
                by_slope: &[],
                errors: &empty,
            }])
        };
        let options = CompareOptions {
            overlap: Overlap::Exclude,
            races: RaceSelection::Shared,
        };
        let c = compare_groups(
            options,
            side(2, 20, 4, 0.9),
            side(1, 10, 1, 0.85),
            1,
            Some(3),
        );
        close(c.performance_difference, 0.05);
        close(c.error_rate_difference, 0.1);
        close(c.loss_pct_difference, 0.0);
        assert_eq!((c.in_both, c.shared_races), (1, Some(3)));

        // Un lado sin carreras no tiene diferencias.
        let none = compare_groups(options, side(2, 20, 4, 0.9), pool_side(&[]), 0, Some(0));
        assert_eq!(none.performance_difference, None);
        assert_eq!(none.b.runners, 0);
    }

    #[test]
    fn options_serialize_in_snake_case() {
        let options = CompareOptions {
            overlap: Overlap::CountInBoth,
            races: RaceSelection::All,
        };
        let json = serde_json::to_string(&options).unwrap();
        assert_eq!(json, r#"{"overlap":"count_in_both","races":"all"}"#);
        assert_eq!(CompareOptions::default().races, RaceSelection::Shared);
    }
}
