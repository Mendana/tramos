//! Paquetes por carrera recibidos (docs/paquete.md), con el fixture anonimizado.

// `allow-unwrap-in-tests` (clippy.toml) no cubre los helpers de un test de integración.
#![allow(clippy::unwrap_used)]

use chrono::{DateTime, TimeDelta, Utc};
use tramos_core::identify::ResultRef;
use tramos_core::importers::spl;
use tramos_core::lost_time::LostTimeConfig;
use tramos_core::model::Event;
use tramos_core::package::{
    PackageInput, PackageRunner, RacePackage, ShareChoice, ShareLevel, build, race_id,
};
use tramos_store::{SaveOutcome, Store};

fn fixture() -> Event {
    let path = format!(
        "{}/../../fixtures/spl/baltanas-anon.spl",
        env!("CARGO_MANIFEST_DIR")
    );
    spl::read(&std::fs::read(path).unwrap()).unwrap()
}

fn package(event: &Event, runner_id: &str, level: ShareLevel, minutes: i64) -> RacePackage {
    let base: DateTime<Utc> = "2026-10-05T10:00:00Z".parse().unwrap();
    build(PackageInput {
        level,
        exported_at: base + TimeDelta::minutes(minutes),
        runner: PackageRunner {
            runner_id: runner_id.into(),
            display_name: "Yo".into(),
        },
        event,
        at: ResultRef {
            class_index: 9,
            result_index: 15,
        },
        format: None,
        config: LostTimeConfig::default(),
        tags: Vec::new(),
        track: None,
    })
    .unwrap()
}

#[test]
fn a_received_package_round_trips() {
    let event = fixture();
    let mut store = Store::open_in_memory().unwrap();
    let sent = package(&event, "a1", ShareLevel::Legs, 0);
    assert_eq!(
        store.save_received_package(&sent).unwrap(),
        SaveOutcome::Created
    );
    let received = store.received_packages().unwrap();
    assert_eq!(received.len(), 1);
    let r = &received[0];
    assert_eq!(
        (r.runner_id.as_str(), r.race_id.as_str()),
        ("a1", race_id(&event).as_str())
    );
    assert_eq!((r.level, r.format_version), (ShareLevel::Legs, 1));
    assert_eq!(r.exported_at, sent.exported_at);
    assert_eq!(RacePackage::parse(&r.content).unwrap(), sent);
}

#[test]
fn a_new_export_replaces_the_old_one_and_an_older_one_is_ignored() {
    let event = fixture();
    let mut store = Store::open_in_memory().unwrap();
    store
        .save_received_package(&package(&event, "a1", ShareLevel::Legs, 0))
        .unwrap();
    // El mismo otra vez: no cambia nada.
    assert_eq!(
        store
            .save_received_package(&package(&event, "a1", ShareLevel::Legs, 0))
            .unwrap(),
        SaveOutcome::Unchanged
    );
    // Igual de reciente pero distinto: sustituye, sin duplicar.
    assert_eq!(
        store
            .save_received_package(&package(&event, "a1", ShareLevel::Aggregates, 0))
            .unwrap(),
        SaveOutcome::Replaced
    );
    // Más nuevo y con menos nivel: sustituye (el corredor ha dejado de compartir los tramos).
    let newer = package(&event, "a1", ShareLevel::Aggregates, 10);
    assert_eq!(
        store.save_received_package(&newer).unwrap(),
        SaveOutcome::Replaced
    );
    // Una copia vieja que llega tarde no pisa la buena.
    assert_eq!(
        store
            .save_received_package(&package(&event, "a1", ShareLevel::Legs, 5))
            .unwrap(),
        SaveOutcome::IgnoredOlder
    );
    // Otro corredor de la misma carrera: otro paquete.
    store
        .save_received_package(&package(&event, "b2", ShareLevel::Legs, 0))
        .unwrap();

    assert_eq!(store.received_package_counts().unwrap(), (2, 2));
    let received = store.received_packages().unwrap();
    let rows: Vec<_> = received
        .iter()
        .map(|r| (r.runner_id.as_str(), r.level))
        .collect();
    assert_eq!(
        rows,
        [("a1", ShareLevel::Aggregates), ("b2", ShareLevel::Legs)]
    );
    assert_eq!(RacePackage::parse(&received[0].content).unwrap(), newer);
}

#[test]
fn the_runner_id_is_generated_once() {
    let mut store = Store::open_in_memory().unwrap();
    let id = store.package_runner_id().unwrap();
    assert_eq!(id.len(), 32);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(store.package_runner_id().unwrap(), id);
    // Otra base, otro identificador.
    let mut other = Store::open_in_memory().unwrap();
    assert_ne!(other.package_runner_id().unwrap(), id);
}

#[test]
fn the_sharing_choice_is_kept_per_result() {
    let mut store = Store::open_in_memory().unwrap();
    let saved = store.save_event(&fixture(), None).unwrap();
    let (mine, other) = (saved.results[9][15], saved.results[9][14]);
    assert_eq!(store.result_sharing(mine).unwrap(), None);
    for choice in [
        ShareChoice::Nothing,
        ShareChoice::Aggregates,
        ShareChoice::Legs,
        ShareChoice::Track,
    ] {
        store.set_result_sharing(mine, Some(choice)).unwrap();
        assert_eq!(store.result_sharing(mine).unwrap(), Some(choice));
    }
    assert_eq!(store.result_sharing(other).unwrap(), None);
    // Sin elección vuelve al ajuste por defecto.
    store.set_result_sharing(mine, None).unwrap();
    assert_eq!(store.result_sharing(mine).unwrap(), None);
}

#[test]
fn received_runners_take_the_name_of_their_latest_package() {
    let event = fixture();
    let mut store = Store::open_in_memory().unwrap();
    let mut first = package(&event, "a1", ShareLevel::Legs, 0);
    first.runner.display_name = "Nombre viejo".into();
    let mut second = package(&event, "a1", ShareLevel::Aggregates, 10);
    second.race.race_id = "otra-carrera".into();
    second.runner.display_name = "Ana".into();
    let mut other = package(&event, "b2", ShareLevel::Legs, 5);
    other.runner.display_name = "berta".into();
    for p in [&first, &second, &other] {
        store.save_received_package(p).unwrap();
    }

    let runners = store.received_runners().unwrap();
    let rows: Vec<_> = runners
        .iter()
        .map(|r| (r.runner_id.as_str(), r.display_name.as_str(), r.packages))
        .collect();
    // Por nombre sin distinguir mayúsculas.
    assert_eq!(rows, [("a1", "Ana", 2), ("b2", "berta", 1)]);
    assert_eq!(runners[0].last_exported_at, second.exported_at);

    let of_a1 = store.received_packages_of("a1").unwrap();
    assert_eq!(of_a1.len(), 2);
    assert!(of_a1.iter().all(|p| p.runner_id == "a1"));
    assert!(store.received_packages_of("zz").unwrap().is_empty());
}
