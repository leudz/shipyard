use shipyard::prelude::*;

#[test]
fn empty_inserted_in_modified() {
    let world = World::new();

    let mut usizes = world.borrow::<&mut usize>();
    usizes.update_pack();
    let modified = usizes.modified();
    modified.inserted();
}

#[test]
fn inserted_in_inserted() {
    let world = World::new();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    usizes.update_pack();
    entities.add_entity(&mut usizes, 0);
    let inserted = usizes.inserted();
    inserted.inserted();
}

#[test]
#[should_panic(
    expected = "called `Result::unwrap()` on an `Err` value: This window doesn't contain the inserted components."
)]
fn inserted_in_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    usizes.update_pack();
    entities.add_entity(&mut usizes, 0);
    let modified = usizes.modified();
    modified.inserted();
}

#[test]
fn inserted_in_inserted_or_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    usizes.update_pack();
    entities.add_entity(&mut usizes, 0);
    let inserted_or_modified = usizes.inserted_or_modified();
    inserted_or_modified.inserted();
}

#[test]
fn empty_modified_in_inserted() {
    let world = World::new();

    let mut usizes = world.borrow::<&mut usize>();
    usizes.update_pack();
    let inserted = usizes.inserted();
    inserted.modified();
}

#[test]
fn modified_in_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    usizes.update_pack();
    entities.add_entity(&mut usizes, 0);
    usizes.clear_inserted();
    (&mut usizes).iter().for_each(|_| {});
    let modified = usizes.modified();
    modified.modified();
}

#[test]
#[should_panic(
    expected = "called `Result::unwrap()` on an `Err` value: This window doesn't contain the modified components."
)]
fn modified_in_inserted() {
    let world = World::new();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    usizes.update_pack();
    entities.add_entity(&mut usizes, 0);
    usizes.clear_inserted();
    (&mut usizes).iter().for_each(|_| {});
    let inserted = usizes.inserted();
    inserted.modified();
}

#[test]
fn modified_in_inserted_or_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    usizes.update_pack();
    entities.add_entity(&mut usizes, 0);
    usizes.clear_inserted();
    (&mut usizes).iter().for_each(|_| {});
    let inserted_or_modified = usizes.inserted_or_modified();
    inserted_or_modified.modified();
}

#[test]
fn empty_inserted_or_modified_in_inserted() {
    let world = World::new();

    let mut usizes = world.borrow::<&mut usize>();
    usizes.update_pack();
    let inserted = usizes.inserted();
    inserted.inserted_or_modified();
}

#[test]
fn empty_inserted_or_modified_in_modified() {
    let world = World::new();

    let mut usizes = world.borrow::<&mut usize>();
    usizes.update_pack();
    let modified = usizes.modified();
    modified.inserted_or_modified();
}

#[test]
fn inserted_or_modified_in_inserted_or_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    usizes.update_pack();
    entities.add_entity(&mut usizes, 0);
    let inserted_or_modified = usizes.inserted_or_modified();
    inserted_or_modified.inserted_or_modified();

    usizes.clear_inserted();
    (&mut usizes).iter().for_each(|_| {});
    let inserted_or_modified = usizes.inserted_or_modified();
    inserted_or_modified.inserted_or_modified();
}

#[test]
#[should_panic(
    expected = "called `Result::unwrap()` on an `Err` value: This window doesn't contain the inserted or modified components."
)]
fn inserted_or_modified_in_inserted() {
    let world = World::new();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    usizes.update_pack();
    entities.add_entity(&mut usizes, 0);
    usizes.clear_inserted();
    (&mut usizes).iter().for_each(|_| {});
    let inserted = usizes.inserted();
    inserted.inserted_or_modified();
}

#[test]
#[should_panic(
    expected = "called `Result::unwrap()` on an `Err` value: This window doesn't contain the inserted or modified components."
)]
fn inserted_or_modified_in_modified() {
    let world = World::new();

    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();
    usizes.update_pack();
    entities.add_entity(&mut usizes, 0);
    let modified = usizes.modified();
    modified.inserted_or_modified();
}
