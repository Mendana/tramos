//! Rivales habituales (#193): los corredores que coinciden con el usuario en el mismo recorrido en
//! muchas carreras, y cómo le va frente a cada uno. Las definiciones están en
//! `docs/historico.md`, "Rivales habituales".
//!
//! Los .spl y las descargas de OReplay traen a todos los corredores de cada categoría: no hace
//! falta nada más. Un rival se reconoce de una carrera a otra por su nombre normalizado
//! ([`normalize_name`]) y, si con el mismo nombre aparecen tarjetas distintas, por la tarjeta.

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::courses::group_by_course;
use crate::event_kind::EventKind;
use crate::history::HistoryFilter;
use crate::identify::{ResultRef, normalize_name};
use crate::loss_breakdown::median;
use crate::lost_time::{LostTimeConfig, RunnerAnalysis, analyze_course};
use crate::model::{Event, RaceStatus};
use crate::race_format::RaceFormat;
use crate::taxonomy::LegTag;

/// Carreras juntas mínimas para ser rival habitual.
pub const MIN_RACES_TOGETHER: usize = 3;

/// Una carrera del usuario.
#[derive(Debug, Clone, Copy)]
pub struct RivalRace<'a> {
    pub date: NaiveDate,
    pub name: Option<&'a str>,
    pub format: Option<RaceFormat>,
    pub kind: EventKind,
    pub event: &'a Event,
    /// Su resultado en la carrera.
    pub at: ResultRef,
    /// Sus etiquetas (`(tramo, etiqueta)`, con el tramo desde 1): deciden qué tramos suyos son
    /// error.
    pub tags: &'a [(usize, LegTag)],
}

/// Diferencia por tramo frente a un rival en un grupo de tramos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct LegGap {
    /// Tramos comparados (los dos con split).
    pub legs: usize,
    /// Mediana de `tu split − el suyo` (s); negativa si eres más rápido. `None` sin tramos.
    pub median_gap_s: Option<f64>,
}

/// Una carrera con un rival.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meeting {
    pub date: NaiveDate,
    pub race_name: Option<String>,
    /// Tiempos de los dos si están clasificados con tiempo; `None` si no.
    pub own_total_s: Option<f64>,
    pub rival_total_s: Option<f64>,
}

/// Un rival habitual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rival {
    /// «Nombre Apellidos» como en su carrera más reciente.
    pub name: String,
    pub club: Option<String>,
    /// Carreras en las que los dos tomasteis la salida en el mismo recorrido.
    pub races: usize,
    /// De ellas, las que los dos acabasteis clasificados con tiempo, y en cuántas le ganas.
    pub races_compared: usize,
    pub races_ahead: usize,
    /// Mediana de `tu tiempo − el suyo` en esas carreras (s); negativa si sueles ganarle.
    pub median_gap_s: Option<f64>,
    /// Tramos que cuentan (ni el último ni los de referencia corta) con split de los dos, y en
    /// cuántos le ganas.
    pub legs_compared: usize,
    pub legs_ahead: usize,
    /// Los tramos en los que no fallaste y en los que sí: si le pierdes por ritmo o por errores.
    pub clean_legs: LegGap,
    pub error_legs: LegGap,
    /// Las carreras juntas, de la más reciente a la más antigua.
    pub meetings: Vec<Meeting>,
}

/// Una aparición de otro corredor en una carrera del usuario.
struct Seen {
    race: usize,
    key: String,
    display: String,
    club: Option<String>,
    si_card: Option<u32>,
    /// Su análisis en el recorrido (índice en `course.runners`).
    runner: usize,
}

/// Lo de cada carrera que hace falta para comparar.
struct RaceData<'a> {
    race: &'a RivalRace<'a>,
    runners: Vec<RunnerAnalysis>,
    own: usize,
    /// Tramos que cuentan (por posición).
    counted: Vec<bool>,
}

