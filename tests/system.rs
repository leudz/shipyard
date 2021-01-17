use shipyard::*;

fn two_views(_: View<u32>, _: ViewMut<u32>) {}
fn two_views_mut(_: ViewMut<u32>, _: ViewMut<u32>) {}
fn all_storages(_: AllStoragesViewMut, _: EntitiesView) {}

#[test]
fn bad_systems() {
    assert_eq!(
        two_views.into_workload_system().err(),
        Some(error::InvalidSystem::MultipleViews)
    );
    assert_eq!(
        two_views_mut.into_workload_system().err(),
        Some(error::InvalidSystem::MultipleViewsMut)
    );
    assert_eq!(
        all_storages.into_workload_system().err(),
        Some(error::InvalidSystem::AllStorages)
    );
}
