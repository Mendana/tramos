//! Tiempo perdido por recorrido (`docs/tiempo-perdido.md`).
//!
//! Adaptación del método de WinSplits. Para cada recorrido ([`CourseGroup`]):
//!
//! 1. Split de cada tramo a partir de las picadas de cada corredor.
//! 2. Referencia del tramo: media del 25 % más rápido de los clasificados (`RaceStatus::Ok`).
//! 3. Índice de rendimiento `IR_i = ref_i / t_i` y rendimiento habitual del corredor (mediana de
//!    sus `IR_i` ponderada por `ref_i`).
//! 4. Tiempo esperado `ref_i / habitual`, pérdida y error según los umbrales de
//!    [`LostTimeConfig`].
//!
//! Los corredores se identifican por su posición en el `Event` (índice de categoría e índice de
//! resultado), nunca por `Runner::id`, que no es único.

use std::iter;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::courses::{ClassRef, CourseGroup, group_by_course};
use crate::model::{
    ControlCode, Course, Event, FINISH_CODE, Punch, RaceResult, RaceStatus, START_CODE,
};

/// Umbral de error en segundos por defecto.
pub const DEFAULT_ERROR_THRESHOLD_S: f64 = 15.0;
/// Umbral de error en porcentaje del tiempo esperado por defecto.
pub const DEFAULT_ERROR_THRESHOLD_PCT: f64 = 10.0;
/// Por debajo de esta referencia (segundos) el tramo se excluye de los análisis de patrones.
pub const SHORT_REFERENCE_S: f64 = 20.0;
/// Con menos corredores clasificados que este número, la referencia del recorrido es débil.
pub const MIN_RUNNERS_FOR_STRONG_REFERENCE: usize = 4;

/// Umbrales de error. Un tramo es error si la pérdida supera **los dos**.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LostTimeConfig {
    /// Pérdida mínima en segundos (estricta: tiene que ser mayor).
    pub error_threshold_s: f64,
    /// Pérdida mínima en porcentaje del tiempo esperado (10 = 10 %; estricta).
    pub error_threshold_pct: f64,
}

impl Default for LostTimeConfig {
    fn default() -> Self {
        Self {
            error_threshold_s: DEFAULT_ERROR_THRESHOLD_S,
            error_threshold_pct: DEFAULT_ERROR_THRESHOLD_PCT,
        }
    }
}

/// Tiempo perdido de toda una carrera: un análisis por recorrido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LostTimeReport {
    /// Umbrales con los que se calculó.
    pub config: LostTimeConfig,
    /// En el orden de [`group_by_course`].
    pub courses: Vec<CourseAnalysis>,
}

/// Tiempo perdido de un recorrido: referencias por tramo y análisis de cada corredor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CourseAnalysis {
    pub course: Course,
    pub classes: Vec<ClassRef>,
    /// Corredores clasificados (`RaceStatus::Ok`) del recorrido: la población de la referencia.
    pub valid_runners: usize,
    /// Menos de [`MIN_RUNNERS_FOR_STRONG_REFERENCE`] clasificados: la referencia es frágil.
    pub weak_reference: bool,
    /// Un elemento por tramo (`n` balizas → `n + 1` tramos).
    pub legs: Vec<LegReference>,
    /// Todos los resultados del recorrido, categoría a categoría y en el orden del fichero.
    pub runners: Vec<RunnerAnalysis>,
}

/// Referencia de un tramo del recorrido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegReference {
    /// Número del tramo, desde 1 (el que sale de la salida).
    pub index: usize,
    pub from: ControlCode,
    pub to: ControlCode,
    /// Clasificados con split en el tramo.
    pub valid_splits: usize,
    /// Splits promediados para la referencia: el 25 % de `valid_splits`, hacia arriba.
    pub reference_count: usize,
    /// `None` si ningún clasificado tiene split en el tramo.
    pub reference_s: Option<f64>,
    /// Último tramo (a meta).
    pub is_last: bool,
    /// Referencia menor de [`SHORT_REFERENCE_S`].
    pub short_reference: bool,
    /// Se excluye de los análisis de patrones (último tramo o referencia corta).
    pub excluded_from_patterns: bool,
}

/// Tiempo perdido de un corredor del recorrido.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerAnalysis {
    /// Posición de la categoría en `Event::classes` (como `ClassRef::index`).
    pub class_index: usize,
    /// Posición del resultado en `Class::results`.
    pub result_index: usize,
    pub status: RaceStatus,
    pub place: Option<u16>,
    /// Meta − salida; `None` si falta alguna de las dos picadas.
    pub total_s: Option<f64>,
    /// Rendimiento habitual (1.0 = 100 %); `None` si no tiene ningún `IR_i`.
    pub usual_performance: Option<f64>,
    /// Suma de las pérdidas de los tramos con error; `None` sin rendimiento habitual.
    pub lost_time_s: Option<f64>,
    pub error_count: usize,
    /// `total_s − lost_time_s`.
    pub time_without_errors_s: Option<f64>,
    pub legs: Vec<RunnerLeg>,
}

