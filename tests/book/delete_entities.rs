use super::*;

#[test]
fn test() {
    let entity_id = EntityId::dead();
    let world = World::new();

    world.borrow::<AllStorages>().delete(entity_id);
}
