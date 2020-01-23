use super::*;

#[test]
fn single() {
    let entity_id = EntityId::dead();
    let world = World::new();

    let mut counts = world.borrow::<&mut Count>();

    let _count = counts.remove(entity_id);
}

#[test]
fn multiple() {
    let entity_id = EntityId::dead();
    let world = World::new();

    let (mut counts, mut empties) = world.borrow::<(&mut Count, &mut Empty)>();

    let (_count, _empty) = Remove::<(Count, Empty)>::remove((&mut counts, &mut empties), entity_id);
}
