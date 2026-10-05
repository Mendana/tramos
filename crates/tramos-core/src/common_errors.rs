//! Errores más comunes (P9 de `docs/preguntas.md`): reparto de los errores del histórico por tipo
//! y subtipo de la taxonomía (`docs/taxonomia.md`), cruzable con los cubos de duración del tramo
//! (P7) y con el formato. Las definiciones están en `docs/historico.md`, "Errores más comunes
//! (P9)".
//!
//! Qué es un error lo decide la etiqueta del corredor cuando la hay; si no, el cálculo
//! ([`leg_status`]). Lo que el corredor marca como físico no es un error de orientación: no
//! entra en el reparto, pero se cuenta aparte.

use serde::{Deserialize, Serialize};

use crate::history::{FORMATS, HistoryFilter, HistoryRace, pattern_legs};
use crate::leg_length::{BUCKET_BOUNDS_S, BUCKETS, bucket_index};
use crate::race_format::RaceFormat;
use crate::runner_report::LegReport;
use crate::taxonomy::{Confirmation, LegTag};

/// Una carrera del histórico con las etiquetas de sus tramos.
#[derive(Debug, Clone, Copy)]
pub struct TaggedRace<'a> {
    pub race: &'a HistoryRace,
    /// `(tramo, etiqueta)`, con el tramo desde 1 como `LegReport::index`.
    pub tags: &'a [(usize, LegTag)],
}

/// Qué fue un tramo, juntando el cálculo y la etiqueta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegStatus {
    /// Sin error.
    Clean,
    /// Error de orientación. `reviewed` es falso si solo lo dice el cálculo.
    Error { reviewed: bool },
    /// Tiempo perdido por el físico, no por orientarse mal.
    Physical,
}

/// Estado de un tramo:
///
/// - Confirmación de la etiqueta: «error» es error; «no» es sin error; «físico» es físico.
/// - Sin confirmación pero con tipo: error (el corredor le ha puesto tipo).
/// - Sin nada de eso: lo que diga el cálculo, sin revisar.
pub fn leg_status(leg: &LegReport, tag: Option<&LegTag>) -> LegStatus {
    match tag.and_then(|t| t.confirmation) {
        Some(Confirmation::Error) => LegStatus::Error { reviewed: true },
        Some(Confirmation::NoError) => LegStatus::Clean,
        Some(Confirmation::Physical) => LegStatus::Physical,
        None if tag.is_some_and(|t| t.error_type.is_some()) => LegStatus::Error { reviewed: true },
        None if leg.is_error => LegStatus::Error { reviewed: false },
        None => LegStatus::Clean,
    }
}

/// Errores de un subtipo (`None` = con tipo pero sin subtipo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtypeCount {
    pub subtype: Option<String>,
    pub errors: usize,
    /// Suma de la pérdida calculada (`p_i`) de esos errores (s).
    pub loss_s: f64,
}

/// Errores de un tipo, con su reparto por subtipo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeCount {
    pub error_type: String,
    pub errors: usize,
    pub loss_s: f64,
    /// De más a menos errores; «sin subtipo» al final.
    pub subtypes: Vec<SubtypeCount>,
}

/// Reparto de los errores de un grupo de tramos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorTypes {
    /// Tramos que cuentan en el grupo (`n`).
    pub legs: usize,
    /// Errores de orientación entre ellos.
    pub errors: usize,
    /// Suma de su pérdida calculada (s).
    pub loss_s: f64,
    /// Errores sin tipo (incluye los sin revisar).
    pub untyped: usize,
    pub untyped_loss_s: f64,
    /// Errores que solo propone el cálculo, sin etiqueta que lo confirme ni tipo.
    pub unreviewed: usize,
    /// Errores con tipo, de más a menos (a igualdad, más pérdida primero; luego por clave).
    pub by_type: Vec<TypeCount>,
    /// Tramos marcados como físicos: no son errores, pero se pierde tiempo.
    pub physical_legs: usize,
    pub physical_loss_s: f64,
}

/// Reparto de un cubo de duración (P7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LengthErrorTypes {
    /// Inicio del cubo (s de referencia), incluido.
    pub from_s: f64,
    /// Final del cubo, excluido; `None` en el último.
    pub to_s: Option<f64>,
    pub types: ErrorTypes,
}

/// Reparto de un formato, entero y por cubos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormatErrorTypes {
    /// `None` = carreras sin formato.
    pub format: Option<RaceFormat>,
    pub total: ErrorTypes,
    pub by_leg_length: Vec<LengthErrorTypes>,
}

