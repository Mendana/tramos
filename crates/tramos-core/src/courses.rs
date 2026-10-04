//! Agrupación de categorías por recorrido.
//!
//! El tiempo perdido se calcula por recorrido, no por categoría (`docs/tiempo-perdido.md`):
//! todas las categorías que corren la misma secuencia de balizas comparten referencia.
//! Dos categorías comparten recorrido si y solo si sus `Course` son idénticos (mismos códigos,
//! en el mismo orden). Ver `docs/modelo.md`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::model::{Class, Course, Event, RaceResult};

/// Referencia a una categoría dentro de un `Event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassRef {
    /// Posición de la categoría en `Event::classes`.
    pub index: usize,
    /// Identificador de la categoría en el fichero de origen (`Class::id`).
    pub id: u32,
    pub name: String,
}

/// Recorrido y categorías que lo corren, en el orden en que aparecen en el `Event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseGroup {
    pub course: Course,
    pub classes: Vec<ClassRef>,
}

impl CourseGroup {
    /// Categorías del grupo, tomadas del `Event` del que salió la agrupación.
    ///
    /// Hay que pasar el mismo `event` que se agrupó: se busca por `ClassRef::index` y los
    /// índices fuera de rango se ignoran.
    pub fn classes_in<'a>(&'a self, event: &'a Event) -> impl Iterator<Item = &'a Class> + 'a {
        self.classes
            .iter()
            .filter_map(|class| event.classes.get(class.index))
    }

    /// Resultados de todas las categorías del recorrido, categoría a categoría.
    ///
    /// Es el conjunto sobre el que se calcula la referencia del tiempo perdido; el filtrado
    /// por estado (`RaceStatus::Ok`) lo hace quien lo consuma.
    pub fn results_in<'a>(&'a self, event: &'a Event) -> impl Iterator<Item = &'a RaceResult> + 'a {
        self.classes_in(event)
            .flat_map(|class| class.results.iter())
    }
}

