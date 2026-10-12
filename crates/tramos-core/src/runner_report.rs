//! Tiempo perdido de un corredor listo para enseñar: su recorrido, sus totales y, por tramo, la
//! referencia del recorrido junto a sus números.
//!
//! Es lo que dan `tramos analizar` (`docs/cli.md`) y la vista de carrera de la app
//! (`docs/app.md`), con la misma función para que coincidan. El cálculo es el de
//! [`crate::lost_time`] (`docs/tiempo-perdido.md`).

use serde::{Deserialize, Serialize};

use crate::consistency::race_consistency;
use crate::courses::{ClassRef, group_by_course};
use crate::gain_loss::{LosingStreak, leg_gains, losing_streaks};
use crate::identify::ResultRef;
use crate::lost_time::{LostTimeConfig, RunnerAnalysis, analyze_course};
use crate::model::{ControlCode, Event, RaceStatus};

/// Informe de un corredor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerReport {
    pub course: CourseSummary,
    pub lost_time: RunnerLostTime,
}

/// Recorrido del corredor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CourseSummary {
    /// Balizas del recorrido, sin salida ni meta.
    pub controls: Vec<ControlCode>,
    /// Categorías que corren este recorrido (comparten referencia).
    pub classes: Vec<ClassRef>,
    pub valid_runners: usize,
    pub weak_reference: bool,
}

/// Tiempo perdido del corredor: totales y un elemento por tramo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerLostTime {
    pub total_s: Option<f64>,
    pub usual_performance: Option<f64>,
    pub lost_time_s: Option<f64>,
    pub error_count: usize,
    pub time_without_errors_s: Option<f64>,
    /// Puesto que habría tenido con su tiempo sin errores ([`place_without_errors`]).
    pub place_without_errors: Option<PlaceWithoutErrors>,
    /// Tiempo ideal del recorrido completo (el acumulado del último tramo).
    pub ideal_time_s: Option<f64>,
    /// Diferencia respecto al tiempo ideal en meta.
    pub behind_ideal_s: Option<f64>,
    /// Rachas de dos o más tramos seguidos perdiendo (P5, [`crate::gain_loss`]).
    pub losing_streaks: Vec<LosingStreak>,
    /// Desviación típica de `IR_i` ponderada por `ref_i` en los tramos de patrones (P10,
    /// [`crate::consistency`]); 1 = 100 puntos de IR.
    pub consistency: Option<f64>,
    pub legs: Vec<LegReport>,
}

/// Puesto en la categoría con el tiempo sin errores (`docs/tiempo-perdido.md`, "Puesto sin
/// errores").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceWithoutErrors {
    /// 1 + clasificados de la categoría (sin contarle a él) con tiempo menor que su tiempo sin
    /// errores.
    pub place: usize,
    /// Clasificados de la categoría con tiempo, él incluido.
    pub classified: usize,
}

/// Un tramo: la referencia del recorrido y los números del corredor juntos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegReport {
    pub index: usize,
    pub from: ControlCode,
    pub to: ControlCode,
    pub split_s: Option<f64>,
    pub elapsed_s: Option<f64>,
    pub place: Option<usize>,
    pub reference_s: Option<f64>,
    pub reference_count: usize,
    pub valid_splits: usize,
    pub performance_index: Option<f64>,
    pub expected_s: Option<f64>,
    pub loss_s: Option<f64>,
    pub loss_pct: Option<f64>,
    pub is_error: bool,
    /// Ganancia `esp_i − t_i` y su acumulado (P5, [`crate::gain_loss`]).
    pub gain_s: Option<f64>,
    pub cumulative_gain_s: Option<f64>,
    pub ideal_elapsed_s: Option<f64>,
    pub behind_ideal_s: Option<f64>,
    pub is_last: bool,
    pub short_reference: bool,
    pub excluded_from_patterns: bool,
}

