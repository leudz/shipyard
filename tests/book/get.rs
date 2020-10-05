use shipyard::*;

#[test]
fn single() {
    let world = World::new();

    let entity_id = world.run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
        entities.add_entity(&mut u32s, 0)
    });

    world.run(|mut u32s: ViewMut<u32>| {
        *(&mut u32s).get(entity_id).unwrap() = 1;
    });
}

#[test]
fn index() {
    let world = World::new();

    let entity_id = world.run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
        entities.add_entity(&mut u32s, 0)
    });

    world.run(|mut u32s: ViewMut<u32>| {
        u32s[entity_id] = 1;
    });
}

#[test]
fn multiple() {
    let world = World::new();

    let entity_id = world.run(
        |mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>, mut usizes: ViewMut<usize>| {
            entities.add_entity((&mut u32s, &mut usizes), (0, 1))
        },
    );

    world.run(|mut u32s: ViewMut<u32>, usizes: View<usize>| {
        let (mut i, &j) = (&mut u32s, &usizes).get(entity_id).unwrap();
        *i += j as u32;
        *i += j as u32;
    });
}
