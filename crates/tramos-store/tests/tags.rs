//! Etiquetas de los tramos (docs/taxonomia.md), con datos sintéticos.

// `allow-unwrap-in-tests` (clippy.toml) no cubre los helpers de un test de integración.
#![allow(clippy::unwrap_used)]

use chrono::NaiveDate;
use tramos_core::model::{Class, Course, Event, RaceResult, RaceStatus, Runner};
use tramos_core::taxonomy::{Confirmation, LegPart, LegTag};
use tramos_store::{ResultId, Store, StoreError};

/// Una carrera sintética con dos corredores; devuelve sus resultados.
fn two_results(store: &mut Store) -> (ResultId, ResultId) {
    let runner = |given: &str| RaceResult {
        runner: Runner {
            given_name: given.into(),
            family_name: "Sintética".into(),
            club: None,
            bib: None,
            si_card: None,
            sex: None,
        },
        status: RaceStatus::Ok,
        place: None,
        punches: vec![],
    };
    let event = Event {
        name: None,
        date: NaiveDate::from_ymd_opt(2026, 10, 3).unwrap(),
        classes: vec![Class {
            id: 1,
            name: "F21A".into(),
            short_name: None,
            course: Course {
                controls: vec![31, 45],
            },
            results: vec![runner("Ana"), runner("Berta")],
        }],
    };
    let saved = store.save_event(&event, None).unwrap();
    (saved.results[0][0], saved.results[0][1])
}

fn full_tag() -> LegTag {
    LegTag {
        confirmation: Some(Confirmation::Error),
        error_type: Some("navigation".into()),
        error_subtype: Some("parallel".into()),
        causes: vec!["rush".into(), "fatigue".into()],
        leg_part: Some(LegPart::Middle),
        perceived_loss_s: Some(40.0),
        effort: Some(8),
        note: Some("Vaguada equivocada".into()),
    }
}

#[test]
fn a_tag_round_trips_with_its_taxonomy_version() {
    let mut store = Store::open_in_memory().unwrap();
    let (ana, berta) = two_results(&mut store);
    let saved = store.save_tag(ana, 2, "0", &full_tag()).unwrap().unwrap();
    assert_eq!(saved.leg_index, 2);
    assert_eq!(saved.taxonomy_version, "0");
    // Las causas vuelven ordenadas por clave.
    let mut expected = full_tag();
    expected.causes = vec!["fatigue".into(), "rush".into()];
    assert_eq!(saved.tag, expected);
    assert_eq!(saved.created_at, saved.updated_at);

    let tags = store.tags(ana).unwrap();
    assert_eq!(tags, [saved]);
    assert!(store.tags(berta).unwrap().is_empty());
}

#[test]
fn saving_again_replaces_the_tag_and_keeps_its_creation_instant() {
    let mut store = Store::open_in_memory().unwrap();
    let (ana, _) = two_results(&mut store);
    let first = store.save_tag(ana, 3, "0", &full_tag()).unwrap().unwrap();
    let only_confirmation = LegTag {
        confirmation: Some(Confirmation::Physical),
        causes: vec!["lack_of_focus".into()],
        ..LegTag::default()
    };
    let second = store
        .save_tag(ana, 3, "1", &only_confirmation)
        .unwrap()
        .unwrap();
    assert_eq!(second.created_at, first.created_at);
    assert!(second.updated_at >= first.updated_at);
    assert_eq!(second.taxonomy_version, "1");

    let tags = store.tags(ana).unwrap();
    assert_eq!(tags.len(), 1);
    // Todo lo de antes se ha ido, causas incluidas.
    assert_eq!(tags[0].tag, only_confirmation);
}

#[test]
fn tags_come_back_by_leg() {
    let mut store = Store::open_in_memory().unwrap();
    let (ana, _) = two_results(&mut store);
    let confirm = |c| LegTag {
        confirmation: Some(c),
        ..LegTag::default()
    };
    for (leg, c) in [
        (5, Confirmation::NoError),
        (1, Confirmation::Error),
        (3, Confirmation::Physical),
    ] {
        store.save_tag(ana, leg, "0", &confirm(c)).unwrap();
    }
    let legs: Vec<_> = store
        .tags(ana)
        .unwrap()
        .iter()
        .map(|t| (t.leg_index, t.tag.confirmation))
        .collect();
    assert_eq!(
        legs,
        [
            (1, Some(Confirmation::Error)),
            (3, Some(Confirmation::Physical)),
            (5, Some(Confirmation::NoError)),
        ]
    );
}

#[test]
fn an_empty_tag_deletes_the_tag() {
    let mut store = Store::open_in_memory().unwrap();
    let (ana, _) = two_results(&mut store);
    store.save_tag(ana, 2, "0", &full_tag()).unwrap();
    assert_eq!(
        store.save_tag(ana, 2, "0", &LegTag::default()).unwrap(),
        None
    );
    assert!(store.tags(ana).unwrap().is_empty());
    // Borrar lo que no está no falla.
    store.delete_tag(ana, 2).unwrap();
}

#[test]
fn delete_tag_removes_only_that_leg() {
    let mut store = Store::open_in_memory().unwrap();
    let (ana, _) = two_results(&mut store);
    store.save_tag(ana, 1, "0", &full_tag()).unwrap();
    store.save_tag(ana, 2, "0", &full_tag()).unwrap();
    store.delete_tag(ana, 1).unwrap();
    let legs: Vec<_> = store
        .tags(ana)
        .unwrap()
        .iter()
        .map(|t| t.leg_index)
        .collect();
    assert_eq!(legs, [2]);
}

#[test]
fn invalid_requests_are_errors() {
    let mut store = Store::open_in_memory().unwrap();
    let (ana, _) = two_results(&mut store);
    assert!(matches!(
        store.save_tag(ana, 0, "0", &full_tag()),
        Err(StoreError::InvalidLegIndex(0))
    ));
    assert!(matches!(
        store.save_tag(ResultId(999), 1, "0", &full_tag()),
        Err(StoreError::ResultNotFound(999))
    ));
    assert!(matches!(
        store.tags(ResultId(999)),
        Err(StoreError::ResultNotFound(999))
    ));
    let nan = LegTag {
        perceived_loss_s: Some(f64::NAN),
        ..LegTag::default()
    };
    assert!(matches!(
        store.save_tag(ana, 1, "0", &nan),
        Err(StoreError::NotANumber("perceived_loss_s"))
    ));
    assert!(store.tags(ana).unwrap().is_empty());
}
