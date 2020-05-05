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
        Some(error::Inserted::NotInbound)
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
        Some(error::Modified::NotInbound)
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
        Some(error::InsertedOrModified::NotInbound)
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
        Some(error::InsertedOrModified::NotInbound)
    );
}
