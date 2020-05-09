use shipyard::*;

#[test]
fn get() {
    let world = World::new();
    world
        .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let owned = entities.add_entity(&mut u32s, 0);
            let shared = entities.add_entity((), ());
            u32s.share(owned, shared);

            assert_eq!(u32s.get(owned), Ok(&0));
            assert_eq!(u32s.get(shared), Ok(&0));

            u32s.unshare(shared);
            assert_eq!(u32s.get(owned), Ok(&0));
            assert!(u32s.get(shared).is_err());
        })
        .unwrap();
}

#[test]
fn get_mut() {
    let world = World::new();
    world
        .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let owned = entities.add_entity(&mut u32s, 0);
            let shared = entities.add_entity((), ());
            u32s.share(owned, shared);

            assert_eq!((&mut u32s).get(owned), Ok(&mut 0));
            assert_eq!((&mut u32s).get(shared), Ok(&mut 0));

            u32s.unshare(shared);
            assert_eq!((&mut u32s).get(owned), Ok(&mut 0));
            assert!((&mut u32s).get(shared).is_err());
        })
        .unwrap();
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