/// Informe del resultado `result` de `event`; `None` si `result` no existe en la carrera.
///
/// Calcula el tiempo perdido de todo el recorrido del corredor (la referencia sale de todas las
/// categorías que lo comparten) y se queda con su parte.
pub fn runner_report(
    event: &Event,
    result: ResultRef,
    config: &LostTimeConfig,
) -> Option<RunnerReport> {
    result.get(event)?;
    let group = group_by_course(event)
        .into_iter()
        .find(|g| g.classes.iter().any(|c| c.index == result.class_index))?;
    let course = analyze_course(event, &group, config);
    let runner = course
        .runners
        .iter()
        .find(|r| r.class_index == result.class_index && r.result_index == result.result_index)?;

    let losses: Vec<Option<f64>> = runner.legs.iter().map(|leg| leg.loss_s).collect();
    let gains = leg_gains(&losses);

    let legs = course
        .legs
        .iter()
        .zip(&runner.legs)
        .zip(&gains)
        .map(|((reference, leg), gain)| LegReport {
            index: reference.index,
            from: reference.from,
            to: reference.to,
            split_s: leg.split_s,
            elapsed_s: leg.elapsed_s,
            place: leg.place,
            reference_s: reference.reference_s,
            reference_count: reference.reference_count,
            valid_splits: reference.valid_splits,
            performance_index: leg.performance_index,
            expected_s: leg.expected_s,
            loss_s: leg.loss_s,
            loss_pct: leg.loss_pct,
            is_error: leg.is_error,
            gain_s: gain.gain_s,
            cumulative_gain_s: gain.cumulative_gain_s,
            ideal_elapsed_s: reference.ideal_elapsed_s,
            behind_ideal_s: leg.behind_ideal_s,
            is_last: reference.is_last,
            short_reference: reference.short_reference,
            excluded_from_patterns: reference.excluded_from_patterns,
        })
        .collect();

    let mut lost_time = RunnerLostTime {
        total_s: runner.total_s,
        usual_performance: runner.usual_performance,
        lost_time_s: runner.lost_time_s,
        error_count: runner.error_count,
        time_without_errors_s: runner.time_without_errors_s,
        place_without_errors: place_without_errors(&course.runners, runner),
        ideal_time_s: course.legs.last().and_then(|l| l.ideal_elapsed_s),
        behind_ideal_s: runner.legs.last().and_then(|l| l.behind_ideal_s),
        losing_streaks: losing_streaks(&gains),
        consistency: None,
        legs,
    };
    lost_time.consistency = race_consistency(&lost_time);

    Some(RunnerReport {
        course: CourseSummary {
            controls: course.course.controls.clone(),
            classes: course.classes.clone(),
            valid_runners: course.valid_runners,
            weak_reference: course.weak_reference,
        },
        lost_time,
    })
}

