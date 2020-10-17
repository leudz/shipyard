use shipyard::*;

#[test]
fn single() {
    let entity_id = EntityId::dead();
    let world = World::new();

    world.run(|mut u32s: ViewMut<u32>| {
        let _i = u32s.remove(entity_id);
    });
}

#[test]
fn multiple() {
    let entity_id = EntityId::dead();
    let world = World::new();

    world.run(|mut u32s: ViewMut<u32>, mut usizes: ViewMut<usize>| {
        let (_i, _j) = (&mut u32s, &mut usizes).remove(entity_id);
    });
}