/// Un tramo de un corredor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerLeg {
    /// Número del tramo, desde 1.
    pub index: usize,
    /// `None` si falta la hora de algún extremo o la diferencia no es positiva.
    pub split_s: Option<f64>,
    /// Tiempo desde la salida hasta la picada que cierra el tramo.
    pub elapsed_s: Option<f64>,
    /// Puesto en el tramo entre los clasificados (1 + clasificados con split menor).
    pub place: Option<usize>,
    /// `IR_i = ref_i / t_i` (1.0 = 100 %).
    pub performance_index: Option<f64>,
    /// `esp_i = ref_i / habitual`.
    pub expected_s: Option<f64>,
    /// `p_i = t_i − esp_i`; negativa si fue más rápido de lo esperado.
    pub loss_s: Option<f64>,
    /// `100 · p_i / esp_i`.
    pub loss_pct: Option<f64>,
    pub is_error: bool,
    /// Diferencia acumulada respecto al tiempo ideal: `elapsed_s − Σ ref_j` hasta este tramo.
    pub behind_ideal_s: Option<f64>,
}

/// Tiempo perdido de todos los recorridos de `event`.
pub fn analyze_event(event: &Event, config: &LostTimeConfig) -> LostTimeReport {
    LostTimeReport {
        config: *config,
        courses: group_by_course(event)
            .iter()
            .map(|group| analyze_course(event, group, config))
            .collect(),
    }
}

/// Un resultado del recorrido con sus horas y splits ya calculados.
struct Entry<'a> {
    class_index: usize,
    result_index: usize,
    result: &'a RaceResult,
    /// Hora de cada punto del recorrido: salida, balizas y meta.
    times: Vec<Option<DateTime<Utc>>>,
    splits: Vec<Option<f64>>,
}

/// Tiempo perdido de un recorrido de `event`.
///
/// `group` tiene que salir de [`group_by_course`] sobre el mismo `event`: las categorías se
/// buscan por `ClassRef::index` y las que no existen se ignoran.
pub fn analyze_course(
    event: &Event,
    group: &CourseGroup,
    config: &LostTimeConfig,
) -> CourseAnalysis {
    let codes: Vec<ControlCode> = iter::once(START_CODE)
        .chain(group.course.controls.iter().copied())
        .chain(iter::once(FINISH_CODE))
        .collect();
    let leg_count = codes.len() - 1;

    let mut entries = Vec::new();
    for class_ref in &group.classes {
        let Some(class) = event.classes.get(class_ref.index) else {
            continue;
        };
        for (result_index, result) in class.results.iter().enumerate() {
            let times = match_punches(&codes, &result.punches);
            let splits = times
                .windows(2)
                .map(|w| seconds_between(w[0], w[1]).filter(|&s| s > 0.0))
                .collect();
            entries.push(Entry {
                class_index: class_ref.index,
                result_index,
                result,
                times,
                splits,
            });
        }
    }

    let valid_runners = entries
        .iter()
        .filter(|e| e.result.status == RaceStatus::Ok)
        .count();

    // Splits de los clasificados por tramo, ordenados de menor a mayor.
    let ok_splits: Vec<Vec<f64>> = (0..leg_count)
        .map(|leg| {
            let mut splits: Vec<f64> = entries
                .iter()
                .filter(|e| e.result.status == RaceStatus::Ok)
                .filter_map(|e| e.splits.get(leg).copied().flatten())
                .collect();
            splits.sort_by(f64::total_cmp);
            splits
        })
        .collect();

    let legs: Vec<LegReference> = ok_splits
        .iter()
        .enumerate()
        .map(|(leg, splits)| {
            let reference_count = splits.len().div_ceil(4);
            let reference_s = mean(splits.get(..reference_count).unwrap_or_default());
            let is_last = leg + 1 == leg_count;
            let short_reference = reference_s.is_some_and(|r| r < SHORT_REFERENCE_S);
            LegReference {
                index: leg + 1,
                from: codes.get(leg).copied().unwrap_or(START_CODE),
                to: codes.get(leg + 1).copied().unwrap_or(FINISH_CODE),
                valid_splits: splits.len(),
                reference_count,
                reference_s,
                is_last,
                short_reference,
                excluded_from_patterns: is_last || short_reference,
            }
        })
        .collect();

    // Tiempo ideal acumulado (suma de referencias); sin valor desde el primer tramo sin referencia.
    let ideal: Vec<Option<f64>> = legs
        .iter()
        .scan(Some(0.0), |acc, leg| {
            *acc = acc.zip(leg.reference_s).map(|(a, r)| a + r);
            Some(*acc)
        })
        .collect();

    let runners = entries
        .iter()
        .map(|entry| analyze_runner(entry, &legs, &ok_splits, &ideal, config))
        .collect();

    CourseAnalysis {
        course: group.course.clone(),
        classes: group.classes.clone(),
        valid_runners,
        weak_reference: valid_runners < MIN_RUNNERS_FOR_STRONG_REFERENCE,
        legs,
        runners,
    }
}

