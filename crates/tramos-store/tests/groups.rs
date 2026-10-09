//! Grupos de atletas (#120, docs/almacenamiento.md).

// `allow-unwrap-in-tests` (clippy.toml) no cubre los helpers de un test de integración.
#![allow(clippy::unwrap_used)]

use tramos_store::{AthleteGroup, AthleteGroupId, Store, StoreError};

fn group(store: &Store, id: AthleteGroupId) -> AthleteGroup {
    store
        .athlete_groups()
        .unwrap()
        .into_iter()
        .find(|g| g.id == id)
        .unwrap()
}

#[test]
fn a_group_is_created_edited_and_its_members_change() {
    let mut store = Store::open_in_memory().unwrap();
    let id = store
        .create_athlete_group("  Juveniles ", " Los de 17 y 18 ", "#1F6FEB")
        .unwrap();
    let g = group(&store, id);
    assert_eq!(
        (g.name.as_str(), g.description.as_str(), g.color.as_str()),
        ("Juveniles", "Los de 17 y 18", "#1f6feb")
    );
    assert!(g.members.is_empty());

    store.add_athlete_group_member(id, "b").unwrap();
    store.add_athlete_group_member(id, "a").unwrap();
    // Meterlo otra vez no lo repite.
    store.add_athlete_group_member(id, "a").unwrap();
    assert_eq!(group(&store, id).members, ["a", "b"]);
    store.remove_athlete_group_member(id, "b").unwrap();
    // Sacar a quien no está no cambia nada.
    store.remove_athlete_group_member(id, "nadie").unwrap();
    assert_eq!(group(&store, id).members, ["a"]);

    store
        .update_athlete_group(id, "Júnior", "", "#aabbcc")
        .unwrap();
    let g = group(&store, id);
    assert_eq!(
        (g.name.as_str(), g.description.as_str(), g.color.as_str()),
        ("Júnior", "", "#aabbcc")
    );
    assert_eq!(g.members, ["a"]);
}

#[test]
fn an_athlete_can_be_in_two_groups() {
    let mut store = Store::open_in_memory().unwrap();
    let relay = store
        .create_athlete_group("Relevos", "", "#000000")
        .unwrap();
    let youth = store
        .create_athlete_group("Juveniles", "", "#ffffff")
        .unwrap();
    for (id, runner) in [(relay, "ana"), (relay, "berta"), (youth, "ana")] {
        store.add_athlete_group_member(id, runner).unwrap();
    }
    let groups = store.athlete_groups().unwrap();
    // Por nombre.
    let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Juveniles", "Relevos"]);
    assert_eq!(groups[0].members, ["ana"]);
    assert_eq!(groups[1].members, ["ana", "berta"]);

    // Sacarla de uno no la saca del otro.
    store.remove_athlete_group_member(relay, "ana").unwrap();
    assert_eq!(group(&store, youth).members, ["ana"]);
}

#[test]
fn deleting_a_group_keeps_the_other_groups_and_their_members() {
    let mut store = Store::open_in_memory().unwrap();
    let a = store.create_athlete_group("A", "", "#111111").unwrap();
    let b = store.create_athlete_group("B", "", "#222222").unwrap();
    store.add_athlete_group_member(a, "ana").unwrap();
    store.add_athlete_group_member(b, "ana").unwrap();
    store.delete_athlete_group(a).unwrap();
    let groups = store.athlete_groups().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].members, ["ana"]);
    assert!(matches!(
        store.delete_athlete_group(a),
        Err(StoreError::GroupNotFound(_))
    ));
}

#[test]
fn invalid_groups_are_not_saved() {
    let mut store = Store::open_in_memory().unwrap();
    assert!(matches!(
        store.create_athlete_group("   ", "", "#123456"),
        Err(StoreError::EmptyGroupName)
    ));
    for color in ["123456", "#12345", "#12345g", "rojo", ""] {
        assert!(
            matches!(
                store.create_athlete_group("G", "", color),
                Err(StoreError::InvalidColor(_))
            ),
            "{color}"
        );
    }
    assert!(store.athlete_groups().unwrap().is_empty());

    let missing = AthleteGroupId(99);
    assert!(matches!(
        store.update_athlete_group(missing, "G", "", "#123456"),
        Err(StoreError::GroupNotFound(99))
    ));
    assert!(matches!(
        store.add_athlete_group_member(missing, "ana"),
        Err(StoreError::GroupNotFound(99))
    ));
    assert!(matches!(
        store.remove_athlete_group_member(missing, "ana"),
        Err(StoreError::GroupNotFound(99))
    ));
}