/// Los rivales habituales en las carreras que pasan `filter`, con al menos `min_races` carreras
/// juntas, de más carreras juntas a menos. `config` y las etiquetas de cada carrera deciden qué
/// tramos tuyos son error.
pub fn rivals(
    races: &[RivalRace<'_>],
    filter: &HistoryFilter,
    config: &LostTimeConfig,
    min_races: usize,
) -> Vec<Rival> {
    let mut data: Vec<RaceData<'_>> = Vec::new();
    let mut seen: Vec<Seen> = Vec::new();
    for race in races
        .iter()
        .filter(|r| filter.includes(r.date, r.format, r.kind))
    {
        let Some(group) = group_by_course(race.event)
            .into_iter()
            .find(|g| g.classes.iter().any(|c| c.index == race.at.class_index))
        else {
            continue;
        };
        let mut course = analyze_course(race.event, &group, config);
        let Some(own) = course.runners.iter().position(|r| {
            r.class_index == race.at.class_index && r.result_index == race.at.result_index
        }) else {
            continue;
        };
        course.runners[own].apply_tags(race.tags);
        if course.runners[own].status == RaceStatus::DidNotStart {
            continue;
        }
        let index = data.len();
        for (i, runner) in course.runners.iter().enumerate() {
            if i == own || runner.status == RaceStatus::DidNotStart {
                continue;
            }
            let Some(result) = (ResultRef {
                class_index: runner.class_index,
                result_index: runner.result_index,
            })
            .get(race.event) else {
                continue;
            };
            let display = format!(
                "{} {}",
                result.runner.given_name.trim(),
                result.runner.family_name.trim()
            )
            .trim()
            .to_string();
            let key = normalize_name(&display);
            if key.is_empty() {
                continue;
            }
            seen.push(Seen {
                race: index,
                key,
                display,
                club: result.runner.club.clone(),
                si_card: result.runner.si_card,
                runner: i,
            });
        }
        data.push(RaceData {
            race,
            counted: course
                .legs
                .iter()
                .map(|l| !l.excluded_from_patterns)
                .collect(),
            runners: course.runners,
            own,
        });
    }

    let mut out: Vec<Rival> = group_seen(&seen)
        .into_iter()
        .filter_map(|group| rival(&data, &group, min_races))
        .collect();
    out.sort_by(|a, b| {
        b.races
            .cmp(&a.races)
            .then(b.legs_compared.cmp(&a.legs_compared))
            .then(a.name.cmp(&b.name))
    });
    out
}

/// Agrupa las apariciones por rival: el mismo nombre normalizado, separado por tarjeta si con
/// ese nombre hay tarjetas distintas. Las apariciones sin tarjeta van con la tarjeta que más se
/// repite con ese nombre (a igualdad, la que sale antes). Una por carrera: si en una carrera hay
/// dos con el mismo nombre y tarjeta, cuenta la primera.
fn group_seen(seen: &[Seen]) -> Vec<Vec<&Seen>> {
    let mut by_name: Vec<(&str, Vec<&Seen>)> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for s in seen {
        let i = *index.entry(&s.key).or_insert_with(|| {
            by_name.push((&s.key, Vec::new()));
            by_name.len() - 1
        });
        by_name[i].1.push(s);
    }
    let mut groups = Vec::new();
    for (_, all) in by_name {
        // Tarjetas de ese nombre, de más a menos apariciones (a igualdad, la primera en salir).
        let mut cards: Vec<(u32, usize)> = Vec::new();
        for card in all.iter().filter_map(|s| s.si_card) {
            match cards.iter_mut().find(|(c, _)| *c == card) {
                Some((_, n)) => *n += 1,
                None => cards.push((card, 1)),
            }
        }
        let mut sorted = cards.clone();
        sorted.sort_by_key(|c| std::cmp::Reverse(c.1));
        let main = sorted.first().map(|(c, _)| *c);
        let mut split: Vec<(Option<u32>, Vec<&Seen>)> = if cards.is_empty() {
            vec![(None, Vec::new())]
        } else {
            cards.iter().map(|(c, _)| (Some(*c), Vec::new())).collect()
        };
        for s in all {
            let card = s.si_card.or(main);
            if let Some((_, group)) = split.iter_mut().find(|(c, _)| *c == card) {
                if !group.iter().any(|g| g.race == s.race) {
                    group.push(s);
                }
            }
        }
        groups.extend(split.into_iter().map(|(_, g)| g).filter(|g| !g.is_empty()));
    }
    groups
}

/// Los números de un rival; `None` con menos de `min_races` carreras.
fn rival(data: &[RaceData<'_>], seen: &[&Seen], min_races: usize) -> Option<Rival> {
    if seen.len() < min_races {
        return None;
    }
    let (mut gaps, mut clean, mut errors) = (Vec::new(), Vec::new(), Vec::new());
    let (mut legs_compared, mut legs_ahead) = (0, 0);
    let mut meetings = Vec::new();
    for s in seen {
        let race = data.get(s.race)?;
        let own = race.runners.get(race.own)?;
        let other = race.runners.get(s.runner)?;
        let total = |r: &RunnerAnalysis| r.total_s.filter(|_| r.status == RaceStatus::Ok);
        let (own_total, rival_total) = (total(own), total(other));
        if let (Some(a), Some(b)) = (own_total, rival_total) {
            gaps.push(a - b);
        }
        for ((mine, theirs), counted) in own.legs.iter().zip(&other.legs).zip(&race.counted) {
            let (Some(a), Some(b), true) = (mine.split_s, theirs.split_s, *counted) else {
                continue;
            };
            legs_compared += 1;
            if a < b {
                legs_ahead += 1;
            }
            if mine.is_error {
                errors.push(a - b);
            } else {
                clean.push(a - b);
            }
        }
        meetings.push(Meeting {
            date: race.race.date,
            race_name: race.race.name.map(str::to_string),
            own_total_s: own_total,
            rival_total_s: rival_total,
        });
    }
    meetings.sort_by_key(|m| std::cmp::Reverse(m.date));
    // El nombre y el club, de la carrera más reciente.
    let latest = seen
        .iter()
        .max_by_key(|s| data.get(s.race).map(|d| d.race.date))?;
    let races_compared = gaps.len();
    let races_ahead = gaps.iter().filter(|g| **g < 0.0).count();
    let leg_gap = |values: &mut Vec<f64>| LegGap {
        legs: values.len(),
        median_gap_s: median(values),
    };
    Some(Rival {
        name: latest.display.clone(),
        club: latest.club.clone(),
        races: seen.len(),
        races_compared,
        races_ahead,
        median_gap_s: median(&mut gaps),
        legs_compared,
        legs_ahead,
        clean_legs: leg_gap(&mut clean),
        error_legs: leg_gap(&mut errors),
        meetings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Class, Course, FINISH_CODE, Punch, RaceResult, Runner, START_CODE};
    use chrono::{DateTime, TimeDelta, Utc};

    /// Un corredor: nombre, tarjeta, estado y segundos a la 31, a la 32 y a meta.
    struct R(&'static str, Option<u32>, RaceStatus, [i64; 3]);

    const OK: RaceStatus = RaceStatus::Ok;

    fn result(r: &R, date: NaiveDate) -> RaceResult {
        let t0: DateTime<Utc> = date.and_hms_opt(10, 0, 0).unwrap().and_utc();
        let at = |s: i64| (r.2 != RaceStatus::DidNotStart).then(|| t0 + TimeDelta::seconds(s));
        let (given, family) = r.0.split_once(' ').unwrap_or((r.0, ""));
        RaceResult {
            runner: Runner {
                given_name: given.into(),
                family_name: family.into(),
                club: Some("Club".into()),
                bib: None,
                si_card: r.1,
                sex: None,
            },
            status: r.2,
            place: None,
            punches: vec![
                Punch {
                    code: START_CODE,
                    time: at(0),
                },
                Punch {
                    code: 31,
                    time: at(r.3[0]),
                },
                Punch {
                    code: 32,
                    time: at(r.3[1]),
                },
                Punch {
                    code: FINISH_CODE,
                    time: at(r.3[2]),
                },
            ],
        }
    }

    /// Una carrera de una categoría con recorrido 31-32. El usuario es el primero.
    fn event(date: &str, runners: &[R]) -> Event {
        let date: NaiveDate = date.parse().unwrap();
        Event {
            name: Some(format!("Carrera {date}")),
            date,
            classes: vec![Class {
                id: 1,
                name: "M21".into(),
                short_name: None,
                course: Course {
                    controls: vec![31, 32],
                },
                results: runners.iter().map(|r| result(r, date)).collect(),
            }],
        }
    }

    const ME: R = R("Yo Mismo", Some(100), OK, [120, 240, 260]);

    fn events() -> Vec<Event> {
        vec![
            event(
                "2026-03-01",
                &[
                    ME,
                    R("Ana Pérez", Some(200), OK, [110, 230, 250]),
                    R("Luis Gil", Some(300), OK, [130, 260, 280]),
                    R("Marta Ruiz", None, OK, [125, 250, 270]),
                ],
            ),
            event(
                "2026-04-01",
                &[
                    // Esta vez fallas en el segundo tramo: 240 s en vez de 120.
                    R("Yo Mismo", Some(100), OK, [120, 360, 380]),
                    R("Ana Pérez", Some(200), OK, [115, 235, 255]),
                    R("Luis Gil", Some(300), OK, [130, 260, 280]),
                    R("Marta Ruiz", None, OK, [125, 250, 270]),
                    // No salió: no cuenta como coincidir.
                    R("Pedro Sanz", Some(400), RaceStatus::DidNotStart, [0, 0, 0]),
                ],
            ),
            event(
                "2026-05-01",
                &[
                    ME,
                    // Sin tarjeta y escrito de otra forma: el mismo rival.
                    R("ANA PEREZ", None, OK, [100, 220, 240]),
                    // El mismo nombre con otra tarjeta: otro corredor.
                    R("Luis Gil", Some(301), OK, [140, 280, 300]),
                    R("Pedro Sanz", Some(400), OK, [140, 280, 300]),
                ],
            ),
        ]
    }

    fn races(events: &[Event]) -> Vec<RivalRace<'_>> {
        events
            .iter()
            .map(|e| RivalRace {
                date: e.date,
                name: e.name.as_deref(),
                format: None,
                kind: EventKind::Race,
                event: e,
                at: ResultRef {
                    class_index: 0,
                    result_index: 0,
                },
                tags: &[],
            })
            .collect()
    }

    fn run(min: usize, filter: &HistoryFilter) -> Vec<Rival> {
        let events = events();
        rivals(&races(&events), filter, &LostTimeConfig::default(), min)
    }

    #[test]
    fn a_rival_is_who_shares_the_course_in_enough_races() {
        let found = run(MIN_RACES_TOGETHER, &HistoryFilter::default());
        let names: Vec<&str> = found.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["ANA PEREZ"]);
        let ana = &found[0];
        assert_eq!(ana.races, 3);
        assert_eq!(ana.club.as_deref(), Some("Club"));
        // Tus 260, 380 y 260 s frente a sus 250, 255 y 240: le pierdes las tres, por 10, 125 y
        // 20 s (mediana 20).
        assert_eq!((ana.races_compared, ana.races_ahead), (3, 0));
        assert_eq!(ana.median_gap_s, Some(20.0));
        // Tramos que cuentan: los dos primeros de cada carrera (el de meta no). No le ganas
        // ninguno (un empate no es ganar).
        assert_eq!((ana.legs_compared, ana.legs_ahead), (6, 0));
        // Tus errores: el segundo tramo de abril (240 frente a 120) y el primero de mayo (120
        // frente a 100, la referencia). Mediana de 120 y 20.
        assert_eq!(ana.error_legs.legs, 2);
        assert_eq!(ana.error_legs.median_gap_s, Some(70.0));
        // En los limpios, 10, 0, 5 y 0 s: mediana 2,5 s. Le pierdes sobre todo al fallar.
        assert_eq!(ana.clean_legs.legs, 4);
        assert_eq!(ana.clean_legs.median_gap_s, Some(2.5));
        // Las carreras, de la más reciente a la más antigua.
        let dates: Vec<String> = ana.meetings.iter().map(|m| m.date.to_string()).collect();
        assert_eq!(dates, ["2026-05-01", "2026-04-01", "2026-03-01"]);
        assert_eq!(ana.meetings[0].rival_total_s, Some(240.0));
    }

    #[test]
    fn your_tags_decide_which_legs_were_errors() {
        use crate::taxonomy::{Confirmation, LegTag};
        let events = events();
        // El primer tramo de mayo (120 frente a 100) lo marcas «No»: no fue error.
        let no = [(
            1,
            LegTag {
                confirmation: Some(Confirmation::NoError),
                ..LegTag::default()
            },
        )];
        let mut races = races(&events);
        races[2].tags = &no;
        let found = rivals(
            &races,
            &HistoryFilter::default(),
            &LostTimeConfig::default(),
            MIN_RACES_TOGETHER,
        );
        let ana = &found[0];
        // Queda el error de abril (120 s); los limpios son 10, 0, 5, 0 y 20: mediana 5.
        assert_eq!(ana.error_legs.legs, 1);
        assert_eq!(ana.error_legs.median_gap_s, Some(120.0));
        assert_eq!(ana.clean_legs.legs, 5);
        assert_eq!(ana.clean_legs.median_gap_s, Some(5.0));
    }

    #[test]
    fn homonyms_with_another_card_are_another_rival() {
        let found = run(2, &HistoryFilter::default());
        let mut names: Vec<(&str, usize)> =
            found.iter().map(|r| (r.name.as_str(), r.races)).collect();
        names.sort();
        // Luis Gil de la tarjeta 300 (dos carreras) y Marta (dos, sin tarjeta); el Luis Gil de la
        // 301 y Pedro (que no salió en abril) solo coinciden una vez.
        assert_eq!(
            names,
            [("ANA PEREZ", 3), ("Luis Gil", 2), ("Marta Ruiz", 2)]
        );
        let luis = found.iter().find(|r| r.name == "Luis Gil").unwrap();
        // En marzo le ganas (260 frente a 280); en abril, no (380 frente a 280).
        assert_eq!((luis.races_compared, luis.races_ahead), (2, 1));
    }

    #[test]
    fn the_history_filter_applies() {
        let filter = HistoryFilter {
            to: Some("2026-04-30".parse().unwrap()),
            ..HistoryFilter::default()
        };
        assert!(run(MIN_RACES_TOGETHER, &filter).is_empty());
        let filter = HistoryFilter {
            kind: Some(EventKind::Training),
            ..HistoryFilter::default()
        };
        assert!(run(1, &filter).is_empty());
    }
}