fn analyze_runner(
    entry: &Entry<'_>,
    legs: &[LegReference],
    ok_splits: &[Vec<f64>],
    ideal: &[Option<f64>],
    config: &LostTimeConfig,
) -> RunnerAnalysis {
    let split = |leg: usize| entry.splits.get(leg).copied().flatten();
    let performance: Vec<Option<f64>> = legs
        .iter()
        .enumerate()
        .map(|(leg, reference)| reference.reference_s.zip(split(leg)).map(|(r, t)| r / t))
        .collect();
    let weighted: Vec<(f64, f64)> = performance
        .iter()
        .zip(legs)
        .filter_map(|(ir, leg)| ir.zip(leg.reference_s))
        .collect();
    let usual = weighted_median(&weighted);

    let start = entry.times.first().copied().flatten();
    let mut lost = usual.map(|_| 0.0);
    let mut error_count = 0;
    let mut runner_legs = Vec::with_capacity(legs.len());
    for (leg, reference) in legs.iter().enumerate() {
        let t = split(leg);
        let elapsed = seconds_between(start, entry.times.get(leg + 1).copied().flatten());
        let place = t.map(|t| {
            1 + ok_splits
                .get(leg)
                .map_or(0, |splits| splits.partition_point(|&s| s < t))
        });
        let performance_index = performance.get(leg).copied().flatten();
        let expected = performance_index
            .and(reference.reference_s.zip(usual))
            .map(|(r, u)| r / u);
        let loss = t.zip(expected).map(|(t, e)| t - e);
        let loss_pct = loss.zip(expected).map(|(p, e)| p / e * 100.0);
        let is_error = loss.zip(loss_pct).is_some_and(|(p, pct)| {
            p > config.error_threshold_s && pct > config.error_threshold_pct
        });
        if is_error {
            lost = lost.zip(loss).map(|(acc, p)| acc + p);
            error_count += 1;
        }
        let behind_ideal = elapsed
            .zip(ideal.get(leg).copied().flatten())
            .map(|(e, i)| e - i);
        runner_legs.push(RunnerLeg {
            index: leg + 1,
            split_s: t,
            elapsed_s: elapsed,
            place,
            performance_index,
            expected_s: expected,
            loss_s: loss,
            loss_pct,
            is_error,
            behind_ideal_s: behind_ideal,
        });
    }

    let total = seconds_between(start, entry.times.last().copied().flatten());
    RunnerAnalysis {
        class_index: entry.class_index,
        result_index: entry.result_index,
        status: entry.result.status,
        place: entry.result.place,
        total_s: total,
        usual_performance: usual,
        lost_time_s: lost,
        error_count,
        time_without_errors_s: total.zip(lost).map(|(t, l)| t - l),
        legs: runner_legs,
    }
}

/// Hora de cada punto esperado del recorrido (`codes`: salida, balizas, meta).
///
/// Se empareja en orden: para cada código se busca la siguiente picada con ese código a partir
/// de la última emparejada. Si no aparece, el punto queda sin hora y la búsqueda sigue desde el
/// mismo sitio; las picadas que sobran se ignoran.
fn match_punches(codes: &[ControlCode], punches: &[Punch]) -> Vec<Option<DateTime<Utc>>> {
    let mut next = 0;
    codes
        .iter()
        .map(|&code| {
            let found = punches
                .get(next..)
                .unwrap_or_default()
                .iter()
                .position(|p| p.code == code)?;
            let index = next + found;
            next = index + 1;
            punches.get(index).and_then(|p| p.time)
        })
        .collect()
}

/// `to − from` en segundos (resolución de milisegundos).
fn seconds_between(from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>) -> Option<f64> {
    let (from, to) = from.zip(to)?;
    Some((to - from).num_milliseconds() as f64 / 1000.0)
}

/// Media aritmética, sumando en el orden dado; `None` si no hay valores.
fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

