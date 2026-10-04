//! Frente al grupo (P4 de `docs/preguntas.md`): los corredores del mismo recorrido que un
//! resultado, con lo que hace falta para superponerlos en las gráficas: la diferencia acumulada
//! respecto al tiempo ideal y la pérdida de cada tramo.
//!
//! Sale del mismo análisis que el informe del corredor ([`crate::lost_time::analyze_course`]),
//! así que sus números coinciden con los de [`crate::runner_report`]. Ver
//! `docs/tiempo-perdido.md`, "Frente al grupo".

use serde::{Deserialize, Serialize};

use crate::courses::group_by_course;
use crate::identify::ResultRef;
use crate::lost_time::{LostTimeConfig, analyze_course};
use crate::model::{ControlCode, Event, RaceStatus};

/// Corredores del recorrido de un resultado, listos para compararse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CourseComparison {
    /// Tramos del recorrido, en orden.
    pub legs: Vec<ComparisonLeg>,
    /// Tiempo ideal del recorrido completo, según `LostTimeConfig::ideal_time`.
    pub ideal_time_s: Option<f64>,
    /// Clasificados por tiempo total y después el resto, en el orden de la carrera.
    pub runners: Vec<ComparedRunner>,
}

/// Un tramo del recorrido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonLeg {
    pub index: usize,
    pub from: ControlCode,
    pub to: ControlCode,
}

/// Un corredor del recorrido con sus series por tramo (una posición por tramo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparedRunner {
    pub result: ResultRef,
    /// Es el resultado por el que se ha pedido la comparación.
    pub is_self: bool,
    pub given_name: String,
    pub family_name: String,
    pub club: Option<String>,
    pub class_name: String,
    pub status: RaceStatus,
    /// Puesto en su categoría (el del fichero).
    pub place: Option<u16>,
    /// Puesto en el recorrido, juntando todas sus categorías: 1 + clasificados con tiempo total
    /// estrictamente menor. Solo los clasificados con tiempo.
    pub course_place: Option<usize>,
    pub total_s: Option<f64>,
    /// Diferencia acumulada respecto al tiempo ideal al acabar cada tramo.
    pub behind_ideal_s: Vec<Option<f64>>,
    /// Pérdida de cada tramo frente a lo esperado con su rendimiento habitual.
    pub loss_s: Vec<Option<f64>>,
    pub is_error: Vec<bool>,
}

