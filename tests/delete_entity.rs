use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>();

    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    drop((entities, usizes, u32s));

    let mut all_storages = world.borrow::<AllStoragesViewMut>();
    assert!(all_storages.delete(entity1));
    assert!(!all_storages.delete(entity1));
    drop(all_storages);

    let (usizes, u32s) = world.borrow::<(View<usize>, View<u32>)>();
    assert_eq!(
        (&usizes).try_get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(
        (&u32s).try_get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<u32>(),
        })
    );
    assert_eq!(usizes.try_get(entity2), Ok(&2));
    assert_eq!(u32s.try_get(entity2), Ok(&3));
}

#[test]
fn tight() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>();

    (&mut usizes, &mut u32s).tight_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    drop((entities, usizes, u32s));

    let mut all_storages = world.borrow::<AllStoragesViewMut>();
    assert!(all_storages.delete(entity1));
    assert!(!all_storages.delete(entity1));
    drop(all_storages);

    let (usizes, u32s) = world.borrow::<(View<usize>, View<u32>)>();

    assert_eq!(
        (&usizes).try_get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(
        (&u32s).try_get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<u32>(),
        })
    );
    assert_eq!(usizes.try_get(entity2), Ok(&2));
    assert_eq!(u32s.try_get(entity2), Ok(&3));
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), Some((&2, &3)));
    assert_eq!(iter.next(), None);
}

#[test]
fn delete_loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>();

    (&mut usizes, &mut u32s).loose_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));

    drop((entities, usizes, u32s));
    world.run(|mut all_storages: AllStoragesViewMut| {
        assert!(all_storages.delete(entity1));
        assert!(!all_storages.delete(entity1));
    });

    world.run(|(usizes, u32s): (View<usize>, View<u32>)| {
        assert_eq!(
            (&usizes).try_get(entity1),
            Err(error::MissingComponent {
                id: entity1,
                name: type_name::<usize>(),
            })
        );
        assert_eq!(
            (&u32s).try_get(entity1),
            Err(error::MissingComponent {
                id: entity1,
                name: type_name::<u32>(),
            })
        );
        assert_eq!(usizes.try_get(entity2), Ok(&2));
        assert_eq!(u32s.try_get(entity2), Ok(&3));
        let mut iter = (&usizes, &u32s).iter();
        assert_eq!(iter.next(), Some((&2, &3)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn delete_tight_loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u64s, mut u32s) =
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u64>, ViewMut<u32>)>();

    (&mut usizes, &mut u64s).tight_pack();
    LoosePack::<(u32,)>::loose_pack((&mut u32s, &mut usizes, &mut u64s));
    let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
    let entity2 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (3, 4, 5));

    drop((entities, usizes, u64s, u32s));
    world.run(|mut all_storages: AllStoragesViewMut| {
        assert!(all_storages.delete(entity1));
        assert!(!all_storages.delete(entity1));
    });

    world.run(
        |(usizes, u64s, u32s): (View<usize>, View<u64>, View<u32>)| {
            assert_eq!(
                (&usizes).try_get(entity1),
                Err(error::MissingComponent {
                    id: entity1,
                    name: type_name::<usize>(),
                })
            );
            assert_eq!(
                (&u64s).try_get(entity1),
                Err(error::MissingComponent {
                    id: entity1,
                    name: type_name::<u64>(),
                })
            );
            assert_eq!(
                (&u32s).try_get(entity1),
                Err(error::MissingComponent {
                    id: entity1,
                    name: type_name::<u32>(),
                })
            );
            assert_eq!(usizes.try_get(entity2), Ok(&3));
            assert_eq!(u64s.try_get(entity2), Ok(&4));
            assert_eq!(u32s.try_get(entity2), Ok(&5));
            let mut tight_iter = (&usizes, &u64s).iter();
            assert_eq!(tight_iter.next(), Some((&3, &4)));
            assert_eq!(tight_iter.next(), None);
            let mut loose_iter = (&usizes, &u64s, &u32s).iter();
            assert_eq!(loose_iter.next(), Some((&3, &4, &5)));
            assert_eq!(loose_iter.next(), None);
        },
    );
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>();

    usizes.update_pack();
    let entity1 = entities.add_entity(&mut usizes, 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    drop((entities, usizes));

    let mut all_storages = world.borrow::<AllStoragesViewMut>();
    assert!(all_storages.delete(entity1));
    assert!(!all_storages.delete(entity1));
    drop(all_storages);

    let mut usizes = world.borrow::<ViewMut<usize>>();
    assert_eq!(
        (&usizes).try_get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(usizes.try_get(entity2), Ok(&2));
    assert_eq!(usizes.deleted().len(), 1);
    assert_eq!(usizes.take_deleted(), vec![(entity1, 0)]);
}
