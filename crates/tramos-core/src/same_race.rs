//! Una carrera con varios atletas (#163): quien entrena ve juntos a los atletas que corrieron la
//! misma carrera (el mismo `race_id`, `docs/paquete.md`), cada uno medido contra su propio ritmo.
//!
//! Cada atleta llega con **su** copia de la carrera (la de su paquete o la propia), su resultado,
//! sus umbrales y sus etiquetas, y se analiza como en su vista de carrera
//! ([`crate::runner_report`] y [`crate::comparison`]): sus números son los que ve él. Después se
//! juntan por recorrido, porque las gráficas por tramo solo tienen sentido entre quienes
//! corrieron las mismas balizas. Ver `docs/tiempo-perdido.md`, "Una carrera con varios atletas".

use serde::{Deserialize, Serialize};

use crate::common_errors::{LegStatus, leg_status};
use crate::comparison::{ComparisonLeg, course_comparison};
use crate::identify::ResultRef;
use crate::lost_time::LostTimeConfig;
use crate::model::{ControlCode, Event};
use crate::package::RaceSummary;
use crate::runner_report::{RunnerReport, runner_report};
use crate::taxonomy::LegTag;

/// Un atleta en la carrera, con lo suyo.
#[derive(Debug, Clone, Copy)]
pub struct SameRaceInput<'a> {
    /// Su copia de la carrera: la de su paquete (solo las categorías de su recorrido) o la propia.
    pub event: &'a Event,
    pub result: ResultRef,
    /// Sus umbrales.
    pub config: LostTimeConfig,
    /// `(tramo, etiqueta)`, con el tramo desde 1 como `LegReport::index`.
    pub tags: &'a [(usize, LegTag)],
}

/// Los atletas de una carrera, juntos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SameRace {
    /// Uno por atleta de la entrada, en el mismo orden. `None` si su resultado no está en su
    /// carrera o no se puede analizar.
    pub runners: Vec<Option<SameRaceRunner>>,
    /// Los recorridos, del que tienen más atletas al que menos (a igualdad, en el orden en que
    /// aparece su primer atleta en la entrada).
    pub courses: Vec<SameRaceCourse>,
}

/// Lo de un atleta: su resumen, como el de su paquete, y su recorrido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SameRaceRunner {
    pub summary: RaceSummary,
    /// Posición de su recorrido en [`SameRace::courses`].
    pub course: usize,
    /// Puesto en el recorrido, juntando todas sus categorías (como en P4). Solo los
    /// clasificados con tiempo.
    pub course_place: Option<usize>,
}

/// Un recorrido de la carrera con sus atletas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SameRaceCourse {
    /// Balizas, sin salida ni meta.
    pub controls: Vec<ControlCode>,
    /// Las categorías que lo corren, en el orden de la carrera (de la copia de su primer atleta).
    pub class_names: Vec<String>,
    pub legs: Vec<ComparisonLeg>,
    /// Tiempo ideal del recorrido entero (de la copia de su primer atleta: con los mismos
    /// originales es el mismo para todos).
    pub ideal_time_s: Option<f64>,
    /// Menos de 4 clasificados en el recorrido.
    pub weak_reference: bool,
    /// Sus atletas, por puesto en el recorrido; los que no tienen puesto, detrás, en el orden de
    /// la entrada.
    pub runners: Vec<CourseRunner>,
    /// Tramos con error en dos o más de sus atletas, en orden de tramo.
    pub shared_errors: Vec<SharedLegError>,
}

/// Las series por tramo de un atleta (una posición por tramo), como en «Frente al grupo».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CourseRunner {
    /// Posición del atleta en la entrada.
    pub runner: usize,
    /// Diferencia acumulada respecto al tiempo ideal al acabar cada tramo.
    pub behind_ideal_s: Vec<Option<f64>>,
    /// Pérdida de cada tramo frente a lo esperado con su rendimiento habitual.
    pub loss_s: Vec<Option<f64>>,
    /// Tramo con error de orientación: lo dice su etiqueta si la hay y, si no, el cálculo
    /// ([`leg_status`]). Lo que etiquetó como físico no es error.
    pub is_error: Vec<bool>,
}

