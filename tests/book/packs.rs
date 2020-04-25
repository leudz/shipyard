use shipyard::*;

#[test]
fn tight() {
    let world = World::new();

    world.run(
        |mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>, mut usizes: ViewMut<usize>| {
            (&mut usizes, &mut u32s).tight_pack();

            let _entity0 = entities.add_entity(&mut usizes, 0);
            let _entity1 = entities.add_entity((&mut usizes, &mut u32s), (1, 11));
        },
    );
}

#[test]
fn loose() {
    let world = World::new();

    world.run(
        |mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>, mut usizes: ViewMut<usize>| {
            // usize's storage will be modified
            LoosePack::<(usize,)>::loose_pack((&mut usizes, &mut u32s));

            let _entity0 = entities.add_entity(&mut usizes, 0);
            let _entity1 = entities.add_entity((&mut usizes, &mut u32s), (1, 11));
        },
    );
}
