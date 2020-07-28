use shipyard::error;
use shipyard::*;

#[test]
fn empty_inserted_in_modified() {
    let world = World::new();

    let mut usizes = world.try_borrow::<ViewMut<usize>>().unwrap();
    usizes.try_update_pack().unwrap();
    let modified = usizes.try_modified().unwrap();
    modified.try_inserted().unwrap();
}

#[test]
fn inserted_in_inserted() {
    let world = World::new();

    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.try_update_pack().unwrap();
    entities.add_entity(&mut usizes, 0);
    let inserted = usizes.try_inserted().unwrap();
    inserted.try_inserted().unwrap();
}

#[test]
fn inserted_in_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.try_update_pack().unwrap();
    entities.add_entity(&mut usizes, 0);
    let modified = usizes.try_modified().unwrap();
    assert_eq!(
        modified.try_inserted().err(),
        Some(error::UpdateWindow::OutOfBounds)
    );
}

#[test]
fn inserted_in_inserted_or_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.try_update_pack().unwrap();
    entities.add_entity(&mut usizes, 0);
    let inserted_or_modified = usizes.try_inserted_or_modified().unwrap();
    inserted_or_modified.try_inserted().unwrap();
}

#[test]
fn empty_modified_in_inserted() {
    let world = World::new();

    let mut usizes = world.try_borrow::<ViewMut<usize>>().unwrap();
    usizes.try_update_pack().unwrap();
    let inserted = usizes.try_inserted().unwrap();
    inserted.try_modified().unwrap();
}

#[test]
fn modified_in_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.try_update_pack().unwrap();
    entities.add_entity(&mut usizes, 0);
    usizes.try_clear_inserted().unwrap();
    (&mut usizes).iter().for_each(|_| {});
    let modified = usizes.try_modified().unwrap();
    modified.try_modified().unwrap();
}

#[test]
fn modified_in_inserted() {
    let world = World::new();

    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.try_update_pack().unwrap();
    entities.add_entity(&mut usizes, 0);
    usizes.try_clear_inserted().unwrap();
    (&mut usizes).iter().for_each(|_| {});
    let inserted = usizes.try_inserted().unwrap();
    assert_eq!(
        inserted.try_modified().err(),
        Some(error::UpdateWindow::OutOfBounds)
    );
}

#[test]
fn modified_in_inserted_or_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.try_update_pack().unwrap();
    entities.add_entity(&mut usizes, 0);
    usizes.try_clear_inserted().unwrap();
    (&mut usizes).iter().for_each(|_| {});
    let inserted_or_modified = usizes.try_inserted_or_modified().unwrap();
    inserted_or_modified.try_modified().unwrap();
}

#[test]
fn empty_inserted_or_modified_in_inserted() {
    let world = World::new();

    let mut usizes = world.try_borrow::<ViewMut<usize>>().unwrap();
    usizes.try_update_pack().unwrap();
    let inserted = usizes.try_inserted().unwrap();
    inserted.try_inserted_or_modified().unwrap();
}

#[test]
fn empty_inserted_or_modified_in_modified() {
    let world = World::new();

    let mut usizes = world.try_borrow::<ViewMut<usize>>().unwrap();
    usizes.try_update_pack().unwrap();
    let modified = usizes.try_modified().unwrap();
    modified.try_inserted_or_modified().unwrap();
}

#[test]
fn inserted_or_modified_in_inserted_or_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.try_update_pack().unwrap();
    entities.add_entity(&mut usizes, 0);
    let inserted_or_modified = usizes.try_inserted_or_modified().unwrap();
    inserted_or_modified.try_inserted_or_modified().unwrap();

    usizes.try_clear_inserted().unwrap();
    (&mut usizes).iter().for_each(|_| {});
    let inserted_or_modified = usizes.try_inserted_or_modified().unwrap();
    inserted_or_modified.try_inserted_or_modified().unwrap();
}

#[test]
fn inserted_or_modified_in_inserted() {
    let world = World::new();

    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.try_update_pack().unwrap();
    entities.add_entity(&mut usizes, 0);
    usizes.try_clear_inserted().unwrap();
    (&mut usizes).iter().for_each(|_| {});
    let inserted = usizes.try_inserted().unwrap();
    assert_eq!(
        inserted.try_inserted_or_modified().err(),
        Some(error::UpdateWindow::OutOfBounds)
    );
}

#[test]
fn inserted_or_modified_in_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.try_update_pack().unwrap();
    entities.add_entity(&mut usizes, 0);
    let modified = usizes.try_modified().unwrap();
    assert_eq!(
        modified.try_inserted_or_modified().err(),
        Some(error::UpdateWindow::OutOfBounds)
    );
}

#[test]
fn simple_window() {
    let world = World::new();

    world
        .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let entity0 = entities.add_entity(&mut u32s, 0);
            let entity1 = entities.add_entity(&mut u32s, 1);

            let window = u32s.try_as_window(0..2).unwrap();
            assert_eq!(window[entity0], 0);
            assert_eq!(window[entity1], 1);
            let window = u32s.try_as_window(1..2).unwrap();
            assert!(window.get(entity0).is_err());
            assert_eq!(window[entity1], 1);

            let window = u32s.try_as_window(0..=1).unwrap();
            assert_eq!(window[entity0], 0);
            assert_eq!(window[entity1], 1);

            assert!(u32s.try_as_window(2..2).is_err());
            assert!(u32s.try_as_window(..3).is_err());
            assert!(u32s.try_as_window(..=2).is_err());
        })
        .unwrap();
}

#[test]
fn simple_window_mut() {
    let world = World::new();

    world
        .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let entity0 = entities.add_entity(&mut u32s, 0);
            let entity1 = entities.add_entity(&mut u32s, 1);

            let window = u32s.try_as_window_mut(0..2).unwrap();
            assert_eq!(window[entity0], 0);
            assert_eq!(window[entity1], 1);
            let window = u32s.try_as_window(1..2).unwrap();
            assert!(window.get(entity0).is_err());
            assert_eq!(window[entity1], 1);

            let window = u32s.try_as_window_mut(0..=1).unwrap();
            assert_eq!(window[entity0], 0);
            assert_eq!(window[entity1], 1);

            assert!(u32s.try_as_window_mut(2..2).is_err());
            assert!(u32s.try_as_window_mut(..3).is_err());
            assert!(u32s.try_as_window_mut(..=2).is_err());
        })
        .unwrap();
}