/// Puesto de `runner` en su categoría si hubiera corrido su tiempo sin errores; `None` si no
/// está clasificado o no tiene tiempo sin errores.
///
/// Se compara con el tiempo **real** de los demás clasificados, que también tuvieron sus
/// errores: no se les quitan. Con el mismo tiempo se comparte puesto. `runners` son los del
/// recorrido; solo cuentan los de la categoría de `runner`.
pub fn place_without_errors(
    runners: &[RunnerAnalysis],
    runner: &RunnerAnalysis,
) -> Option<PlaceWithoutErrors> {
    if runner.status != RaceStatus::Ok {
        return None;
    }
    let time = runner.time_without_errors_s?;
    let classified: Vec<&RunnerAnalysis> = runners
        .iter()
        .filter(|r| r.class_index == runner.class_index && r.status == RaceStatus::Ok)
        .filter(|r| r.total_s.is_some())
        .collect();
    let faster = classified
        .iter()
        .filter(|r| r.result_index != runner.result_index)
        .filter(|r| r.total_s.is_some_and(|t| t < time))
        .count();
    Some(PlaceWithoutErrors {
        place: 1 + faster,
        classified: classified.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lost_time::analyze_event;
    use crate::model::{
        Class, Course, FINISH_CODE, Punch, RaceResult, RaceStatus, Runner, START_CODE,
    };
    use chrono::{DateTime, NaiveDate, TimeDelta, Utc};

    fn at(s: i64) -> Option<DateTime<Utc>> {
        let t0 = DateTime::parse_from_rfc3339("2026-10-03T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        Some(t0 + TimeDelta::seconds(s))
    }

    /// Resultado con salida en 0 y picadas en `times` (31 y meta).
    fn result(times: [i64; 2]) -> RaceResult {
        RaceResult {
            runner: Runner {
                given_name: String::new(),
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
                class(1, "A", vec![31], vec![result([60, 120]), result([90, 150])]),
                class(2, "B", vec![40], vec![]),
                class(3, "C", vec![31], vec![result([70, 160])]),
            ],
        }
    }

    #[test]
    fn report_joins_reference_and_runner_legs_of_the_shared_course() {
        let e = event();
        let config = LostTimeConfig::default();
        let report = runner_report(
            &e,
            ResultRef {
                class_index: 2,
                result_index: 0,
            },
            &config,
        )
        .unwrap();
        // La referencia sale de las categorías A y C, que comparten recorrido.
        let names: Vec<&str> = report
            .course
            .classes
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(names, ["A", "C"]);
        assert_eq!(report.course.valid_runners, 3);
        assert_eq!(report.course.controls, [31]);

        // Coincide con el análisis completo de la carrera.
        let full = analyze_event(&e, &config);
        let course = &full.courses[0];
        let runner = course.runners.iter().find(|r| r.class_index == 2).unwrap();
        assert_eq!(report.lost_time.total_s, runner.total_s);
        assert_eq!(report.lost_time.legs.len(), 2);
        for ((leg, reference), own) in report
            .lost_time
            .legs
            .iter()
            .zip(&course.legs)
            .zip(&runner.legs)
        {
            assert_eq!(
                (leg.index, leg.from, leg.to),
                (reference.index, reference.from, reference.to)
            );
            assert_eq!(leg.reference_s, reference.reference_s);
            assert_eq!(leg.split_s, own.split_s);
            assert_eq!(leg.loss_s, own.loss_s);
            assert_eq!(leg.is_error, own.is_error);
            assert_eq!(leg.gain_s, own.loss_s.map(|p| -p));
        }
        assert_eq!(report.lost_time.legs[1].to, FINISH_CODE);
        assert!(report.lost_time.legs[1].is_last);
        // Sin el último tramo queda uno solo: no hay consistencia.
        assert_eq!(report.lost_time.consistency, None);
    }

    #[test]
    fn missing_result_has_no_report() {
        let e = event();
        let config = LostTimeConfig::default();
        let missing = |class_index, result_index| {
            runner_report(
                &e,
                ResultRef {
                    class_index,
                    result_index,
                },
                &config,
            )
        };
        assert_eq!(missing(0, 2), None);
        assert_eq!(missing(9, 0), None);
        // La categoría B no tiene resultados.
        assert_eq!(missing(1, 0), None);
    }

    /// Corredor del recorrido con lo que mira el puesto sin errores.
    fn analysis(
        class_index: usize,
        result_index: usize,
        status: RaceStatus,
        total_s: Option<f64>,
        time_without_errors_s: Option<f64>,
    ) -> RunnerAnalysis {
        RunnerAnalysis {
            class_index,
            result_index,
            status,
            place: None,
            total_s,
            usual_performance: None,
            lost_time_s: None,
            error_count: 0,
            time_without_errors_s,
            legs: vec![],
        }
    }

    #[test]
    fn place_without_errors_counts_faster_classified_of_the_same_class() {
        let ok = RaceStatus::Ok;
        let runners = [
            analysis(0, 0, ok, Some(600.0), Some(600.0)),
            analysis(0, 1, ok, Some(700.0), Some(650.0)),
            // Él: 900 s, 680 s sin errores.
            analysis(0, 2, ok, Some(900.0), Some(680.0)),
            // Empata con su tiempo sin errores: comparten puesto, no le adelanta.
            analysis(0, 3, ok, Some(680.0), Some(680.0)),
            analysis(0, 4, ok, Some(800.0), Some(700.0)),
            // No clasificado más rápido y clasificado sin tiempo: no cuentan.
            analysis(0, 5, RaceStatus::NotClassified, Some(500.0), None),
            analysis(0, 6, ok, None, None),
            // Otra categoría del mismo recorrido: no cuenta.
            analysis(1, 0, ok, Some(400.0), Some(400.0)),
        ];
        // Se mira el tiempo real de los demás (600, 700, 680 y 800 s), no su tiempo sin
        // errores: solo el de 600 s es más rápido que 680 s.
        assert_eq!(
            place_without_errors(&runners, &runners[2]),
            Some(PlaceWithoutErrors {
                place: 2,
                classified: 5
            })
        );
        // El ganador sin errores sigue primero.
        assert_eq!(
            place_without_errors(&runners, &runners[0]),
            Some(PlaceWithoutErrors {
                place: 1,
                classified: 5
            })
        );
        // Sin clasificar o sin tiempo sin errores no hay puesto.
        assert_eq!(place_without_errors(&runners, &runners[5]), None);
        assert_eq!(place_without_errors(&runners, &runners[6]), None);
    }

    #[test]
    fn report_carries_the_place_without_errors() {
        let e = event();
        let config = LostTimeConfig::default();
        let report = runner_report(
            &e,
            ResultRef {
                class_index: 0,
                result_index: 0,
            },
            &config,
        )
        .unwrap();
        // El ganador de A (120 s) sigue primero entre los dos clasificados de A; C no cuenta
        // aunque comparta recorrido.
        assert_eq!(
            report.lost_time.place_without_errors,
            Some(PlaceWithoutErrors {
                place: 1,
                classified: 2
            })
        );
    }
}
