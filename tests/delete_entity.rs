use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    drop((entities, usizes, u32s));

    let mut all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();
    assert!(all_storages.delete(entity1));
    assert!(!all_storages.delete(entity1));
    drop(all_storages);

    let (usizes, u32s) = world.try_borrow::<(View<usize>, View<u32>)>().unwrap();
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
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(u32s.get(entity2), Ok(&3));
}

#[test]
fn tight() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_tight_pack().unwrap();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    drop((entities, usizes, u32s));

    let mut all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();
    assert!(all_storages.delete(entity1));
    assert!(!all_storages.delete(entity1));
    drop(all_storages);

    let (usizes, u32s) = world.try_borrow::<(View<usize>, View<u32>)>().unwrap();

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
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(u32s.get(entity2), Ok(&3));
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), Some((&2, &3)));
    assert_eq!(iter.next(), None);
}

#[test]
fn delete_loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_loose_pack().unwrap();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));

    drop((entities, usizes, u32s));
    world
        .try_run(|mut all_storages: AllStoragesViewMut| {
            assert!(all_storages.delete(entity1));
            assert!(!all_storages.delete(entity1));
        })
        .unwrap();

    world
        .try_run(|(usizes, u32s): (View<usize>, View<u32>)| {
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
            assert_eq!(usizes.get(entity2), Ok(&2));
            assert_eq!(u32s.get(entity2), Ok(&3));
            let mut iter = (&usizes, &u32s).iter();
            assert_eq!(iter.next(), Some((&2, &3)));
            assert_eq!(iter.next(), None);
        })
        .unwrap();
}

#[test]
fn delete_tight_loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u64s, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u64>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u64s).try_tight_pack().unwrap();
    LoosePack::<(u32,)>::try_loose_pack((&mut u32s, &mut usizes, &mut u64s)).unwrap();
    let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
    let entity2 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (3, 4, 5));

    drop((entities, usizes, u64s, u32s));
    world
        .try_run(|mut all_storages: AllStoragesViewMut| {
            assert!(all_storages.delete(entity1));
            assert!(!all_storages.delete(entity1));
        })
        .unwrap();

    world
        .try_run(
            |(usizes, u64s, u32s): (View<usize>, View<u64>, View<u32>)| {
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
                assert_eq!(usizes.get(entity2), Ok(&3));
                assert_eq!(u64s.get(entity2), Ok(&4));
                assert_eq!(u32s.get(entity2), Ok(&5));
                let mut tight_iter = (&usizes, &u64s).iter();
                assert_eq!(tight_iter.next(), Some((&3, &4)));
                assert_eq!(tight_iter.next(), None);
                let mut loose_iter = (&usizes, &u64s, &u32s).iter();
                assert_eq!(loose_iter.next(), Some((&3, &4, &5)));
                assert_eq!(loose_iter.next(), None);
            },
        )
        .unwrap();
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();

    usizes.try_update_pack().unwrap();
    let entity1 = entities.add_entity(&mut usizes, 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    drop((entities, usizes));

    let mut all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();
    assert!(all_storages.delete(entity1));
    assert!(!all_storages.delete(entity1));
    drop(all_storages);

    let mut usizes = world.try_borrow::<ViewMut<usize>>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(usizes.try_deleted().unwrap().len(), 1);
    assert_eq!(usizes.try_take_deleted().unwrap(), vec![(entity1, 0)]);
}
