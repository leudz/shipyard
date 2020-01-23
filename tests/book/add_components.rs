use super::*;

#[test]
fn single() {
    let world = World::new();

    let entity_id = world.run::<EntitiesMut, _, _>(|mut entities| entities.add_entity((), ()));

    let (entities, mut positions) = world.borrow::<(Entities, &mut Position)>();

    entities.add_component(&mut positions, Position { x: 0.0, y: 10.0 }, entity_id);
}

#[test]
fn multiple() {
    let world = World::new();

    let entity_id = world.run::<EntitiesMut, _, _>(|mut entities| entities.add_entity((), ()));

    let (entities, mut positions, mut fruits) =
        world.borrow::<(Entities, &mut Position, &mut Fruit)>();

    entities.add_component(
        (&mut positions, &mut fruits),
        (Position { x: 0.0, y: 10.0 }, Fruit::new_orange()),
        entity_id,
    );
}
