//! Agrupación por recorrido sobre el fixture público de Baltanás.
//!
//! La agrupación esperada sale de `fixtures/spl/baltanas-anon.expected.json` (lector de
//! referencia): 18 categorías en 9 recorridos.

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use tramos_core::courses::group_by_course;
use tramos_core::importers::spl;
use tramos_core::model::Event;

fn load() -> Event {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/spl/baltanas-anon.spl");
    let data = std::fs::read(path).unwrap();
    spl::read(&data).unwrap()
}

#[test]
fn baltanas_has_nine_courses() {
    let event = load();
    let groups = group_by_course(&event);

    // (balizas del recorrido, categorías en orden del fichero, corredores del recorrido)
    let expected: [(usize, &[&str], usize); 9] = [
        (11, &["ALEVÍN", "OPENAMARILLO"], 33),
        (11, &["F-CAD", "F-VET B", "OPEN ROJO"], 52),
        (12, &["F-JUN", "M-VET B"], 36),
        (13, &["F-SEN"], 12),
        (12, &["F-VET A", "M-CAD", "M-VET C"], 52),
        (11, &["F-VET C", "M-VET D"], 16),
        (15, &["M-JUN", "M-VET A"], 26),
        (20, &["M-SEN"], 24),
        (12, &["OPEN NARANJA", "F-VET D"], 24),
    ];

    let got: Vec<(usize, Vec<&str>, usize)> = groups
        .iter()
        .map(|g| {
            (
                g.course.controls.len(),
                g.classes.iter().map(|c| c.name.as_str()).collect(),
                g.results_in(&event).count(),
            )
        })
        .collect();
    let expected: Vec<(usize, Vec<&str>, usize)> = expected
        .iter()
        .map(|(n, names, runners)| (*n, names.to_vec(), *runners))
        .collect();
    assert_eq!(got, expected);

    // Cada categoría aparece en un solo grupo y su referencia apunta a ella.
    let total: usize = groups.iter().map(|g| g.classes.len()).sum();
    assert_eq!(total, event.classes.len());
    for group in &groups {
        for (class_ref, class) in group.classes.iter().zip(group.classes_in(&event)) {
            assert_eq!(class_ref.id, class.id);
            assert_eq!(class_ref.name, class.name);
            assert_eq!(class.course, group.course);
        }
    }

    // Primeras balizas de los recorridos que empiezan igual: comprueban que no se confunden.
    assert_eq!(groups[0].course.controls[..4], [31, 41, 57, 33]);
    assert_eq!(groups[1].course.controls[..4], [31, 41, 58, 42]);
}
