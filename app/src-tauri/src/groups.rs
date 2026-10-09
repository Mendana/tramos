//! Grupos de atletas de quien entrena (#120, `docs/app.md`, "Atletas"): crear, cambiar y borrar
//! grupos, y meter o sacar atletas. Se guardan en la base propia (`docs/almacenamiento.md`); los
//! miembros van por el `runner_id` de sus paquetes.

use serde::{Deserialize, Serialize};
use tramos_store::{AthleteGroupId, Store, StoreError};

use crate::coach::own_runner;

/// Lo que se edita de un grupo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupFields {
    pub name: String,
    pub description: String,
    /// `#rrggbb`.
    pub color: String,
}

/// Un grupo con sus miembros.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupInfo {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub color: String,
    /// `runner_id` de sus miembros. Puede haber alguno que ya no está entre los candidatos (sus
    /// paquetes se han borrado): sigue en el grupo.
    pub members: Vec<String>,
}

/// Alguien a quien se puede meter en un grupo: un atleta del que hay paquetes o quien usa la app.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupCandidate {
    pub runner_id: String,
    pub display_name: String,
    /// Es quien usa la app, con su propio identificador.
    pub is_self: bool,
}

/// Los grupos y a quién se puede meter en ellos.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupsView {
    pub groups: Vec<GroupInfo>,
    /// Quien usa la app primero (si tiene carreras) y después sus atletas, como en el selector.
    pub candidates: Vec<GroupCandidate>,
}

pub fn groups_view(store: &mut Store) -> Result<GroupsView, StoreError> {
    let groups = store
        .athlete_groups()?
        .into_iter()
        .map(|g| GroupInfo {
            id: g.id.0,
            name: g.name,
            description: g.description,
            color: g.color,
            members: g.members,
        })
        .collect();
    let mut candidates = Vec::new();
    if let Some(me) = own_runner(store)? {
        candidates.push(GroupCandidate {
            runner_id: me.runner_id,
            display_name: me.display_name,
            is_self: true,
        });
    }
    candidates.extend(
        store
            .received_runners()?
            .into_iter()
            .map(|r| GroupCandidate {
                runner_id: r.runner_id,
                display_name: r.display_name,
                is_self: false,
            }),
    );
    Ok(GroupsView { groups, candidates })
}

pub fn create(store: &mut Store, group: &GroupFields) -> Result<i64, StoreError> {
    Ok(store
        .create_athlete_group(&group.name, &group.description, &group.color)?
        .0)
}

pub fn update(store: &mut Store, id: i64, group: &GroupFields) -> Result<(), StoreError> {
    store.update_athlete_group(
        AthleteGroupId(id),
        &group.name,
        &group.description,
        &group.color,
    )
}

pub fn delete(store: &mut Store, id: i64) -> Result<(), StoreError> {
    store.delete_athlete_group(AthleteGroupId(id))
}

