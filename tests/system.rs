use shipyard::*;

fn two_views(_: View<u32>, _: ViewMut<u32>) {}
fn two_views_mut(_: ViewMut<u32>, _: ViewMut<u32>) {}
fn all_storages(_: AllStoragesViewMut, _: EntitiesView) {}

#[test]
fn bad_systems() {
    assert_eq!(
        WorkloadSystem::new(|world| world.try_run(two_views), two_views).err(),
        Some(error::InvalidSystem::MultipleViews)
    );
    assert_eq!(
        WorkloadSystem::new(|world| world.try_run(two_views_mut), two_views_mut).err(),
        Some(error::InvalidSystem::MultipleViewsMut)
    );
    assert_eq!(
        WorkloadSystem::new(|world| world.try_run(all_storages), all_storages).err(),
        Some(error::InvalidSystem::AllStorages)
    );
}