/// P9: el reparto de todos los tramos, por cubo, por formato y por formato y cubo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommonErrors {
    pub total: ErrorTypes,
    /// Los seis cubos de P7, en orden, aunque estén vacíos.
    pub by_leg_length: Vec<LengthErrorTypes>,
    /// Sprint, media, larga y sin formato, en ese orden, aunque estén vacíos.
    pub by_format: Vec<FormatErrorTypes>,
}

/// Sumas de un grupo.
#[derive(Debug, Default, Clone)]
struct Accumulator {
    legs: usize,
    errors: usize,
    loss_s: f64,
    untyped: usize,
    untyped_loss_s: f64,
    unreviewed: usize,
    /// `(tipo, subtipo, errores, pérdida)`.
    typed: Vec<(String, Option<String>, usize, f64)>,
    physical_legs: usize,
    physical_loss_s: f64,
}

impl Accumulator {
    fn add(&mut self, status: LegStatus, tag: Option<&LegTag>, loss_s: f64) {
        self.legs += 1;
        match status {
            LegStatus::Clean => {}
            LegStatus::Physical => {
                self.physical_legs += 1;
                self.physical_loss_s += loss_s;
            }
            LegStatus::Error { reviewed } => {
                self.errors += 1;
                self.loss_s += loss_s;
                if !reviewed {
                    self.unreviewed += 1;
                }
                match tag.and_then(|t| t.error_type.as_ref().map(|ty| (ty, &t.error_subtype))) {
                    None => {
                        self.untyped += 1;
                        self.untyped_loss_s += loss_s;
                    }
                    Some((ty, sub)) => {
                        match self
                            .typed
                            .iter_mut()
                            .find(|(t, s, _, _)| t == ty && s == sub)
                        {
                            Some(entry) => {
                                entry.2 += 1;
                                entry.3 += loss_s;
                            }
                            None => self.typed.push((ty.clone(), sub.clone(), 1, loss_s)),
                        }
                    }
                }
            }
        }
    }

    fn finish(&self) -> ErrorTypes {
        let mut by_type: Vec<TypeCount> = Vec::new();
        for (ty, sub, errors, loss_s) in &self.typed {
            let entry = match by_type.iter_mut().position(|t| &t.error_type == ty) {
                Some(i) => &mut by_type[i],
                None => {
                    by_type.push(TypeCount {
                        error_type: ty.clone(),
                        errors: 0,
                        loss_s: 0.0,
                        subtypes: Vec::new(),
                    });
                    let last = by_type.len() - 1;
                    &mut by_type[last]
                }
            };
            entry.errors += errors;
            entry.loss_s += loss_s;
            entry.subtypes.push(SubtypeCount {
                subtype: sub.clone(),
                errors: *errors,
                loss_s: *loss_s,
            });
        }
        for t in &mut by_type {
            // «Sin subtipo» al final; el resto, de más a menos.
            t.subtypes.sort_by(|a, b| {
                a.subtype
                    .is_none()
                    .cmp(&b.subtype.is_none())
                    .then(b.errors.cmp(&a.errors))
                    .then(b.loss_s.total_cmp(&a.loss_s))
                    .then(a.subtype.cmp(&b.subtype))
            });
        }
        by_type.sort_by(|a, b| {
            b.errors
                .cmp(&a.errors)
                .then(b.loss_s.total_cmp(&a.loss_s))
                .then(a.error_type.cmp(&b.error_type))
        });
        ErrorTypes {
            legs: self.legs,
            errors: self.errors,
            loss_s: self.loss_s,
            untyped: self.untyped,
            untyped_loss_s: self.untyped_loss_s,
            unreviewed: self.unreviewed,
            by_type,
            physical_legs: self.physical_legs,
            physical_loss_s: self.physical_loss_s,
        }
    }
}

/// Las sumas de un formato: el total y una por cubo.
#[derive(Debug, Default, Clone)]
struct Grid {
    total: Accumulator,
    buckets: [Accumulator; BUCKETS],
}

impl Grid {
    fn by_leg_length(&self) -> Vec<LengthErrorTypes> {
        self.buckets
            .iter()
            .zip(BUCKET_BOUNDS_S)
            .enumerate()
            .map(|(i, (acc, from_s))| LengthErrorTypes {
                from_s,
                to_s: BUCKET_BOUNDS_S.get(i + 1).copied(),
                types: acc.finish(),
            })
            .collect()
    }
}