/// Agrupa las categorías de `event` por recorrido.
///
/// Dos categorías van al mismo grupo si su secuencia de balizas es exactamente igual: mismos
/// códigos en el mismo orden. Los grupos salen en el orden de la primera categoría de cada
/// recorrido y, dentro de cada grupo, las categorías conservan su orden en `event.classes`.
/// Las categorías sin balizas (recorrido vacío) forman un grupo más.
pub fn group_by_course(event: &Event) -> Vec<CourseGroup> {
    let mut groups: Vec<CourseGroup> = Vec::new();
    let mut by_course: HashMap<&Course, usize> = HashMap::new();
    for (index, class) in event.classes.iter().enumerate() {
        let class_ref = ClassRef {
            index,
            id: class.id,
            name: class.name.clone(),
        };
        let group = *by_course.entry(&class.course).or_insert_with(|| {
            groups.push(CourseGroup {
                course: class.course.clone(),
                classes: Vec::new(),
            });
            groups.len() - 1
        });
        if let Some(group) = groups.get_mut(group) {
            group.classes.push(class_ref);
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RaceStatus, Runner};
    use chrono::NaiveDate;
    use serde_json::json;

    fn result(runner_id: u32) -> RaceResult {
        RaceResult {
            runner: Runner {
                id: runner_id,
                given_name: format!("Corredor {runner_id}"),
                family_name: String::new(),
                club: None,
                bib: None,
                si_card: None,
                sex: None,
            },
            status: RaceStatus::Ok,
            place: None,
            punches: Vec::new(),
        }
    }

    fn class(id: u32, name: &str, controls: &[u16], runners: &[u32]) -> Class {
        Class {
            id,
            name: name.into(),
            short_name: None,
            course: Course {
                controls: controls.to_vec(),
            },
            results: runners.iter().copied().map(result).collect(),
        }
    }

    fn event(classes: Vec<Class>) -> Event {
        Event {
            name: None,
            date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
            classes,
        }
    }

    fn names(group: &CourseGroup) -> Vec<&str> {
        group.classes.iter().map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn classes_sharing_a_course_are_grouped_in_order() {
        let event = event(vec![
            class(10, "M21A", &[31, 32, 33], &[1, 2]),
            class(11, "F21A", &[31, 34], &[3]),
            class(12, "M35", &[31, 32, 33], &[4]),
            class(13, "F35", &[31, 34], &[5, 6]),
            class(14, "M50", &[31, 32, 33], &[]),
        ]);
        let groups = group_by_course(&event);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].course.controls, vec![31, 32, 33]);
        assert_eq!(names(&groups[0]), ["M21A", "M35", "M50"]);
        assert_eq!(
            groups[0].classes,
            vec![
                ClassRef {
                    index: 0,
                    id: 10,
                    name: "M21A".into()
                },
                ClassRef {
                    index: 2,
                    id: 12,
                    name: "M35".into()
                },
                ClassRef {
                    index: 4,
                    id: 14,
                    name: "M50".into()
                },
            ]
        );
        assert_eq!(groups[1].course.controls, vec![31, 34]);
        assert_eq!(names(&groups[1]), ["F21A", "F35"]);
    }

    #[test]
    fn same_controls_in_another_order_is_another_course() {
        let event = event(vec![
            class(1, "A", &[31, 32, 33], &[]),
            class(2, "B", &[33, 32, 31], &[]),
            class(3, "C", &[31, 33, 32], &[]),
        ]);
        let groups = group_by_course(&event);
        assert_eq!(groups.len(), 3);
        assert!(groups.iter().all(|g| g.classes.len() == 1));
    }

    #[test]
    fn prefix_of_a_course_is_another_course() {
        let event = event(vec![
            class(1, "A", &[31, 32], &[]),
            class(2, "B", &[31, 32, 33], &[]),
        ]);
        assert_eq!(group_by_course(&event).len(), 2);
    }

    #[test]
    fn empty_courses_form_their_own_group() {
        let event = event(vec![
            class(1, "SIN RECORRIDO 1", &[], &[]),
            class(2, "A", &[31], &[]),
            class(3, "SIN RECORRIDO 2", &[], &[]),
        ]);
        let groups = group_by_course(&event);
        assert_eq!(groups.len(), 2);
        assert!(groups[0].course.controls.is_empty());
        assert_eq!(names(&groups[0]), ["SIN RECORRIDO 1", "SIN RECORRIDO 2"]);
        assert_eq!(names(&groups[1]), ["A"]);
    }

    #[test]
    fn event_without_classes_has_no_groups() {
        assert!(group_by_course(&event(Vec::new())).is_empty());
    }

    #[test]
    fn results_in_joins_results_of_all_classes_of_the_course() {
        let event = event(vec![
            class(1, "A", &[31, 32], &[1, 2]),
            class(2, "B", &[40], &[3]),
            class(3, "C", &[31, 32], &[4]),
        ]);
        let groups = group_by_course(&event);

        let classes: Vec<u32> = groups[0].classes_in(&event).map(|c| c.id).collect();
        assert_eq!(classes, [1, 3]);
        let runners: Vec<u32> = groups[0].results_in(&event).map(|r| r.runner.id).collect();
        assert_eq!(runners, [1, 2, 4]);
        let runners: Vec<u32> = groups[1].results_in(&event).map(|r| r.runner.id).collect();
        assert_eq!(runners, [3]);
    }

    #[test]
    fn references_missing_from_the_event_are_skipped() {
        let full = event(vec![class(1, "A", &[31], &[1]), class(2, "B", &[31], &[2])]);
        let groups = group_by_course(&full);
        let partial = event(vec![class(1, "A", &[31], &[1])]);
        let runners: Vec<u32> = groups[0]
            .results_in(&partial)
            .map(|r| r.runner.id)
            .collect();
        assert_eq!(runners, [1]);
    }

    #[test]
    fn group_serializes_to_json() {
        let event = event(vec![class(7, "F21A", &[31, 45], &[])]);
        let groups = group_by_course(&event);
        let value = serde_json::to_value(&groups).unwrap();
        assert_eq!(
            value,
            json!([{
                "course": { "controls": [31, 45] },
                "classes": [{ "index": 0, "id": 7, "name": "F21A" }]
            }])
        );
        let back: Vec<CourseGroup> = serde_json::from_value(value).unwrap();
        assert_eq!(back, groups);
    }
}
