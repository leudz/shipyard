use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new();

    world
        .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            let entity0 = entities.add_entity(&mut u32s, 0);
            let entity1 = entities.add_entity(&mut u32s, 1);

            u32s.try_apply_mut(entity0, entity1, core::mem::swap)
                .unwrap();

            assert_eq!(u32s[entity0], 1);
            assert_eq!(u32s[entity1], 0);
        })
        .unwrap();
}

#[test]
fn update() {
    let world = World::new();

    world
        .try_run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
            u32s.try_update_pack().unwrap();

            let entity0 = entities.add_entity(&mut u32s, 0);

            u32s.try_clear_inserted().unwrap();

            let entity1 = entities.add_entity(&mut u32s, 1);

            u32s.try_apply_mut(entity0, entity1, core::mem::swap)
                .unwrap();

            assert_eq!(u32s[entity0], 1);
            assert_eq!(u32s[entity1], 0);

            let mut inserted = u32s.try_inserted().unwrap().iter();
            assert_eq!(inserted.next(), Some(&0));
            assert_eq!(inserted.next(), None);

            let mut modified = u32s.try_modified().unwrap().iter();
            assert_eq!(modified.next(), Some(&1));
            assert_eq!(modified.next(), None);
        })
        .unwrap();
}
