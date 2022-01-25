use super::{F32, U32};
use shipyard::*;

#[test]
fn test1() {
    let world = World::new();

    world.run(
        |mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>, mut f32s: ViewMut<F32>| {
            let _entity0 = entities.add_entity(&mut u32s, U32(10));
            let _entity1 = entities.add_entity(&mut f32s, F32(20.0));
            let _entity2 = entities.add_entity(&mut u32s, U32(30));
        },
    );
}

#[test]
fn test2() {
    let world = World::new();

    let [entity0, entity1] = world.run(
        |mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>, mut f32s: ViewMut<F32>| {
            let result = [
                entities.add_entity(&mut u32s, U32(10)),
                entities.add_entity(&mut f32s, F32(20.0)),
            ];
            let _entity2 = entities.add_entity(&mut u32s, U32(30));

            result
        },
    );

    world.run(|mut all_storages: AllStoragesViewMut| {
        all_storages.delete_entity(entity0);
        all_storages.delete_entity(entity1);
    });
}
