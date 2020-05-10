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
                    u32s.share(owned, shared);

                    assert_eq!(u32s.get(owned), Ok(&0));
                    assert_eq!(u32s.get(shared), Ok(&0));

                    assert_eq!(u32s.try_remove(shared).unwrap(), Some(OldComponent::Shared));
                    assert_eq!(u32s.get(owned), Ok(&0));
                    assert!(u32s.get(shared).is_err());

                    u32s.share(owned, shared);
                    u32s.unshare(shared);
                    assert_eq!(u32s.get(owned), Ok(&0));
                    assert!(u32s.get(shared).is_err());

                    u32s.share(owned, shared);
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
                    u32s.share(owned, shared);

                    assert_eq!((&mut u32s).get(owned), Ok(&mut 0));
                    assert_eq!((&mut u32s).get(shared), Ok(&mut 0));

                    assert_eq!(u32s.try_remove(shared).unwrap(), Some(OldComponent::Shared));
                    assert_eq!((&mut u32s).get(owned), Ok(&mut 0));
                    assert!((&mut u32s).get(shared).is_err());

                    u32s.unshare(shared);
                    assert_eq!((&mut u32s).get(owned), Ok(&mut 0));
                    assert!((&mut u32s).get(shared).is_err());

                    u32s.share(owned, shared);
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
            u32s.share(owned, shared);

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
            u32s.share(owned1, shared1);
            u32s.share(shared1, shared2);

            assert_eq!(u32s.get(owned1), Ok(&1));
            assert_eq!(u32s.get(owned2), Ok(&2));
            assert_eq!(u32s.get(shared1), Ok(&1));
            assert_eq!(u32s.get(shared2), Ok(&1));

            u32s.unshare(shared1);
            assert_eq!(u32s.get(owned1), Ok(&1));
            assert_eq!(u32s.get(owned2), Ok(&2));
            assert!(u32s.get(shared1).is_err());
            assert!(u32s.get(shared2).is_err());

            u32s.share(owned2, shared1);
            assert_eq!(u32s.get(owned1), Ok(&1));
            assert_eq!(u32s.get(owned2), Ok(&2));
            assert_eq!(u32s.get(shared1), Ok(&2));
            assert_eq!(u32s.get(shared2), Ok(&2));
        })
        .unwrap();
}
