use super::*;

#[test]
fn single() {
    let world = World::new();

    let (mut entities, mut empties) = world.borrow::<(EntitiesMut, &mut Empty)>();

    let _entity = entities.add_entity(&mut empties, Empty);
}

#[test]
fn multiple() {
    let world = World::new();

    let (mut entities, mut empties, mut counts) =
        world.borrow::<(EntitiesMut, &mut Empty, &mut Count)>();

    let _entity = entities.add_entity((&mut empties, &mut counts), (Empty, Count(0)));
}

#[test]
fn none() {
    let world = World::new();

    let _entity = world.borrow::<EntitiesMut>().add_entity((), ());
}
