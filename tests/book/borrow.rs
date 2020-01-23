use super::*;

#[test]
fn all() {
    let world = World::new();

    let mut _all_storages = world.borrow::<AllStorages>();
}

#[test]
fn multiple() {
    let world = World::new();

    let (mut _entities, mut _empties) = world.borrow::<(EntitiesMut, &mut Empty)>();
}
