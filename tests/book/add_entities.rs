use shipyard::*;

#[test]
fn single() {
    let world = World::new();

    world
        .run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let _entity = entities.add_entity(&mut u32s, 0);
        })
        .unwrap();
}

#[test]
fn multiple() {
    let world = World::new();

    world
        .run(
            |mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>, mut usize: ViewMut<usize>| {
                let _entity = entities.add_entity((&mut u32s, &mut usize), (0, 10));
            },
        )
        .unwrap();
}

#[test]
fn none() {
    let world = World::new();

    world
        .run(|mut entities: EntitiesViewMut| {
            let _entity = entities.add_entity((), ());
        })
        .unwrap();
}