/// Corredores del recorrido de `result`; `None` si `result` no existe en la carrera.
pub fn course_comparison(
    event: &Event,
    result: ResultRef,
    config: &LostTimeConfig,
) -> Option<CourseComparison> {
    result.get(event)?;
    let group = group_by_course(event)
        .into_iter()
        .find(|g| g.classes.iter().any(|c| c.index == result.class_index))?;
    let course = analyze_course(event, &group, config);

    let classified_total =
        |status: RaceStatus, total: Option<f64>| total.filter(|_| status == RaceStatus::Ok);
    let totals: Vec<f64> = course
        .runners
        .iter()
        .filter_map(|r| classified_total(r.status, r.total_s))
        .collect();

    let mut runners: Vec<ComparedRunner> = course
        .runners
        .iter()
        .filter_map(|r| {
            let at = ResultRef {
                class_index: r.class_index,
                result_index: r.result_index,
            };
            let entry = at.get(event)?;
            let class = event.classes.get(r.class_index)?;
            let course_place = classified_total(r.status, r.total_s)
                .map(|t| 1 + totals.iter().filter(|&&other| other < t).count());
            Some(ComparedRunner {
                result: at,
                is_self: at == result,
                given_name: entry.runner.given_name.clone(),
                family_name: entry.runner.family_name.clone(),
                club: entry.runner.club.clone(),
                class_name: class.name.clone(),
                status: r.status,
                place: r.place,
                course_place,
                total_s: r.total_s,
                behind_ideal_s: r.legs.iter().map(|l| l.behind_ideal_s).collect(),
                loss_s: r.legs.iter().map(|l| l.loss_s).collect(),
                is_error: r.legs.iter().map(|l| l.is_error).collect(),
            })
        })
        .collect();
    // Orden estable: los clasificados por tiempo (empates en el orden de la carrera), el resto
    // detrás tal cual.
    runners.sort_by(|a, b| match (a.course_place, b.course_place) {
        (Some(pa), Some(pb)) => pa.cmp(&pb),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    Some(CourseComparison {
        legs: course
            .legs
            .iter()
            .map(|l| ComparisonLeg {
                index: l.index,
                from: l.from,
                to: l.to,
            })
            .collect(),
        ideal_time_s: course.legs.last().and_then(|l| l.ideal_elapsed_s),
        runners,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Class, Course, FINISH_CODE, Punch, RaceResult, Runner, START_CODE};
    use crate::runner_report::runner_report;
    use chrono::{DateTime, NaiveDate, TimeDelta, Utc};

    fn at(s: i64) -> Option<DateTime<Utc>> {
        let t0 = DateTime::parse_from_rfc3339("2026-10-03T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        Some(t0 + TimeDelta::seconds(s))
    }

    /// Resultado con salida en 0 y picadas en `times` (31 y meta).
    fn result(name: &str, status: RaceStatus, times: [i64; 2]) -> RaceResult {
        RaceResult {
            runner: Runner {
                given_name: name.into(),
                family_name: String::new(),
                club: None,
                bib: None,
                si_card: None,
                sex: None,
            },
            status,
            place: None,
            punches: vec![
                Punch {
                    code: START_CODE,
                    time: at(0),
                },
                Punch {
                    code: 31,
                    time: at(times[0]),
                },
                Punch {
                    code: FINISH_CODE,
                    time: at(times[1]),
                },
            ],
        }
    }

    /// Dos categorías con el mismo recorrido y una tercera con otro.
    fn event() -> Event {
        let class = |id, name: &str, controls: Vec<u16>, results| Class {
            id,
            name: name.into(),
            short_name: None,
            course: Course { controls },
            results,
        };
        Event {
            name: None,
            date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
            classes: vec![
                class(
                    1,
                    "A",
                    vec![31],
                    vec![
                        result("a0", RaceStatus::Ok, [90, 150]),
                        result("a1", RaceStatus::NotClassified, [50, 100]),
                    ],
                ),
                class(
                    2,
                    "B",
                    vec![40],
                    vec![result("b0", RaceStatus::Ok, [10, 20])],
                ),
                class(
                    3,
                    "C",
                    vec![31],
                    vec![
                        result("c0", RaceStatus::Ok, [60, 120]),
                        result("c1", RaceStatus::Ok, [70, 150]),
                    ],
                ),
            ],
        }
    }

    fn me() -> ResultRef {
        ResultRef {
            class_index: 0,
            result_index: 0,
        }
    }

    #[test]
    fn runners_of_the_shared_course_sorted_by_total_time() {
        let c = course_comparison(&event(), me(), &LostTimeConfig::default()).unwrap();
        let names: Vec<&str> = c.runners.iter().map(|r| r.given_name.as_str()).collect();
        // c0 120 s; a0 y c1 empatan a 150 s (orden de la carrera); a1 no clasificado, al final.
        // b0 corre otro recorrido.
        assert_eq!(names, ["c0", "a0", "c1", "a1"]);
        let places: Vec<_> = c.runners.iter().map(|r| r.course_place).collect();
        assert_eq!(places, [Some(1), Some(2), Some(2), None]);
        let selves: Vec<_> = c.runners.iter().map(|r| r.is_self).collect();
        assert_eq!(selves, [false, true, false, false]);
        assert_eq!(c.runners[0].class_name, "C");
        assert_eq!(c.legs.len(), 2);
        assert_eq!((c.legs[1].from, c.legs[1].to), (31, FINISH_CODE));
    }

    #[test]
    fn series_match_each_runner_report() {
        let e = event();
        let config = LostTimeConfig::default();
        let c = course_comparison(&e, me(), &config).unwrap();
        for runner in &c.runners {
            let report = runner_report(&e, runner.result, &config).unwrap();
            let legs = &report.lost_time.legs;
            assert_eq!(runner.total_s, report.lost_time.total_s);
            assert_eq!(
                runner.behind_ideal_s,
                legs.iter().map(|l| l.behind_ideal_s).collect::<Vec<_>>()
            );
            assert_eq!(
                runner.loss_s,
                legs.iter().map(|l| l.loss_s).collect::<Vec<_>>()
            );
            assert_eq!(c.ideal_time_s, report.lost_time.ideal_time_s);
        }
    }

    #[test]
    fn winner_finishes_behind_the_ideal_by_total_minus_ideal() {
        let c = course_comparison(&event(), me(), &LostTimeConfig::default()).unwrap();
        let winner = &c.runners[0];
        let ideal = c.ideal_time_s.unwrap();
        let at_finish = winner.behind_ideal_s.last().copied().flatten().unwrap();
        assert!((at_finish - (winner.total_s.unwrap() - ideal)).abs() < 1e-9);
    }

    #[test]
    fn missing_result_has_no_comparison() {
        let e = event();
        let missing = ResultRef {
            class_index: 0,
            result_index: 5,
        };
        assert_eq!(
            course_comparison(&e, missing, &LostTimeConfig::default()),
            None
        );
    }
}
