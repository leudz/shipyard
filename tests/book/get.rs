use super::*;

#[test]
fn single() {
    let world = World::new();

    let entity_id =
        world.run::<(EntitiesMut, &mut Position), _, _>(|(mut entities, mut positions)| {
            entities.add_entity(&mut positions, Position { x: 0.0, y: 0.0 })
        });

    let mut positions = world.borrow::<&mut Position>();

    *(&mut positions).get(entity_id).unwrap() = Position { x: 5.0, y: 6.0 };
}

#[test]
fn index() {
    let world = World::new();

    let entity_id =
        world.run::<(EntitiesMut, &mut Position), _, _>(|(mut entities, mut positions)| {
            entities.add_entity(&mut positions, Position { x: 0.0, y: 0.0 })
        });

    let mut positions = world.borrow::<&mut Position>();

    positions[entity_id] = Position { x: 5.0, y: 6.0 };
}

#[test]
fn multiple() {
    let world = World::new();

    let entity_id =
        world.run::<(EntitiesMut, &mut Position), _, _>(|(mut entities, mut positions)| {
            entities.add_entity(&mut positions, Position { x: 0.0, y: 0.0 })
        });

    let (mut positions, velocities) = world.borrow::<(&mut Position, &Velocity)>();

    if let Some((pos, vel)) = (&mut positions, &velocities).get(entity_id) {
        pos.x += vel.x;
        pos.y += vel.y;
    }
}
