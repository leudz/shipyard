use shipyard::*;

#[allow(unused)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

fn two_views(_: View<U32>, _: ViewMut<U32>) {}
fn two_views_mut(_: ViewMut<U32>, _: ViewMut<U32>) {}
fn two_views_mut_mid(_: ViewMut<U32>, _: ViewMut<U32>, _: EntitiesView, _: EntitiesView) {}
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
        two_views_mut_mid.into_workload_system().err(),
        Some(error::InvalidSystem::MultipleViewsMut)
    );
    assert_eq!(
        all_storages.into_workload_system().err(),
        Some(error::InvalidSystem::AllStorages)
    );
}
