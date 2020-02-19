use shipyard::prelude::*;

#[test]
fn tight() {
    let world = World::new();

    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();
    (&mut usizes, &mut u32s).tight_pack();

    let _entity0 = entities.add_entity(&mut usizes, 0);
    let _entity1 = entities.add_entity((&mut usizes, &mut u32s), (1, 11));
}

#[test]
fn loose() {
    let world = World::new();

    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();
    LoosePack::<(usize,)>::loose_pack((&mut usizes, &mut u32s));

    let _entity0 = entities.add_entity(&mut usizes, 0);
    let _entity1 = entities.add_entity((&mut usizes, &mut u32s), (1, 11));
}
