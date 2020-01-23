use super::*;

#[test]
fn single() {
    let entity_id = EntityId::dead();
    let world = World::new();

    let mut counts = world.borrow::<&mut Count>();

    counts.delete(entity_id);
}

#[test]
fn multiple() {
    let entity_id = EntityId::dead();
    let world = World::new();

    let (mut counts, mut empties) = world.borrow::<(&mut Count, &mut Empty)>();

    Delete::<(Count, Empty)>::delete((&mut counts, &mut empties), entity_id);
}

#[test]
fn strip() {
    let entity_id = EntityId::dead();
    let world = World::new();

    let mut all_storages = world.borrow::<AllStorages>();

    all_storages.strip(entity_id);
}