/// Reparte los tramos que cuentan ([`pattern_legs`]) de las carreras que pasan `filter` (las
/// mismas que [`crate::history::history`]: con rendimiento habitual). Los tramos sin cubo (no
/// debería haberlos: los de referencia corta ya no cuentan) entran en los totales pero no en
/// ningún cubo.
pub fn common_errors(races: &[TaggedRace], filter: &HistoryFilter) -> CommonErrors {
    let mut all = Grid::default();
    // FORMATS y, al final, sin formato.
    let mut formats: [Grid; FORMATS.len() + 1] = Default::default();
    let counted = races.iter().filter(|r| {
        filter.includes(r.race.date, r.race.format) && r.race.lost_time.usual_performance.is_some()
    });
    for tagged in counted {
        let format_index = tagged
            .race
            .format
            .and_then(|f| FORMATS.iter().position(|&g| g == f))
            .unwrap_or(FORMATS.len());
        for leg in pattern_legs(&tagged.race.lost_time) {
            let tag = tagged
                .tags
                .iter()
                .find(|(index, _)| *index == leg.index)
                .map(|(_, tag)| tag);
            let status = leg_status(leg, tag);
            let loss_s = leg.loss_s.unwrap_or(0.0);
            let bucket = leg.reference_s.and_then(bucket_index);
            for grid in [&mut all, &mut formats[format_index]] {
                grid.total.add(status, tag, loss_s);
                if let Some(acc) = bucket.and_then(|b| grid.buckets.get_mut(b)) {
                    acc.add(status, tag, loss_s);
                }
            }
        }
    }
    let format_keys = FORMATS.iter().copied().map(Some).chain([None]);
    CommonErrors {
        total: all.total.finish(),
        by_leg_length: all.by_leg_length(),
        by_format: format_keys
            .zip(&formats)
            .map(|(format, grid)| FormatErrorTypes {
                format,
                total: grid.total.finish(),
                by_leg_length: grid.by_leg_length(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner_report::RunnerLostTime;

    /// Tramo: referencia (s), pérdida (s), error según el cálculo y si es el último.
    #[derive(Clone, Copy)]
    struct L(f64, f64, bool, bool);

    fn race(date: &str, format: Option<RaceFormat>, usual: Option<f64>, legs: &[L]) -> HistoryRace {
        let legs = legs
            .iter()
            .enumerate()
            .map(|(i, &L(reference, loss, error, last))| {
                let short = reference < 20.0;
                LegReport {
                    index: i + 1,
                    from: 31,
                    to: 32,
                    split_s: Some(reference + loss),
                    elapsed_s: None,
                    place: None,
                    reference_s: Some(reference),
                    reference_count: 1,
                    valid_splits: 4,
                    performance_index: Some(1.0),
                    expected_s: Some(reference),
                    loss_s: Some(loss),
                    loss_pct: Some(loss / reference * 100.0),
                    is_error: error,
                    gain_s: None,
                    cumulative_gain_s: None,
                    ideal_elapsed_s: None,
                    behind_ideal_s: None,
                    is_last: last,
                    short_reference: short,
                    excluded_from_patterns: last || short,
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
                consistency: None,
                legs,
            },
        }
    }

    fn tag(
        confirmation: Option<Confirmation>,
        error_type: Option<&str>,
        subtype: Option<&str>,
    ) -> LegTag {
        LegTag {
            confirmation,
            error_type: error_type.map(Into::into),
            error_subtype: subtype.map(Into::into),
            ..LegTag::default()
        }
    }

    const fn leg(reference: f64, loss: f64, error: bool) -> L {
        L(reference, loss, error, false)
    }

    const fn last(reference: f64, loss: f64, error: bool) -> L {
        L(reference, loss, error, true)
    }

    /// Carreras sintéticas con sus etiquetas:
    ///
    /// - A, sprint: 25 s (error, 15 s; Sí, navegación · paralelo), 28 s (sin error según el
    ///   cálculo, 0 s; Sí, ataque), 45 s (error, 20 s; sin etiqueta: sin revisar), 50 s (error,
    ///   8 s; Físico), 90 s (error, 12 s; No) y el último (error, 40 s; con tipo, no cuenta).
    /// - B, media: 100 s (error, 30 s; sin confirmar pero con navegación · paralelo), 200 s
    ///   (error, 60 s; Sí, navegación · pasarse), 300 s (5 s, sin etiqueta), 15 s (error,
    ///   referencia corta: no cuenta) y el último.
    /// - C, larga, sin rendimiento habitual: no cuenta.
    /// - D, sin formato: 500 s (error, 50 s; Sí, sin tipo) y el último.
    fn data() -> Vec<(HistoryRace, Vec<(usize, LegTag)>)> {
        use Confirmation::*;
        vec![
            (
                race(
                    "2026-03-01",
                    Some(RaceFormat::Sprint),
                    Some(0.9),
                    &[
                        leg(25.0, 15.0, true),
                        leg(28.0, 0.0, false),
                        leg(45.0, 20.0, true),
                        leg(50.0, 8.0, true),
                        leg(90.0, 12.0, true),
                        last(30.0, 40.0, true),
                    ],
                ),
                vec![
                    (1, tag(Some(Error), Some("navigation"), Some("parallel"))),
                    (2, tag(Some(Error), Some("attack"), None)),
                    (4, tag(Some(Physical), None, None)),
                    (5, tag(Some(NoError), None, None)),
                    (6, tag(Some(Error), Some("navigation"), None)),
                ],
            ),
            (
                race(
                    "2026-04-01",
                    Some(RaceFormat::Middle),
                    Some(0.85),
                    &[
                        leg(100.0, 30.0, true),
                        leg(200.0, 60.0, true),
                        leg(300.0, 5.0, false),
                        leg(15.0, 9.0, true),
                        last(60.0, 0.0, false),
                    ],
                ),
                vec![
                    (1, tag(None, Some("navigation"), Some("parallel"))),
                    (2, tag(Some(Error), Some("navigation"), Some("overshoot"))),
                    (4, tag(Some(Error), Some("attack"), None)),
                ],
            ),
            (
                race(
                    "2026-05-01",
                    Some(RaceFormat::Long),
                    None,
                    &[leg(500.0, 50.0, true), last(30.0, 0.0, false)],
                ),
                vec![(1, tag(Some(Error), Some("attack"), None))],
            ),
            (
                race(
                    "2026-06-01",
                    None,
                    Some(0.9),
                    &[leg(500.0, 50.0, true), last(30.0, 0.0, false)],
                ),
                vec![(1, tag(Some(Error), None, None))],
            ),
        ]
    }

    fn run(filter: &HistoryFilter) -> CommonErrors {
        let data = data();
        let tagged: Vec<_> = data
            .iter()
            .map(|(race, tags)| TaggedRace { race, tags })
            .collect();
        common_errors(&tagged, filter)
    }

    fn sub(subtype: Option<&str>, errors: usize, loss_s: f64) -> SubtypeCount {
        SubtypeCount {
            subtype: subtype.map(Into::into),
            errors,
            loss_s,
        }
    }

    #[test]
    fn the_tag_decides_and_the_calculation_fills_in() {
        let race = race(
            "2026-03-01",
            None,
            Some(1.0),
            &[leg(60.0, 20.0, true), leg(60.0, 0.0, false)],
        );
        let (error, clean) = (&race.lost_time.legs[0], &race.lost_time.legs[1]);
        use Confirmation::*;
        let reviewed = LegStatus::Error { reviewed: true };
        assert_eq!(
            leg_status(error, None),
            LegStatus::Error { reviewed: false }
        );
        assert_eq!(leg_status(clean, None), LegStatus::Clean);
        assert_eq!(
            leg_status(error, Some(&tag(Some(NoError), None, None))),
            LegStatus::Clean
        );
        assert_eq!(
            leg_status(error, Some(&tag(Some(Physical), None, None))),
            LegStatus::Physical
        );
        assert_eq!(
            leg_status(clean, Some(&tag(Some(Error), None, None))),
            reviewed
        );
        // Con tipo y sin confirmar: error, aunque el cálculo diga que no.
        assert_eq!(
            leg_status(clean, Some(&tag(None, Some("attack"), None))),
            reviewed
        );
        // Una etiqueta con solo contexto no cambia lo que dice el cálculo.
        let context = LegTag {
            effort: Some(9),
            ..LegTag::default()
        };
        assert_eq!(
            leg_status(error, Some(&context)),
            LegStatus::Error { reviewed: false }
        );
        assert_eq!(leg_status(clean, Some(&context)), LegStatus::Clean);
    }

    #[test]
    fn errors_are_split_by_type_and_subtype() {
        let total = run(&HistoryFilter::default()).total;
        // A cuenta 5 tramos, B 3 y D 1.
        assert_eq!(total.legs, 9);
        // A1, A2, A3, B1, B2 y D1.
        assert_eq!(total.errors, 6);
        assert_eq!(total.loss_s, 15.0 + 0.0 + 20.0 + 30.0 + 60.0 + 50.0);
        // A3 (sin revisar) y D1.
        assert_eq!((total.untyped, total.untyped_loss_s), (2, 70.0));
        assert_eq!(total.unreviewed, 1);
        assert_eq!((total.physical_legs, total.physical_loss_s), (1, 8.0));
        assert_eq!(
            total.by_type,
            [
                TypeCount {
                    error_type: "navigation".into(),
                    errors: 3,
                    loss_s: 105.0,
                    subtypes: vec![
                        sub(Some("parallel"), 2, 45.0),
                        sub(Some("overshoot"), 1, 60.0)
                    ],
                },
                TypeCount {
                    error_type: "attack".into(),
                    errors: 1,
                    loss_s: 0.0,
                    subtypes: vec![sub(None, 1, 0.0)],
                },
            ]
        );
    }

    #[test]
    fn errors_cross_with_format() {
        let common = run(&HistoryFilter::default());
        let formats: Vec<_> = common.by_format.iter().map(|f| f.format).collect();
        assert_eq!(
            formats,
            [
                Some(RaceFormat::Sprint),
                Some(RaceFormat::Middle),
                Some(RaceFormat::Long),
                None
            ]
        );
        let sprint = &common.by_format[0].total;
        assert_eq!((sprint.legs, sprint.errors, sprint.loss_s), (5, 3, 35.0));
        assert_eq!((sprint.untyped, sprint.unreviewed), (1, 1));
        assert_eq!(sprint.physical_legs, 1);
        // Un error cada uno: primero el que más pierde.
        let types: Vec<_> = sprint
            .by_type
            .iter()
            .map(|t| t.error_type.as_str())
            .collect();
        assert_eq!(types, ["navigation", "attack"]);

        let middle = &common.by_format[1].total;
        assert_eq!((middle.legs, middle.errors, middle.untyped), (3, 2, 0));
        // C no tiene rendimiento habitual.
        assert_eq!(common.by_format[2].total.legs, 0);
        assert!(common.by_format[2].total.by_type.is_empty());
        let none = &common.by_format[3].total;
        assert_eq!((none.legs, none.errors, none.untyped), (1, 1, 1));
    }

    #[test]
    fn errors_cross_with_leg_length() {
        let common = run(&HistoryFilter::default());
        let summary = |buckets: &[LengthErrorTypes]| -> Vec<_> {
            buckets
                .iter()
                .map(|b| {
                    (
                        b.from_s,
                        b.types.legs,
                        b.types.errors,
                        b.types.physical_legs,
                    )
                })
                .collect()
        };
        assert_eq!(
            summary(&common.by_leg_length),
            [
                (20.0, 2, 2, 0),  // A1, A2
                (30.0, 2, 1, 1),  // A3; A4 físico
                (60.0, 2, 1, 0),  // A5 (No), B1
                (120.0, 1, 1, 0), // B2
                (240.0, 1, 0, 0), // B3
                (480.0, 1, 1, 0), // D1
            ]
        );
        assert_eq!(common.by_leg_length[5].to_s, None);
        assert_eq!(common.by_leg_length[0].to_s, Some(30.0));
        // Formato y cubo a la vez: sprint de 20–30 s.
        let sprint_short = &common.by_format[0].by_leg_length[0].types;
        let types: Vec<_> = sprint_short
            .by_type
            .iter()
            .map(|t| (t.error_type.as_str(), t.errors))
            .collect();
        assert_eq!(types, [("navigation", 1), ("attack", 1)]);
        // Los tramos de cada formato suman los del total en cada cubo.
        for (i, bucket) in common.by_leg_length.iter().enumerate() {
            let sum: usize = common
                .by_format
                .iter()
                .map(|f| f.by_leg_length[i].types.legs)
                .sum();
            assert_eq!(sum, bucket.types.legs);
        }
    }

    #[test]
    fn the_history_filter_applies() {
        let filter = HistoryFilter {
            format: Some(RaceFormat::Middle),
            ..HistoryFilter::default()
        };
        let common = run(&filter);
        assert_eq!((common.total.legs, common.total.errors), (3, 2));
        assert_eq!(common.by_format[0].total.legs, 0);
        let empty = run(&HistoryFilter {
            from: "2027-01-01".parse().ok(),
            ..HistoryFilter::default()
        });
        assert_eq!(empty.total.legs, 0);
        assert!(empty.total.by_type.is_empty());
    }
}
