use shipyard::*;

#[test]
fn get() {
    let world = World::new();
    world
        .try_run(|mut all_storages: AllStoragesViewMut| {
            let (owned, shared) = all_storages
                .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
                    let owned = entities.add_entity(&mut u32s, 0);
                    let shared = entities.add_entity((), ());
                    u32s.try_share(owned, shared).unwrap();

                    assert_eq!(u32s.get(owned), Ok(&0));
                    assert_eq!(u32s.get(shared), Ok(&0));

                    assert_eq!(u32s.remove(shared), Some(OldComponent::Shared));
                    assert_eq!(u32s.get(owned), Ok(&0));
                    assert!(u32s.get(shared).is_err());

                    u32s.try_share(owned, shared).unwrap();
                    u32s.try_unshare(shared).unwrap();
                    assert_eq!(u32s.get(owned), Ok(&0));
                    assert!(u32s.get(shared).is_err());

                    u32s.try_share(owned, shared).unwrap();
                    (owned, shared)
                })
                .unwrap();

            all_storages.delete(owned);

            all_storages
                .try_run(|u32s: View<u32>| {
                    assert!(u32s.get(shared).is_err());
                })
                .unwrap();
        })
        .unwrap()
}

#[test]
fn get_mut() {
    let world = World::new();
    world
        .try_run(|mut all_storages: AllStoragesViewMut| {
            let (owned, shared) = all_storages
                .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
                    let owned = entities.add_entity(&mut u32s, 0);
                    let shared = entities.add_entity((), ());
                    u32s.try_share(owned, shared).unwrap();

                    assert_eq!(*(&mut u32s).get(owned).unwrap(), 0);
                    assert_eq!(*(&mut u32s).get(shared).unwrap(), 0);

                    assert_eq!(u32s.remove(shared), Some(OldComponent::Shared));
                    assert_eq!(*(&mut u32s).get(owned).unwrap(), 0);
                    assert!((&mut u32s).get(shared).is_err());

                    assert_eq!(u32s.try_unshare(shared).err(), Some(error::Unshare));
                    assert_eq!(*(&mut u32s).get(owned).unwrap(), 0);
                    assert!((&mut u32s).get(shared).is_err());

                    u32s.try_share(owned, shared).unwrap();
                    (owned, shared)
                })
                .unwrap();

            all_storages.delete(owned);

            all_storages
                .try_run(|mut u32s: ViewMut<u32>| {
                    assert!((&mut u32s).get(shared).is_err());
                })
                .unwrap();
        })
        .unwrap()
}

#[test]
fn iter() {
    let world = World::new();
    world
        .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let owned = entities.add_entity(&mut u32s, 0);
            let shared = entities.add_entity((), ());
            u32s.try_share(owned, shared).unwrap();

            let mut iter = u32s.iter();
            assert_eq!(iter.next(), Some(&0));
            assert_eq!(iter.next(), None);
        })
        .unwrap();
}

#[test]
fn double_shared() {
    let world = World::new();
    world
        .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let owned1 = entities.add_entity(&mut u32s, 1);
            let owned2 = entities.add_entity(&mut u32s, 2);
            let shared1 = entities.add_entity((), ());
            let shared2 = entities.add_entity((), ());
            u32s.try_share(owned1, shared1).unwrap();
            u32s.try_share(shared1, shared2).unwrap();

            assert_eq!(u32s.get(owned1), Ok(&1));
            assert_eq!(u32s.get(owned2), Ok(&2));
            assert_eq!(u32s.get(shared1), Ok(&1));
            assert_eq!(u32s.get(shared2), Ok(&1));

            u32s.try_unshare(shared1).unwrap();
            assert_eq!(u32s.get(owned1), Ok(&1));
            assert_eq!(u32s.get(owned2), Ok(&2));
            assert!(u32s.get(shared1).is_err());
            assert!(u32s.get(shared2).is_err());

            u32s.try_share(owned2, shared1).unwrap();
            assert_eq!(u32s.get(owned1), Ok(&1));
            assert_eq!(u32s.get(owned2), Ok(&2));
            assert_eq!(u32s.get(shared1), Ok(&2));
            assert_eq!(u32s.get(shared2), Ok(&2));
        })
        .unwrap();
}

#[test]
fn shared_override() {
    let world = World::new();

    world
        .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let owned = entities.add_entity(&mut u32s, 0);
            let shared = entities.add_entity((), ());

            u32s.try_share(owned, shared).unwrap();

            u32s.remove(owned);
            entities.try_add_component(shared, &mut u32s, 1).unwrap();

            u32s.get(shared).unwrap();
            u32s.remove(shared);
            assert!(u32s.get(shared).is_err());
        })
        .unwrap();
}

#[test]
fn self_shared() {
    let world = World::new();

    world
        .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let shared = entities.add_entity((), ());

            u32s.try_share(shared, shared).unwrap();

            assert!(u32s.get(shared).is_err());
        })
        .unwrap();
}

#[test]
fn share_all() {
    let world = World::new();

    let mut all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();
    let (mut entities, mut u32s, mut usizes) = all_storages
        .try_borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<usize>)>()
        .unwrap();

    let e0 = entities.add_entity(&mut u32s, 0);
    let e1 = entities.add_entity(&mut usizes, 1);

    drop((entities, u32s, usizes));

    all_storages.share(e0, e1);

    let (u32s, usizes) = all_storages
        .try_borrow::<(View<u32>, View<usize>)>()
        .unwrap();

    assert_eq!(u32s.fast_get(e1), Ok(&0));
    assert_eq!(usizes.fast_get(e1), Ok(&1));
}
