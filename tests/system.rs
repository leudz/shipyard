use shipyard::*;

fn two_views(_: View<u32>, _: ViewMut<u32>) {}
fn two_views_mut(_: ViewMut<u32>, _: ViewMut<u32>) {}
fn all_storages(_: AllStoragesViewMut, _: EntitiesView) {}

#[test]
fn bad_systems() {
    let world = World::new();

    assert_eq!(
        world
            .try_add_workload("")
            .unwrap()
            .try_with_system(system!(two_views))
            .err(),
        Some(error::InvalidSystem::MultipleViews)
    );
    assert_eq!(
        world
            .try_add_workload("")
            .unwrap()
            .try_with_system(system!(two_views_mut))
            .err(),
        Some(error::InvalidSystem::MultipleViewsMut)
    );
    assert_eq!(
        world
            .try_add_workload("")
            .unwrap()
            .try_with_system(system!(all_storages))
            .err(),
        Some(error::InvalidSystem::AllStorages)
    );
}
