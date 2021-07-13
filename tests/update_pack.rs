use shipyard::*;

#[test]
fn clear_inserted() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>().unwrap();
    usizes.track_all();

    let e0 = entities.add_entity(&mut usizes, 0);

    usizes.clear_all_inserted();

    usizes[e0] += 1;

    let e1 = entities.add_entity(&mut usizes, 1);
    let e2 = entities.add_entity(&mut usizes, 2);

    usizes.clear_inserted(e0);
    usizes.clear_inserted(e1);

    assert_eq!(usizes.is_inserted(e0), false);
    assert_eq!(usizes.is_modified(e0), true);
    assert_eq!(usizes.is_inserted(e1), false);
    assert_eq!(usizes.is_modified(e1), false);
    assert_eq!(usizes.is_inserted(e2), true);
    assert_eq!(usizes.is_modified(e2), false);
}

#[test]
fn clear_modified() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>().unwrap();
    usizes.track_all();

    let e0 = entities.add_entity(&mut usizes, 0);
    let e1 = entities.add_entity(&mut usizes, 1);

    usizes.clear_all_inserted();

    usizes[e0] += 1;
    usizes[e1] += 1;

    let e2 = entities.add_entity(&mut usizes, 2);

    usizes.clear_modified(e0);
    usizes.clear_modified(e2);

    assert_eq!(usizes.is_inserted(e0), false);
    assert_eq!(usizes.is_modified(e0), false);
    assert_eq!(usizes.is_inserted(e1), false);
    assert_eq!(usizes.is_modified(e1), true);
    assert_eq!(usizes.is_inserted(e2), true);
    assert_eq!(usizes.is_modified(e2), false);
}

#[test]
fn clear_inserted_and_modified() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>().unwrap();
    usizes.track_all();

    let e0 = entities.add_entity(&mut usizes, 0);

    usizes.clear_all_inserted();

    usizes[e0] += 1;

    let e1 = entities.add_entity(&mut usizes, 1);

    usizes.clear_inserted_and_modified(e0);
    usizes.clear_inserted_and_modified(e1);

    assert_eq!(usizes.is_inserted(e0), false);
    assert_eq!(usizes.is_modified(e0), false);
    assert_eq!(usizes.is_inserted(e1), false);
    assert_eq!(usizes.is_modified(e1), false);
}
