use shipyard::*;

#[test]
fn test1() {
    let world = World::new();
    let (mut entities, mut u32s, mut f32s) = world.borrow::<(EntitiesMut, &mut u32, &mut f32)>();
    let _entity0 = entities.add_entity(&mut u32s, 10);
    let _entity1 = entities.add_entity(&mut f32s, 20.0);
    let _entity2 = entities.add_entity(&mut u32s, 30);
}

#[test]
fn test2() {
    let world = World::new();
    let entity0;
    let entity1;
    {
        let (mut entities, mut u32s, mut f32s) =
            world.borrow::<(EntitiesMut, &mut u32, &mut f32)>();
        entity0 = entities.add_entity(&mut u32s, 10);
        entity1 = entities.add_entity(&mut f32s, 20.0);
        let _entity2 = entities.add_entity(&mut u32s, 30);
    }
    let mut all_storages = world.borrow::<AllStorages>();
    all_storages.delete(entity0);
    all_storages.delete(entity1);
}
