//! Identificar al corredor en el fixture público de Baltanás (anonimizado: los nombres son
//! seudónimos y la tarjeta es el número de orden del corredor).

// Fichero de test: `allow-unwrap-in-tests` no cubre las funciones auxiliares.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use tramos_core::identify::{Identification, ResultRef, RunnerIdentity, identify_runner};
use tramos_core::importers::spl;
use tramos_core::model::Event;

fn baltanas() -> Event {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/spl/baltanas-anon.spl");
    spl::read(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn baltanas_runner_is_found_by_card_and_by_name() {
    let event = baltanas();
    // El corredor del FIT sintético (`tools/fit_sintetico.py --card 143`): 16.º de M-SEN.
    let expected = ResultRef {
        class_index: 9,
        result_index: 15,
    };

    let by_card = identify_runner(
        &event,
        &RunnerIdentity {
            si_card: Some(143),
            full_name: None,
        },
    );
    let Identification::Unique {
        candidate,
        name_mismatch: false,
    } = by_card
    else {
        panic!("{by_card:?}");
    };
    assert_eq!(candidate.result, expected);
    let result = expected.get(&event).unwrap();
    assert_eq!(event.classes[9].name, "M-SEN");
    assert_eq!(result.place, Some(16));
    assert_eq!(result.runner.si_card, Some(143));

    // Por nombre (seudónimo «N143_» «Apellido143_______»; el relleno `_` separa palabras).
    let by_name = identify_runner(
        &event,
        &RunnerIdentity {
            si_card: None,
            full_name: Some("n143 APELLIDO143".into()),
        },
    );
    assert!(matches!(
        by_name,
        Identification::Unique { candidate, name_mismatch: false } if candidate.result == expected
    ));
}
