//! Formato de una carrera (sprint, media, larga) y su sugerencia al importar.
//!
//! El formato no viene en el .spl. Al importar se sugiere por el tiempo de los ganadores y el
//! corredor lo confirma (P6 en `docs/preguntas.md`). Es de la carrera, no de la categoría: se
//! usa la mediana de los ganadores de todas las categorías, para que una categoría rara (un
//! solo corredor, muy lento) no decida. Los cortes están en `docs/modelo.md`, "Formato de
//! carrera".

use serde::{Deserialize, Serialize};

use crate::alignment::median;
use crate::model::{Class, Event, FINISH_CODE, RaceResult, RaceStatus, START_CODE};

/// Ganador por debajo de esto (s): sprint (~15 min).
pub const SPRINT_MAX_WINNER_S: f64 = 25.0 * 60.0;
/// Ganador por debajo de esto (s), y no sprint: media (~35 min). Desde aquí, larga (50–60 min).
pub const MIDDLE_MAX_WINNER_S: f64 = 45.0 * 60.0;

/// Formato de la carrera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RaceFormat {
    /// ~15 min, urbano.
    Sprint,
    /// ~35 min, monte técnico.
    Middle,
    /// 50–60 min, elección de ruta.
    Long,
}

impl RaceFormat {
    /// Nombre en español, para la interfaz.
    pub fn label(self) -> &'static str {
        match self {
            Self::Sprint => "sprint",
            Self::Middle => "media",
            Self::Long => "larga",
        }
    }
}

/// Formato que corresponde al tiempo del ganador (s).
pub fn format_for_winner(winner_s: f64) -> RaceFormat {
    if winner_s < SPRINT_MAX_WINNER_S {
        RaceFormat::Sprint
    } else if winner_s < MIDDLE_MAX_WINNER_S {
        RaceFormat::Middle
    } else {
        RaceFormat::Long
    }
}

/// Tiempo (s) del ganador de la categoría: el menor tiempo de salida a meta entre los
/// clasificados con las dos horas. `None` si no hay ninguno.
pub fn winner_time_s(class: &Class) -> Option<f64> {
    class
        .results
        .iter()
        .filter(|r| r.status == RaceStatus::Ok)
        .filter_map(race_time_s)
        .filter(|t| t.is_finite() && *t > 0.0)
        .min_by(f64::total_cmp)
}

/// Mediana de los tiempos de los ganadores de las categorías (s); `None` si ninguna tiene
/// ganador con tiempo.
pub fn median_winner_time_s(event: &Event) -> Option<f64> {
    median(event.classes.iter().filter_map(winner_time_s))
}

/// Formato sugerido para la carrera; `None` si ninguna categoría tiene ganador con tiempo.
pub fn suggest_format(event: &Event) -> Option<RaceFormat> {
    median_winner_time_s(event).map(format_for_winner)
}

/// De la picada de salida a la de meta (s), si las dos tienen hora.
fn race_time_s(result: &RaceResult) -> Option<f64> {
    let time_of = |code| {
        result
            .punches
            .iter()
            .find(|p| p.code == code)
            .and_then(|p| p.time)
    };
    let (start, finish) = (time_of(START_CODE)?, time_of(FINISH_CODE)?);
    Some((finish - start).num_milliseconds() as f64 / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Course, Punch, Runner};
    use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
    use serde_json::json;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-03T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn result(status: RaceStatus, start_s: Option<i64>, finish_s: Option<i64>) -> RaceResult {
        let at = |s: Option<i64>| s.map(|s| t0() + TimeDelta::seconds(s));
        RaceResult {
            runner: Runner {
                given_name: String::new(),
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
                    time: at(start_s),
                },
                Punch {
                    code: 31,
                    time: None,
                },
                Punch {
                    code: FINISH_CODE,
                    time: at(finish_s),
                },
            ],
        }
    }

    fn class(results: Vec<RaceResult>) -> Class {
        Class {
            id: 1,
            name: "F21A".into(),
            short_name: None,
            course: Course { controls: vec![31] },
            results,
        }
    }

    #[test]
    fn cuts_between_formats() {
        assert_eq!(format_for_winner(15.0 * 60.0), RaceFormat::Sprint);
        assert_eq!(format_for_winner(25.0 * 60.0 - 1.0), RaceFormat::Sprint);
        assert_eq!(format_for_winner(25.0 * 60.0), RaceFormat::Middle);
        assert_eq!(format_for_winner(35.0 * 60.0), RaceFormat::Middle);
        assert_eq!(format_for_winner(45.0 * 60.0), RaceFormat::Long);
        assert_eq!(format_for_winner(60.0 * 60.0), RaceFormat::Long);
    }

    #[test]
    fn winner_is_the_fastest_classified_runner() {
        let c = class(vec![
            // No clasificado más rápido: no cuenta.
            result(RaceStatus::NotClassified, Some(0), Some(600)),
            result(RaceStatus::Ok, Some(100), Some(100 + 2100)),
            result(RaceStatus::Ok, Some(0), Some(1900)),
            // Sin hora de meta: no cuenta.
            result(RaceStatus::Ok, Some(0), None),
        ]);
        assert_eq!(winner_time_s(&c), Some(1900.0));
    }

    fn event(classes: Vec<Class>) -> Event {
        Event {
            name: None,
            date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
            classes,
        }
    }

    #[test]
    fn suggestion_uses_the_median_winner_of_the_event() {
        let winner = |s: i64| class(vec![result(RaceStatus::Ok, Some(0), Some(s))]);
        // Tres sprints de 12–16 min y una categoría con un solo corredor de 54 min.
        let e = event(vec![winner(720), winner(900), winner(960), winner(3240)]);
        assert_eq!(median_winner_time_s(&e), Some(930.0));
        assert_eq!(suggest_format(&e), Some(RaceFormat::Sprint));
        // Las categorías sin ganador con tiempo no cuentan.
        let e = event(vec![winner(2400), class(vec![])]);
        assert_eq!(suggest_format(&e), Some(RaceFormat::Middle));
    }

    #[test]
    fn no_suggestion_without_a_timed_winner() {
        let c = class(vec![
            result(RaceStatus::Ok, None, Some(600)),
            result(RaceStatus::DidNotStart, None, None),
        ]);
        assert_eq!(winner_time_s(&c), None);
        assert_eq!(suggest_format(&event(vec![c, class(vec![])])), None);
    }

    #[test]
    fn json_and_labels() {
        assert_eq!(
            serde_json::to_value(RaceFormat::Middle).unwrap(),
            json!("middle")
        );
        assert_eq!(
            serde_json::from_value::<RaceFormat>(json!("long")).unwrap(),
            RaceFormat::Long
        );
        let labels: Vec<&str> = [RaceFormat::Sprint, RaceFormat::Middle, RaceFormat::Long]
            .map(RaceFormat::label)
            .to_vec();
        assert_eq!(labels, ["sprint", "media", "larga"]);
    }
}