pub fn set_member(
    store: &mut Store,
    id: i64,
    runner_id: &str,
    member: bool,
) -> Result<(), StoreError> {
    if member {
        store.add_athlete_group_member(AthleteGroupId(id), runner_id)
    } else {
        store.remove_athlete_group_member(AthleteGroupId(id), runner_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach::{group_view, runner_view};
    use crate::package::race_package;
    use crate::race_map::tests::imported;
    use tramos_core::history::HistoryFilter;
    use tramos_core::package::{RacePackage, ShareLevel};

    fn fields(name: &str) -> GroupFields {
        GroupFields {
            name: name.into(),
            description: String::new(),
            color: "#1f6feb".into(),
        }
    }

    /// Quien entrena recibe la misma carrera de tres atletas sintéticos (`a`, `b` y `c`), cada
    /// uno en otro puesto de la categoría.
    fn three_athletes() -> Store {
        let (mut runner, result_id) = imported(false);
        let original = race_package(&mut runner, result_id, ShareLevel::Legs).unwrap();
        let mut coach = Store::open_in_memory().unwrap();
        for (i, id) in ["a", "b", "c"].into_iter().enumerate() {
            let mut package = RacePackage::parse(&original.to_json().unwrap()).unwrap();
            package.runner.runner_id = id.into();
            package.runner.display_name = id.to_uppercase();
            if let Some(course) = package.course.as_mut() {
                course.result.result_index = i;
            }
            coach.save_received_package(&package).unwrap();
        }
        coach
    }

    #[test]
    fn the_group_view_of_a_group_only_counts_its_members() {
        let mut coach = three_athletes();
        let first = create(&mut coach, &fields("Primero")).unwrap();
        let second = create(&mut coach, &fields("Segundo")).unwrap();
        for (group, runner) in [(first, "a"), (first, "b"), (second, "b"), (second, "c")] {
            set_member(&mut coach, group, runner, true).unwrap();
        }
        let filter = HistoryFilter::default();
        let ids = |view: &crate::coach::GroupView| -> Vec<String> {
            view.runners
                .iter()
                .map(|r| r.runner.runner_id.clone())
                .collect()
        };

        let all = group_view(&mut coach, &filter, false, None).unwrap();
        assert_eq!(ids(&all), ["a", "b", "c"]);
        assert_eq!(all.comparison.shared_races[0].results.len(), 3);
        assert_eq!(all.total.races, 3);
        assert_eq!(all.group, None);

        let one = group_view(&mut coach, &filter, false, Some(AthleteGroupId(first))).unwrap();
        assert_eq!(ids(&one), ["a", "b"]);
        assert_eq!(one.group, Some(first));
        // La carrera compartida y el cara a cara, solo entre ellos.
        assert_eq!(one.comparison.shared_races[0].results.len(), 2);
        assert_eq!(one.comparison.head_to_head.len(), 2);
        // Los totales, los de sus dos filas juntas.
        let rows: Vec<_> = one
            .runners
            .iter()
            .map(|r| r.row.as_ref().unwrap().stats)
            .collect();
        assert_eq!(one.total.races, 2);
        assert_eq!(one.total.legs, rows[0].legs + rows[1].legs);
        assert_eq!(one.total.errors, rows[0].errors + rows[1].errors);

        // `b` está en los dos.
        let two = group_view(&mut coach, &filter, false, Some(AthleteGroupId(second))).unwrap();
        assert_eq!(ids(&two), ["b", "c"]);

        // Un grupo que no existe es un error.
        assert!(group_view(&mut coach, &filter, false, Some(AthleteGroupId(99))).is_err());
    }

    #[test]
    fn whoever_coaches_joins_a_group_with_their_own_id() {
        let (mut both, _) = imported(false);
        let (mut athlete, result_id) = imported(false);
        let package = race_package(&mut athlete, result_id, ShareLevel::Legs).unwrap();
        both.save_received_package(&package).unwrap();

        let view = groups_view(&mut both).unwrap();
        assert_eq!(view.candidates.len(), 2);
        let me = &view.candidates[0];
        assert!(me.is_self);
        assert_eq!(me.runner_id, both.package_runner_id().unwrap());

        let group = create(&mut both, &fields("Con quien entrena")).unwrap();
        set_member(&mut both, group, &me.runner_id.clone(), true).unwrap();
        // En un grupo, cuenta quien es miembro, aunque «Incluirme» esté desmarcada.
        let filter = HistoryFilter::default();
        let only_me = group_view(&mut both, &filter, false, Some(AthleteGroupId(group))).unwrap();
        assert_eq!(only_me.runners.len(), 1);
        assert!(only_me.runners[0].is_self);
    }

    #[test]
    fn groups_are_edited_and_deleting_one_keeps_the_packages() {
        let mut coach = three_athletes();
        let id = create(&mut coach, &fields("Juveniles")).unwrap();
        set_member(&mut coach, id, "a", true).unwrap();
        set_member(&mut coach, id, "c", true).unwrap();
        set_member(&mut coach, id, "c", false).unwrap();
        update(
            &mut coach,
            id,
            &GroupFields {
                name: "Júnior".into(),
                description: "Temporada de otoño".into(),
                color: "#AA00CC".into(),
            },
        )
        .unwrap();
        let view = groups_view(&mut coach).unwrap();
        assert_eq!(
            view.groups,
            [GroupInfo {
                id,
                name: "Júnior".into(),
                description: "Temporada de otoño".into(),
                color: "#aa00cc".into(),
                members: vec!["a".into()],
            }]
        );
        // Sin carreras propias, solo los atletas.
        assert!(view.candidates.iter().all(|c| !c.is_self));
        assert_eq!(view.candidates.len(), 3);

        let packages = coach.received_packages().unwrap();
        delete(&mut coach, id).unwrap();
        assert!(groups_view(&mut coach).unwrap().groups.is_empty());
        assert_eq!(coach.received_packages().unwrap(), packages);
        assert!(runner_view(&coach, "a").is_ok());
        assert!(update(&mut coach, id, &fields("Otra vez")).is_err());
    }
}
