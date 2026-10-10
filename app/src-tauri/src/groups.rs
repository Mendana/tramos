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
    fn two_groups_are_compared_with_both_options() {
        use crate::coach::compare_athlete_groups;
        use tramos_core::group_compare::{CompareOptions, Overlap, RaceSelection};

        let mut coach = three_athletes();
        let first = create(&mut coach, &fields("Primero")).unwrap();
        let second = create(&mut coach, &fields("Segundo")).unwrap();
        for (group, runner) in [(first, "a"), (first, "b"), (second, "b"), (second, "c")] {
            set_member(&mut coach, group, runner, true).unwrap();
        }
        let filter = HistoryFilter::default();
        let compare = |coach: &mut Store, a: i64, b: i64, overlap, races| {
            compare_athlete_groups(
                coach,
                &filter,
                AthleteGroupId(a),
                AthleteGroupId(b),
                CompareOptions { overlap, races },
            )
            .unwrap()
        };

        // `b` en los dos: cuenta en los dos o en ninguno. Los tres corrieron la misma carrera,
        // así que es de los dos grupos.
        let both = compare(
            &mut coach,
            first,
            second,
            Overlap::CountInBoth,
            RaceSelection::Shared,
        );
        assert_eq!((both.a.runners, both.b.runners, both.in_both), (2, 2, 1));
        assert_eq!(both.shared_races, Some(1));
        assert_eq!((both.a.stats.races, both.b.stats.races), (2, 2));
        let apart = compare(
            &mut coach,
            first,
            second,
            Overlap::Exclude,
            RaceSelection::Shared,
        );
        assert_eq!((apart.a.runners, apart.b.runners, apart.in_both), (1, 1, 1));
        // Cada lado, los números de su único miembro.
        let a_alone = group_view(&mut coach, &filter, false, None)
            .unwrap()
            .runners[0]
            .row
            .as_ref()
            .unwrap()
            .stats;
        assert_eq!(apart.a.stats.legs, a_alone.legs);
        assert_eq!(apart.a.stats.errors, a_alone.errors);
        assert_eq!(
            apart.performance_difference.is_some(),
            apart.a.stats.mean_performance.is_some() && apart.b.stats.mean_performance.is_some()
        );

        // Un grupo frente a sí mismo: la carrera solo la corre la misma gente, así que con
        // «solo las de los dos» no entra ninguna; con «todas», sí.
        let alone = create(&mut coach, &fields("Solo a")).unwrap();
        set_member(&mut coach, alone, "a", true).unwrap();
        let shared = compare(
            &mut coach,
            alone,
            alone,
            Overlap::CountInBoth,
            RaceSelection::Shared,
        );
        assert_eq!(shared.shared_races, Some(0));
        assert_eq!((shared.a.stats.races, shared.a.runners), (0, 0));
        let all = compare(
            &mut coach,
            alone,
            alone,
            Overlap::CountInBoth,
            RaceSelection::All,
        );
        assert_eq!(all.shared_races, None);
        assert_eq!((all.a.stats.races, all.b.stats.races), (1, 1));
        assert_eq!(all.performance_difference, Some(0.0));
    }

    #[test]
    fn the_group_stats_pool_the_statistics_of_its_members() {
        use crate::coach::athlete_group_stats;
        use crate::history::history_view;
        use tramos_core::group::group_total;

        let mut coach = three_athletes();
        let id = create(&mut coach, &fields("Juveniles")).unwrap();
        // «fantasma» no tiene paquetes: no cuenta.
        for runner in ["a", "b", "fantasma"] {
            set_member(&mut coach, id, runner, true).unwrap();
        }
        let filter = HistoryFilter::default();
        let group = athlete_group_stats(&mut coach, &filter, AthleteGroupId(id)).unwrap();
        assert_eq!(group.name, "Juveniles");
        let names: Vec<&str> = group
            .members
            .iter()
            .map(|m| m.runner.display_name.as_str())
            .collect();
        assert_eq!(names, ["A", "B"]);
        assert!(group.members.iter().all(|m| m.races == 1 && !m.is_self));
        assert_eq!(group.missing, 1);
        assert_eq!(group.stats.runners, 2);

        // Lo mismo que las Estadísticas de cada uno, juntas.
        let own: Vec<_> = ["a", "b"]
            .iter()
            .map(|r| history_view(&runner_view(&coach, r).unwrap().store, &filter).unwrap())
            .collect();
        let stats = &group.stats;
        assert_eq!(
            stats.history.total,
            group_total(&[own[0].history.total, own[1].history.total])
        );
        assert_eq!(stats.history.total.races, 2);
        let legs: usize = stats.by_leg_length.iter().map(|b| b.legs).sum();
        assert_eq!(legs, stats.history.total.legs);
        assert_eq!(
            stats.common_errors.total.errors,
            own[0].common_errors.total.errors + own[1].common_errors.total.errors
        );
        assert_eq!(
            stats.after_error.after_clean.legs,
            own[0].after_error.after_clean.legs + own[1].after_error.after_clean.legs
        );
        assert_eq!(
            stats.by_slope.races_without_track,
            own[0].by_slope.races_without_track + own[1].by_slope.races_without_track
        );
        assert_eq!(
            stats.days_off.without_previous,
            own[0].days_off.without_previous + own[1].days_off.without_previous
        );

        // Un grupo que no existe es un error.
        assert!(athlete_group_stats(&mut coach, &filter, AthleteGroupId(99)).is_err());
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
