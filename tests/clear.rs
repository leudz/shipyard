use core::any::type_name;
use shipyard::error;
use shipyard::prelude::*;

#[test]
fn no_pack() {
    let world = World::new();

    let (mut entities, mut u32s) = world.borrow::<(EntitiesMut, &mut u32)>();

    entities.add_entity(&mut u32s, 0);
    entities.add_entity(&mut u32s, 1);
    entities.add_entity(&mut u32s, 2);

    drop((entities, u32s));
    world.borrow::<AllStorages>().clear();

    let (mut entities, mut u32s) = world.borrow::<(EntitiesMut, &mut u32)>();

    assert_eq!(u32s.len(), 0);
    let entity0 = entities.add_entity(&mut u32s, 3);
    let entity1 = entities.add_entity(&mut u32s, 4);
    let entity2 = entities.add_entity(&mut u32s, 5);
    let entity3 = entities.add_entity(&mut u32s, 5);

    assert_eq!(
        "EntityId { index: 0, version: 1 }",
        format!("{:?}", entity0)
    );
    assert_eq!(
        "EntityId { index: 1, version: 1 }",
        format!("{:?}", entity1)
    );
    assert_eq!(
        "EntityId { index: 2, version: 1 }",
        format!("{:?}", entity2)
    );
    assert_eq!(
        "EntityId { index: 3, version: 0 }",
        format!("{:?}", entity3)
    );
}

#[test]
fn tight() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();

    (&mut usizes, &mut u32s).tight_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    drop((entities, usizes, u32s));

    let mut all_storages = world.borrow::<AllStorages>();
    all_storages.clear();
    drop(all_storages);

    let (usizes, u32s) = world.borrow::<(&usize, &u32)>();

    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(
        (&u32s).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<u32>(),
        })
    );
    assert_eq!(
        usizes.get(entity2),
        Err(error::MissingComponent {
            id: entity2,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(
        u32s.get(entity2),
        Err(error::MissingComponent {
            id: entity2,
            name: type_name::<u32>(),
        })
    );
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), None);
    assert_eq!(usizes.len(), 0);
    assert_eq!(u32s.len(), 0);
}

#[test]
fn delete_loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();

    (&mut usizes, &mut u32s).loose_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));

    drop((entities, usizes, u32s));
    world.run::<AllStorages, _, _>(|mut all_storages| {
        all_storages.clear();
    });

    world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
        assert_eq!(
            (&usizes).get(entity1),
            Err(error::MissingComponent {
                id: entity1,
                name: type_name::<usize>(),
            })
        );
        assert_eq!(
            (&u32s).get(entity1),
            Err(error::MissingComponent {
                id: entity1,
                name: type_name::<u32>(),
            })
        );
        assert_eq!(
            usizes.get(entity2),
            Err(error::MissingComponent {
                id: entity2,
                name: type_name::<usize>(),
            })
        );
        assert_eq!(
            u32s.get(entity2),
            Err(error::MissingComponent {
                id: entity2,
                name: type_name::<u32>(),
            })
        );
        let mut iter = (&usizes, &u32s).iter();
        assert_eq!(iter.next(), None);
        assert_eq!(usizes.len(), 0);
    });
}

#[test]
fn delete_tight_loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u64s, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u64, &mut u32)>();

    (&mut usizes, &mut u64s).tight_pack();
    LoosePack::<(u32,)>::loose_pack((&mut u32s, &mut usizes, &mut u64s));
    let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
    let entity2 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (3, 4, 5));

    drop((entities, usizes, u64s, u32s));
    world.run::<AllStorages, _, _>(|mut all_storages| {
        all_storages.clear();
    });

    world.run::<(&usize, &u64, &u32), _, _>(|(usizes, u64s, u32s)| {
        assert_eq!(
            (&usizes).get(entity1),
            Err(error::MissingComponent {
                id: entity1,
                name: type_name::<usize>(),
            })
        );
        assert_eq!(
            (&u64s).get(entity1),
            Err(error::MissingComponent {
                id: entity1,
                name: type_name::<u64>(),
            })
        );
        assert_eq!(
            (&u32s).get(entity1),
            Err(error::MissingComponent {
                id: entity1,
                name: type_name::<u32>(),
            })
        );
        assert_eq!(
            usizes.get(entity2),
            Err(error::MissingComponent {
                id: entity2,
                name: type_name::<usize>(),
            })
        );
        assert_eq!(
            u64s.get(entity2),
            Err(error::MissingComponent {
                id: entity2,
                name: type_name::<u64>(),
            })
        );
        assert_eq!(
            u32s.get(entity2),
            Err(error::MissingComponent {
                id: entity2,
                name: type_name::<u32>(),
            })
        );
        let mut tight_iter = (&usizes, &u64s).iter();
        assert_eq!(tight_iter.next(), None);
        let mut loose_iter = (&usizes, &u64s, &u32s).iter();
        assert_eq!(loose_iter.next(), None);
        assert_eq!(usizes.len(), 0);
        assert_eq!(u64s.len(), 0);
        assert_eq!(u32s.len(), 0);
    });
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();

    usizes.update_pack();
    let entity1 = entities.add_entity(&mut usizes, 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    drop((entities, usizes));

    let mut all_storages = world.borrow::<AllStorages>();
    all_storages.clear();
    drop(all_storages);

    let mut usizes = world.borrow::<&mut usize>();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(
        usizes.get(entity2),
        Err(error::MissingComponent {
            id: entity2,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(usizes.deleted().len(), 2);
    assert_eq!(usizes.take_deleted(), vec![(entity1, 0), (entity2, 2)]);
    assert_eq!(usizes.len(), 0);
}
