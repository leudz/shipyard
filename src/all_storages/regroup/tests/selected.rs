use super::*;

/// Asserts that selecting every storage lists each of them once, sorted by id.
#[test]
fn all_sorted() {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    world.add_entity(C);
    let mut all_storages = world.all_storages_mut().unwrap();
    let storages = &mut all_storages.storages.get_mut().components_and_custom;
    let mut expected: Vec<StorageId> = storages.iter().map(|(storage_id, _)| storage_id).collect();
    expected.sort_unstable();

    let selected = Selected::all(storages);

    assert_eq!(selected.ids().collect::<Vec<_>>(), expected);
}

/// Asserts that reaching a storage that isn't selected panics instead of reaching it.
#[test]
#[should_panic(expected = "regroup can only reach the storages it selected")]
fn unselected() {
    let world = World::builder_for_test().with_group::<(A, B)>().build();
    let mut all_storages = world.all_storages_mut().unwrap();
    let selected = Selected::all(&mut all_storages.storages.get_mut().components_and_custom);

    selected.get(id::<C>());
}