/// Un tramo en el que fallaron dos atletas o más.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedLegError {
    /// Desde 1.
    pub leg: usize,
    pub from: ControlCode,
    pub to: ControlCode,
    /// Los atletas que fallaron, en el orden de [`SameRaceCourse::runners`].
    pub errors: Vec<RunnerLegError>,
}

/// El error de un atleta en un tramo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerLegError {
    /// Posición del atleta en la entrada.
    pub runner: usize,
    /// Su pérdida calculada en el tramo.
    pub loss_s: Option<f64>,
    /// Lo ha confirmado con su etiqueta (si no, solo lo dice el cálculo).
    pub reviewed: bool,
    /// Su etiqueta del tramo, si la tiene.
    pub tag: Option<LegTag>,
}

/// Un atleta ya analizado.
struct Analyzed {
    report: RunnerReport,
    summary: RaceSummary,
    course_place: Option<usize>,
    statuses: Vec<LegStatus>,
}

/// Los atletas de `inputs` juntos, cada uno con su copia de la carrera y sus umbrales.
pub fn same_race(inputs: &[SameRaceInput<'_>]) -> SameRace {
    let analyzed: Vec<Option<Analyzed>> = inputs.iter().map(analyze).collect();

    // Recorridos por sus balizas, en el orden de su primer atleta.
    let mut keys: Vec<&[ControlCode]> = Vec::new();
    let mut members: Vec<Vec<usize>> = Vec::new();
    for (i, a) in analyzed.iter().enumerate() {
        let Some(a) = a else { continue };
        let controls = a.report.course.controls.as_slice();
        match keys.iter().position(|k| *k == controls) {
            Some(k) => members[k].push(i),
            None => {
                keys.push(controls);
                members.push(vec![i]);
            }
        }
    }
    // Más atletas primero; a igualdad, el orden de antes (la ordenación es estable).
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by_key(|&k| std::cmp::Reverse(members[k].len()));

    let mut course_of = vec![None; inputs.len()];
    let mut courses = Vec::with_capacity(order.len());
    for (position, &k) in order.iter().enumerate() {
        let mut ids = members[k].clone();
        for &i in &ids {
            course_of[i] = Some(position);
        }
        ids.sort_by(|&a, &b| {
            let place = |i: usize| analyzed[i].as_ref().and_then(|a| a.course_place);
            match (place(a), place(b)) {
                (Some(pa), Some(pb)) => pa.cmp(&pb),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
        let runners: Vec<(usize, &Analyzed)> = ids
            .iter()
            .filter_map(|&i| analyzed[i].as_ref().map(|a| (i, a)))
            .collect();
        courses.push(course(inputs, &runners, members[k][0]));
    }

    SameRace {
        runners: analyzed
            .into_iter()
            .zip(course_of)
            .map(|(a, course)| {
                let (a, course) = (a?, course?);
                Some(SameRaceRunner {
                    summary: a.summary,
                    course,
                    course_place: a.course_place,
                })
            })
            .collect(),
        courses,
    }
}

/// Un atleta como en su vista de carrera: su informe, su resumen, su puesto en el recorrido y
/// el estado de cada tramo con sus etiquetas.
fn analyze(input: &SameRaceInput<'_>) -> Option<Analyzed> {
    let report = runner_report(input.event, input.result, &input.config)?;
    let summary = RaceSummary::of(input.event, input.result, &report)?;
    let course_place = course_comparison(input.event, input.result, &input.config)?
        .runners
        .iter()
        .find(|r| r.is_self)
        .and_then(|r| r.course_place);
    let statuses = report
        .lost_time
        .legs
        .iter()
        .map(|leg| {
            let tag = input
                .tags
                .iter()
                .find(|(index, _)| *index == leg.index)
                .map(|(_, tag)| tag);
            leg_status(leg, tag)
        })
        .collect();
    Some(Analyzed {
        report,
        summary,
        course_place,
        statuses,
    })
}

/// El recorrido de `runners` (ya ordenados), con los tramos y el ideal de la copia del atleta
/// `first` (el primero de la entrada en este recorrido).
fn course(
    inputs: &[SameRaceInput<'_>],
    runners: &[(usize, &Analyzed)],
    first: usize,
) -> SameRaceCourse {
    let base = runners
        .iter()
        .find(|(i, _)| *i == first)
        .map(|(_, a)| &a.report);
    let legs: Vec<ComparisonLeg> = base
        .map(|r| {
            r.lost_time
                .legs
                .iter()
                .map(|l| ComparisonLeg {
                    index: l.index,
                    from: l.from,
                    to: l.to,
                })
                .collect()
        })
        .unwrap_or_default();
    let class_names = base
        .map(|r| r.course.classes.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();

    let shared_errors = legs
        .iter()
        .enumerate()
        .filter_map(|(position, leg)| {
            let errors: Vec<RunnerLegError> = runners
                .iter()
                .filter_map(|&(i, a)| {
                    let LegStatus::Error { reviewed } = *a.statuses.get(position)? else {
                        return None;
                    };
                    Some(RunnerLegError {
                        runner: i,
                        loss_s: a.report.lost_time.legs.get(position)?.loss_s,
                        reviewed,
                        tag: inputs[i]
                            .tags
                            .iter()
                            .find(|(index, _)| *index == leg.index)
                            .map(|(_, tag)| tag.clone()),
                    })
                })
                .collect();
            (errors.len() >= 2).then_some(SharedLegError {
                leg: leg.index,
                from: leg.from,
                to: leg.to,
                errors,
            })
        })
        .collect();

    SameRaceCourse {
        controls: base.map(|r| r.course.controls.clone()).unwrap_or_default(),
        class_names,
        ideal_time_s: base.and_then(|r| r.lost_time.ideal_time_s),
        weak_reference: base.is_some_and(|r| r.course.weak_reference),
        legs,
        runners: runners
            .iter()
            .map(|&(i, a)| CourseRunner {
                runner: i,
                behind_ideal_s: a
                    .report
                    .lost_time
                    .legs
                    .iter()
                    .map(|l| l.behind_ideal_s)
                    .collect(),
                loss_s: a.report.lost_time.legs.iter().map(|l| l.loss_s).collect(),
                is_error: a
                    .statuses
                    .iter()
                    .map(|s| matches!(s, LegStatus::Error { .. }))
                    .collect(),
            })
            .collect(),
        shared_errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Class, Course, FINISH_CODE, Punch, RaceResult, RaceStatus, Runner};
    use crate::model::{ControlCode as Code, START_CODE};
    use crate::package::shared_course;
    use crate::taxonomy::Confirmation;
    use chrono::{DateTime, NaiveDate, TimeDelta, Utc};

    fn at(s: i64) -> Option<DateTime<Utc>> {
        let t0 = DateTime::parse_from_rfc3339("2026-10-03T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        Some(t0 + TimeDelta::seconds(s))
    }

    /// Resultado clasificado con salida en 0, picada en `control` a `times[0]` y meta a
    /// `times[1]`.
    fn result(name: &str, control: Code, times: [i64; 2]) -> RaceResult {
        RaceResult {
            runner: Runner {
                given_name: name.into(),
                family_name: String::new(),
                club: None,
                bib: None,
                si_card: None,
                sex: None,
            },
            status: RaceStatus::Ok,
            place: None,
            punches: vec![
                Punch {
                    code: START_CODE,
                    time: at(0),
                },
                Punch {
                    code: control,
                    time: at(times[0]),
                },
                Punch {
                    code: FINISH_CODE,
                    time: at(times[1]),
                },
            ],
        }
    }

    /// A y C comparten el recorrido 31 (4 corredores); B corre el 40 (2 corredores).
    ///
    /// Recorrido 31: splits del tramo 1 = 60, 60, 60, 70 → ref 60; del tramo 2 = 90, 60, 50,
    /// 110 → ref 50. Ideal acumulado: 60 y 110.
    fn event() -> Event {
        let class = |id, name: &str, control: Code, results| Class {
            id,
            name: name.into(),
            short_name: None,
            course: Course {
                controls: vec![control],
            },
            results,
        };
        Event {
            name: Some("Carrera inventada".into()),
            date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
            classes: vec![
                class(
                    1,
                    "A",
                    31,
                    vec![result("a0", 31, [60, 150]), result("a1", 31, [60, 120])],
                ),
                class(
                    2,
                    "B",
                    40,
                    vec![result("b0", 40, [10, 20]), result("b1", 40, [12, 30])],
                ),
                class(
                    3,
                    "C",
                    31,
                    vec![result("c0", 31, [60, 110]), result("c1", 31, [70, 180])],
                ),
            ],
        }
    }

    fn r(class_index: usize, result_index: usize) -> ResultRef {
        ResultRef {
            class_index,
            result_index,
        }
    }

    fn input<'a>(
        event: &'a Event,
        at: ResultRef,
        tags: &'a [(usize, LegTag)],
    ) -> SameRaceInput<'a> {
        SameRaceInput {
            event,
            result: at,
            config: LostTimeConfig::default(),
            tags,
        }
    }

    fn close(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-9)
    }

    #[test]
    fn athletes_of_the_same_course_go_together_and_each_one_against_their_own_pace() {
        let e = event();
        // a0 (A), c1 (C) y b1 (B): dos en el recorrido 31 y uno en el 40.
        let inputs = [
            input(&e, r(0, 0), &[]),
            input(&e, r(2, 1), &[]),
            input(&e, r(1, 1), &[]),
        ];
        let race = same_race(&inputs);

        assert_eq!(race.courses.len(), 2);
        let shared = &race.courses[0];
        assert_eq!(shared.controls, [31]);
        assert_eq!(shared.class_names, ["A", "C"]);
        assert_eq!(shared.legs.len(), 2);
        assert!(close(shared.ideal_time_s, 110.0));
        assert!(!shared.weak_reference);
        let other = &race.courses[1];
        assert_eq!(other.controls, [40]);
        assert!(other.weak_reference);
        assert_eq!(other.runners.len(), 1);
        assert_eq!(other.runners[0].runner, 2);

        // Totales 110 (c0), 120 (a1), 150 (a0), 180 (c1): a0 es 3.º y c1 4.º del recorrido.
        let runners: Vec<usize> = shared.runners.iter().map(|r| r.runner).collect();
        assert_eq!(runners, [0, 1]);
        let a0 = race.runners[0].as_ref().unwrap();
        let c1 = race.runners[1].as_ref().unwrap();
        assert_eq!((a0.course, a0.course_place), (0, Some(3)));
        assert_eq!((c1.course, c1.course_place), (0, Some(4)));
        assert_eq!(a0.summary.class_name, "A");
        assert_eq!(c1.summary.class_name, "C");

        // a0: IR 1 y 50/90; habitual 1 (mediana ponderada por 60 y 50) → pierde 40 s en el 2.
        let a0_series = &shared.runners[0];
        assert!(close(a0_series.loss_s[0], 0.0));
        assert!(close(a0_series.loss_s[1], 40.0));
        assert!(close(a0_series.behind_ideal_s[0], 0.0));
        assert!(close(a0_series.behind_ideal_s[1], 40.0));
        assert_eq!(a0_series.is_error, [false, true]);
        assert!(close(a0.summary.lost_time_s, 40.0));
        assert_eq!(a0.summary.error_count, 1);
        // c1: IR 60/70 y 50/110; habitual 6/7 → esperado 58,33 s en el 2: pierde 51,67 s.
        let c1_series = &shared.runners[1];
        assert!(close(c1_series.loss_s[0], 0.0));
        assert!(close(c1_series.loss_s[1], 110.0 - 50.0 * 7.0 / 6.0));
        assert!(close(c1_series.behind_ideal_s[0], 10.0));
        assert!(close(c1_series.behind_ideal_s[1], 70.0));
        assert!(close(c1.summary.total_s, 180.0));

        // Los dos fallaron el tramo 2.
        assert_eq!(shared.shared_errors.len(), 1);
        let leg = &shared.shared_errors[0];
        assert_eq!((leg.leg, leg.from, leg.to), (2, 31, FINISH_CODE));
        let who: Vec<usize> = leg.errors.iter().map(|e| e.runner).collect();
        assert_eq!(who, [0, 1]);
        assert!(leg.errors.iter().all(|e| !e.reviewed && e.tag.is_none()));
        assert!(close(leg.errors[0].loss_s, 40.0));

        // b1, solo en su recorrido: sin comparación ni errores compartidos.
        let b1 = race.runners[2].as_ref().unwrap();
        assert_eq!((b1.course, b1.course_place), (1, Some(2)));
        assert!(other.shared_errors.is_empty());
        assert!(close(other.runners[0].behind_ideal_s[1], 10.0));
    }

    #[test]
    fn each_athlete_with_their_own_reduced_copy_gives_the_same_numbers() {
        let e = event();
        let full = same_race(&[input(&e, r(0, 0), &[]), input(&e, r(2, 1), &[])]);
        // Como llegan en los paquetes: solo las categorías de su recorrido.
        let a0 = shared_course(&e, r(0, 0)).unwrap();
        let c1 = shared_course(&e, r(2, 1)).unwrap();
        assert_eq!(c1.event.classes.len(), 2);
        let reduced = same_race(&[
            input(&a0.event, a0.result, &[]),
            input(&c1.event, c1.result, &[]),
        ]);
        assert_eq!(reduced, full);
    }

    #[test]
    fn the_tags_decide_what_was_an_error() {
        let e = event();
        let typed = LegTag {
            confirmation: Some(Confirmation::Error),
            error_type: Some("direccion".into()),
            ..LegTag::default()
        };
        let a0_tags = [(2, typed.clone())];
        // c1 dice que el tramo 2 no fue error: ya solo falla a0, que no es «varios».
        let not_an_error = [(
            2,
            LegTag {
                confirmation: Some(Confirmation::NoError),
                ..LegTag::default()
            },
        )];
        let race = same_race(&[
            input(&e, r(0, 0), &a0_tags),
            input(&e, r(2, 1), &not_an_error),
        ]);
        let course = &race.courses[0];
        assert!(course.shared_errors.is_empty());
        assert_eq!(course.runners[1].is_error, [false, false]);
        // El cálculo no cambia: la pérdida sigue ahí.
        assert!(close(course.runners[1].loss_s[1], 110.0 - 50.0 * 7.0 / 6.0));

        // Con c1 sin etiqueta, vuelven a ser dos; el de a0 lleva su etiqueta y va revisado.
        let race = same_race(&[input(&e, r(0, 0), &a0_tags), input(&e, r(2, 1), &[])]);
        let leg = &race.courses[0].shared_errors[0];
        assert!(leg.errors[0].reviewed);
        assert_eq!(leg.errors[0].tag.as_ref(), Some(&typed));
        assert!(!leg.errors[1].reviewed);

        // Lo físico tampoco es error de orientación.
        let physical = [(
            2,
            LegTag {
                confirmation: Some(Confirmation::Physical),
                ..LegTag::default()
            },
        )];
        let race = same_race(&[input(&e, r(2, 1), &physical)]);
        assert_eq!(race.courses[0].runners[0].is_error, [false, false]);
    }

    #[test]
    fn a_single_athlete_has_their_course_and_nothing_to_compare() {
        let e = event();
        let race = same_race(&[input(&e, r(1, 1), &[])]);
        assert_eq!(race.runners.len(), 1);
        assert_eq!(race.courses.len(), 1);
        assert_eq!(race.courses[0].runners.len(), 1);
        assert!(race.courses[0].shared_errors.is_empty());
    }

    #[test]
    fn a_result_that_is_not_in_the_race_is_left_out() {
        let e = event();
        let race = same_race(&[input(&e, r(0, 9), &[]), input(&e, r(0, 1), &[])]);
        assert_eq!(race.runners[0], None);
        assert!(race.runners[1].is_some());
        assert_eq!(race.courses.len(), 1);
        let runners: Vec<usize> = race.courses[0].runners.iter().map(|r| r.runner).collect();
        assert_eq!(runners, [1]);
        assert!(same_race(&[]).courses.is_empty());
    }

    #[test]
    fn the_course_with_more_athletes_goes_first() {
        let e = event();
        // b0 y b1 en el 40; a1 en el 31: el 40 va primero aunque su primer atleta llegue después.
        let race = same_race(&[
            input(&e, r(0, 1), &[]),
            input(&e, r(1, 1), &[]),
            input(&e, r(1, 0), &[]),
        ]);
        let controls: Vec<&[Code]> = race.courses.iter().map(|c| c.controls.as_slice()).collect();
        assert_eq!(controls, [&[40][..], &[31][..]]);
        // b0 (20 s) va antes que b1 (30 s).
        let runners: Vec<usize> = race.courses[0].runners.iter().map(|r| r.runner).collect();
        assert_eq!(runners, [2, 1]);
        assert_eq!(race.runners[0].as_ref().unwrap().course, 1);
    }
}
