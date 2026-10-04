//! Ida y vuelta por SQLite del fixture público de Baltanás. Sus corredores no traen id (#47,
//! #64): cada resultado se guarda por su posición, aunque coincidan nombre o club.

// `allow-unwrap-in-tests` (clippy.toml) no cubre los helpers de un test de integración.
#![allow(clippy::unwrap_used)]

use std::collections::HashSet;
use std::path::PathBuf;

use tramos_core::importers::spl;
use tramos_core::model::Event;
use tramos_store::{SourceFileKind, Store};

fn baltanas() -> (Vec<u8>, Event) {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/spl/baltanas-anon.spl");
    let data = std::fs::read(path).unwrap();
    let event = spl::read(&data).unwrap();
    (data, event)
}

#[test]
fn baltanas_round_trips() {
    let (data, event) = baltanas();

    let mut store = Store::open_in_memory().unwrap();
    let source = store
        .save_source_file(SourceFileKind::Spl, "baltanas-anon.spl", &data)
        .unwrap();
    let saved = store.save_event(&event, Some(source)).unwrap();

    // Un resultado por corredor: ninguno se pierde ni se mezcla.
    let result_ids: Vec<_> = saved.results.iter().flatten().collect();
    assert_eq!(result_ids.len(), 275);
    assert_eq!(result_ids.iter().collect::<HashSet<_>>().len(), 275);
    assert_eq!(store.result_ids(saved.id).unwrap(), saved.results);

    assert_eq!(store.load_event(saved.id).unwrap(), event);

    // Guardar la misma carrera otra vez tampoco choca con nada.
    let again = store.save_event(&event, Some(source)).unwrap();
    assert_ne!(again.id, saved.id);
    assert_eq!(store.load_event(again.id).unwrap(), event);
}