/// Mediana ponderada de pares `(valor, peso)`, con pesos positivos.
///
/// Se ordena por valor (a igual valor, por posición en `pairs`) y se acumulan los pesos: es el
/// primer valor con el que el acumulado supera la mitad del total; si lo iguala (tolerancia
/// relativa de 1e-9), la media de ese valor y el siguiente. Con pesos iguales coincide con la
/// mediana de siempre.
fn weighted_median(pairs: &[(f64, f64)]) -> Option<f64> {
    let mut sorted = pairs.to_vec();
    // `sort_by` es estable: a igual valor se conserva el orden de los tramos.
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = sorted.iter().map(|&(_, w)| w).sum();
    let half = total / 2.0;
    let eps = total * 1e-9;
    let mut cumulative = 0.0;
    for (k, &(value, weight)) in sorted.iter().enumerate() {
        cumulative += weight;
        if cumulative > half + eps {
            return Some(value);
        }
        if cumulative >= half - eps {
            return Some(match sorted.get(k + 1) {
                Some(&(next, _)) => (value + next) / 2.0,
                None => value,
            });
        }
    }
    sorted.last().map(|&(value, _)| value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Class, Runner};
    use chrono::{NaiveDate, TimeZone};
    use serde_json::json;

    const EPS: f64 = 1e-9;

    fn close(got: Option<f64>, want: f64) {
        let got = got.unwrap_or(f64::NAN);
        assert!((got - want).abs() < 1e-6, "{got} != {want}");
    }

    /// Instante a `s` segundos de las 10:00:00Z del día de la carrera.
    fn at(s: f64) -> DateTime<Utc> {
        let base = Utc.with_ymd_and_hms(2026, 10, 3, 10, 0, 0).unwrap();
        base + chrono::TimeDelta::milliseconds((s * 1000.0).round() as i64)
    }

    fn runner() -> Runner {
        Runner {
            id: 1, // repetido a propósito: no se usa como clave
            given_name: String::new(),
            family_name: String::new(),
            club: None,
            bib: None,
            si_card: None,
            sex: None,
        }
    }

    /// Resultado con picadas en `controls` y splits dados (`None` = picada sin hora en el destino).
    fn result(status: RaceStatus, controls: &[u16], splits: &[Option<f64>]) -> RaceResult {
        let codes: Vec<u16> = iter::once(START_CODE)
            .chain(controls.iter().copied())
            .chain(iter::once(FINISH_CODE))
            .collect();
        let mut clock = 0.0;
        let mut punches = vec![Punch {
            code: START_CODE,
            time: Some(at(0.0)),
        }];
        for (code, split) in codes.iter().skip(1).zip(splits) {
            // Si falta la hora, el reloj avanza 60 s igualmente (el split que no se registró).
            clock += split.unwrap_or(60.0);
            let time = split.map(|_| at(clock));
            punches.push(Punch { code: *code, time });
        }
        RaceResult {
            runner: runner(),
            status,
            place: None,
            punches,
        }
    }

    fn ok(controls: &[u16], splits: &[f64]) -> RaceResult {
        let splits: Vec<Option<f64>> = splits.iter().copied().map(Some).collect();
        result(RaceStatus::Ok, controls, &splits)
    }

    fn event(classes: Vec<(&str, &[u16], Vec<RaceResult>)>) -> Event {
        Event {
            name: None,
            date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
            classes: classes
                .into_iter()
                .enumerate()
                .map(|(i, (name, controls, mut results))| {
                    // Puestos por orden de tiempo total entre los clasificados.
                    let mut order: Vec<usize> = (0..results.len())
                        .filter(|&r| results[r].status == RaceStatus::Ok)
                        .collect();
                    order.sort_by_key(|&r| results[r].punches.last().and_then(|p| p.time));
                    for (place, r) in order.into_iter().enumerate() {
                        results[r].place = Some(place as u16 + 1);
                    }
                    Class {
                        id: i as u32 + 100,
                        name: name.into(),
                        short_name: None,
                        course: Course {
                            controls: controls.to_vec(),
                        },
                        results,
                    }
                })
                .collect(),
        }
    }

    fn single_course(event: &Event) -> CourseAnalysis {
        let report = analyze_event(event, &LostTimeConfig::default());
        assert_eq!(report.courses.len(), 1);
        report.courses.into_iter().next().unwrap()
    }

    /// Ejemplo de `docs/tiempo-perdido.md`: 4 corredores, 2 tramos (salida → 31 → meta).
    fn doc_example() -> Event {
        event(vec![(
            "M21",
            &[31],
            vec![
                ok(&[31], &[60.0, 66.0]), // A
                ok(&[31], &[62.0, 60.0]), // B
                ok(&[31], &[70.0, 75.0]), // C
                ok(&[31], &[90.0, 60.0]), // D
            ],
        )])
    }

    #[test]
    fn doc_example_reference_and_performance_index() {
        let course = single_course(&doc_example());
        assert_eq!(course.valid_runners, 4);
        assert!(!course.weak_reference);
        assert_eq!(course.legs.len(), 2);

        let leg1 = &course.legs[0];
        assert_eq!((leg1.index, leg1.from, leg1.to), (1, START_CODE, 31));
        assert_eq!((leg1.valid_splits, leg1.reference_count), (4, 1));
        close(leg1.reference_s, 60.0);
        assert!(!leg1.is_last && !leg1.excluded_from_patterns);

        let leg2 = &course.legs[1];
        assert_eq!((leg2.from, leg2.to), (31, FINISH_CODE));
        close(leg2.reference_s, 60.0);
        assert!(leg2.is_last && leg2.excluded_from_patterns && !leg2.short_reference);

        // Corredor D: 90 s en el tramo 1 → IR_1 = 60 / 90 = 0,667.
        let d = &course.runners[3];
        close(d.legs[0].performance_index, 2.0 / 3.0);
        assert!((d.legs[0].performance_index.unwrap() - 0.667).abs() < 0.001);
        close(d.legs[1].performance_index, 1.0);
    }

    #[test]
    fn doc_example_runner_d_loss_and_error() {
        let course = single_course(&doc_example());
        let d = &course.runners[3];
        // Pesos iguales (60 y 60): el acumulado iguala la mitad → media de 0,667 y 1 = 0,833.
        close(d.usual_performance, 5.0 / 6.0);
        // esp = 60 / 0,833 = 72 s en los dos tramos.
        close(d.legs[0].expected_s, 72.0);
        close(d.legs[1].expected_s, 72.0);
        // Tramo 1: 90 − 72 = 18 s (25 %) → error (18 > 15 y 25 % > 10 %).
        close(d.legs[0].loss_s, 18.0);
        close(d.legs[0].loss_pct, 25.0);
        assert!(d.legs[0].is_error);
        // Tramo 2: 60 − 72 = −12 s (−16,7 %) → no es error.
        close(d.legs[1].loss_s, -12.0);
        close(d.legs[1].loss_pct, -100.0 / 6.0);
        assert!(!d.legs[1].is_error);
        // Totales: 150 s, 18 s perdidos, 132 s sin errores.
        close(d.total_s, 150.0);
        close(d.lost_time_s, 18.0);
        assert_eq!(d.error_count, 1);
        close(d.time_without_errors_s, 132.0);
        // Puesto por tramo: último en el 1; empatado en el 2 con B (60 s) → 1.º.
        assert_eq!(d.legs[0].place, Some(4));
        assert_eq!(d.legs[1].place, Some(1));
        // Tiempo ideal: 60 y 120 s; D pasa a 90 y 150 s → +30 y +30 s.
        close(d.legs[0].elapsed_s, 90.0);
        close(d.legs[0].behind_ideal_s, 30.0);
        close(d.legs[1].behind_ideal_s, 30.0);
    }

    #[test]
    fn doc_example_leg_places_use_competition_ranking() {
        let course = single_course(&doc_example());
        let places: Vec<Vec<Option<usize>>> = course
            .runners
            .iter()
            .map(|r| r.legs.iter().map(|l| l.place).collect())
            .collect();
        // Tramo 2: B y D empatan a 60 s (1.º), A 66 s (3.º), C 75 s (4.º).
        assert_eq!(
            places,
            vec![
                vec![Some(1), Some(3)],
                vec![Some(2), Some(1)],
                vec![Some(3), Some(4)],
                vec![Some(4), Some(1)],
            ]
        );
    }

    #[test]
    fn doc_example_runner_a() {
        let course = single_course(&doc_example());
        let a = &course.runners[0];
        // IR = 1 y 60/66 = 0,909; pesos iguales → habitual = (1 + 0,909) / 2 = 0,954545…
        let usual = (1.0 + 60.0 / 66.0) / 2.0;
        close(a.usual_performance, usual);
        // esp = 60 / 0,9545 = 62,857 s; tramo 2: 66 − 62,857 = 3,143 s (5 %) → no es error.
        close(a.legs[1].expected_s, 60.0 / usual);
        close(a.legs[1].loss_s, 66.0 - 60.0 / usual);
        close(a.legs[1].loss_pct, 5.0);
        assert_eq!(a.error_count, 0);
        close(a.lost_time_s, 0.0);
        close(a.time_without_errors_s, 126.0);
    }

    #[test]
    fn reference_is_mean_of_fastest_quarter_rounded_up() {
        // 5 corredores → 25 % = 1,25 → 2 corredores: (50 + 54) / 2 = 52.
        let controls: &[u16] = &[31];
        let results = [50.0, 80.0, 54.0, 70.0, 60.0]
            .iter()
            .map(|&s| ok(controls, &[s, 30.0]))
            .collect();
        let course = single_course(&event(vec![("A", controls, results)]));
        assert_eq!(course.legs[0].reference_count, 2);
        close(course.legs[0].reference_s, 52.0);
        // 9 corredores → 2,25 → 3; un solo corredor → 1.
        assert_eq!(9_usize.div_ceil(4), 3);
        let one = single_course(&event(vec![(
            "A",
            controls,
            vec![ok(controls, &[42.0, 30.0])],
        )]));
        assert_eq!(one.legs[0].reference_count, 1);
        close(one.legs[0].reference_s, 42.0);
    }

    #[test]
    fn weighted_median_cases() {
        assert_eq!(weighted_median(&[]), None);
        assert_eq!(weighted_median(&[(0.8, 30.0)]), Some(0.8));
        // Pesos iguales, número impar: el central.
        assert_eq!(
            weighted_median(&[(0.9, 1.0), (0.7, 1.0), (1.1, 1.0)]),
            Some(0.9)
        );
        // Pesos iguales, número par: media de los dos centrales.
        let m = weighted_median(&[(0.9, 2.0), (0.7, 2.0), (1.1, 2.0), (1.0, 2.0)]).unwrap();
        assert!((m - 0.95).abs() < EPS);
        // Un tramo con mucho peso arrastra la mediana: total 100, mitad 50;
        // acumulado 10 (0,6), 20 (0,8), 90 (1,0) → 1,0.
        assert_eq!(
            weighted_median(&[(1.0, 70.0), (0.6, 10.0), (0.8, 10.0), (1.2, 10.0)]),
            Some(1.0)
        );
        // Acumulado exactamente en la mitad: 30 + 20 = 50 de 100 → media de 0,8 y 1,0.
        let m = weighted_median(&[(0.6, 30.0), (0.8, 20.0), (1.0, 25.0), (1.2, 25.0)]).unwrap();
        assert!((m - 0.9).abs() < EPS);
    }

    #[test]
    fn usual_performance_is_weighted_by_reference() {
        // Tres tramos con referencias 100, 20 y 30 (un solo clasificado más rápido en cada uno
        // con 4 corredores). El corredor X: IR 1,0 (peso 100), 0,5 (20) y 0,6 (30).
        // Total 150, mitad 75: acumulado 20 (0,5), 50 (0,6), 150 (1,0) → habitual 1,0.
        let controls: &[u16] = &[31, 32];
        let results = vec![
            ok(controls, &[100.0, 40.0, 50.0]), // X: IR 1, 20/40 = 0,5 y 30/50 = 0,6
            ok(controls, &[110.0, 40.0, 50.0]),
            ok(controls, &[120.0, 45.0, 55.0]),
            ok(controls, &[130.0, 20.0, 30.0]),
        ];
        let course = single_course(&event(vec![("A", controls, results)]));
        let refs: Vec<f64> = course.legs.iter().map(|l| l.reference_s.unwrap()).collect();
        assert_eq!(refs, [100.0, 20.0, 30.0]);
        let x = &course.runners[0];
        close(x.usual_performance, 1.0);
        // esp = ref: tramo 2 pierde 20 s (100 %) → error; tramo 3 pierde 20 s (66,7 %) → error.
        close(x.legs[1].loss_s, 20.0);
        close(x.legs[2].loss_s, 20.0);
        assert!(x.legs[1].is_error && x.legs[2].is_error);
        assert_eq!(x.error_count, 2);
        close(x.lost_time_s, 40.0);
        close(x.total_s, 190.0);
        close(x.time_without_errors_s, 150.0);
        // La referencia de 20 s no es corta (< 20 estricto); el último tramo sí se excluye.
        assert!(!course.legs[1].short_reference);
        assert!(course.legs[2].excluded_from_patterns);
    }

    /// Un corredor con habitual 1 (IR = 1 en tres tramos de 300 s, que pesan más de la mitad
    /// mientras `r4 < 900`) y un cuarto tramo con split `t4 > r4` sobre una referencia `r4`.
    fn threshold_case(r4: f64, t4: f64, config: &LostTimeConfig) -> RunnerLeg {
        let controls: &[u16] = &[31, 32, 33];
        // Cuatro corredores iguales fijan las referencias (300, 300, 300, r4): con 5 corredores
        // se promedian los 2 más rápidos. X repite los tres primeros tramos y hace t4 en el cuarto.
        let mut results: Vec<RaceResult> = (0..4)
            .map(|_| ok(controls, &[300.0, 300.0, 300.0, r4]))
            .collect();
        results.push(ok(controls, &[300.0, 300.0, 300.0, t4]));
        let report = analyze_event(&event(vec![("A", controls, results)]), config);
        let x = &report.courses[0].runners[4];
        close(x.usual_performance, 1.0);
        x.legs[3].clone()
    }

    #[test]
    fn error_needs_both_thresholds() {
        let default = LostTimeConfig::default();
        // 16 s y 16 % → error.
        assert!(threshold_case(100.0, 116.0, &default).is_error);
        // 15 s exactos: no supera el umbral (estricto).
        assert!(!threshold_case(100.0, 115.0, &default).is_error);
        // 20 s pero solo 5 % (referencia 400 s) → no es error.
        let leg = threshold_case(400.0, 420.0, &default);
        close(leg.loss_pct, 5.0);
        assert!(!leg.is_error);
        // 50 % pero solo 10 s (referencia 20 s) → no es error.
        let leg = threshold_case(20.0, 30.0, &default);
        close(leg.loss_pct, 50.0);
        assert!(!leg.is_error);
    }

    #[test]
    fn thresholds_are_configurable() {
        let strict = LostTimeConfig {
            error_threshold_s: 5.0,
            error_threshold_pct: 4.0,
        };
        assert!(threshold_case(400.0, 420.0, &strict).is_error);
        assert!(threshold_case(20.0, 30.0, &strict).is_error);
        let lax = LostTimeConfig {
            error_threshold_s: 30.0,
            error_threshold_pct: 10.0,
        };
        assert!(!threshold_case(100.0, 116.0, &lax).is_error);
    }

    #[test]
    fn short_reference_and_weak_reference_are_flagged() {
        // Tres clasificados → referencia débil. Tramo 2 con referencia 19 s → corta.
        let controls: &[u16] = &[31, 32];
        let results = vec![
            ok(controls, &[60.0, 19.0, 30.0]),
            ok(controls, &[70.0, 25.0, 30.0]),
            ok(controls, &[80.0, 22.0, 30.0]),
            result(
                RaceStatus::NotClassified,
                controls,
                &[Some(10.0), Some(5.0), Some(5.0)],
            ),
        ];
        let course = single_course(&event(vec![("A", controls, results)]));
        assert_eq!(course.valid_runners, 3);
        assert!(course.weak_reference);
        let flags: Vec<(bool, bool, bool)> = course
            .legs
            .iter()
            .map(|l| (l.short_reference, l.is_last, l.excluded_from_patterns))
            .collect();
        assert_eq!(
            flags,
            [
                (false, false, false),
                (true, false, true),
                (false, true, true)
            ]
        );
    }

    #[test]
    fn unclassified_runners_are_analyzed_but_do_not_set_the_reference() {
        let controls: &[u16] = &[31];
        let results = vec![
            ok(controls, &[60.0, 30.0]),
            ok(controls, &[64.0, 32.0]),
            ok(controls, &[70.0, 36.0]),
            ok(controls, &[80.0, 40.0]),
            // Mucho más rápido, pero no clasificado: no cuenta para la referencia.
            result(
                RaceStatus::NotClassified,
                controls,
                &[Some(40.0), Some(20.0)],
            ),
        ];
        let course = single_course(&event(vec![("A", controls, results)]));
        assert_eq!(course.valid_runners, 4);
        close(course.legs[0].reference_s, 60.0);
        close(course.legs[1].reference_s, 30.0);
        let nc = &course.runners[4];
        assert_eq!(nc.status, RaceStatus::NotClassified);
        assert_eq!(nc.place, None);
        close(nc.legs[0].performance_index, 1.5);
        close(nc.usual_performance, 1.5);
        // Puesto en el tramo frente a los clasificados: el más rápido.
        assert_eq!(nc.legs[0].place, Some(1));
    }

    #[test]
    fn missing_punch_time_removes_both_adjacent_legs() {
        let controls: &[u16] = &[31, 32];
        let results = vec![
            ok(controls, &[60.0, 60.0, 30.0]),
            ok(controls, &[60.0, 60.0, 30.0]),
            ok(controls, &[60.0, 60.0, 30.0]),
            ok(controls, &[60.0, 60.0, 30.0]),
            // Picada de la 31 sin hora: sin split en los tramos 1 y 2.
            result(RaceStatus::Ok, controls, &[None, Some(60.0), Some(30.0)]),
        ];
        let course = single_course(&event(vec![("A", controls, results)]));
        assert_eq!(course.legs[0].valid_splits, 4);
        let x = &course.runners[4];
        let splits: Vec<Option<f64>> = x.legs.iter().map(|l| l.split_s).collect();
        assert_eq!(splits, [None, None, Some(30.0)]);
        assert_eq!(x.legs[0].loss_s, None);
        assert!(!x.legs[0].is_error);
        assert_eq!(x.legs[0].place, None);
        // El tiempo acumulado del tramo 2 sigue disponible (salida → 32: 60 + 60 s).
        close(x.legs[1].elapsed_s, 120.0);
        close(x.legs[1].behind_ideal_s, 0.0);
        close(x.usual_performance, 1.0);
        close(x.total_s, 150.0);
    }

    #[test]
    fn did_not_start_has_no_numbers() {
        let controls: &[u16] = &[31];
        let mut dns = result(RaceStatus::DidNotStart, controls, &[None, None]);
        dns.punches[0].time = None;
        let results = vec![ok(controls, &[60.0, 30.0]), dns];
        let course = single_course(&event(vec![("A", controls, results)]));
        let x = &course.runners[1];
        assert_eq!(x.total_s, None);
        assert_eq!(x.usual_performance, None);
        assert_eq!(x.lost_time_s, None);
        assert_eq!(x.time_without_errors_s, None);
        assert!(x.legs.iter().all(|l| l.split_s.is_none() && !l.is_error));
        // Un solo clasificado: referencia débil.
        assert!(course.weak_reference);
    }

    #[test]
    fn punches_are_matched_in_course_order() {
        let codes = [START_CODE, 31, 32, FINISH_CODE];
        let p = |code, s| Punch {
            code,
            time: Some(at(s)),
        };
        // Picada de más (45) y la 32 que falta: se emparejan 31 y meta; la 32 queda sin hora.
        let punches = [
            p(START_CODE, 0.0),
            p(31, 50.0),
            p(45, 70.0),
            p(FINISH_CODE, 150.0),
        ];
        assert_eq!(
            match_punches(&codes, &punches),
            [Some(at(0.0)), Some(at(50.0)), None, Some(at(150.0))]
        );
        // Baliza repetida en el recorrido: cada aparición toma la siguiente picada.
        let codes = [START_CODE, 31, 32, 31, FINISH_CODE];
        let punches = [
            p(START_CODE, 0.0),
            p(31, 10.0),
            p(32, 20.0),
            p(31, 30.0),
            p(FINISH_CODE, 40.0),
        ];
        assert_eq!(
            match_punches(&codes, &punches),
            punches.iter().map(|p| p.time).collect::<Vec<_>>()
        );
    }

    #[test]
    fn non_positive_split_is_ignored() {
        let controls: &[u16] = &[31];
        let mut bad = ok(controls, &[60.0, 30.0]);
        // La 31 con la misma hora que la salida: split 0 → sin split.
        bad.punches[1].time = bad.punches[0].time;
        let results = vec![ok(controls, &[60.0, 30.0]), bad];
        let course = single_course(&event(vec![("A", controls, results)]));
        assert_eq!(course.legs[0].valid_splits, 1);
        assert_eq!(course.runners[1].legs[0].split_s, None);
    }

    #[test]
    fn courses_are_shared_between_classes_and_runners_keep_their_position() {
        let controls: &[u16] = &[31];
        let ev = event(vec![
            (
                "M21",
                controls,
                vec![ok(controls, &[60.0, 30.0]), ok(controls, &[70.0, 30.0])],
            ),
            ("F21", &[40], vec![ok(&[40], &[50.0, 20.0])]),
            (
                "M35",
                controls,
                vec![ok(controls, &[50.0, 30.0]), ok(controls, &[90.0, 30.0])],
            ),
        ]);
        let report = analyze_event(&ev, &LostTimeConfig::default());
        assert_eq!(report.courses.len(), 2);
        let shared = &report.courses[0];
        assert_eq!(shared.valid_runners, 4);
        // 4 corredores → la referencia es el de M35 (50 s).
        close(shared.legs[0].reference_s, 50.0);
        let positions: Vec<(usize, usize)> = shared
            .runners
            .iter()
            .map(|r| (r.class_index, r.result_index))
            .collect();
        assert_eq!(positions, [(0, 0), (0, 1), (2, 0), (2, 1)]);
    }

    #[test]
    fn report_serializes_to_json() {
        let report = analyze_event(&doc_example(), &LostTimeConfig::default());
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(
            value["config"],
            json!({"error_threshold_s": 15.0, "error_threshold_pct": 10.0})
        );
        let course = &value["courses"][0];
        assert_eq!(course["classes"][0]["name"], json!("M21"));
        assert_eq!(
            course["legs"][1],
            json!({
                "index": 2, "from": 31, "to": FINISH_CODE, "valid_splits": 4,
                "reference_count": 1, "reference_s": 60.0, "is_last": true,
                "short_reference": false, "excluded_from_patterns": true
            })
        );
        let d = &course["runners"][3];
        assert_eq!(d["status"], json!("ok"));
        assert_eq!(d["class_index"], json!(0));
        assert_eq!(d["result_index"], json!(3));
        assert_eq!(d["legs"][0]["is_error"], json!(true));
        let back: LostTimeReport = serde_json::from_value(value).unwrap();
        assert_eq!(back, report);
        // La configuración admite campos ausentes (valores por defecto).
        let partial: LostTimeConfig =
            serde_json::from_value(json!({"error_threshold_s": 20.0})).unwrap();
        assert_eq!(partial.error_threshold_pct, DEFAULT_ERROR_THRESHOLD_PCT);
    }
}
