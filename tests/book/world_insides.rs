use shipyard::*;

#[test]
fn test1() {
    let world = World::new();

    world.run(
        |mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>, mut f32s: ViewMut<f32>| {
            let _entity0 = entities.add_entity(&mut u32s, 10);
            let _entity1 = entities.add_entity(&mut f32s, 20.0);
            let _entity2 = entities.add_entity(&mut u32s, 30);
        },
    );
}

#[test]
fn test2() {
    let world = World::new();

    let [entity0, entity1] = world.run(
        |mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>, mut f32s: ViewMut<f32>| {
            let result = [
                entities.add_entity(&mut u32s, 10),
                entities.add_entity(&mut f32s, 20.0),
            ];
            let _entity2 = entities.add_entity(&mut u32s, 30);

            result
        },
    );

    world.run(|mut all_storages: AllStoragesViewMut| {
        all_storages.delete_entity(entity0);
        all_storages.delete_entity(entity1);
    });
}
